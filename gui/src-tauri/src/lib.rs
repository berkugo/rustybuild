use oximake::{find_workspace_root, parse_and_graph, build_and_collect_output, build_and_stream_output, clean_project_and_stream_output, convert_cmake_to_toml_files};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use tauri::{Emitter, EventTarget, Manager};

/// Shared flag for build cancellation (set by cancel_build, read by build loop).
struct BuildCancel(Arc<AtomicBool>);

#[tauri::command]
fn parse_build_toml(path: String) -> Result<oximake::ProjectInfo, String> {
    parse_and_graph(PathBuf::from(path).as_path())
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn run_build(
    config_path: String,
    targets: Option<Vec<String>>,
    clean: bool,
    jobs: Option<u32>,
    ignore_errors: bool,
) -> Result<BuildOutput, String> {
    let (success, lines) = build_and_collect_output(
        PathBuf::from(&config_path).as_path(),
        targets,
        clean,
        jobs.map(|j| j as usize),
        ignore_errors,
    )?;
    Ok(BuildOutput { success, lines })
}

/// Payload for build-finished event (thread-safe serialization to frontend).
#[derive(Clone, serde::Serialize)]
struct BuildFinishedPayload {
    success: bool,
    total: usize,
    successful: usize,
    failed: usize,
}

/// Request cancellation of the current build. Build will stop after the current job.
#[tauri::command]
fn cancel_build(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(state) = app.try_state::<BuildCancel>() {
        state.0.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Starts build (or clean+build) in a background thread and emits progress via events.
/// Emits are scheduled on the main thread so the frontend receives them.
/// jobs: None = auto. ignore_errors: continue after a target fails (like make -i).
#[tauri::command]
fn run_build_async(
    app: tauri::AppHandle,
    config_path: String,
    targets: Option<Vec<String>>,
    clean: bool,
    jobs: Option<u32>,
    ignore_errors: bool,
) -> Result<(), String> {
    let cancel = app.try_state::<BuildCancel>().map(|s| s.0.clone()).unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    cancel.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send("[INFO] Build started.".to_string());
        let path = PathBuf::from(&config_path);
        // Use workspace root if this file is included by a parent build.toml (CMake-like: build from root).
        let path = match find_workspace_root(path.as_path()) {
            Some(root) => {
                let _ = tx.send(format!("[INFO] Using workspace root: {}", root.display()));
                root
            }
            None => path,
        };
        let tx_for_error = tx.clone();
        let jobs_usize = jobs.map(|j| j as usize);
        let build_handle = std::thread::spawn(move || {
            match build_and_stream_output(path.as_path(), targets, clean, jobs_usize, ignore_errors, tx, Some(cancel)) {
                Ok(_) => {}
                Err(e) => {
                    let _ = tx_for_error.send(format!("[ERROR] {}", e));
                    let _ = tx_for_error.send("__OXIMAKE_FINISH__\tfalse\t0\t0\t1".to_string());
                }
            }
        });
        for line in rx {
            if line.starts_with("__OXIMAKE_FINISH__") {
                let parts: Vec<&str> = line.split('\t').collect();
                let success = parts.get(1).map(|s| *s == "true").unwrap_or(false);
                let total = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                let successful = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                let failed = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
                let payload = BuildFinishedPayload {
                    success,
                    total,
                    successful,
                    failed,
                };
                let app_emit = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-finished", payload);
                });
            } else {
                let app_emit = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-output", line);
                });
            }
        }
        if let Err(e) = build_handle.join() {
            let err_msg = format!("[ERROR] Build thread panicked: {:?}", e);
            let app_emit = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-output", err_msg);
                let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-finished", BuildFinishedPayload {
                    success: false,
                    total: 0,
                    successful: 0,
                    failed: 1,
                });
            });
        }
    });
    Ok(())
}

/// Cleans the project only (removes .o, .a, .so, exes in output dirs). Emits same events as build for UI.
#[tauri::command]
fn run_clean_async(app: tauri::AppHandle, config_path: String) -> Result<(), String> {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let path = PathBuf::from(&config_path);
        let path = match find_workspace_root(path.as_path()) {
            Some(root) => {
                let _ = tx.send(format!("[INFO] Using workspace root: {}", root.display()));
                root
            }
            None => path,
        };
        let tx_for_error = tx.clone();
        let clean_handle = std::thread::spawn(move || {
            match clean_project_and_stream_output(path.as_path(), tx) {
                Ok(()) => {}
                Err(e) => {
                    let _ = tx_for_error.send(format!("[ERROR] {}", e));
                    let _ = tx_for_error.send("__OXIMAKE_FINISH__\tfalse\t0\t0\t1".to_string());
                }
            }
        });
        for line in rx {
            if line.starts_with("__OXIMAKE_FINISH__") {
                let parts: Vec<&str> = line.split('\t').collect();
                let success = parts.get(1).map(|s| *s == "true").unwrap_or(false);
                let total = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                let successful = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                let failed = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
                let payload = BuildFinishedPayload {
                    success,
                    total,
                    successful,
                    failed,
                };
                let app_emit = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-finished", payload);
                });
            } else {
                let app_emit = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-output", line);
                });
            }
        }
        if let Err(e) = clean_handle.join() {
            let err_msg = format!("[ERROR] Clean thread panicked: {:?}", e);
            let app_emit = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-output", err_msg);
                let _ = app_emit.emit_to(EventTarget::webview_window("main"), "build-finished", BuildFinishedPayload {
                    success: false,
                    total: 0,
                    successful: 0,
                    failed: 1,
                });
            });
        }
    });
    Ok(())
}

