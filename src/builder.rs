// ============================================================================
// builder.rs — Parallel multi-threaded build manager
// ============================================================================
//
// This module processes the levels from DAG topological order. Targets in
// each level are independent and can be built in parallel. After all targets
// in a level complete, the next level is processed. Build stops on any error.
// ============================================================================

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::compiler::{self, CompileResult};
use crate::config::ResolvedProject;
use crate::dag::{self, BuildOrder};
use crate::options::BuildOptions;

// ---------------------------------------------------------------------------
// Build result
// ---------------------------------------------------------------------------
#[derive(Debug)]
pub struct BuildResult {
    pub success: bool,
    #[allow(dead_code)]
    pub results: Vec<CompileResult>,
    pub total_targets: usize,
    pub successful_targets: usize,
    pub failed_targets: usize,
}

// ---------------------------------------------------------------------------
// Paralel derleme yöneticisi
// ---------------------------------------------------------------------------

/// Builds the whole project in the given order. Targets in each level
/// are built in parallel threads.
/// When `output_tx` is Some, each message line is sent to the channel (for streaming to GUI).
/// When `cancel` is Some and becomes true, the build stops after the current job.
pub fn build_project(
    project: &ResolvedProject,
    order: &BuildOrder,
    options: &BuildOptions,
    output_tx: Option<mpsc::Sender<String>>,
    cancel: Option<Arc<AtomicBool>>,
) -> BuildResult {
    let total_targets = order.levels.iter().map(|l| l.len()).sum::<usize>();
    let quiet = options.show_quiet_output();
    let verbose = options.show_verbose_output();

    if !quiet {
        if verbose {
            println!("\n╔══════════════════════════════════════════════════╗");
            println!("║            C++ BUILD STARTING                    ║");
            println!("║  Project: {:<40} ║", project.name);
            println!("║  Version: {:<40} ║", project.version);
            println!("║  Target count: {:<30} ║", total_targets);
            println!("╚══════════════════════════════════════════════════╝\n");
        } else {
            println!("  Building {} targets...\n", total_targets);
        }
    }

    let n_jobs = options.jobs.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1)
    });

    let result = build_ninja_style(project, order, n_jobs, options, output_tx.as_ref(), cancel);

    if !quiet && result.success && verbose {
        println!("╔══════════════════════════════════════════════════╗");
        println!("║            BUILD COMPLETED SUCCESSFULLY          ║");
        println!("║  Total: {} targets, {} successful           ║", result.total_targets, result.successful_targets);
        println!("╚══════════════════════════════════════════════════╝");
    }

    result
}

// ---------------------------------------------------------------------------
// Ninja-style: single global job queue of compile + link jobs; -j N = exactly N concurrent jobs.
// ---------------------------------------------------------------------------
#[derive(Clone)]
enum Job {
    Compile {
        target_name: String,
        source_idx: usize,
        source: PathBuf,
        obj_path: PathBuf,
    },
    Link {
        target_name: String,
        object_files: Vec<PathBuf>,
        built_deps: HashMap<String, PathBuf>,
        link_deps: Option<Vec<String>>,
    },
}

enum JobResult {
    Compile {
        target_name: String,
        source_idx: usize,
        obj_path: PathBuf,
        success: bool,
        messages: Vec<String>,
    },
    Link(CompileResult),
}

