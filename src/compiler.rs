// ============================================================================
// compiler.rs — Compiler command construction and execution
// ============================================================================
//
// This module builds and runs the appropriate compiler commands per target type.
// Supported operations:
//   - .cpp → .o compilation (sources to object files)
//   - Linking .o files into an executable
//   - Archiving .o files into a static library (.a)
//   - Linking .o files into a shared library (.so on Unix, .dll on Windows)
//
// LD_LIBRARY_PATH is set automatically for shared library resolution.
// ============================================================================

use std::path::{Path, PathBuf};
use std::process::Command;

use rayon::prelude::*;
use crate::config::{ResolvedTarget, TargetType};

// ---------------------------------------------------------------------------
// Cross-platform path for compiler args: use forward slashes so the compiler
// always receives a single, unambiguous path. Fixes Windows backslash
// mangling (CreateProcess/C runtime); no-op on Unix where paths already use /.
// GCC, Clang, and MinGW accept forward slashes on all platforms.
// On Windows, strip the verbatim prefix "\\?\" so the compiler gets "C:/..."
// instead of "//?/C:/..." (which some tools misparse as "//filename").
// ---------------------------------------------------------------------------
fn path_arg(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    let s = if s.starts_with(r"\\?\") { s[4..].to_string() } else { s };
    s.replace('\\', "/")
}

/// Shared library filename: lib{name}.so on Unix, lib{name}.dll on Windows.
fn shared_lib_filename(name: &str) -> String {
    let ext = if cfg!(windows) { "dll" } else { "so" };
    format!("lib{}.{}", name, ext)
}

// ---------------------------------------------------------------------------
// Compilation result
// ---------------------------------------------------------------------------
#[derive(Debug)]
pub struct CompileResult {
    pub target_name: String,
    pub success: bool,
    pub output_path: PathBuf,
    pub messages: Vec<String>,
}

// ---------------------------------------------------------------------------
// Build a single target
// ---------------------------------------------------------------------------

/// Builds the given target: compiles sources to object files, then links
/// according to target type. `link_deps`: for executable/shared_lib, the
/// dependency list (transitive, in link order). If None, target.deps is used.
pub fn build_target(
    target: &ResolvedTarget,
    built_targets: &std::collections::HashMap<String, PathBuf>,
    link_deps: Option<&[String]>,
) -> CompileResult {
    let mut messages: Vec<String> = Vec::new();

    messages.push(format!(
        "=== Building target '{}' (type: {:?}) ===",
        target.name, target.target_type
    ));

    // Skip targets with no sources (INTERFACE libraries, etc.)
    if target.sources.is_empty() {
        messages.push(format!(
            "[SKIP] Target '{}' has no sources (INTERFACE library or empty target). Skipping build.",
            target.name
        ));
        return CompileResult {
            target_name: target.name.clone(),
            success: true, // Mark as successful since we're intentionally skipping
            output_path: PathBuf::new(),
            messages,
        };
    }

    // Create output directory
    let output_dir = &target.output_dir;
    let obj_dir = output_dir.join("obj").join(&target.name);
    if let Err(e) = std::fs::create_dir_all(&obj_dir) {
        return CompileResult {
            target_name: target.name.clone(),
            success: false,
            output_path: PathBuf::new(),
            messages: vec![format!("Failed to create output directory: {}", e)],
        };
    }

    // --- Step 1: Compile source files to object files (incremental + parallel) ---
    let mut object_files: Vec<PathBuf> = Vec::with_capacity(target.sources.len());
    let mut to_compile: Vec<(PathBuf, PathBuf)> = Vec::new();

    for source in &target.sources {
        let obj_name = source
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".o";
        let obj_path = obj_dir.join(&obj_name);
        object_files.push(obj_path.clone());

        let should_compile = if obj_path.exists() {
            let source_mtime = std::fs::metadata(source).and_then(|m| m.modified()).ok();
            let obj_mtime = std::fs::metadata(&obj_path).and_then(|m| m.modified()).ok();
            match (source_mtime, obj_mtime) {
                (Some(src_time), Some(obj_time)) => src_time > obj_time,
                _ => true,
            }
        } else {
            true
        };

        if !should_compile {
            messages.push(format!("  [SKIP] {} (up-to-date)", source.display()));
            continue;
        }
        to_compile.push((source.clone(), obj_path));
    }

    // Parallel compile: run all needed compiles concurrently to use multiple cores
    if !to_compile.is_empty() {
        let compile_results: Vec<Result<(PathBuf, Vec<String>), String>> = to_compile
            .par_iter()
            .map(|(source, obj_path)| compile_one_source(target, source, obj_path))
            .collect();
        for res in compile_results {
            match res {
                Ok((_, mut log)) => messages.append(&mut log),
                Err(e) => {
                    messages.push(format!("  [ERROR] {}", e));
                    return CompileResult {
                        target_name: target.name.clone(),
                        success: false,
                        output_path: PathBuf::new(),
                        messages,
                    };
                }
            }
        }
    }

    let needs_rebuild = !to_compile.is_empty();

    // --- Step 2: Linking according to target type (incremental: only if needed) ---
    let deps_for_link = link_deps.unwrap_or(&target.deps);
    
    // Determine final output path
    let final_output_path = match target.target_type {
        TargetType::Executable => target.output_dir.join(&target.name),
        TargetType::StaticLib => {
            let lib_name = format!("lib{}.a", target.name);
            target.output_dir.join(&lib_name)
        }
        TargetType::SharedLib => {
            let lib_name = shared_lib_filename(&target.name);
            target.output_dir.join(&lib_name)
        }
    };

    // Check if we need to relink (incremental build)
    let needs_relink = if final_output_path.exists() {
        // Check if any object file is newer than the output
        let output_mtime = std::fs::metadata(&final_output_path)
            .and_then(|m| m.modified())
            .ok();
        
        if let Some(out_time) = output_mtime {
            // Check if any object file is newer
            let obj_newer = object_files.iter().any(|obj| {
                std::fs::metadata(obj)
                    .and_then(|m| m.modified())
                    .map(|t| t > out_time)
                    .unwrap_or(false)
            });
            
            // Also check if any dependency library is newer
            let dep_newer = deps_for_link.iter().any(|dep_name| {
                if let Some(dep_path) = built_targets.get(dep_name) {
                    std::fs::metadata(dep_path)
                        .and_then(|m| m.modified())
                        .map(|t| t > out_time)
                        .unwrap_or(false)
                } else {
                    false
                }
            });
            
            needs_rebuild || obj_newer || dep_newer
        } else {
            // Can't get mtime, always relink to be safe
            true
        }
    } else {
        // Output doesn't exist, must link
        true
    };

    let final_output = if !needs_relink {
        messages.push(format!(
            "  [SKIP] Linking '{}' (up-to-date)",
            target.name
        ));
        Ok(final_output_path)
    } else {
        match target.target_type {
            TargetType::Executable => link_executable(target, &object_files, built_targets, deps_for_link, &mut messages),
            TargetType::StaticLib => create_static_lib(target, &object_files, &mut messages),
            TargetType::SharedLib => link_shared_lib(target, &object_files, built_targets, deps_for_link, &mut messages),
        }
    };

    match final_output {
        Ok(path) => {
            messages.push(format!(
                "  [OK] '{}' → {}",
                target.name,
                path.display()
            ));
            CompileResult {
                target_name: target.name.clone(),
                success: true,
                output_path: path,
                messages,
            }
        }
        Err(e) => {
            messages.push(format!("  [ERROR] Linking failed: {}", e));
            CompileResult {
                target_name: target.name.clone(),
                success: false,
                output_path: PathBuf::new(),
                messages,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Compile one source or skip if up-to-date (for Ninja-style single-job use)
// ---------------------------------------------------------------------------
pub fn compile_one_source_or_skip(
    target: &ResolvedTarget,
    source: &Path,
    obj_path: &Path,
) -> Result<(PathBuf, Vec<String>), String> {
    if let Some(parent) = obj_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let should_compile = if obj_path.exists() {
        let source_mtime = std::fs::metadata(source).and_then(|m| m.modified()).ok();
        let obj_mtime = std::fs::metadata(obj_path).and_then(|m| m.modified()).ok();
        match (source_mtime, obj_mtime) {
            (Some(src_time), Some(obj_time)) => src_time > obj_time,
            _ => true,
        }
    } else {
        true
    };
    if !should_compile {
        let mut msgs = Vec::new();
        msgs.push(format!("  [SKIP] {} (up-to-date)", source.display()));
        return Ok((obj_path.to_path_buf(), msgs));
    }
    compile_one_source(target, source, obj_path)
}

// ---------------------------------------------------------------------------
// Compile a single source file (used for parallel compilation)
// ---------------------------------------------------------------------------
fn compile_one_source(
    target: &ResolvedTarget,
    source: &Path,
    obj_path: &Path,
) -> Result<(PathBuf, Vec<String>), String> {
    let mut msgs = Vec::new();
    msgs.push(format!(
        "  [COMPILE] {} → {}",
        source.display(),
        obj_path.display()
    ));

    let mut cmd = Command::new(target.compiler.command());
    cmd.arg("-c");
    cmd.arg(path_arg(source));
    cmd.arg("-o").arg(path_arg(obj_path));
    if target.target_type == TargetType::SharedLib {
        cmd.arg("-fPIC");
    }
    for include_dir in &target.include_dirs {
        cmd.arg("-I").arg(path_arg(include_dir));
    }
    if let Some(std) = target.cxx_standard {
        cmd.arg(format!("-std=c++{}", std));
    }
    for flag in &target.compiler_flags {
        cmd.arg(flag);
    }
    for flag in &target.flags {
        cmd.arg(flag);
    }
    let ld_path = build_ld_library_path(&target.lib_dirs);
    if !ld_path.is_empty() {
        cmd.env("LD_LIBRARY_PATH", &ld_path);
    }

    msgs.push(format!("    Command: {:?}", cmd));

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stdout.is_empty() {
                msgs.push(format!("    stdout: {}", stdout.trim()));
            }
            if !stderr.is_empty() {
                msgs.push(format!("    stderr: {}", stderr.trim()));
            }
            if !output.status.success() {
                let mut err = format!(
                    "Compilation of '{}' failed (exit {:?})",
                    source.display(),
                    output.status.code()
                );
                if !stderr.is_empty() {
                    err.push_str("\n");
                    err.push_str(stderr.trim());
                }
                if !stdout.is_empty() {
                    err.push_str("\n--- stdout ---\n");
                    err.push_str(stdout.trim());
                }
                return Err(err);
            }
            Ok((obj_path.to_path_buf(), msgs))
        }
        Err(e) => Err(format!("Failed to run compiler: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Run only the link step (for Ninja-style: link job after all compiles for target)
// ---------------------------------------------------------------------------
pub fn run_link_step(
    target: &ResolvedTarget,
    object_files: &[PathBuf],
    built_targets: &std::collections::HashMap<String, PathBuf>,
    link_deps: Option<&[String]>,
) -> CompileResult {
    let mut messages = Vec::new();
    if object_files.is_empty() && target.sources.is_empty() {
        messages.push(format!("[SKIP] Target '{}' has no sources. Skipping build.", target.name));
        return CompileResult {
            target_name: target.name.clone(),
            success: true,
            output_path: PathBuf::new(),
            messages,
        };
    }
    let deps_for_link = link_deps.unwrap_or(&target.deps);
    let final_output_path = match target.target_type {
        TargetType::Executable => target.output_dir.join(&target.name),
        TargetType::StaticLib => {
            let lib_name = format!("lib{}.a", target.name);
            target.output_dir.join(&lib_name)
        }
        TargetType::SharedLib => {
            let lib_name = shared_lib_filename(&target.name);
            target.output_dir.join(&lib_name)
        }
    };
    if let Some(parent) = final_output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let needs_relink = if final_output_path.exists() {
        let output_mtime = std::fs::metadata(&final_output_path).and_then(|m| m.modified()).ok();
        if let Some(out_time) = output_mtime {
            let obj_newer = object_files.iter().any(|obj| {
                std::fs::metadata(obj)
                    .and_then(|m| m.modified())
                    .map(|t| t > out_time)
                    .unwrap_or(false)
            });
            let dep_newer = deps_for_link.iter().any(|dep_name| {
                built_targets
                    .get(dep_name)
                    .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
                    .map(|t| t > out_time)
                    .unwrap_or(false)
            });
            obj_newer || dep_newer
        } else {
            true
        }
    } else {
        true
    };
    let final_output = if !needs_relink {
        messages.push(format!("  [SKIP] Linking '{}' (up-to-date)", target.name));
        Ok(final_output_path)
    } else {
        match target.target_type {
            TargetType::Executable => link_executable(target, object_files, built_targets, deps_for_link, &mut messages),
            TargetType::StaticLib => create_static_lib(target, object_files, &mut messages),
            TargetType::SharedLib => link_shared_lib(target, object_files, built_targets, deps_for_link, &mut messages),
        }
    };
    match final_output {
        Ok(path) => {
            messages.push(format!("  [OK] '{}' → {}", target.name, path.display()));
            CompileResult {
                target_name: target.name.clone(),
                success: true,
                output_path: path,
                messages,
            }
        }
        Err(e) => {
            messages.push(format!("  [ERROR] Linking failed: {}", e));
            CompileResult {
                target_name: target.name.clone(),
                success: false,
                output_path: PathBuf::new(),
                messages,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Create executable (linking)
// ---------------------------------------------------------------------------
fn link_executable(
    target: &ResolvedTarget,
    object_files: &[PathBuf],
    built_targets: &std::collections::HashMap<String, PathBuf>,
    dep_names: &[String],
    messages: &mut Vec<String>,
) -> Result<PathBuf, String> {
    let output_path = target.output_dir.join(&target.name);

    messages.push(format!(
        "  [LINK] Creating executable: {}",
        output_path.display()
    ));

    let mut cmd = Command::new(target.compiler.command());

    // Object files
    for obj in object_files {
        cmd.arg(path_arg(obj));
    }

    // Output file
    cmd.arg("-o").arg(path_arg(&output_path));

    // Add built dependency libraries (transitive, correct order)
    add_dependency_link_args(&mut cmd, dep_names, built_targets);

    // Library directories (-L)
    for lib_dir in &target.lib_dirs {
        cmd.arg("-L").arg(path_arg(lib_dir));
    }

    // Libraries (-l)
    for lib in &target.libs {
        cmd.arg("-l").arg(lib);
    }

    // Linker flags only (compiler flags are not passed to link)
    for flag in &target.linker_flags {
        cmd.arg(flag);
    }

    // Set LD_LIBRARY_PATH
    let ld_path = build_ld_library_path(&target.lib_dirs);
    if !ld_path.is_empty() {
        cmd.env("LD_LIBRARY_PATH", &ld_path);
    }

    run_command(cmd, messages)?;
    Ok(output_path)
}

// ---------------------------------------------------------------------------
// Create static library (.a)
// ---------------------------------------------------------------------------
fn create_static_lib(
    target: &ResolvedTarget,
    object_files: &[PathBuf],
    messages: &mut Vec<String>,
) -> Result<PathBuf, String> {
    let lib_name = format!("lib{}.a", target.name);
    let output_path = target.output_dir.join(&lib_name);

    messages.push(format!(
        "  [ARCHIVE] Creating static library: {}",
        output_path.display()
    ));

    // Archive with ar
    let mut cmd = Command::new("ar");
    cmd.arg("rcs");
    cmd.arg(path_arg(&output_path));

    for obj in object_files {
        cmd.arg(path_arg(obj));
    }

    run_command(cmd, messages)?;
    Ok(output_path)
}

// ---------------------------------------------------------------------------
// Create shared library (.so on Unix, .dll on Windows)
// ---------------------------------------------------------------------------
fn link_shared_lib(
    target: &ResolvedTarget,
    object_files: &[PathBuf],
    built_targets: &std::collections::HashMap<String, PathBuf>,
    dep_names: &[String],
    messages: &mut Vec<String>,
) -> Result<PathBuf, String> {
    let lib_name = shared_lib_filename(&target.name);
    let output_path = target.output_dir.join(&lib_name);

    messages.push(format!(
        "  [LINK] Creating shared library: {}",
        output_path.display()
    ));

    let mut cmd = Command::new(target.compiler.command());
    cmd.arg("-shared");

    // Object files
    for obj in object_files {
        cmd.arg(path_arg(obj));
    }

    // Output file
    cmd.arg("-o").arg(path_arg(&output_path));

    // Add dependency libraries (transitive, correct order)
    add_dependency_link_args(&mut cmd, dep_names, built_targets);

    // Library directories (-L)
    for lib_dir in &target.lib_dirs {
        cmd.arg("-L").arg(path_arg(lib_dir));
    }

    // Libraries (-l)
    for lib in &target.libs {
        cmd.arg("-l").arg(lib);
    }

    // Linker flags only
    for flag in &target.linker_flags {
        cmd.arg(flag);
    }

    // Set LD_LIBRARY_PATH
    let ld_path = build_ld_library_path(&target.lib_dirs);
    if !ld_path.is_empty() {
        cmd.env("LD_LIBRARY_PATH", &ld_path);
    }

    run_command(cmd, messages)?;
    Ok(output_path)
}

// ---------------------------------------------------------------------------
// Add link arguments for built dependency libraries
// ---------------------------------------------------------------------------
// We pass the full path to each built library so the linker always finds it.
// This avoids -l/ -L lookup differences (e.g. MinGW often looks for .a/.dll.a
// and may not resolve libfoo.so when given -lfoo).
fn add_dependency_link_args(
    cmd: &mut Command,
    dep_names: &[String],
    built_targets: &std::collections::HashMap<String, PathBuf>,
) {
    for dep_name in dep_names {
        if let Some(dep_path) = built_targets.get(dep_name) {
            if dep_path.exists() {
                cmd.arg(path_arg(dep_path));
            } else {
                // Fallback: -L dir -l name (for external or missing libs)
                if let Some(parent) = dep_path.parent() {
                    cmd.arg("-L").arg(path_arg(parent));
                }
                let filename = dep_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let lib_name = if filename.starts_with("lib") {
                    &filename[3..]
                } else {
                    &filename[..]
                };
                cmd.arg(format!("-l{}", lib_name));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Build LD_LIBRARY_PATH
// ---------------------------------------------------------------------------
/// Combines the given library directories with the current LD_LIBRARY_PATH.
fn build_ld_library_path(lib_dirs: &[PathBuf]) -> String {
    let mut paths: Vec<String> = lib_dirs
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Append existing LD_LIBRARY_PATH
    if let Ok(existing) = std::env::var("LD_LIBRARY_PATH") {
        for part in existing.split(':') {
            if !part.is_empty() {
                paths.push(part.to_string());
            }
        }
    }

    paths.join(":")
}

// ---------------------------------------------------------------------------
// Helper: run command and capture output
// ---------------------------------------------------------------------------
fn run_command(mut cmd: Command, messages: &mut Vec<String>) -> Result<(), String> {
    messages.push(format!("    Command: {:?}", cmd));

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stdout.is_empty() {
                messages.push(format!("    stdout: {}", stdout.trim()));
            }
            if !stderr.is_empty() {
                messages.push(format!("    stderr: {}", stderr.trim()));
            }

            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Command failed (exit code: {:?})\nstderr: {}",
                    output.status.code(),
                    stderr.trim()
                ))
            }
        }
        Err(e) => Err(format!("Failed to run command: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Print LD_LIBRARY_PATH to console (for debugging)
// ---------------------------------------------------------------------------
pub fn print_ld_library_path_info() {
    match std::env::var("LD_LIBRARY_PATH") {
        Ok(val) => {
            println!("[LD_LIBRARY_PATH] Current value:");
            for path in val.split(':') {
                if !path.is_empty() {
                    let exists = Path::new(path).exists();
                    println!(
                        "  {} {}",
                        if exists { "✓" } else { "✗" },
                        path
                    );
                }
            }
        }
        Err(_) => {
            println!("[LD_LIBRARY_PATH] Not set.");
        }
    }
}
