// ============================================================================
// main.rs — ngmake CLI entry point (binary: ngm)
// ============================================================================

mod config;
mod dag;
mod compiler;
mod builder;
mod options;
mod cmake_converter;

use std::path::PathBuf;
use std::process;
use std::time::Instant;

use clap::Parser;
use options::BuildOptions;

fn main() {
    let options = BuildOptions::parse();

    // Handle subcommands
    if let Some(command) = &options.command {
        match command {
            options::Command::Convert { cmake, output, verbose } => {
                use std::path::Path;
                use std::fs;
                
                println!("Converting CMakeLists.txt: {}", cmake.display());
                println!("⚠️  Note: CMake converter is in BETA. Complex CMake projects may require manual adjustments.");
                
                match cmake_converter::convert_cmake_to_toml_files(cmake.as_path()) {
                    Ok(toml_files) => {
                        let base_path = cmake.parent().unwrap_or(Path::new("."));
                        let mut created_files = Vec::new();
                        
                        // Write all build.toml files
                        for (rel_path, content) in &toml_files {
                            let full_path = if rel_path == "build.toml" {
                                base_path.join("build.toml")
                            } else {
                                base_path.join(rel_path)
                            };
                            
                            // Create directory if needed
                            if let Some(parent) = full_path.parent() {
                                if let Err(e) = fs::create_dir_all(parent) {
                                    eprintln!("✗ Error creating directory {}: {}", parent.display(), e);
                                    std::process::exit(1);
                                }
                            }
                            
                            match fs::write(&full_path, content) {
                                Ok(_) => {
                                    created_files.push(full_path.clone());
                                    if *verbose {
                                        println!("  Created: {}", full_path.display());
                                    }
                                }
                                Err(e) => {
                                    eprintln!("✗ Error writing {}: {}", full_path.display(), e);
                                    std::process::exit(1);
                                }
                            }
                        }
                        
                        println!("✓ Converted successfully!");
                        println!("  Created {} build.toml file(s):", created_files.len());
                        for file in &created_files {
                            println!("    - {}", file.display());
                        }
                        
                        // If output was specified, also write the root build.toml there
                        if let Some(output_path) = output {
                            if let Some(root_toml) = toml_files.get("build.toml") {
                                if let Err(e) = fs::write(&output_path, root_toml) {
                                    eprintln!("✗ Error writing output file {}: {}", output_path.display(), e);
                                    std::process::exit(1);
                                }
                                println!("  Root build.toml also written to: {}", output_path.display());
                            }
                        }
                        
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("✗ Error converting CMakeLists.txt: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            options::Command::Build { config, target, clean, verbose, quiet, no_ld_path, jobs, ignore_errors } => {
                let build_options = BuildOptions {
                    command: None,
                    config: config.clone(),
                    targets: target.clone(),
                    clean: *clean,
                    verbose: *verbose,
                    quiet: *quiet,
                    no_ld_path: *no_ld_path,
                    jobs: *jobs,
                    ignore_errors: *ignore_errors,
                };
                run_build(build_options);
                return;
            }
            options::Command::Clean { config, verbose } => {
                run_clean(config.as_path(), *verbose);
                return;
            }
            options::Command::Init { name, cpp_version, project_type, dir } => {
                use std::fs;
                use std::env;
                
                let project_name = name.clone().unwrap_or_else(|| {
                    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    current_dir.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("my_project")
                        .to_string()
                });
                
                let target_dir = dir.clone().unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
                let project_path = if name.is_some() {
                    target_dir.join(&project_name)
                } else {
                    target_dir.clone()
                };
                
                // Check if directory already exists (only if creating new subdirectory)
                if name.is_some() && project_path.exists() {
                    eprintln!("✗ Error: Directory '{}' already exists", project_name);
                    std::process::exit(1);
                }
                
                // Create project directory if needed
                if name.is_some() {
                    fs::create_dir_all(&project_path).unwrap_or_else(|e| {
                        eprintln!("✗ Error creating directory: {}", e);
                        std::process::exit(1);
                    });
                }
                
                // Create directory structure
                let src_dir = project_path.join("src");
                let include_dir = project_path.join("include");
                let build_dir = project_path.join("build");
                
                fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
                    eprintln!("✗ Error creating src directory: {}", e);
                    std::process::exit(1);
                });
                fs::create_dir_all(&include_dir).unwrap_or_else(|e| {
                    eprintln!("✗ Error creating include directory: {}", e);
                    std::process::exit(1);
                });
                fs::create_dir_all(&build_dir).unwrap_or_else(|e| {
                    eprintln!("✗ Error creating build directory: {}", e);
                    std::process::exit(1);
                });
                
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
                        fs::write(src_dir.join("main.cpp"), main_cpp).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating main.cpp: {}", e);
                            std::process::exit(1);
                        });
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
                        fs::write(include_dir.join(&header_name), header_content).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating header: {}", e);
                            std::process::exit(1);
                        });
                        
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
                        fs::write(src_dir.join(format!("{}.cpp", project_name)), lib_cpp).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating library source: {}", e);
                            std::process::exit(1);
                        });
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
                        fs::create_dir_all(&lib_dir).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating lib directory: {}", e);
                            std::process::exit(1);
                        });
                        
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
                        fs::write(src_dir.join("main.cpp"), main_cpp).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating main.cpp: {}", e);
                            std::process::exit(1);
                        });
                        
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
                        fs::write(include_dir.join(&header_name), header_content).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating header: {}", e);
                            std::process::exit(1);
                        });
                        
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
                        fs::write(lib_dir.join(format!("{}.cpp", project_name)), lib_cpp).unwrap_or_else(|e| {
                            eprintln!("✗ Error creating library source: {}", e);
                            std::process::exit(1);
                        });
                    }
                    _ => {
                        eprintln!("✗ Error: Unknown project type '{}'. Use: executable, library, or mixed", project_type);
                        std::process::exit(1);
                    }
                }
                
                // Write build.toml
                let build_toml_path = project_path.join("build.toml");
                fs::write(&build_toml_path, &toml_content).unwrap_or_else(|e| {
                    eprintln!("✗ Error writing build.toml: {}", e);
                    std::process::exit(1);
                });
                
                println!("✓ Project '{}' initialized successfully!", project_name);
                println!("  Location: {}", project_path.display());
                println!("  C++ Standard: C++{}", cpp_version);
                println!("  Project Type: {}", project_type);
                println!("  Build config: {}", build_toml_path.display());
                println!("\n  Next steps:");
                println!("    cd {}", if name.is_some() { &project_name } else { "." });
                println!("    ngm build");
                
                std::process::exit(0);
            }
        }
    }

    // Legacy mode: no subcommand, use direct options
    let build_path = &options.config;
    if !build_path.exists() {
        eprintln!(
            "[ERROR] File '{}' not found!\n\
             Usage: ngm --config <file> or ngm -c <file>",
            build_path.display()
        );
        process::exit(1);
    }

    let start_time = Instant::now();
    let quiet = options.show_quiet_output();

    // --- LD_LIBRARY_PATH (optional) ---
    if !options.no_ld_path && !quiet && options.verbose {
        println!("─────────────────────────────────────────────────────");
        println!("  ngmake v{}", env!("CARGO_PKG_VERSION"));
        println!("─────────────────────────────────────────────────────");
        compiler::print_ld_library_path_info();
        println!();
    }

    if !quiet && options.verbose {
        println!("[1/3] Parsing configuration file: {}", build_path.display());
    }

    let project = match config::parse_build_file(build_path.as_path(), options.show_verbose_output()) {
        Ok(p) => {
            if !quiet && options.verbose {
                println!(
                    "  Project: {} v{} ({} targets loaded)",
                    p.name,
                    p.version,
                    p.targets.len()
                );
            }
            p
        }
        Err(e) => {
            eprintln!("[ERROR] Configuration parse error: {}", e);
            process::exit(1);
        }
    };

    if project.targets.is_empty() {
        if !quiet {
            println!("[INFO] No targets defined. Nothing to do.");
        }
        return;
    }

    if !quiet && options.verbose {
        println!("\n  Defined targets:");
        for (name, target) in &project.targets {
            println!(
                "    • {} ({:?}) - {} source file(s), deps: {:?}",
                name,
                target.target_type,
                target.sources.len(),
                target.deps
            );
        }
        println!();
    }

    // --- Clean (remove output dirs first) ---
    if options.clean {
        let mut dirs: Vec<_> = project
            .targets
            .values()
            .map(|t| t.output_dir.clone())
            .collect();
        dirs.sort();
        dirs.dedup();
        for d in &dirs {
            if d.exists() {
                if !quiet && options.verbose {
                    println!("[CLEAN] Removing {}...", d.display());
                }
                let _ = std::fs::remove_dir_all(d);
            }
        }
        if !quiet && options.verbose {
            println!();
        }
    }

    if !quiet && options.verbose {
        println!("[2/3] Building dependency graph (DAG)...");
    }

    let full_order = match dag::build_order(&project) {
        Ok(order) => order,
        Err(e) => {
            eprintln!("[ERROR] Dependency resolution error: {}", e);
            process::exit(1);
        }
    };

    // Filter order for selected targets only
    let build_order = match &options.targets {
        Some(t) => match dag::filter_order_for_targets(&project, &full_order, t) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[ERROR] {}", e);
                process::exit(1);
            }
        },
        None => full_order,
    };

    if build_order.levels.is_empty() {
        if !quiet {
            println!("[INFO] No targets to build.");
        }
        return;
    }

    if !quiet && options.verbose {
        println!("[DAG] Topological order:");
        for (i, level) in build_order.levels.iter().enumerate() {
            println!("  Level {}: {:?}", i, level);
        }
        println!();
    }

    // --- Build ---
    if !quiet && options.verbose {
        println!("[3/3] Starting build...\n");
    }

    let result = builder::build_project(&project, &build_order, &options, None, None);

    // --- Report ---
    let elapsed = start_time.elapsed();
    if !quiet {
        if options.verbose {
            println!("\n─────────────────────────────────────────────────────");
            println!("  Build Report");
            println!("─────────────────────────────────────────────────────");
            println!("  Total targets : {}", result.total_targets);
            println!("  Successful    : {}", result.successful_targets);
            println!("  Failed        : {}", result.failed_targets);
            println!("  Duration      : {:.2?}", elapsed);
            println!(
                "  Status        : {}",
                if result.success { "SUCCESS ✓" } else { "FAILED ✗" }
            );
            println!("─────────────────────────────────────────────────────");
        } else if result.success {
            println!("  {} targets in {:.2?}", result.successful_targets, elapsed);
        } else {
            println!(
                "  {} OK, {} failed in {:.2?}",
                result.successful_targets,
                result.failed_targets,
                elapsed
            );
        }
    } else if result.success {
        println!(
            "OK {} targets, {:.2?}",
            result.successful_targets,
            elapsed
        );
    }

    if !result.success {
        process::exit(1);
    }
}