fn build_ninja_style(
    project: &ResolvedProject,
    order: &BuildOrder,
    n_jobs: usize,
    options: &BuildOptions,
    output_tx: Option<&mpsc::Sender<String>>,
    cancel: Option<Arc<AtomicBool>>,
) -> BuildResult {
    let total_targets: usize = order.levels.iter().map(|l| l.len()).sum();
    let quiet = options.show_quiet_output();
    let verbose = options.show_verbose_output();

    if let Some(tx) = output_tx {
        let _ = tx.send(format!("__ngmake_TOTAL__\t{}", total_targets));
    }

    let built_targets: Arc<Mutex<HashMap<String, PathBuf>>> = Arc::new(Mutex::new(HashMap::new()));
    let obj_files: Arc<Mutex<HashMap<String, Vec<Option<PathBuf>>>>> = Arc::new(Mutex::new(HashMap::new()));
    let compile_jobs_added: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    let dependents: HashMap<String, Vec<String>> = {
        let mut d: HashMap<String, Vec<String>> = HashMap::new();
        for (name, target) in &project.targets {
            for dep in &target.deps {
                d.entry(dep.clone()).or_default().push(name.clone());
            }
        }
        d
    };

    let job_queue: Arc<(Mutex<VecDeque<Option<Job>>>, Condvar)> =
        Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
    let (result_tx, result_rx) = mpsc::sync_channel::<JobResult>(0);
    let project_ref = Arc::new(project.targets.clone());
    let n_workers = n_jobs.max(1);

    for _ in 0..n_workers {
        let job_queue = Arc::clone(&job_queue);
        let result_tx = result_tx.clone();
        let project_ref = Arc::clone(&project_ref);
        let _ = thread::spawn(move || {
            loop {
                let job = {
                    let mut q = job_queue.0.lock().unwrap();
                    while q.is_empty() {
                        q = job_queue.1.wait(q).unwrap();
                    }
                    q.pop_front().flatten()
                };
                let Some(job) = job else { break };
                match job {
                    Job::Compile { target_name, source_idx, source, obj_path } => {
                        let Some(target) = project_ref.get(&target_name) else { continue };
                        match compiler::compile_one_source_or_skip(target, &source, &obj_path) {
                            Ok((path, messages)) => {
                                let _ = result_tx.send(JobResult::Compile {
                                    target_name,
                                    source_idx,
                                    obj_path: path,
                                    success: true,
                                    messages,
                                });
                            }
                            Err(e) => {
                                let _ = result_tx.send(JobResult::Compile {
                                    target_name,
                                    source_idx,
                                    obj_path: obj_path.clone(),
                                    success: false,
                                    messages: vec![format!("  [ERROR] {}", e)],
                                });
                            }
                        }
                    }
                    Job::Link { target_name, object_files, built_deps, link_deps } => {
                        let Some(target) = project_ref.get(&target_name) else { continue };
                        let result = compiler::run_link_step(
                            target,
                            &object_files,
                            &built_deps,
                            link_deps.as_deref(),
                        );
                        let _ = result_tx.send(JobResult::Link(result));
                    }
                }
            }
        });
    }
    drop(result_tx);

    let mut all_results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;
    let mut in_flight = 0usize;
    let mut build_failed = false;

    let level0 = order.levels.first().map(|l| l.as_slice()).unwrap_or(&[]);
    let mut headers_sent: HashSet<String> = HashSet::new();

    fn send_header(tx: Option<&mpsc::Sender<String>>, name: &str, target: &crate::config::ResolvedTarget, headers_sent: &mut HashSet<String>) {
        if let Some(tx) = tx {
            if headers_sent.insert(name.to_string()) {
                let line = format!(
                    "=== Building target '{}' (type: {:?}) ===",
                    name, target.target_type
                );
                let _ = tx.send(format!("[TARGET:{}] {}", name, line));
            }
        }
    }

    fn push_compile_jobs(
        project: &ResolvedProject,
        target_name: &str,
        order: &BuildOrder,
        job_queue: &Arc<(Mutex<VecDeque<Option<Job>>>, Condvar)>,
        built_targets: &Arc<Mutex<HashMap<String, PathBuf>>>,
        output_tx: Option<&mpsc::Sender<String>>,
        headers_sent: &mut HashSet<String>,
    ) -> usize {
        let target = match project.targets.get(target_name) {
            Some(t) => t,
            None => return 0,
        };
        send_header(output_tx, target_name, target, headers_sent);
        if target.sources.is_empty() {
            let link_deps = link_deps_for_target(project, target_name, order);
            let built = built_targets.lock().unwrap().clone();
            let built_deps = built_deps_subset(&built, link_deps.as_deref());
            job_queue.0.lock().unwrap().push_back(Some(Job::Link {
                target_name: target_name.to_string(),
                object_files: vec![],
                built_deps,
                link_deps,
            }));
            job_queue.1.notify_all();
            return 1;
        }
        let obj_dir = target.output_dir.join("obj").join(target_name);
        let mut n = 0usize;
        for (idx, source) in target.sources.iter().enumerate() {
            let obj_name = source
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
                + ".o";
            let obj_path = obj_dir.join(&obj_name);
            let job = Job::Compile {
                target_name: target_name.to_string(),
                source_idx: idx,
                source: source.clone(),
                obj_path,
            };
            job_queue.0.lock().unwrap().push_back(Some(job));
            n += 1;
        }
        job_queue.1.notify_all();
        n
    }

    for target_name in level0 {
        obj_files.lock().unwrap().entry(target_name.to_string()).or_insert_with(|| {
            project.targets.get(target_name).map(|t| vec![None; t.sources.len()]).unwrap_or_default()
        });
        let n = push_compile_jobs(project, target_name, order, &job_queue, &built_targets, output_tx, &mut headers_sent);
        in_flight += n;
        compile_jobs_added.lock().unwrap().insert(target_name.to_string());
    }

    while in_flight > 0 {
        let result = match result_rx.recv() {
            Ok(r) => r,
            Err(_) => break,
        };
        in_flight -= 1;

        match result {
            JobResult::Compile { target_name, source_idx, obj_path, success, messages } => {
                if !quiet && (verbose || output_tx.is_some()) {
                    for msg in &messages {
                        let is_error = msg.contains("[ERROR]");
                        if is_error { eprintln!("{}", msg); } else if verbose || !msg.contains("Command:") { println!("{}", msg); }
                    }
                } else if !quiet && !verbose {
                    for msg in &messages {
                        if msg.contains("[ERROR]") { eprintln!("{}", msg); }
                    }
                }
                if let Some(tx) = output_tx {
                    for msg in &messages {
                        // Send each line separately so GUI shows full compiler stderr (IPC may not preserve newlines in one payload)
                        for line in msg.split('\n') {
                            let _ = tx.send(format!("[TARGET:{}] {}", target_name, line));
                        }
                    }
                }
                if !success {
                    build_failed = true;
                    failed += 1;
                    if !quiet && !verbose && output_tx.is_none() {
                        let done = successful + failed;
                        let pct = if total_targets > 0 { (done * 100) / total_targets } else { 0 };
                        println!("  [{:>3}/{} {:>3}%] {} (failed)", done, total_targets, pct, target_name);
                    }
                    all_results.push(CompileResult {
                        target_name: target_name.clone(),
                        success: false,
                        output_path: PathBuf::new(),
                        messages,
                    });
                    continue;
                }
                obj_files.lock().unwrap().entry(target_name.clone()).or_default()[source_idx] = Some(obj_path);
                let of = obj_files.lock().unwrap().get(&target_name).cloned().unwrap_or_default();
                let all_done = of.iter().all(Option::is_some);
                if all_done {
                    let target = project.targets.get(&target_name).unwrap();
                    let object_files: Vec<PathBuf> = of.into_iter().map(|o| o.unwrap()).collect();
                    let link_deps = link_deps_for_target(project, &target_name, order);
                    let built = built_targets.lock().unwrap().clone();
                    let built_deps = built_deps_subset(&built, link_deps.as_deref());
                    job_queue.0.lock().unwrap().push_back(Some(Job::Link {
                        target_name: target_name.clone(),
                        object_files,
                        built_deps,
                        link_deps,
                    }));
                    job_queue.1.notify_all();
                    in_flight += 1;
                }
            }
            JobResult::Link(result) => {
                if result.success {
                    built_targets.lock().unwrap().insert(result.target_name.clone(), result.output_path.clone());
                    successful += 1;
                } else {
                    failed += 1;
                    build_failed = true;
                }
                let continue_on_failure = options.ignore_errors;
                if result.success || continue_on_failure {
                    for dep_name in dependents.get(&result.target_name).cloned().unwrap_or_default() {
                        let added = compile_jobs_added.lock().unwrap().contains(dep_name.as_str());
                        if added {
                            continue;
                        }
                        let deps_ready = project.targets.get(&dep_name).map(|t| {
                            t.deps.iter().all(|d| built_targets.lock().unwrap().contains_key(d))
                        }).unwrap_or(false);
                        if deps_ready {
                            obj_files.lock().unwrap().entry(dep_name.clone()).or_insert_with(|| {
                                project.targets.get(&dep_name).map(|t| vec![None; t.sources.len()]).unwrap_or_default()
                            });
                            let n = push_compile_jobs(project, &dep_name, order, &job_queue, &built_targets, output_tx, &mut headers_sent);
                            in_flight += n;
                            compile_jobs_added.lock().unwrap().insert(dep_name);
                        }
                    }
                }
                if !quiet && (verbose || output_tx.is_some()) {
                    for msg in &result.messages {
                        let is_error = msg.contains("[ERROR]");
                        if is_error { eprintln!("{}", msg); } else if verbose || !msg.contains("Command:") { println!("{}", msg); }
                    }
                } else if !quiet && !verbose {
                    for msg in &result.messages {
                        if msg.contains("[ERROR]") { eprintln!("{}", msg); }
                    }
                }
                if !quiet && !verbose && output_tx.is_none() {
                    let done = successful + failed;
                    let pct = if total_targets > 0 { (done * 100) / total_targets } else { 0 };
                    println!("  [{:>3}/{} {:>3}%] {}", done, total_targets, pct, result.target_name);
                }
                if let Some(tx) = output_tx {
                    for msg in &result.messages {
                        for line in msg.split('\n') {
                            let _ = tx.send(format!("[TARGET:{}] {}", result.target_name, line));
                        }
                    }
                }
                all_results.push(result);
            }
        }
        if cancel.as_ref().map(|c| c.load(Ordering::Relaxed)).unwrap_or(false) {
            if let Some(tx) = output_tx {
                let _ = tx.send("[INFO] Build cancelled by user.".to_string());
            }
            break;
        }
    }

    {
        let mut q = job_queue.0.lock().unwrap();
        for _ in 0..n_workers {
            q.push_back(None);
        }
        job_queue.1.notify_all();
    }

    let cancelled = cancel.as_ref().map(|c| c.load(Ordering::Relaxed)).unwrap_or(false);
    let success = !cancelled && !build_failed && failed == 0;
    if let Some(tx) = output_tx {
        let _ = tx.send(format!(
            "--- {} targets, {} successful, {} failed ---",
            total_targets, successful, failed
        ));
        let _ = tx.send(format!("__ngmake_FINISH__\t{}\t{}\t{}\t{}", success, total_targets, successful, failed));
    }

    BuildResult {
        success,
        results: all_results,
        total_targets,
        successful_targets: successful,
        failed_targets: failed,
    }
}

