use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CmakeTarget {
    pub name: String,
    pub target_type: String, // "executable", "static_lib", "shared_lib"
    pub sources: Vec<String>,
    pub include_dirs: Vec<String>,
    pub lib_dirs: Vec<String>,
    pub libs: Vec<String>,
    pub flags: Vec<String>,
    pub deps: Vec<String>,
    pub compile_definitions: Vec<String>,
    pub source_dir: Option<String>, // Directory where this target's CMakeLists.txt is located (relative to project root)
}

#[derive(Debug, Clone)]
pub struct CmakeProject {
    pub name: String,
    pub version: String,
    pub targets: Vec<CmakeTarget>,
    pub subdirectories: Vec<String>,
    pub variables: HashMap<String, String>,
}

// Extract a CMake command with proper parenthesis matching (handles multiline)
fn extract_command(content: &str, command: &str) -> Vec<Vec<String>> {
    let mut results = Vec::new();
    // Match command at word boundary, case-insensitive
    let pattern = format!(r#"\b{}\s*\("#, regex::escape(command));
    
    if let Ok(re) = regex::RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
    {
        for mat in re.find_iter(content) {
            let start = mat.end();
            let mut depth = 1;
            let mut end = start;
            let mut in_string = false;
            let mut escape_next = false;
            let mut found_end = false;
            
            for (i, ch) in content[start..].char_indices() {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                
                match ch {
                    '\\' if in_string => {
                        escape_next = true;
                    }
                    '"' => {
                        in_string = !in_string;
                    }
                    '(' if !in_string => {
                        depth += 1;
                    }
                    ')' if !in_string => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i;
                            found_end = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            
            if found_end {
                let args_str = &content[start..end];
                // Split arguments, handling quoted strings
                let args = parse_cmake_args(args_str);
                if !args.is_empty() {
                    results.push(args);
                }
            }
        }
    }
    
    results
}

// Parse CMake arguments, handling quoted strings and variables
fn parse_cmake_args(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;
    
    for ch in args_str.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        
        match ch {
            '\\' => {
                escape_next = true;
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' | '\n' | '\r' if !in_quotes => {
                if !current.is_empty() {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        args.push(trimmed);
                    }
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    
    if !current.is_empty() {
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            args.push(trimmed);
        }
    }
    
    args
}

// Normalize include directory path relative to project root
fn normalize_include_path(path: &str, cmake_dir_str: &str) -> String {
    // If path starts with cmake_dir_str, it's already relative to project root
    // Otherwise, join cmake_dir_str with path and normalize
    let full_path = if path.starts_with(cmake_dir_str) {
        Path::new(path).to_path_buf()
    } else {
        Path::new(cmake_dir_str).join(path)
    };
    
    let mut parts = Vec::new();
    for component in full_path.components() {
        match component {
            std::path::Component::Normal(p) => {
                parts.push(p.to_string_lossy().to_string());
            }
            std::path::Component::ParentDir => {
                if !parts.is_empty() {
                    parts.pop();
                }
            }
            std::path::Component::CurDir => {}
            _ => {}
        }
    }
    
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

// Resolve CMake variables (simple resolution)
fn resolve_variable(value: &str, variables: &HashMap<String, String>) -> String {
    let mut result = value.to_string();
    let var_pattern = regex::Regex::new(r#"\$\{([^}]+)\}"#).unwrap();
    
    for cap in var_pattern.captures_iter(value) {
        let var_name = &cap[1];
        if let Some(var_value) = variables.get(var_name) {
            result = result.replace(&format!("${{{}}}", var_name), var_value);
        } else {
            // Common CMake variables
            match var_name {
                "CMAKE_CURRENT_SOURCE_DIR" | "CMAKE_SOURCE_DIR" | "PROJECT_SOURCE_DIR" => {
                    result = result.replace(&format!("${{{}}}", var_name), ".");
                }
                "CMAKE_CURRENT_BINARY_DIR" | "CMAKE_BINARY_DIR" | "PROJECT_BINARY_DIR" => {
                    result = result.replace(&format!("${{{}}}", var_name), "build");
                }
                "CMAKE_MODULE_PATH" => {
                    result = result.replace(&format!("${{{}}}", var_name), "cmake");
                }
                _ => {
                    // Leave as is if unknown
                }
            }
        }
    }
    
    result
}

pub fn parse_cmake_lists(path: &Path) -> Result<CmakeProject, String> {
    // Store the root CMakeLists.txt path for relative path calculations
    let root_path = path.parent().unwrap_or_else(|| Path::new("."));
    parse_cmake_lists_recursive(path, root_path, true, 0)
}

fn parse_cmake_lists_recursive(path: &Path, root_path: &Path, is_root: bool, depth: usize) -> Result<CmakeProject, String> {
    // Limit recursion depth to avoid infinite loops
    if depth > 10 {
        return Err("Maximum recursion depth exceeded".to_string());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CMakeLists.txt: {}", e))?;

    // Get the directory containing this CMakeLists.txt
    let cmake_dir = path.parent().unwrap_or_else(|| Path::new("."));
    
    // Calculate relative path from root_path to cmake_dir (for PROJECT_SOURCE_DIR variable)
    let cmake_dir_str = if is_root {
        ".".to_string()
    } else {
        cmake_dir.strip_prefix(root_path)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| ".".to_string())
    };
    
    // Calculate relative path from root_path to cmake_dir (for source_dir in targets)
    let cmake_dir_str_for_targets = if is_root {
        None // Root CMakeLists.txt - sources are relative to project root
    } else {
        // For subdirectories, calculate relative path from root_path
        match cmake_dir.strip_prefix(root_path) {
            Ok(rel_path) => {
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                if rel_str.is_empty() {
                    None
                } else {
                    Some(rel_str)
                }
            }
            Err(_) => {
                // If strip_prefix fails, use the directory name as fallback
                Some(cmake_dir.file_name().unwrap_or_default().to_string_lossy().to_string())
            }
        }
    };
    
    let mut project = CmakeProject {
        name: "unnamed_project".to_string(),
        version: "0.1.0".to_string(),
        targets: Vec::new(),
        subdirectories: Vec::new(),
        variables: HashMap::new(),
    };
    
    // Set PROJECT_SOURCE_DIR to the directory containing this CMakeLists.txt (relative to root)
    // This is needed for resolving paths in generator expressions like $<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/include>
    project.variables.insert("PROJECT_SOURCE_DIR".to_string(), cmake_dir_str.clone());
    // Also set CMAKE_CURRENT_SOURCE_DIR
    project.variables.insert("CMAKE_CURRENT_SOURCE_DIR".to_string(), cmake_dir_str.clone());

    // Remove comments
    let content_no_comments: String = content
        .lines()
        .map(|line| {
            if let Some(comment_pos) = line.find('#') {
                &line[..comment_pos]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Parse set() commands for variables
    for cmd_args in extract_command(&content_no_comments, "set") {
        if cmd_args.len() >= 2 {
            let var_name = cmd_args[0].clone();
            // Skip CACHE, INTERNAL, FORCE, etc. keywords
            let mut value_parts = Vec::new();
            let mut skip_next = false;
            for (_i, arg) in cmd_args.iter().enumerate().skip(1) {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                let arg_upper = arg.to_uppercase();
                if arg_upper == "CACHE" || arg_upper == "INTERNAL" || arg_upper == "FORCE" || arg_upper == "PARENT_SCOPE" {
                    // Stop at these keywords
                    break;
                }
                if arg_upper == "STRING" || arg_upper == "BOOL" || arg_upper == "PATH" || arg_upper == "FILEPATH" {
                    // These are CACHE type specifiers, skip them
                    skip_next = true;
                    continue;
                }
                value_parts.push(arg.clone());
            }
            if !value_parts.is_empty() {
                let var_value = value_parts.join(" ");
                project.variables.insert(var_name, var_value);
            }
        }
    }

    // Parse project()
    for cmd_args in extract_command(&content_no_comments, "project") {
        if !cmd_args.is_empty() {
            // First argument is always the project name (unless it's a keyword)
            let mut name_idx = 0;
            let first_arg_upper = cmd_args[0].to_uppercase();
            if first_arg_upper == "LANGUAGES" || first_arg_upper == "VERSION" {
                // This shouldn't happen, but handle it
                if cmd_args.len() > 1 {
                    name_idx = 1;
                }
            }
            project.name = resolve_variable(&cmd_args[name_idx], &project.variables);
            
            // Look for VERSION
            for i in 0..cmd_args.len() {
                if cmd_args[i].to_uppercase() == "VERSION" && i + 1 < cmd_args.len() {
                    project.version = resolve_variable(&cmd_args[i + 1], &project.variables);
                    break;
                }
            }
        }
    }

    // Parse add_executable()
    for cmd_args in extract_command(&content_no_comments, "add_executable") {
        if !cmd_args.is_empty() {
            let mut target_name = resolve_variable(&cmd_args[0], &project.variables);
            
            // Skip if still contains unresolved variables
            if target_name.contains("${") || target_name.contains("$<") {
                continue;
            }
            
            // Normalize namespace targets (bsoncxx::test -> bsoncxx_test)
            target_name = target_name.replace("::", "_");
            
            // Collect and split sources (handle space-separated strings)
            let mut sources = Vec::new();
            for arg in &cmd_args[1..] {
                let resolved = resolve_variable(arg, &project.variables);
                // Split by space and filter out empty strings
                // Keep sources even if they contain unresolved variables - they might be valid paths
                for source in resolved.split_whitespace() {
                    let source = source.trim();
                    if !source.is_empty() {
                        // Only skip if the entire source is just a variable (e.g., "${VAR}")
                        if !(source.starts_with("${") && source.ends_with("}") && source.len() > 3) {
                            sources.push(source.to_string());
                        }
                    }
                }
            }
            
            // Check if target already exists (from recursive parsing)
            if let Some(existing_target) = project.targets.iter_mut().find(|t| t.name == target_name) {
                // Merge sources if they don't exist
                for source in sources {
                    if !existing_target.sources.contains(&source) {
                        existing_target.sources.push(source);
                    }
                }
            } else {
                // New target - store the CMakeLists.txt directory for this target
                let target = CmakeTarget {
                    name: target_name,
                    target_type: "executable".to_string(),
                    sources,
                    include_dirs: Vec::new(),
                    lib_dirs: Vec::new(),
                    libs: Vec::new(),
                    flags: Vec::new(),
                    deps: Vec::new(),
                    compile_definitions: Vec::new(),
                    source_dir: cmake_dir_str_for_targets.clone(),
                };
                project.targets.push(target);
            }
        }
    }

    // Parse bsoncxx_add_library() - this is a function that creates a library with specific include directories
    for cmd_args in extract_command(&content_no_comments, "bsoncxx_add_library") {
        if cmd_args.len() < 2 {
            continue;
        }
        
        let target_name = resolve_variable(&cmd_args[0], &project.variables);
        if target_name.contains("${") || target_name.contains("$<") {
            continue;
        }
        
        let normalized_target_name = target_name.replace("::", "_");
        
        // bsoncxx_add_library sets include directories: ${PROJECT_SOURCE_DIR}/include/bsoncxx/v_noabi, ${PROJECT_SOURCE_DIR}/include, etc.
        // We need to add these to the target
        let include_dirs_to_add = vec![
            format!("{}/include/bsoncxx/v_noabi", cmake_dir_str),
            format!("{}/include", cmake_dir_str),
            format!("{}/lib/bsoncxx/v_noabi", cmake_dir_str),
            format!("{}/lib", cmake_dir_str),
            format!("{}/lib", "build"), // PROJECT_BINARY_DIR/lib
        ];
        
        // Find or create the target
        if let Some(target) = project.targets.iter_mut().find(|t| t.name == normalized_target_name) {
            // Add include directories if they don't exist
            for include_dir in &include_dirs_to_add {
                // Normalize the path
                let normalized = normalize_include_path(include_dir, &cmake_dir_str);
                if !target.include_dirs.contains(&normalized) {
                    target.include_dirs.push(normalized);
                }
            }
        } else {
            // Create a new target (it will be created by add_library later, but we set include dirs now)
            // Actually, we should wait for add_library to create it, so we'll handle this in add_library parsing
        }
    }

    // Parse add_library()
    for cmd_args in extract_command(&content_no_comments, "add_library") {
        if cmd_args.is_empty() {
            continue;
        }

        // Skip ALIAS libraries - they're just references, not real targets
        if cmd_args.len() >= 2 && cmd_args[1].to_uppercase() == "ALIAS" {
            continue;
        }

        // Handle INTERFACE libraries - they have no sources but can be dependencies
        // We'll add them to the target list but with empty sources (they'll be skipped in convert_to_toml)
        let is_interface = (cmd_args.len() >= 2 && cmd_args[1].to_uppercase() == "INTERFACE") ||
                          (cmd_args[0].to_uppercase() == "INTERFACE");
        
        if is_interface {
            if cmd_args.len() >= 2 && cmd_args[1].to_uppercase() == "INTERFACE" {
                // Format: add_library(name INTERFACE)
                let target_name = resolve_variable(&cmd_args[0], &project.variables);
                if !target_name.contains("${") && !target_name.contains("$<") {
                    let target_name = target_name.replace("::", "_");
                    // Check if target already exists
                    if !project.targets.iter().any(|t| t.name == target_name) {
                        let target = CmakeTarget {
                            name: target_name,
                            target_type: "static_lib".to_string(), // Type doesn't matter, it won't be built
                            sources: Vec::new(), // Empty sources - will be skipped in convert_to_toml
                            include_dirs: Vec::new(),
                            lib_dirs: Vec::new(),
                            libs: Vec::new(),
                            flags: Vec::new(),
                            deps: Vec::new(),
                            compile_definitions: Vec::new(),
                            source_dir: cmake_dir_str_for_targets.clone(),
                        };
                        project.targets.push(target);
                    }
                }
            }
            continue; // Don't process as regular library
        }

        let lib_type = &cmd_args[0];
        let (mut target_name, target_type, sources_start) = if lib_type == "STATIC" {
            if cmd_args.len() >= 2 {
                (
                    resolve_variable(&cmd_args[1], &project.variables),
                    "static_lib",
                    2,
                )
            } else {
                continue;
            }
        } else if lib_type == "SHARED" {
            if cmd_args.len() >= 2 {
                (
                    resolve_variable(&cmd_args[1], &project.variables),
                    "shared_lib",
                    2,
                )
            } else {
                continue;
            }
        } else {
            (
                resolve_variable(&cmd_args[0], &project.variables),
                "static_lib",
                1,
            )
        };
        
        // Skip if still contains unresolved variables
        if target_name.contains("${") || target_name.contains("$<") {
            continue;
        }
        
        // Normalize namespace targets (bsoncxx::test -> bsoncxx_test)
        target_name = target_name.replace("::", "_");

        // Collect and split sources (handle space-separated strings)
        let mut sources = Vec::new();
        for arg in &cmd_args[sources_start..] {
            let resolved = resolve_variable(arg, &project.variables);
            // Split by space and filter out empty strings
            // Keep sources even if they contain unresolved variables - they might be valid paths
            for source in resolved.split_whitespace() {
                let source = source.trim();
                if !source.is_empty() {
                    // Only skip if the entire source is just a variable (e.g., "${VAR}")
                    if !(source.starts_with("${") && source.ends_with("}") && source.len() > 3) {
                        sources.push(source.to_string());
                    }
                }
            }
        }
        
        // Check if target already exists (from recursive parsing)
        if let Some(existing_target) = project.targets.iter_mut().find(|t| t.name == target_name) {
            // Merge sources if they don't exist
            for source in sources {
                if !existing_target.sources.contains(&source) {
                    existing_target.sources.push(source);
                }
            }
            
            // If this is a bsoncxx library (created by bsoncxx_add_library), add standard include directories
            if target_name.starts_with("bsoncxx") {
                // bsoncxx libraries always have these include directories relative to project root
                // v_noabi is needed for v1 includes to work (CMake creates symlinks)
                let include_dirs_to_add = vec![
                    "src/bsoncxx/include/bsoncxx/v_noabi".to_string(),
                    "src/bsoncxx/include".to_string(),
                    "src/bsoncxx/lib/bsoncxx/v_noabi".to_string(),
                    "src/bsoncxx/lib".to_string(),
                ];
                for include_dir in &include_dirs_to_add {
                    if !existing_target.include_dirs.contains(include_dir) {
                        existing_target.include_dirs.push(include_dir.clone());
                    }
                }
            }
        } else {
            // New target
            let mut include_dirs = Vec::new();
            
            // If this is a bsoncxx library (created by bsoncxx_add_library), add standard include directories
            if target_name.starts_with("bsoncxx") {
                // bsoncxx libraries always have these include directories relative to project root
                let include_dirs_to_add = vec![
                    "src/bsoncxx/include/bsoncxx/v_noabi".to_string(),
                    "src/bsoncxx/include".to_string(),
                    "src/bsoncxx/lib/bsoncxx/v_noabi".to_string(),
                    "src/bsoncxx/lib".to_string(),
                ];
                include_dirs.extend(include_dirs_to_add);
            }
            
            let target = CmakeTarget {
                name: target_name,
                target_type: target_type.to_string(),
                sources,
                include_dirs,
                lib_dirs: Vec::new(),
                libs: Vec::new(),
                flags: Vec::new(),
                deps: Vec::new(),
                compile_definitions: Vec::new(),
                source_dir: cmake_dir_str_for_targets.clone(),
            };
            project.targets.push(target);
        }
    }

    // Parse target_sources() - adds sources to existing targets
    for cmd_args in extract_command(&content_no_comments, "target_sources") {
        if cmd_args.len() >= 2 {
            let target_name = &cmd_args[0];
            let visibility = if cmd_args.len() > 2 && (cmd_args[1] == "PUBLIC" || cmd_args[1] == "PRIVATE" || cmd_args[1] == "INTERFACE") {
                &cmd_args[1]
            } else {
                ""
            };
            
            let sources_start = if visibility.is_empty() { 1 } else { 2 };
            if let Some(target) = project.targets.iter_mut().find(|t| t.name == *target_name) {
                for arg in &cmd_args[sources_start..] {
                    let resolved = resolve_variable(arg, &project.variables);
                    // Split by space and filter out empty strings
                    // Keep sources even if they contain unresolved variables - they might be valid paths
                    for source in resolved.split_whitespace() {
                        let source = source.trim();
                        if !source.is_empty() {
                            // Only skip if the entire source is just a variable (e.g., "${VAR}")
                            if !(source.starts_with("${") && source.ends_with("}") && source.len() > 3) {
                                if !target.sources.contains(&source.to_string()) {
                                    target.sources.push(source.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Parse target_link_libraries()
    for cmd_args in extract_command(&content_no_comments, "target_link_libraries") {
        if cmd_args.is_empty() {
            continue;
        }

        let mut target_name = resolve_variable(&cmd_args[0], &project.variables);
        
        // Skip if still contains unresolved variables
        if target_name.contains("${") || target_name.contains("$<") {
            continue;
        }
        
        // Normalize namespace targets
        target_name = target_name.replace("::", "_");
        
        let mut args_start = 1;
        
        // Skip visibility keywords
        if cmd_args.len() > 1 && (cmd_args[1] == "PUBLIC" || cmd_args[1] == "PRIVATE" || cmd_args[1] == "INTERFACE") {
            args_start = 2;
        }

        // Collect all project target names first (before mutable borrow)
        let project_target_names: Vec<String> = project.targets.iter().map(|t| t.name.clone()).collect();

        if let Some(target) = project.targets.iter_mut().find(|t| t.name == target_name) {
            // First pass: check if this target links to any bsoncxx libraries
            let mut needs_bsoncxx_includes = false;
            for arg in &cmd_args[args_start..] {
                let resolved = resolve_variable(arg, &project.variables);
                if !resolved.contains("${") && !resolved.contains("$<") && resolved.contains("bsoncxx") {
                    needs_bsoncxx_includes = true;
                    break;
                }
            }
            
            // Add bsoncxx include directories if needed
            if needs_bsoncxx_includes {
                // bsoncxx include directories relative to project root
                // v_noabi is needed for v1 includes to work (CMake creates symlinks)
                let bsoncxx_includes = vec![
                    "src/bsoncxx/include/bsoncxx/v_noabi".to_string(),
                    "src/bsoncxx/include".to_string(),
                ];
                for include_dir in &bsoncxx_includes {
                    if !target.include_dirs.contains(include_dir) {
                        target.include_dirs.push(include_dir.clone());
                    }
                }
            }
            
            for arg in &cmd_args[args_start..] {
                let resolved = resolve_variable(arg, &project.variables);
                
                // Skip if still contains unresolved variables (e.g., "${target_name}")
                if resolved.contains("${") || resolved.contains("$<") {
                    continue;
                }
                
                // Normalize namespace targets (bsoncxx::test -> bsoncxx_test)
                let normalized = if resolved.contains("::") {
                    resolved.replace("::", "_")
                } else {
                    resolved
                };
                
                if normalized.starts_with("-L") {
                    target.lib_dirs.push(normalized[2..].to_string());
                } else if normalized.starts_with("-l") {
                    target.libs.push(normalized[2..].to_string());
                } else {
                    // Check if it's a valid project target (using the pre-collected list)
                    let is_project_target = project_target_names.contains(&normalized);
                    
                    if is_project_target {
                        // Valid project target - add as dependency
                        if !target.deps.contains(&normalized) {
                            target.deps.push(normalized);
                        }
                    } else {
                        // Not a project target - could be external library (find_package) or library name
                        // Handle special cases like Threads (CMake find_package)
                        if normalized.to_uppercase() == "THREADS" || normalized == "Threads::Threads" || normalized == "Threads" {
                            // Threads library requires -pthread flag, not -lThreads
                            if !target.flags.contains(&"-pthread".to_string()) {
                                target.flags.push("-pthread".to_string());
                            }
                            // Don't add to libs
                            continue;
                        } else if !normalized.starts_with("-") && !normalized.contains("/") && !normalized.contains("\\") {
                            // Remove common prefixes like Catch2_Catch2 -> Catch2
                            let lib_name = if normalized.contains("_") && normalized.chars().any(|c| c.is_uppercase()) {
                                // Pattern like Catch2_Catch2 -> Catch2
                                normalized.split('_').next().unwrap_or(&normalized).to_string()
                            } else {
                                normalized.clone()
                            };
                            if !target.libs.contains(&lib_name) {
                                target.libs.push(lib_name);
                            }
                        } else {
                            // It's a library flag or path, add to libs
                            target.libs.push(normalized);
                        }
                    }
                }
            }
        }
    }

    // Parse target_include_directories()
    for cmd_args in extract_command(&content_no_comments, "target_include_directories") {
        if cmd_args.len() < 2 {
            continue;
        }

        let target_name = &cmd_args[0];
        let mut args_start = 1;
        
        // Skip visibility keywords
        if cmd_args.len() > 1 && (cmd_args[1] == "PUBLIC" || cmd_args[1] == "PRIVATE" || cmd_args[1] == "INTERFACE") {
            args_start = 2;
        }

        // Normalize target name (handle namespace)
        let mut normalized_target_name = resolve_variable(target_name, &project.variables);
        if normalized_target_name.contains("${") || normalized_target_name.contains("$<") {
            continue;
        }
        normalized_target_name = normalized_target_name.replace("::", "_");
        
        if let Some(target) = project.targets.iter_mut().find(|t| t.name == normalized_target_name) {
            for dir in &cmd_args[args_start..] {
                // Handle generator expressions: $<BUILD_INTERFACE:path> -> path
                let mut processed_dir = dir.clone();
                if processed_dir.starts_with("$<BUILD_INTERFACE:") && processed_dir.ends_with(">") {
                    // Extract path from $<BUILD_INTERFACE:path>
                    let start = "$<BUILD_INTERFACE:".len();
                    let end = processed_dir.len() - 1;
                    processed_dir = processed_dir[start..end].to_string();
                } else if processed_dir.starts_with("$<") {
                    // Skip other generator expressions
                    continue;
                }
                
                let mut resolved = resolve_variable(&processed_dir, &project.variables);
                // Skip if still contains unresolved variables
                if resolved.contains("${") {
                    continue;
                }
                
                // Resolve the include directory relative to CMakeLists.txt location (cmake_dir_str)
                // to make it relative to project root
                if !resolved.starts_with("/") {
                    // It's a relative path
                    // If resolved already starts with a path component (not "../" or "./"), 
                    // it's already relative to project root (from variable resolution)
                    // Otherwise, it's relative to CMakeLists.txt location, so join with cmake_dir_str
                    let full_path = if resolved.starts_with("../") || resolved.starts_with("./") || 
                                      (!resolved.contains('/') && resolved != ".." && resolved != ".") {
                        // Relative to CMakeLists.txt location, join with cmake_dir_str
                        Path::new(&cmake_dir_str).join(&resolved)
                    } else {
                        // Already a path from project root (e.g., "src/bsoncxx/test/../..")
                        Path::new(&resolved).to_path_buf()
                    };
                    
                    // Resolve the path manually (without canonicalize which requires file to exist)
                    let mut parts = Vec::new();
                    
                    // Process all components of the full path
                    for component in full_path.components() {
                        match component {
                            std::path::Component::Normal(p) => {
                                parts.push(p.to_string_lossy().to_string());
                            }
                            std::path::Component::ParentDir => {
                                if !parts.is_empty() {
                                    parts.pop();
                                }
                            }
                            std::path::Component::CurDir => {}
                            _ => {}
                        }
                    }
                    
                    // Join parts to get path relative to project root
                    if parts.is_empty() {
                        resolved = ".".to_string();
                    } else {
                        resolved = parts.join("/");
                    }
                }
                
                if !target.include_dirs.contains(&resolved) {
                    target.include_dirs.push(resolved);
                }
            }
        }
    }

    // Parse target_compile_options()
    for cmd_args in extract_command(&content_no_comments, "target_compile_options") {
        if cmd_args.len() < 2 {
            continue;
        }

        let target_name = &cmd_args[0];
        let mut args_start = 1;
        
        // Skip visibility keywords
        if cmd_args.len() > 1 && (cmd_args[1] == "PUBLIC" || cmd_args[1] == "PRIVATE" || cmd_args[1] == "INTERFACE") {
            args_start = 2;
        }

        if let Some(target) = project.targets.iter_mut().find(|t| t.name == *target_name) {
            for opt in &cmd_args[args_start..] {
                let resolved = resolve_variable(opt, &project.variables);
                if !target.flags.contains(&resolved) {
                    target.flags.push(resolved);
                }
            }
        }
    }

    // Parse target_compile_definitions()
    for cmd_args in extract_command(&content_no_comments, "target_compile_definitions") {
        if cmd_args.len() < 2 {
            continue;
        }

        let target_name = &cmd_args[0];
        let mut args_start = 1;
        
        if cmd_args.len() > 1 && (cmd_args[1] == "PUBLIC" || cmd_args[1] == "PRIVATE" || cmd_args[1] == "INTERFACE") {
            args_start = 2;
        }

        if let Some(target) = project.targets.iter_mut().find(|t| t.name == *target_name) {
            for def in &cmd_args[args_start..] {
                let resolved = resolve_variable(def, &project.variables);
                if !target.compile_definitions.contains(&resolved) {
                    target.compile_definitions.push(resolved);
                }
            }
        }
    }

    // Parse add_subdirectory() - recursively parse subdirectories to find targets
    for cmd_args in extract_command(&content_no_comments, "add_subdirectory") {
        if !cmd_args.is_empty() {
            // First argument is always the directory name
            let dir = resolve_variable(&cmd_args[0], &project.variables);
            let dir_clone = dir.clone();
            
            // Only add to subdirectories list if this is the root
            if is_root && !project.subdirectories.contains(&dir) {
                project.subdirectories.push(dir);
            }
            
            // Recursively parse subdirectory's CMakeLists.txt to find targets with deps, libs, etc.
            let base_dir = path.parent().unwrap_or(path);
            let subdir_path = base_dir.join(&dir_clone).join("CMakeLists.txt");
            if subdir_path.exists() {
                match parse_cmake_lists_recursive(&subdir_path, root_path, false, depth + 1) {
                    Ok(sub_project) => {
                        // Merge targets from subdirectory, avoiding duplicates
                        for sub_target in sub_project.targets {
                            // Check if target with same name already exists
                            if let Some(existing_target) = project.targets.iter_mut().find(|t| t.name == sub_target.name) {
                                // Merge: combine sources, deps, libs, etc.
                                for source in sub_target.sources {
                                    if !existing_target.sources.contains(&source) {
                                        existing_target.sources.push(source);
                                    }
                                }
                                for dep in sub_target.deps {
                                    if !existing_target.deps.contains(&dep) {
                                        existing_target.deps.push(dep);
                                    }
                                }
                                for lib in sub_target.libs {
                                    if !existing_target.libs.contains(&lib) {
                                        existing_target.libs.push(lib);
                                    }
                                }
                                for include_dir in sub_target.include_dirs {
                                    if !existing_target.include_dirs.contains(&include_dir) {
                                        existing_target.include_dirs.push(include_dir);
                                    }
                                }
                                for flag in sub_target.flags {
                                    if !existing_target.flags.contains(&flag) {
                                        existing_target.flags.push(flag);
                                    }
                                }
                                for def in sub_target.compile_definitions {
                                    if !existing_target.compile_definitions.contains(&def) {
                                        existing_target.compile_definitions.push(def);
                                    }
                                }
                            } else {
                                // New target, add it
                                project.targets.push(sub_target);
                            }
                        }
                        // Merge variables
                        for (k, v) in sub_project.variables {
                            if !project.variables.contains_key(&k) {
                                project.variables.insert(k, v);
                            }
                        }
                    }
                    Err(_e) => {
                        // Silently continue if subdirectory parse fails
                    }
                }
            }
        }
    }

    // Parse find_package() - note: we can't resolve packages, but we can log them
    for cmd_args in extract_command(&content_no_comments, "find_package") {
        if !cmd_args.is_empty() {
            // Store package name for potential future use
            let _package_name = &cmd_args[0];
            // Could add to a packages list if needed
        }
    }

    Ok(project)
}

pub fn convert_to_toml(cmake_project: &CmakeProject, _base_path: &Path) -> Result<String, String> {
    let mut toml = String::new();
    
    toml.push_str(&format!("name = \"{}\"\n", cmake_project.name));
    toml.push_str(&format!("version = \"{}\"\n\n", cmake_project.version));

    // Note: We don't add includes for subdirectories because:
    // 1. Recursive parsing already merged all targets from subdirectories into the main project
    // 2. Subdirectories don't have build.toml files (they're CMake projects, not TOML projects)
    // 3. All targets, deps, libs, etc. are already in the main build.toml

    // Add targets
    for target in &cmake_project.targets {
        toml.push_str("[[target]]\n");
        toml.push_str(&format!("name = \"{}\"\n", target.name));
        toml.push_str(&format!("type = \"{}\"\n", target.target_type));
        
        // Sources - INTERFACE libraries have empty sources but we still need them for dependency resolution
        if target.sources.is_empty() {
            // For INTERFACE libraries or targets with no sources, add an empty sources array
            // These targets won't be built but are needed for dependency resolution
            toml.push_str("sources = []\n");
        } else {
            toml.push_str("sources = [\n");
            for source in &target.sources {
                // Skip header files (.h, .hpp, .hh, .hxx) - they're not compiled
                if source.ends_with(".h") || source.ends_with(".hpp") || source.ends_with(".hh") || source.ends_with(".hxx") {
                    continue;
                }
                
                // Skip unresolved CMake variables (e.g., ${PROJECT_SOURCE_DIR})
                if source.contains("${") {
                    continue;
                }
                
                // Skip invalid source files (e.g., "OBJECT")
                if source == "OBJECT" || source.contains("$<") {
                    continue;
                }
                
                // If target has a source_dir, prepend it to the source path
                let source_path = if let Some(ref source_dir) = target.source_dir {
                    // source_dir is already relative to root_path (project root)
                    // base_path is the CMakeLists.txt's parent (project root)
                    // So we can directly use source_dir
                    format!("{}/{}", source_dir, source)
                } else {
                    source.clone()
                };
                toml.push_str(&format!("    \"{}\",\n", source_path));
            }
            toml.push_str("]\n");
        }

        // Collect all include directories (from target + source directories)
        let mut all_include_dirs = target.include_dirs.clone();
        let mut source_dirs_added: HashSet<String> = HashSet::new();
        
        // Add source file directories as include directories
        // This helps with includes like #include "client_helpers.hh" in the same directory
        for source in &target.sources {
            if let Some(ref source_dir) = target.source_dir {
                // source_dir is like "src/mongocxx/test", add it as include directory
                if !source_dirs_added.contains(source_dir) {
                    source_dirs_added.insert(source_dir.clone());
                    if !all_include_dirs.contains(source_dir) {
                        all_include_dirs.push(source_dir.clone());
                    }
                }
            } else {
                // No source_dir, try to extract from source path
                if let Some(parent) = Path::new(source).parent() {
                    let parent_str = parent.to_string_lossy().to_string();
                    if !parent_str.is_empty() && !source_dirs_added.contains(&parent_str) {
                        source_dirs_added.insert(parent_str.clone());
                        if !all_include_dirs.contains(&parent_str) {
                            all_include_dirs.push(parent_str);
                        }
                    }
                }
            }
        }
        
        if !all_include_dirs.is_empty() {
            toml.push_str("include_dirs = [\n");
            for dir in &all_include_dirs {
                // Include directories are already resolved relative to project root during parsing
                // (in target_include_directories parsing), so we can use them directly
                toml.push_str(&format!("    \"{}\",\n", dir));
            }
            toml.push_str("]\n");
        }

        if !target.lib_dirs.is_empty() {
            toml.push_str("lib_dirs = [\n");
            for dir in &target.lib_dirs {
                toml.push_str(&format!("    \"{}\",\n", dir));
            }
            toml.push_str("]\n");
        }

        if !target.libs.is_empty() {
            toml.push_str("libs = [\n");
            for lib in &target.libs {
                toml.push_str(&format!("    \"{}\",\n", lib));
            }
            toml.push_str("]\n");
        }

        if !target.flags.is_empty() {
            toml.push_str("flags = [\n");
            for flag in &target.flags {
                // Skip CMake generator expressions (e.g., $<$<CXX_COMPILER_ID:MSVC>:/Gv>)
                if flag.contains("$<") {
                    continue;
                }
                toml.push_str(&format!("    \"{}\",\n", flag));
            }
            toml.push_str("]\n");
        }

        // Add compile definitions as -D flags
        if !target.compile_definitions.is_empty() {
            if target.flags.is_empty() {
                toml.push_str("flags = [\n");
            }
            for def in &target.compile_definitions {
                toml.push_str(&format!("    \"-D{}\",\n", def));
            }
            if target.flags.is_empty() {
                toml.push_str("]\n");
            }
        }

        if !target.deps.is_empty() {
            toml.push_str("deps = [\n");
            for dep in &target.deps {
                toml.push_str(&format!("    \"{}\",\n", dep));
            }
            toml.push_str("]\n");
        }

        toml.push_str("compiler = \"g++\"\n");
        toml.push_str("\n");
    }

    Ok(toml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parse_mongo_cxx_driver() {
        let path = Path::new("/Users/buguroglu/Desktop/rust_build_tool/mongo-cxx-driver/CMakeLists.txt");
        if !path.exists() {
            println!("Skipping test - mongo-cxx-driver not found");
            return;
        }

        match parse_cmake_lists(path) {
            Ok(project) => {
                assert_eq!(project.name, "MONGO_CXX_DRIVER");
                assert!(project.subdirectories.len() >= 8, "Expected at least 8 subdirectories, got {}", project.subdirectories.len());
                println!("✓ Project name: {}", project.name);
                println!("✓ Subdirectories found: {}", project.subdirectories.len());
                for subdir in &project.subdirectories {
                    println!("  - {}", subdir);
                }
            }
            Err(e) => {
                panic!("Parse failed: {}", e);
            }
        }
    }
}