fn run_clean(config_path: &std::path::Path, verbose: bool) {
    if !config_path.exists() {
        eprintln!(
            "[ERROR] File '{}' not found!\n\
             Usage: ngm clean --config <file> or ngm clean -c <file>",
            config_path.display()
        );
        process::exit(1);
    }
    let project = match config::parse_build_file(config_path, false) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[ERROR] {}", e);
            process::exit(1);
        }
    };
    let mut dirs: Vec<_> = project.targets.values().map(|t| t.output_dir.clone()).collect();
    dirs.sort();
    dirs.dedup();
    let mut removed = 0usize;
    for d in &dirs {
        if d.exists() {
            if verbose {
                println!("  Removing {}", d.display());
            }
            if let Err(e) = std::fs::remove_dir_all(d) {
                eprintln!("[ERROR] Failed to remove {}: {}", d.display(), e);
                process::exit(1);
            }
            removed += 1;
        }
    }
    if removed == 0 {
        println!("  No build output to clean.");
    } else {
        println!("  Cleaned {} director{}.", removed, if removed == 1 { "y" } else { "ies" });
    }
}

fn run_build(options: BuildOptions) {
    let build_path = &options.config;
    if !build_path.exists() {
        eprintln!(
            "[ERROR] File '{}' not found!\n\
             Usage: ngm --config <file> or ngm -c <file>",
            build_path.display()
        );
        process::exit(1);
    }

    let start_time = Instant::now();
    let quiet = options.show_quiet_output();

    // --- LD_LIBRARY_PATH (only in verbose) ---
    if !options.no_ld_path && !quiet && options.verbose {
        println!("─────────────────────────────────────────────────────");
        println!("  ngmake v{}", env!("CARGO_PKG_VERSION"));
        println!("─────────────────────────────────────────────────────");
        compiler::print_ld_library_path_info();
        println!();
    }

    // --- Parse configuration ---
    if !quiet && options.verbose {
        println!("[1/3] Parsing configuration file: {}", build_path.display());
    }

    let project = match config::parse_build_file(build_path.as_path(), options.show_verbose_output()) {
        Ok(p) => {
            if !quiet && options.verbose {
                println!(
                    "  Project: {} v{} ({} targets loaded)",
                    p.name,
                    p.version,
                    p.targets.len()
                );
            }
            p
        }
        Err(e) => {
            eprintln!("[ERROR] Configuration parse error: {}", e);
            process::exit(1);
        }
    };

    if project.targets.is_empty() {
        if !quiet {
            println!("[INFO] No targets defined. Nothing to do.");
        }
        return;
    }

    if !quiet && options.verbose {
        println!("\n  Defined targets:");
        for (name, target) in &project.targets {
            println!(
                "    • {} ({:?}) - {} source file(s), deps: {:?}",
                name,
                target.target_type,
                target.sources.len(),
                target.deps
            );
        }
        println!();
    }

    // --- Clean (remove output dirs first) ---
    if options.clean {
        let mut dirs: Vec<_> = project
            .targets
            .values()
            .map(|t| t.output_dir.clone())
            .collect();
        dirs.sort();
        dirs.dedup();
        for d in &dirs {
            if d.exists() {
                if !quiet && options.verbose {
                    println!("[CLEAN] Removing {}...", d.display());
                }
                let _ = std::fs::remove_dir_all(d);
            }
        }
        if !quiet && options.verbose {
            println!();
        }
    }

    // --- DAG and topological order ---
    if !quiet && options.verbose {
        println!("[2/3] Building dependency graph (DAG)...");
    }

    let full_order = match dag::build_order(&project) {
        Ok(order) => order,
        Err(e) => {
            eprintln!("[ERROR] Dependency resolution error: {}", e);
            process::exit(1);
        }
    };

    // Filter order for selected targets only
    let build_order = match &options.targets {
        Some(t) => match dag::filter_order_for_targets(&project, &full_order, t) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[ERROR] {}", e);
                process::exit(1);
            }
        },
        None => full_order,
    };

    if build_order.levels.is_empty() {
        if !quiet {
            println!("[INFO] No targets to build.");
        }
        return;
    }

    if !quiet && options.verbose {
        println!("[DAG] Topological order:");
        for (i, level) in build_order.levels.iter().enumerate() {
            println!("  Level {}: {:?}", i, level);
        }
        println!();
    }

    // --- Build ---
    if !quiet && options.verbose {
        println!("[3/3] Starting build...\n");
    }

    let result = builder::build_project(&project, &build_order, &options, None, None);

    let elapsed = start_time.elapsed();
    if !quiet {
        if options.verbose {
            println!("\n─────────────────────────────────────────────────────");
            println!("  Build Report");
            println!("─────────────────────────────────────────────────────");
            println!("  Total targets : {}", result.total_targets);
            println!("  Successful    : {}", result.successful_targets);
            println!("  Failed        : {}", result.failed_targets);
            println!("  Duration      : {:.2?}", elapsed);
            println!(
                "  Status        : {}",
                if result.success { "SUCCESS ✓" } else { "FAILED ✗" }
            );
            println!("─────────────────────────────────────────────────────");
        } else if result.success {
            println!("  {} targets in {:.2?}", result.successful_targets, elapsed);
        } else {
            println!(
                "  {} OK, {} failed in {:.2?}",
                result.successful_targets,
                result.failed_targets,
                elapsed
            );
        }
    } else if result.success {
        println!(
            "OK {} targets, {:.2?}",
            result.successful_targets,
            elapsed
        );
    }

    if !result.success {
        process::exit(1);
    }
}