/// Returns the number of logical CPUs (for jobs selector). At least 1.
#[tauri::command]
fn get_max_jobs() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(64) as u32)
        .unwrap_or(1)
        .max(1)
}

#[tauri::command]
fn open_file_dialog() -> Result<Option<String>, String> {
    use rfd::FileDialog;
    
    let path = FileDialog::new()
        .add_filter("TOML Files", &["toml"])
        .add_filter("All Files", &["*"])
        .set_title("Select build.toml file")
        .pick_file();
    
    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn open_cmake_dialog() -> Result<Option<String>, String> {
    use rfd::FileDialog;
    
    let path = FileDialog::new()
        .add_filter("CMakeLists.txt", &["txt"])
        .add_filter("All Files", &["*"])
        .set_title("Select CMakeLists.txt file")
        .pick_file();
    
    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn convert_cmake_to_toml(cmake_path: String) -> Result<ConvertResult, String> {
    use std::fs;

    let path = PathBuf::from(&cmake_path);
    let base_path = path.parent().ok_or("Invalid path")?;
    
    // Convert to multiple build.toml files (one per directory)
    let toml_files = convert_cmake_to_toml_files(&path)?;
    
    // Write all build.toml files to disk
    let mut build_toml_files = Vec::new();
    for (rel_path, content) in &toml_files {
        let full_path = if rel_path == "build.toml" {
            base_path.join("build.toml")
        } else {
            base_path.join(rel_path)
        };
        
        // Create directory if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        
        fs::write(&full_path, content).map_err(|e| format!("Failed to write {}: {}", full_path.display(), e))?;
        build_toml_files.push(full_path.to_string_lossy().to_string());
    }
    
    // Get root build.toml content
    let root_toml = toml_files.get("build.toml")
        .ok_or("Root build.toml not found")?;
    
    Ok(ConvertResult {
        toml_content: root_toml.clone(),
        project_root: base_path.to_string_lossy().to_string(),
        build_toml_files,
    })
}

#[tauri::command]
fn find_build_toml_files(root_path: String) -> Result<Vec<String>, String> {
    use std::fs;
    
    let root = PathBuf::from(root_path);
    let mut files = Vec::new();
    
    fn walk_dir(dir: &std::path::Path, files: &mut Vec<String>) -> Result<(), String> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                
                if path.is_dir() {
                    // Skip hidden directories and common build/output directories
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !dir_name.starts_with('.') && 
                       dir_name != "build" && 
                       dir_name != "target" &&
                       dir_name != "node_modules" {
                        walk_dir(&path, files)?;
                    }
                } else if path.is_file() {
                    if path.file_name().and_then(|n| n.to_str()) == Some("build.toml") {
                        files.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
        Ok(())
    }
    
    walk_dir(&root, &mut files)?;
    Ok(files)
}

#[tauri::command]
fn init_project(project_name: String, cpp_version: String, project_type: String) -> Result<InitResult, String> {
    use std::fs;
    use rfd::FileDialog;
    
    // Open directory picker
    let project_dir = FileDialog::new()
        .set_title("Select directory for new project")
        .pick_folder()
        .ok_or("No directory selected")?;
    
    let project_path = project_dir.join(&project_name);
    
    // Check if directory already exists
    if project_path.exists() {
        return Err(format!("Directory '{}' already exists", project_name));
    }
    
    // Create project directory
    fs::create_dir_all(&project_path).map_err(|e| format!("Failed to create directory: {}", e))?;
    
    // Create directory structure
    let src_dir = project_path.join("src");
    let include_dir = project_path.join("include");
    let build_dir = project_path.join("build");
    
    fs::create_dir_all(&src_dir).map_err(|e| format!("Failed to create src: {}", e))?;
    fs::create_dir_all(&include_dir).map_err(|e| format!("Failed to create include: {}", e))?;
    fs::create_dir_all(&build_dir).map_err(|e| format!("Failed to create build: {}", e))?;
    
    // Generate build.toml content
    let mut toml_content = format!(
        r#"[project]
name = "{}"
version = "0.1.0"

"#,
        project_name
    );
    
    match project_type.as_str() {
        "executable" => {
            toml_content.push_str(
                &format!(
                    r#"[[target]]
name = "{}"
type = "executable"
sources = ["src/**/*.cpp"]
include_dirs = ["include"]
flags = ["-O2", "-Wall", "-std=c++{}"]
compiler = "g++"
output_dir = "build"
"#,
                    project_name, cpp_version
                )
            );
            
            // Create main.cpp
            let main_cpp = format!(
                r#"#include <iostream>

int main() {{
    std::cout << "Hello from {}!" << std::endl;
    return 0;
}}
"#,
                project_name
            );
            fs::write(src_dir.join("main.cpp"), main_cpp)
                .map_err(|e| format!("Failed to create main.cpp: {}", e))?;
        }
        "library" => {
            toml_content.push_str(
                &format!(
                    r#"[[target]]
name = "{}"
type = "static"
sources = ["src/**/*.cpp"]
include_dirs = ["include"]
flags = ["-O2", "-Wall", "-std=c++{}"]
compiler = "g++"
output_dir = "build"
"#,
                    project_name, cpp_version
                )
            );
            
            // Create library header
            let header_name = format!("{}.h", project_name);
            let header_content = format!(
                r#"#ifndef {}_H
#define {}_H

namespace {} {{
    void hello();
}}

#endif
"#,
                project_name.to_uppercase(),
                project_name.to_uppercase(),
                project_name
            );
            fs::write(include_dir.join(&header_name), header_content)
                .map_err(|e| format!("Failed to create header: {}", e))?;
            
            // Create library source
            let lib_cpp = format!(
                r#"#include "{}.h"
#include <iostream>

namespace {} {{
    void hello() {{
        std::cout << "Hello from {} library!" << std::endl;
    }}
}}
"#,
                project_name, project_name, project_name
            );
            fs::write(src_dir.join(format!("{}.cpp", project_name)), lib_cpp)
                .map_err(|e| format!("Failed to create library source: {}", e))?;
        }
        "mixed" => {
            toml_content.push_str(
                &format!(
                    r#"[[target]]
name = "{}_lib"
type = "static"
sources = ["src/lib/**/*.cpp"]
include_dirs = ["include"]
flags = ["-O2", "-Wall", "-std=c++{}"]
compiler = "g++"
output_dir = "build"

[[target]]
name = "{}"
type = "executable"
sources = ["src/main.cpp"]
include_dirs = ["include"]
libs = ["{}_lib"]
flags = ["-O2", "-Wall", "-std=c++{}"]
compiler = "g++"
output_dir = "build"
"#,
                    project_name, cpp_version, project_name, project_name, cpp_version
                )
            );
            
            // Create lib directory
            let lib_dir = src_dir.join("lib");
            fs::create_dir_all(&lib_dir).map_err(|e| format!("Failed to create lib: {}", e))?;
            
            // Create main.cpp
            let main_cpp = format!(
                r#"#include <iostream>
#include "{}.h"

int main() {{
    {}::hello();
    std::cout << "Hello from {}!" << std::endl;
    return 0;
}}
"#,
                project_name, project_name, project_name
            );
            fs::write(src_dir.join("main.cpp"), main_cpp)
                .map_err(|e| format!("Failed to create main.cpp: {}", e))?;
            
            // Create library header
            let header_name = format!("{}.h", project_name);
            let header_content = format!(
                r#"#ifndef {}_H
#define {}_H

namespace {} {{
    void hello();
}}

#endif
"#,
                project_name.to_uppercase(),
                project_name.to_uppercase(),
                project_name
            );
            fs::write(include_dir.join(&header_name), header_content)
                .map_err(|e| format!("Failed to create header: {}", e))?;
            
            // Create library source
            let lib_cpp = format!(
                r#"#include "{}.h"
#include <iostream>

namespace {} {{
    void hello() {{
        std::cout << "Hello from {} library!" << std::endl;
    }}
}}
"#,
                project_name, project_name, project_name
            );
            fs::write(lib_dir.join(format!("{}.cpp", project_name)), lib_cpp)
                .map_err(|e| format!("Failed to create library source: {}", e))?;
        }
        _ => return Err(format!("Unknown project type: {}", project_type)),
    }
    
    // Write build.toml
    let build_toml_path = project_path.join("build.toml");
    fs::write(&build_toml_path, &toml_content)
        .map_err(|e| format!("Failed to write build.toml: {}", e))?;
    
    Ok(InitResult {
        toml_content,
        config_path: build_toml_path.to_string_lossy().to_string(),
        project_root: project_path.to_string_lossy().to_string(),
    })
}

#[derive(serde::Serialize)]
struct InitResult {
    toml_content: String,
    config_path: String,
    project_root: String,
}

#[derive(serde::Serialize)]
struct ConvertResult {
    toml_content: String,
    project_root: String,
    build_toml_files: Vec<String>,
}

#[derive(serde::Serialize)]
struct BuildOutput {
    success: bool,
    lines: Vec<String>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(BuildCancel(Arc::new(AtomicBool::new(false))))
        .invoke_handler(tauri::generate_handler![
            parse_build_toml,
            read_file,
            write_file,
            run_build,
            run_build_async,
            cancel_build,
            run_clean_async,
            get_max_jobs,
            open_file_dialog,
            open_cmake_dialog,
            convert_cmake_to_toml,
            find_build_toml_files,
            init_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
