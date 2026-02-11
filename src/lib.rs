// ============================================================================
// lib.rs — Library API (shared by CLI and GUI)
// ============================================================================

pub mod config;
pub mod dag;
pub mod compiler;
pub mod builder;
pub mod options;
pub mod cmake_converter;

pub use config::{find_workspace_root, parse_build_file, Compiler, ResolvedProject, ResolvedTarget, TargetType};
pub use dag::{build_order, filter_order_for_targets, BuildOrder};
pub use options::BuildOptions;
pub use cmake_converter::{parse_cmake_lists, convert_to_toml, convert_cmake_to_toml_files};

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

/// Cleans the project: removes all output dirs (containing .o, .a, .so and built exes).
/// Sends progress lines to `output_tx` and a final `__ngmake_FINISH__\ttrue\t0\t0\t0`.
pub fn clean_project_and_stream_output(
    config_path: &Path,
    output_tx: mpsc::Sender<String>,
) -> Result<(), String> {
    let path = config_path.to_path_buf();
    let project = parse_build_file(&path, false)?;
    let mut dirs: Vec<_> = project.targets.values().map(|t| t.output_dir.clone()).collect();
    dirs.sort();
    dirs.dedup();
    let _ = output_tx.send("[CLEAN] Cleaning project output directories.".to_string());
    for d in &dirs {
        if d.exists() {
            let _ = output_tx.send(format!("[CLEAN] Removing {}", d.display()));
            if let Err(e) = std::fs::remove_dir_all(d) {
                let _ = output_tx.send(format!("[ERROR] Failed to remove {}: {}", d.display(), e));
                let _ = output_tx.send("__ngmake_FINISH__\tfalse\t0\t0\t1".to_string());
                return Err(e.to_string());
            }
        }
    }
    let _ = output_tx.send("[CLEAN] Done.".to_string());
    let _ = output_tx.send("__ngmake_FINISH__\ttrue\t0\t0\t0".to_string());
    Ok(())
}

/// Project info and DAG graph data for the GUI
#[derive(serde::Serialize)]
pub struct ProjectInfo {
    pub project: ResolvedProject,
    pub build_order: BuildOrder,
    pub graph_nodes: Vec<GraphNode>,
    pub graph_edges: Vec<GraphEdge>,
}

#[derive(serde::Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub target_type: String,
    pub level: usize,
}

#[derive(serde::Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// Parses build.toml and produces node/edge lists for graph display.
pub fn parse_and_graph(path: &Path) -> Result<ProjectInfo, String> {
    let project = parse_build_file(path, false)?;
    let order = build_order(&project)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for (level_idx, level) in order.levels.iter().enumerate() {
        for name in level {
            if let Some(t) = project.targets.get(name) {
                let target_type = match &t.target_type {
                    config::TargetType::Executable => "executable",
                    config::TargetType::StaticLib => "static_lib",
                    config::TargetType::SharedLib => "shared_lib",
                };
                nodes.push(GraphNode {
                    id: name.clone(),
                    label: name.clone(),
                    target_type: target_type.to_string(),
                    level: level_idx,
                });
                for dep in &t.deps {
                    edges.push(GraphEdge {
                        from: dep.clone(),
                        to: name.clone(),
                    });
                }
            }
        }
    }

    Ok(ProjectInfo {
        project,
        build_order: order,
        graph_nodes: nodes,
        graph_edges: edges,
    })
}

/// Runs the build (optional clean) and returns all output lines.
pub fn build_and_collect_output(
    config_path: &Path,
    targets: Option<Vec<String>>,
    clean: bool,
    jobs: Option<usize>,
    ignore_errors: bool,
) -> Result<(bool, Vec<String>), String> {
    let path = config_path.to_path_buf();
    let project = parse_build_file(&path, false)?;
    let full_order = build_order(&project)?;
    let order = match &targets {
        Some(t) => filter_order_for_targets(&project, &full_order, t)?,
        None => full_order,
    };
    if order.levels.is_empty() {
        return Ok((true, vec!["[INFO] No targets to build.".to_string()]));
    }

    if clean {
        let mut dirs: Vec<_> = project.targets.values().map(|t| t.output_dir.clone()).collect();
        dirs.sort();
        dirs.dedup();
        for d in &dirs {
            if d.exists() {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }

    let opts = BuildOptions {
        command: None,
        config: path,
        targets,
        clean: false,
        verbose: true,
        quiet: false,
        no_ld_path: true,
        jobs,
        ignore_errors,
    };

    let result = builder::build_project(&project, &order, &opts, None, None);
    let mut lines = Vec::new();
    for r in &result.results {
        for msg in &r.messages {
            lines.push(msg.clone());
        }
    }
    lines.push(format!(
        "--- {} targets, {} successful, {} failed ---",
        result.total_targets,
        result.successful_targets,
        result.failed_targets
    ));
    Ok((result.success, lines))
}

/// Runs the build in the current thread and sends each output line to `output_tx`.
/// Sends a final line `__ngmake_FINISH__\t{success}\t{total}\t{successful}\t{failed}` before closing the channel.
/// Call from a background thread; another thread should receive from the paired receiver and emit to the GUI.
/// `jobs`: parallel job count; None = auto. `ignore_errors`: continue building after a target fails (like make -i).
/// `cancel`: when Some and set to true, build stops after the current job.
pub fn build_and_stream_output(
    config_path: &Path,
    targets: Option<Vec<String>>,
    clean: bool,
    jobs: Option<usize>,
    ignore_errors: bool,
    output_tx: mpsc::Sender<String>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<bool, String> {
    let path = config_path.to_path_buf();
    let project = parse_build_file(&path, false)?;
    let full_order = build_order(&project)?;
    let order = match &targets {
        Some(t) => filter_order_for_targets(&project, &full_order, t)?,
        None => full_order,
    };
    if order.levels.is_empty() {
        let _ = output_tx.send("[INFO] No targets to build.".to_string());
        let _ = output_tx.send("__ngmake_FINISH__\ttrue\t0\t0\t0".to_string());
        return Ok(true);
    }

    if clean {
        let mut dirs: Vec<_> = project.targets.values().map(|t| t.output_dir.clone()).collect();
        dirs.sort();
        dirs.dedup();
        for d in &dirs {
            if d.exists() {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }

    let opts = BuildOptions {
        command: None,
        config: path,
        targets,
        clean: false,
        verbose: true,
        quiet: false,
        no_ld_path: true,
        jobs,
        ignore_errors,
    };

    let result = builder::build_project(&project, &order, &opts, Some(output_tx), cancel);
    Ok(result.success)
}