/// Level-by-level build (when no -j: all targets in each level in parallel).
fn build_level_by_level(
    project: &ResolvedProject,
    order: &BuildOrder,
    options: &BuildOptions,
) -> BuildResult {
    let total_targets = order.levels.iter().map(|l| l.len()).sum::<usize>();
    let built_targets: Arc<Mutex<HashMap<String, PathBuf>>> = Arc::new(Mutex::new(HashMap::new()));
    let quiet = options.show_quiet_output();
    let verbose = options.show_verbose_output();
    let mut all_results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    for (level_idx, level) in order.levels.iter().enumerate() {
        if !quiet {
            println!(
                "┌─── Level {} ─── ({} target(s): {:?}) ───┐",
                level_idx,
                level.len(),
                level
            );
        }

        let level_results = build_level_parallel(level, project, &built_targets, order, None);

        let mut level_failed = false;
        for result in &level_results {
            if !quiet {
                for msg in &result.messages {
                    let is_error = msg.contains("[ERROR]");
                    if is_error {
                        eprintln!("{}", msg);
                    } else if verbose || !msg.contains("Command:") {
                        println!("{}", msg);
                    }
                }
            }
            if result.success {
                successful += 1;
            } else {
                failed += 1;
                level_failed = true;
            }
        }
        all_results.extend(level_results);

        if level_failed {
            if !quiet {
                eprintln!(
                    "\n[ERROR] One or more targets in level {} failed. Stopping build.",
                    level_idx
                );
            }
            return BuildResult {
                success: false,
                results: all_results,
                total_targets,
                successful_targets: successful,
                failed_targets: failed,
            };
        }
        if !quiet {
            println!("└─── Level {} done ───┘\n", level_idx);
        }
    }

    BuildResult {
        success: true,
        results: all_results,
        total_targets,
        successful_targets: successful,
        failed_targets: failed,
    }
}

