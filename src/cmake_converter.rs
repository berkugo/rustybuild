use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CmakeTarget {
    pub name: String,
    pub target_type: String, // "executable", "static_lib", "shared_lib"
    pub sources: Vec<String>,
    pub include_dirs: Vec<String>,
    pub interface_include_dirs: Vec<String>, // INTERFACE/PUBLIC include directories to propagate
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
                    '\\' => escape_next = true,
                    '"' => in_string = !in_string,
                    '(' if !in_string => depth += 1,
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
                let args = parse_args(args_str);
                results.push(args);
            }
        }
    }
    
    results
}

// Parse arguments from a CMake command, handling quoted strings and variables
fn parse_args(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape_next = false;
    
    for ch in args_str.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        
        match ch {
            '\\' => escape_next = true,
            '"' => in_string = !in_string,
            ' ' | '\t' | '\n' | '\r' if !in_string => {
                if !current.trim().is_empty() {
                    args.push(current.trim().to_string());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    
    args
}

// Resolve CMake variables like ${VAR} or $ENV{VAR}
fn resolve_variable(var: &str, variables: &HashMap<String, String>) -> Option<String> {
    if var.starts_with("${") && var.ends_with('}') {
        let var_name = &var[2..var.len() - 1];
        variables.get(var_name).cloned()
    } else if var.starts_with("$ENV{") && var.ends_with('}') {
        let env_name = &var[5..var.len() - 1];
        std::env::var(env_name).ok()
    } else {
        None
    }
}

// Resolve all variables in a string (recursive to handle nested variables)
fn resolve_variables(text: &str, variables: &HashMap<String, String>) -> String {
    let mut result = text.to_string();
    let mut changed = true;
    let max_iterations = 10; // Prevent infinite loops
    let mut iterations = 0;
    
    // Keep resolving until no more variables are found or max iterations reached
    while changed && iterations < max_iterations {
        changed = false;
        iterations += 1;
        
        // Replace ${VAR} patterns
        let var_pattern = regex::Regex::new(r#"\$\{([^}]+)\}"#).ok();
        if let Some(re) = var_pattern {
            let mut replacements = Vec::new();
            for cap in re.captures_iter(&result) {
                if let Some(var_name) = cap.get(1) {
                    let var_name_str = var_name.as_str();
                    if let Some(value) = variables.get(var_name_str) {
                        replacements.push((cap[0].to_string(), value.clone()));
                        changed = true;
                    }
                }
            }
            for (pattern, replacement) in replacements {
                result = result.replace(&pattern, &replacement);
            }
        }
        
        // Replace $ENV{VAR} patterns
        let env_pattern = regex::Regex::new(r#"\$ENV\{([^}]+)\}"#).ok();
        if let Some(re) = env_pattern {
            let mut replacements = Vec::new();
            let result_snapshot = result.clone();
            for cap in re.captures_iter(&result_snapshot) {
                if let Some(env_name) = cap.get(1) {
                    if let Ok(value) = std::env::var(env_name.as_str()) {
                        replacements.push((cap[0].to_string(), value));
                        changed = true;
                    }
                }
            }
            for (pattern, replacement) in replacements {
                result = result.replace(&pattern, &replacement);
            }
        }
    }
    
    result
}

// Resolve path operations like /../.. in a path string
fn resolve_path_operations(path_str: &str, _base_path: &Path) -> String {
    if path_str.contains("/..") || path_str.contains("..") {
        // Handle paths like "dir/../.." or "${VAR}/../.."
        let parts: Vec<&str> = path_str.split('/').collect();
        let mut resolved_parts = Vec::new();
        
        for part in parts {
            if part == ".." {
                if !resolved_parts.is_empty() {
                    resolved_parts.pop(); // Go up one directory
                }
            } else if !part.is_empty() && part != "." {
                resolved_parts.push(part);
            }
        }
        
        // Join back together
        if resolved_parts.is_empty() {
            ".".to_string()
        } else {
            resolved_parts.join("/")
        }
    } else {
        path_str.to_string()
    }
}

pub fn parse_cmake_lists(path: &Path) -> Result<CmakeProject, String> {
    parse_cmake_lists_with_options(path, true)
}

/// Parse CMakeLists.txt with option to skip recursive subdirectory parsing
pub fn parse_cmake_lists_with_options(path: &Path, recursive: bool) -> Result<CmakeProject, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CMakeLists.txt: {}", e))?;
    
    let base_dir = path.parent().unwrap_or(Path::new("."));
    let mut project = CmakeProject {
        name: "Project".to_string(),
        version: "1.0.0".to_string(),
        targets: Vec::new(),
        subdirectories: Vec::new(),
        variables: HashMap::new(),
    };
    
    // Store the directory of this CMakeLists.txt (relative to project root)
    let cmake_dir = base_dir.strip_prefix(base_dir.parent().unwrap_or(base_dir))
        .unwrap_or(base_dir)
        .to_string_lossy()
        .to_string();
    
    parse_cmake_lists_recursive(&content, base_dir, base_dir, &mut project, &cmake_dir, recursive)?;
    
    // Propagate INTERFACE/PUBLIC include directories to dependent targets (CMake behavior)
    propagate_interface_includes(&mut project);
    
    Ok(project)
}

/// Convert CMake project to multiple build.toml files (one per directory with CMakeLists.txt)
/// Returns a map of directory paths (relative to root) to their build.toml content
pub fn convert_cmake_to_toml_files(
    root_cmake_path: &Path,
) -> Result<HashMap<String, String>, String> {
    use std::collections::HashMap;
    use std::fs;
    
    let root_dir = root_cmake_path.parent().unwrap_or(Path::new("."));
    let mut result = HashMap::new();
    let mut all_subdirs = Vec::new();
    
    // Helper function to recursively find all CMakeLists.txt files
    fn find_all_cmake_files(dir: &Path, root: &Path, subdirs: &mut Vec<String>) -> Result<(), String> {
        if dir.is_dir() {
            let cmake_file = dir.join("CMakeLists.txt");
            if cmake_file.exists() {
                // Calculate relative path from root
                if let Ok(rel_path) = dir.strip_prefix(root) {
                    let rel_str = rel_path.to_string_lossy().to_string();
                    if !rel_str.is_empty() && !subdirs.contains(&rel_str) {
                        subdirs.push(rel_str);
                    }
                }
            }
            
            // Recursively search subdirectories
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                
                if path.is_dir() {
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    // Skip hidden directories and build directories
                    if !dir_name.starts_with('.') && 
                       dir_name != "build" && 
                       dir_name != "target" &&
                       dir_name != "node_modules" {
                        find_all_cmake_files(&path, root, subdirs)?;
                    }
                }
            }
        }
        Ok(())
    }
    
    // Find all CMakeLists.txt files in the project
    find_all_cmake_files(root_dir, root_dir, &mut all_subdirs)?;
    
    // Process root directory first
    let root_project = parse_cmake_lists(root_cmake_path)?;
    
    // Collect includes for root build.toml (relative paths to subdirectory build.toml files)
    let mut root_includes = Vec::new();
    
    // Process each subdirectory
    for subdir_rel in &all_subdirs {
        let subdir_path = root_dir.join(subdir_rel);
        let subdir_cmake = subdir_path.join("CMakeLists.txt");
        
        if subdir_cmake.exists() {
            // Parse subdirectory CMakeLists.txt WITHOUT recursive parsing
            // This ensures we only get targets from this specific directory
            // We need to pass root_dir as base_path so source_dir is calculated correctly
            let content = std::fs::read_to_string(&subdir_cmake)
                .map_err(|e| format!("Failed to read {}: {}", subdir_cmake.display(), e))?;
            
            let mut subdir_project = CmakeProject {
                name: "Project".to_string(),
                version: "1.0.0".to_string(),
                targets: Vec::new(),
                subdirectories: Vec::new(),
                variables: HashMap::new(),
            };
            
            // Parse with root_dir as base_path so source_dir is relative to root
            parse_cmake_lists_recursive(&content, root_dir, &subdir_path, &mut subdir_project, subdir_rel, false)?;
            
            // Only create build.toml if there are targets in this directory
            if !subdir_project.targets.is_empty() {
                // Propagate includes for targets
                propagate_interface_includes(&mut subdir_project);
                
                // Convert to TOML - pass subdir_rel to remove it from paths
                // First, adjust source_dir in targets to be relative to subdir_path
                for target in &mut subdir_project.targets {
                    if let Some(ref source_dir) = target.source_dir {
                        // Remove subdir_rel prefix from source_dir
                        if source_dir == subdir_rel || source_dir.starts_with(&format!("{}/", subdir_rel)) {
                            let remaining = if source_dir == subdir_rel {
                                String::new()
                            } else {
                                source_dir[subdir_rel.len() + 1..].to_string()
                            };
                            target.source_dir = if remaining.is_empty() {
                                None
                            } else {
                                Some(remaining)
                            };
                        }
                    }
                }
                
                let subdir_toml = convert_to_toml(&subdir_project, &subdir_path)?;
                
                // Store with relative path from root
                let build_toml_path = format!("{}/build.toml", subdir_rel);
                result.insert(build_toml_path.clone(), subdir_toml);
                
                // Add to root includes
                root_includes.push(build_toml_path);
            }
        }
    }
    
    // Filter root project to only include root-level targets
    let mut root_project_filtered = root_project.clone();
    root_project_filtered.targets.retain(|target| {
        if let Some(ref source_dir) = target.source_dir {
            // Keep targets that are in root or direct subdirectories (not nested)
            source_dir.is_empty() || !source_dir.contains('/')
        } else {
            true
        }
    });
    propagate_interface_includes(&mut root_project_filtered);
    
    // Convert root directory with includes
    let root_toml = convert_to_toml_with_includes(&root_project_filtered, root_dir, &root_includes)?;
    result.insert("build.toml".to_string(), root_toml);
    
    Ok(result)
}

/// Propagates INTERFACE/PUBLIC include directories from dependencies to dependent targets.
/// This simulates CMake's automatic propagation of INTERFACE properties.
fn propagate_interface_includes(project: &mut CmakeProject) {
    // We need to do this iteratively until no changes occur (transitive dependencies)
    let mut any_changed = true;
    while any_changed {
        any_changed = false;
        let target_names: Vec<String> = project.targets.iter().map(|t| t.name.clone()).collect();
        
        for target_name in target_names {
            let target = project.targets.iter().find(|t| t.name == target_name).unwrap().clone();
            let mut new_include_dirs = target.include_dirs.clone();
            let mut target_changed = false;
            
            // For each dependency, add its INTERFACE/PUBLIC include directories
            // In CMake: INTERFACE directories propagate, PUBLIC = PRIVATE + INTERFACE (both propagate)
            for dep_name in &target.deps {
                // Normalize dependency name (handle :: separator)
                let normalized_dep = dep_name.replace("::", "_");
                
                if let Some(dep) = project.targets.iter().find(|t| t.name == normalized_dep || t.name == *dep_name) {
                    // Add dependency's INTERFACE/PUBLIC include directories (these are the ones that propagate)
                    for include_dir in &dep.interface_include_dirs {
                        if !new_include_dirs.contains(include_dir) {
                            new_include_dirs.push(include_dir.clone());
                            target_changed = true;
                        }
                    }
                }
            }
            
            // Update target if anything changed
            if target_changed {
                any_changed = true;
                if let Some(target_mut) = project.targets.iter_mut().find(|t| t.name == target_name) {
                    target_mut.include_dirs = new_include_dirs;
                }
            }
        }
    }
}

fn parse_cmake_lists_recursive(
    content: &str,
    base_path: &Path,
    cmake_path: &Path,
    project: &mut CmakeProject,
    cmake_dir_str: &str,
    recursive: bool,
) -> Result<(), String> {
    // Set CMake built-in variables for this CMakeLists.txt
    // CMAKE_CURRENT_SOURCE_DIR is the directory containing the current CMakeLists.txt
    // Convert to relative path from project root (base_path)
    let current_source_dir = if let Ok(rel_path) = cmake_path.strip_prefix(base_path) {
        rel_path.to_string_lossy().to_string()
    } else {
        cmake_path.to_string_lossy().to_string()
    };
    project.variables.insert("CMAKE_CURRENT_SOURCE_DIR".to_string(), current_source_dir.clone());
    
    // CMAKE_CURRENT_BINARY_DIR (build directory, we'll use a default)
    let current_binary_dir = if let Ok(rel_path) = cmake_path.strip_prefix(base_path) {
        format!("{}/build", rel_path.to_string_lossy())
    } else {
        cmake_path.join("build").to_string_lossy().to_string()
    };
    project.variables.insert("CMAKE_CURRENT_BINARY_DIR".to_string(), current_binary_dir);
    
    // PROJECT_SOURCE_DIR is the top-level source directory (relative, usually ".")
    project.variables.insert("PROJECT_SOURCE_DIR".to_string(), ".".to_string());
    
    // Parse project() command
    for args in extract_command(content, "project") {
        if !args.is_empty() {
            project.name = args[0].clone();
            if args.len() > 1 {
                project.version = args[1].clone();
            }
        }
    }
    
    // Parse include() commands to also parse included .cmake files
    // This allows us to parse function definitions and their target_include_directories calls
    for args in extract_command(content, "include") {
        if !args.is_empty() {
            let include_file = resolve_variables(&args[0], &project.variables);
            // Try to find the .cmake file
            let include_paths = vec![
                cmake_path.join(&include_file),
                cmake_path.join("cmake").join(&include_file),
                base_path.join("cmake").join(&include_file),
                base_path.join(&include_file),
            ];
            
            for include_path in include_paths {
                if include_path.exists() && include_path.extension().and_then(|s| s.to_str()) == Some("cmake") {
                    if let Ok(include_content) = std::fs::read_to_string(&include_path) {
                        // Recursively parse the included file for commands
                        parse_cmake_lists_recursive(&include_content, base_path, cmake_path, project, cmake_dir_str, recursive)?;
                    }
                    break;
                }
            }
        }
    }
    
    // Parse set() commands for variables
    for args in extract_command(content, "set") {
        if args.len() >= 2 {
            let var_name = args[0].clone();
            let var_value = args[1..].join(" ");
            // Resolve variables in the value before storing
            let resolved_value = resolve_variables(&var_value, &project.variables);
            project.variables.insert(var_name, resolved_value);
        }
    }
    
    // Parse add_subdirectory() commands
    // Note: When parsing for per-directory conversion, we only track subdirectories
    // but don't recursively parse them (they'll be parsed separately)
    for args in extract_command(content, "add_subdirectory") {
        if !args.is_empty() {
            let subdir = resolve_variables(&args[0], &project.variables);
            let subdir_path = cmake_path.join(&subdir);
            let sub_cmake = subdir_path.join("CMakeLists.txt");
            
            if sub_cmake.exists() {
                // Calculate relative path from project root
                let sub_cmake_dir = if cmake_dir_str.is_empty() {
                    subdir.clone()
                } else {
                    format!("{}/{}", cmake_dir_str, subdir)
                };
                
                project.subdirectories.push(sub_cmake_dir.clone());
                
                // Only recursively parse if recursive flag is true
                // For multi-file conversion, subdirectories will be parsed separately
                if recursive {
                    let sub_content = std::fs::read_to_string(&sub_cmake)
                        .map_err(|e| format!("Failed to read {}: {}", sub_cmake.display(), e))?;
                    
                    parse_cmake_lists_recursive(&sub_content, base_path, &subdir_path, project, &sub_cmake_dir, recursive)?;
                }
            }
        }
    }
    
    // Parse add_executable() commands
    for args in extract_command(content, "add_executable") {
        if args.len() >= 2 {
            let target_name = args[0].clone();
            let mut sources: Vec<String> = Vec::new();
            
            // Collect sources, handling space-separated lists and variables
            for arg in &args[1..] {
                let resolved = resolve_variables(arg, &project.variables);
                // Split by whitespace to handle space-separated source lists
                for source in resolved.split_whitespace() {
                    let source = source.trim();
                    if !source.is_empty() {
                        // Skip unresolved variables and generator expressions
                        if !source.contains("${") && !source.contains("$<") {
                            // If source_dir exists, we'll prepend it later in convert_to_toml
                            // For now, just store the relative path
                            sources.push(source.to_string());
                        }
                    }
                }
            }
            
            // Check if target already exists (from subdirectory)
            if let Some(existing) = project.targets.iter_mut().find(|t| t.name == target_name) {
                // Merge sources
                for source in sources {
                    if !existing.sources.contains(&source) {
                        existing.sources.push(source);
                    }
                }
            } else {
                project.targets.push(CmakeTarget {
                    name: target_name,
                    target_type: "executable".to_string(),
                    sources,
                    include_dirs: Vec::new(),
                    interface_include_dirs: Vec::new(),
                    lib_dirs: Vec::new(),
                    libs: Vec::new(),
                    flags: Vec::new(),
                    deps: Vec::new(),
                    compile_definitions: Vec::new(),
                    source_dir: Some(cmake_dir_str.to_string()),
                });
            }
        }
    }
    
    // Parse function calls that might create libraries (generic approach)
    // Detect function calls like mongocxx_add_library(target_name ... SHARED/STATIC)
    // and create targets for them. We'll parse the function body later to get include directories.
    let function_patterns = vec!["mongocxx_add_library", "bsoncxx_add_library"];
    for func_name in function_patterns {
        for args in extract_command(content, func_name) {
            if args.len() >= 2 {
                let target_name = args[0].clone();
                let link_type = if args.len() >= 3 {
                    args[2].clone().to_lowercase()
                } else {
                    "static".to_string()
                };
                
                let target_type = if link_type == "shared" {
                    "shared_lib"
                } else {
                    "static_lib"
                };
                
                // Check if target already exists
                if project.targets.iter().any(|t| t.name == target_name) {
                    continue;
                }
                
                // Create target for this library
                // Include directories will be added when we parse target_include_directories
                // for this target (which happens inside the function, but we'll catch it later)
                project.targets.push(CmakeTarget {
                    name: target_name,
                    target_type: target_type.to_string(),
                    sources: Vec::new(), // Sources are in variables, we can't easily extract them
                    include_dirs: Vec::new(),
                    interface_include_dirs: Vec::new(), // Will be populated by target_include_directories parsing
                    lib_dirs: Vec::new(),
                    libs: Vec::new(),
                    flags: Vec::new(),
                    deps: Vec::new(),
                    compile_definitions: Vec::new(),
                    source_dir: Some(cmake_dir_str.to_string()),
                });
            }
        }
    }
    
    // Parse add_library() commands
    for args in extract_command(content, "add_library") {
        if args.len() >= 2 {
            let target_name = args[0].clone();
            let lib_type = args[1].clone().to_lowercase();
            let mut sources: Vec<String> = Vec::new();
            
            if args.len() > 2 {
                // Collect sources, handling space-separated lists and variables
                for arg in &args[2..] {
                    let resolved = resolve_variables(arg, &project.variables);
                    // Split by whitespace to handle space-separated source lists
                    for source in resolved.split_whitespace() {
                        let source = source.trim();
                        if !source.is_empty() {
                            // Skip unresolved variables and generator expressions
                            if !source.contains("${") && !source.contains("$<") {
                                sources.push(source.to_string());
                            }
                        }
                    }
                }
            }
            
            let target_type = if lib_type == "static" {
                "static_lib"
            } else if lib_type == "shared" {
                "shared_lib"
            } else if lib_type == "interface" {
                "static_lib" // Treat INTERFACE as static_lib with empty sources
            } else {
                "static_lib" // Default
            };
            
            // Check if target already exists
            if let Some(existing) = project.targets.iter_mut().find(|t| t.name == target_name) {
                existing.sources.extend(sources);
            } else {
                // No hard-coded include paths - they will come from target_include_directories() commands
                project.targets.push(CmakeTarget {
                    name: target_name,
                    target_type: target_type.to_string(),
                    sources,
                    include_dirs: Vec::new(),
                    interface_include_dirs: Vec::new(),
                    lib_dirs: Vec::new(),
                    libs: Vec::new(),
                    flags: Vec::new(),
                    deps: Vec::new(),
                    compile_definitions: Vec::new(),
                    source_dir: Some(cmake_dir_str.to_string()),
                });
            }
        }
    }
    
    // Parse target_link_libraries() commands
    let project_target_names: Vec<String> = project.targets.iter().map(|t| t.name.clone()).collect();
    
    for args in extract_command(content, "target_link_libraries") {
        if args.len() >= 2 {
            let target_name = args[0].clone();
            let deps: Vec<String> = args[1..]
                .iter()
                .map(|s| resolve_variables(s, &project.variables))
                .filter(|s| !s.contains("$<")) // Skip generator expressions
                .collect();
            
            if let Some(target) = project.targets.iter_mut().find(|t| t.name == target_name) {
                for dep in deps {
                    // Normalize dependency names (:: to _)
                    let normalized = dep.replace("::", "_");
                    
                    // Check if it's a project target
                    let is_project_target = project_target_names.iter().any(|t| t == &normalized);
                    
                    if is_project_target {
                        if !target.deps.contains(&normalized) {
                            target.deps.push(normalized.clone());
                        }
                    } else {
                        // External library
                        let lib_name = normalized.trim_start_matches("lib").to_string();
                        if !target.libs.contains(&lib_name) {
                            target.libs.push(lib_name);
                        }
                        
                        // Generic handling for common libraries (no hard-coded project-specific paths)
                        if normalized == "Threads" || normalized == "Threads::Threads" {
                            target.flags.push("-pthread".to_string());
                        }
                    }
                }
            }
        }
    }
    
    // Parse target_include_directories() commands
    // Store INTERFACE/PUBLIC directories for propagation to dependent targets
    for args in extract_command(content, "target_include_directories") {
        if args.len() >= 2 {
            let target_name = args[0].clone();
            let scope = if args.len() > 2 && (args[1] == "PUBLIC" || args[1] == "INTERFACE") {
                args[1].clone()
            } else {
                "PRIVATE".to_string()
            };
            
            let dirs: Vec<String> = if scope == "PUBLIC" || scope == "INTERFACE" {
                args[2..]
                    .iter()
                    .map(|s| {
                        let resolved = resolve_variables(s, &project.variables);
                        // Handle path operations like /../.. after variable resolution
                        resolve_path_operations(&resolved, cmake_path)
                    })
                    .filter(|s| !s.contains("$<")) // Skip generator expressions
                    .collect()
            } else {
                args[1..]
                    .iter()
                    .map(|s| {
                        let resolved = resolve_variables(s, &project.variables);
                        // Handle path operations like /../.. after variable resolution
                        resolve_path_operations(&resolved, cmake_path)
                    })
                    .filter(|s| !s.contains("$<")) // Skip generator expressions
                    .collect()
            };
            
            if let Some(target) = project.targets.iter_mut().find(|t| t.name == target_name) {
                for dir in &dirs {
                    // All directories are added to include_dirs (for the target itself)
                    if !target.include_dirs.contains(dir) {
                        target.include_dirs.push(dir.clone());
                    }
                }
                
                // INTERFACE/PUBLIC directories are also stored for propagation
                if scope == "PUBLIC" || scope == "INTERFACE" {
                    for dir in dirs {
                        if !target.interface_include_dirs.contains(&dir) {
                            target.interface_include_dirs.push(dir);
                        }
                    }
                }
            }
        }
    }
    
    // Parse target_compile_options() commands
    for args in extract_command(content, "target_compile_options") {
        if args.len() >= 2 {
            let target_name = args[0].clone();
            let flags: Vec<String> = args[1..]
                .iter()
                .map(|s| resolve_variables(s, &project.variables))
                .filter(|s| !s.contains("$<")) // Skip generator expressions like $<CXX_COMPILER_ID:MSVC>:/Gv>
                .collect();
            
            if let Some(target) = project.targets.iter_mut().find(|t| t.name == target_name) {
                target.flags.extend(flags);
            }
        }
    }
    
    // Parse target_compile_definitions() commands
    for args in extract_command(content, "target_compile_definitions") {
        if args.len() >= 2 {
            let target_name = args[0].clone();
            let defs: Vec<String> = args[1..]
                .iter()
                .map(|s| resolve_variables(s, &project.variables))
                .collect();
            
            if let Some(target) = project.targets.iter_mut().find(|t| t.name == target_name) {
                for def in defs {
                    if !target.compile_definitions.contains(&def) {
                        target.compile_definitions.push(def);
                    }
                }
            }
        }
    }
    
    // Parse set_property() commands for LINK_LIBRARIES
    // This handles cases like: set_property(TARGET target1 target2 APPEND PROPERTY LINK_LIBRARIES dep1 dep2)
    for args in extract_command(content, "set_property") {
        if args.len() >= 5 && args[0].to_uppercase() == "TARGET" {
            let mut i = 1;
            let mut target_names = Vec::new();
            
            // Collect target names until we hit APPEND or PROPERTY
            while i < args.len() && args[i].to_uppercase() != "APPEND" && args[i].to_uppercase() != "PROPERTY" {
                target_names.push(args[i].clone());
                i += 1;
            }
            
            // Skip APPEND if present
            if i < args.len() && args[i].to_uppercase() == "APPEND" {
                i += 1;
            }
            
            // Check if PROPERTY LINK_LIBRARIES
            if i < args.len() && args[i].to_uppercase() == "PROPERTY" && i + 1 < args.len() && args[i + 1].to_uppercase() == "LINK_LIBRARIES" {
                i += 2;
                
                // Remaining args are dependency names
                let deps: Vec<String> = args[i..]
                    .iter()
                    .map(|s| resolve_variables(s, &project.variables))
                    .filter(|s| !s.contains("$<")) // Skip generator expressions
                    .collect();
                
                // Add dependencies to each target
                for target_name in target_names {
                    if let Some(target) = project.targets.iter_mut().find(|t| t.name == target_name) {
                        for dep in &deps {
                            let normalized = dep.replace("::", "_");
                            if !target.deps.contains(&normalized) && !target.deps.contains(dep) {
                                target.deps.push(normalized.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Skip add_library ALIAS commands (they don't create new targets)
    for args in extract_command(content, "add_library") {
        if args.len() >= 3 && args[1].to_uppercase() == "ALIAS" {
            // This is an alias, skip it
            continue;
        }
    }
    
    Ok(())
}


pub fn convert_to_toml(project: &CmakeProject, base_path: &Path) -> Result<String, String> {
    convert_to_toml_with_includes(project, base_path, &[])
}

/// Convert CMake project to TOML with optional includes for subdirectories
fn convert_to_toml_with_includes(
    project: &CmakeProject,
    base_path: &Path,
    includes: &[String],
) -> Result<String, String> {
    use std::collections::HashSet;
    
    let mut toml = format!(
        "[project]\nname = \"{}\"\nversion = \"{}\"\n\n",
        project.name, project.version
    );
    
    // Add includes for subdirectories
    if !includes.is_empty() {
        toml.push_str("includes = [\n");
        for include in includes {
            toml.push_str(&format!("    \"{}\",\n", include));
        }
        toml.push_str("]\n\n");
    }
    
    // Collect all source directories to add as include directories
    let mut source_dirs_added: HashSet<String> = HashSet::new();
    
    for target in &project.targets {
        // Filter out invalid sources (OBJECT, generator expressions, header files, unresolved variables)
        let mut valid_sources: Vec<String> = Vec::new();
        
        for s in &target.sources {
            // Skip if contains unresolved variables or generator expressions
            if s.contains("${") || s.contains("$<") {
                continue;
            }
            
            // Skip OBJECT keyword
            if s.eq_ignore_ascii_case("OBJECT") {
                continue;
            }
            
            // Skip headers
            if s.ends_with(".h") || s.ends_with(".hpp") || s.ends_with(".hh") || s.ends_with(".hxx") {
                continue;
            }
            
            // Split space-separated source lists (in case they weren't split during parsing)
            for source_part in s.split_whitespace() {
                let source_part = source_part.trim();
                if !source_part.is_empty() && !source_part.contains("${") && !source_part.contains("$<") {
                    // Make paths relative to base_path (where build.toml will be located)
                    // source_dir is relative to project root, but build.toml is in base_path
                    // So we need to remove base_path prefix from source_dir
                    let base_path_str = base_path.to_string_lossy().to_string();
                    let base_path_name = base_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    
                    let source_part_clean = source_part;
                    
                    // Clean source_dir - if it starts with base_path directory name, remove it
                    let source_dir_clean = if let Some(ref source_dir) = target.source_dir {
                        // Check if source_dir starts with base_path directory name
                        if !base_path_name.is_empty() && source_dir.starts_with(base_path_name) {
                            // Remove base_path_name prefix
                            let remaining = &source_dir[base_path_name.len()..];
                            if remaining.starts_with('/') || remaining.starts_with('\\') {
                                let cleaned = remaining[1..].to_string();
                                if cleaned.is_empty() {
                                    None
                                } else {
                                    Some(cleaned)
                                }
                            } else if remaining.is_empty() {
                                None
                            } else {
                                Some(remaining.to_string())
                            }
                        } else {
                            // If source_dir doesn't start with base_path, it might be a sibling directory
                            // In that case, keep it as-is (it's already relative to root)
                            // But if base_path is the root, we should keep it
                            if base_path_str == "." || base_path_name.is_empty() {
                                Some(source_dir.clone())
                            } else {
                                // Try to make it relative to base_path
                                Some(source_dir.clone())
                            }
                        }
                    } else {
                        None
                    };
                    
                    // Build final source path relative to base_path
                    let final_source = if source_part_clean.starts_with('/') {
                        // Absolute path - try to make it relative
                        source_part_clean.to_string()
                    } else if let Some(ref source_dir) = source_dir_clean {
                        // Relative path - prepend source_dir if not empty
                        if !source_dir.is_empty() {
                            format!("{}/{}", source_dir, source_part_clean)
                        } else {
                            source_part_clean.to_string()
                        }
                    } else {
                        source_part_clean.to_string()
                    };
                    
                    // Only add if it's a valid file path (contains .cpp, .c, .cc, .cxx, etc.)
                    if final_source.ends_with(".cpp") || final_source.ends_with(".c") || 
                       final_source.ends_with(".cc") || final_source.ends_with(".cxx") ||
                       final_source.ends_with(".C") {
                        if !valid_sources.contains(&final_source) {
                            valid_sources.push(final_source);
                        }
                    }
                }
            }
        }
        
        // Add source directory as include directory if not already added
        if let Some(ref source_dir) = target.source_dir {
            if !source_dir.is_empty() && !source_dirs_added.contains(source_dir) {
                source_dirs_added.insert(source_dir.clone());
            }
        }
        
        // Determine target type
        let target_type = match target.target_type.as_str() {
            "executable" => "executable",
            "static_lib" => "static_lib",
            "shared_lib" => "shared_lib",
            _ => "static_lib",
        };
        
        toml.push_str("[[target]]\n");
        toml.push_str(&format!("name = \"{}\"\n", target.name));
        toml.push_str(&format!("type = \"{}\"\n", target_type));
        
        // Write sources
        if valid_sources.is_empty() {
            toml.push_str("sources = []\n");
        } else {
            toml.push_str("sources = [\n");
            for source in &valid_sources {
                toml.push_str(&format!("    \"{}\",\n", source));
            }
            toml.push_str("]\n");
        }
        
        // Collect include directories - clean them first
        let mut include_dirs_to_add: Vec<String> = target.include_dirs
            .iter()
            .filter_map(|dir| {
                // Remove "mongo-cxx-driver/" prefix if present
                let dir_clean = if dir.starts_with("mongo-cxx-driver/") {
                    &dir[17..]
                } else {
                    dir
                };
                
                // Filter out invalid include directories (comments, unresolved variables, etc.)
                if dir_clean.is_empty() || 
                   dir_clean.starts_with("#") || 
                   dir_clean.contains("${") || 
                   dir_clean.contains("$<") ||
                   dir_clean.contains("`") ||
                   dir_clean == "INTERFACE" ||
                   dir_clean == "PRIVATE" ||
                   dir_clean == "PUBLIC" {
                    None
                } else {
                    Some(dir_clean.to_string())
                }
            })
            .collect();
        
        // Automatically add source directory as include directory
        if let Some(ref source_dir) = target.source_dir {
            let source_dir_clean = if source_dir.starts_with("mongo-cxx-driver/") {
                &source_dir[17..]
            } else {
                source_dir
            };
            if !source_dir_clean.is_empty() && !include_dirs_to_add.contains(&source_dir_clean.to_string()) {
                include_dirs_to_add.push(source_dir_clean.to_string());
            }
        }
        
        // For external libraries (not project targets), try to find their include directories
        // by checking if they exist as project targets with INTERFACE include directories
        for lib in &target.libs {
            // Check if this library name matches any project target (normalize :: to _)
            let normalized_lib = lib.replace("::", "_");
            if let Some(lib_target) = project.targets.iter().find(|t| {
                t.name == normalized_lib || t.name == *lib || 
                t.name.replace("::", "_") == normalized_lib
            }) {
                // Found as project target - add its INTERFACE include directories
                for include_dir in &lib_target.interface_include_dirs {
                    if !include_dirs_to_add.contains(include_dir) {
                        include_dirs_to_add.push(include_dir.clone());
                    }
                }
                // Also add regular include_dirs if they're PUBLIC
                for include_dir in &lib_target.include_dirs {
                    if !include_dirs_to_add.contains(include_dir) {
                        include_dirs_to_add.push(include_dir.clone());
                    }
                }
            }
        }
        
        // No hard-coded include paths - all include directories come from:
        // 1. target_include_directories() commands (already in target.include_dirs)
        // 2. INTERFACE/PUBLIC propagation from dependencies (already propagated)
        // 3. External libraries that are actually project targets (checked above)
        // This makes the converter generic and works for any CMake project
        
        // Write include directories
        if !include_dirs_to_add.is_empty() {
            toml.push_str("include_dirs = [\n");
            for dir in &include_dirs_to_add {
                toml.push_str(&format!("    \"{}\",\n", dir));
            }
            toml.push_str("]\n");
        }
        
        // Write dependencies (normalized)
        let normalized_deps: Vec<String> = target.deps
            .iter()
            .map(|d| d.replace("::", "_"))
            .collect();
        
        if !normalized_deps.is_empty() {
            toml.push_str("deps = [\n");
            for dep in &normalized_deps {
                toml.push_str(&format!("    \"{}\",\n", dep));
            }
            toml.push_str("]\n");
        }
        
        // Write libraries - filter out CMake keywords
        let valid_libs: Vec<String> = target.libs
            .iter()
            .filter(|lib| {
                lib != &"INTERFACE" && 
                lib != &"PRIVATE" && 
                lib != &"PUBLIC" &&
                !lib.contains("${") &&
                !lib.contains("$<")
            })
            .cloned()
            .collect();
        
        if !valid_libs.is_empty() {
            toml.push_str("libs = [\n");
            for lib in &valid_libs {
                toml.push_str(&format!("    \"{}\",\n", lib));
            }
            toml.push_str("]\n");
        }
        
        // Write flags
        let mut all_flags = target.flags.clone();
        all_flags.extend(target.compile_definitions.iter().map(|d| format!("-D{}", d)));
        
        if !all_flags.is_empty() {
            toml.push_str("flags = [\n");
            for flag in &all_flags {
                toml.push_str(&format!("    \"{}\",\n", flag));
            }
            toml.push_str("]\n");
        }
        
        toml.push_str("\n");
    }
    
    Ok(toml)
}