// ---------------------------------------------------------------------------
// Transitive dependencies for executable/shared_lib in link order
// ---------------------------------------------------------------------------
fn link_deps_for_target(
    project: &ResolvedProject,
    target_name: &str,
    order: &BuildOrder,
) -> Option<Vec<String>> {
    let target = project.targets.get(target_name)?;
    match target.target_type {
        crate::config::TargetType::Executable | crate::config::TargetType::SharedLib => {
            Some(dag::transitive_deps_in_link_order(project, target_name, order))
        }
        crate::config::TargetType::StaticLib => None,
    }
}

/// Only the built paths needed for this target's link deps (avoids cloning full map).
fn built_deps_subset(
    built: &HashMap<String, PathBuf>,
    dep_names: Option<&[String]>,
) -> HashMap<String, PathBuf> {
    let mut out = HashMap::with_capacity(dep_names.map(|d| d.len()).unwrap_or(0));
    if let Some(deps) = dep_names {
        for name in deps {
            if let Some(p) = built.get(name) {
                out.insert(name.clone(), p.clone());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Build targets in one level in parallel (up to N concurrent when -j is set)
// ---------------------------------------------------------------------------
fn build_level_parallel(
    level: &[String],
    project: &ResolvedProject,
    built_targets: &Arc<Mutex<HashMap<String, PathBuf>>>,
    order: &BuildOrder,
    jobs: Option<usize>,
) -> Vec<CompileResult> {
    let chunk_size = jobs.unwrap_or(level.len());
    if chunk_size == 0 {
        return vec![];
    }

    let mut all_results = Vec::new();

    for chunk in level.chunks(chunk_size) {
        let chunk_results = build_chunk_parallel(chunk, project, built_targets, order);
        all_results.extend(chunk_results);
    }

    all_results
}

/// Build a subset of targets in parallel (one thread per target in chunk)
fn build_chunk_parallel(
    chunk: &[String],
    project: &ResolvedProject,
    built_targets: &Arc<Mutex<HashMap<String, PathBuf>>>,
    order: &BuildOrder,
) -> Vec<CompileResult> {
    if chunk.is_empty() {
        return vec![];
    }
    if chunk.len() == 1 {
        let target_name = &chunk[0];
        if let Some(target) = project.targets.get(target_name) {
            let bt = built_targets.lock().unwrap().clone();
            let link_deps = link_deps_for_target(project, target_name, order);
            let result = compiler::build_target(target, &bt, link_deps.as_deref());
            if result.success {
                built_targets
                    .lock()
                    .unwrap()
                    .insert(result.target_name.clone(), result.output_path.clone());
            }
            return vec![result];
        }
        return vec![];
    }

    let mut handles = Vec::new();
    for target_name in chunk {
        if let Some(target) = project.targets.get(target_name) {
            let target_clone = target.clone();
            let bt_arc = Arc::clone(built_targets);
            let link_deps = link_deps_for_target(project, target_name, order);
            let bt_snapshot = bt_arc.lock().unwrap().clone();

            let handle = thread::spawn(move || {
                let result = compiler::build_target(&target_clone, &bt_snapshot, link_deps.as_deref());
                (result, bt_arc)
            });
            handles.push(handle);
        }
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok((result, bt_arc)) => {
                if result.success {
                    bt_arc
                        .lock()
                        .unwrap()
                        .insert(result.target_name.clone(), result.output_path.clone());
                }
                results.push(result);
            }
            Err(_) => {
                results.push(CompileResult {
                    target_name: "unknown".to_string(),
                    success: false,
                    output_path: PathBuf::new(),
                    messages: vec!["[ERROR] Thread panicked unexpectedly!".to_string()],
                });
            }
        }
    }
    results
}
