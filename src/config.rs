// ============================================================================
// config.rs — TOML config structures and nested build.toml parsing
// ============================================================================
//
// This module deserializes project and target definitions from build.toml
// and recursively loads submodule build.toml files via the `includes` field.
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Compiler type: GCC, GPP (g++), or Clang
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Compiler {
    Gcc,
    #[serde(rename = "g++")]
    Gpp,
    Clang,
}

impl Compiler {
    /// Returns the compiler command name
    pub fn command(&self) -> &str {
        match self {
            Compiler::Gcc => "gcc",
            Compiler::Gpp => "g++",
            Compiler::Clang => "clang++",
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler::Gpp
    }
}

// ---------------------------------------------------------------------------
// Target type: executable, static library, or shared library
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Executable,
    #[serde(rename = "static_lib")]
    StaticLib,
    #[serde(rename = "shared_lib")]
    SharedLib,
}

impl Default for TargetType {
    fn default() -> Self {
        TargetType::Executable
    }
}

// ---------------------------------------------------------------------------
// Target: full configuration for one build target
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    /// Unique target name (e.g. "mylib", "myapp")
    pub name: String,

    /// Target type: executable, static_lib, shared_lib
    #[serde(default, rename = "type")]
    pub target_type: TargetType,

    /// Source files (glob patterns supported, e.g. "src/**/*.cpp")
    #[serde(default)]
    pub sources: Vec<String>,

    /// Include directories (-I)
    #[serde(default)]
    pub include_dirs: Vec<String>,

    /// Library search directories (-L)
    #[serde(default)]
    pub lib_dirs: Vec<String>,

    /// Libraries to link (-l, e.g. "pthread", "m")
    #[serde(default)]
    pub libs: Vec<String>,

    /// Legacy flags: applied at compile time (prefer compiler_flags / linker_flags)
    #[serde(default)]
    pub flags: Vec<String>,

    /// C++ standard (e.g. 17 → -std=c++17). Optional; if unset, no -std is added.
    #[serde(default)]
    pub cxx_standard: Option<u32>,

    /// Compiler-only flags (compile step; e.g. "-O2", "-Wall")
    #[serde(default)]
    pub compiler_flags: Vec<String>,

    /// Linker-only flags (link step; e.g. "-Wl,--as-needed")
    #[serde(default)]
    pub linker_flags: Vec<String>,

    /// Other target names this target depends on (DAG deps)
    #[serde(default)]
    pub deps: Vec<String>,

    /// Compiler to use (gcc, g++, clang)
    #[serde(default)]
    pub compiler: Compiler,

    /// Output directory (default: "build")
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

fn default_output_dir() -> String {
    "build".to_string()
}

// ---------------------------------------------------------------------------
// Project config: top-level build.toml
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// Project name
    #[serde(default = "default_project_name")]
    pub name: String,

    /// Project version
    #[serde(default = "default_version")]
    pub version: String,

    /// C++ standard for the whole project (only root [project] is used; overrides from includes are ignored)
    #[serde(default)]
    pub cxx_standard: Option<u32>,

    /// Paths to submodule build.toml files (nested configs)
    #[serde(default)]
    pub includes: Vec<String>,

    /// Target list
    #[serde(default, rename = "target")]
    pub targets: Vec<TargetConfig>,
}

fn default_project_name() -> String {
    "unnamed_project".to_string()
}

fn default_version() -> String {
    "0.1.0".to_string()
}

// ---------------------------------------------------------------------------
// Module config: included build.toml files use [module], not [project]
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleConfig {
    #[serde(default = "default_module_name")]
    pub name: String,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default, rename = "target")]
    pub targets: Vec<TargetConfig>,
}

fn default_module_name() -> String {
    "unnamed_module".to_string()
}

// ---------------------------------------------------------------------------
// Flattened structure holding all targets
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedProject {
    /// Project name
    pub name: String,
    /// Project version
    pub version: String,
    /// C++ standard for the whole project (from root [project] only; applied to all targets)
    pub cxx_standard: Option<u32>,
    /// All targets (root + submodules), name → target map
    pub targets: HashMap<String, ResolvedTarget>,
}

/// Resolved target: paths are absolute
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedTarget {
    pub name: String,
    pub target_type: TargetType,
    /// Expanded source file paths (globs resolved)
    pub sources: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub lib_dirs: Vec<PathBuf>,
    pub libs: Vec<String>,
    pub flags: Vec<String>,
    /// C++ standard (e.g. 17 → -std=c++17)
    pub cxx_standard: Option<u32>,
    pub compiler_flags: Vec<String>,
    pub linker_flags: Vec<String>,
    pub deps: Vec<String>,
    pub compiler: Compiler,
    pub output_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Workspace root detection (for GUI: build from root when opening a leaf)
// ---------------------------------------------------------------------------

/// Given a path to a build.toml (e.g. a leaf like `libs/security/build.toml`),
/// walks up the directory tree and returns the path to the first parent
/// `build.toml` that lists this file in its `includes`. If found, building
/// should use that root config so all cross-module deps are available (CMake-like).
/// Returns `None` if no such parent exists (e.g. already at root or not included).
pub fn find_workspace_root(current_build_toml: &Path) -> Option<PathBuf> {
    let current = current_build_toml.canonicalize().ok()?;
    let mut dir = current.parent()?;
    loop {
        let candidate_root = dir.join("build.toml");
        if candidate_root.exists() {
            let content = std::fs::read_to_string(&candidate_root).ok()?;
            let root: toml::Value = toml::from_str(&content).ok()?;
            let includes = root
                .get("project")
                .and_then(|p| p.get("includes"))
                .or_else(|| root.get("includes"))
                .and_then(|v| v.as_array())?;
            for inc in includes {
                let s = inc.as_str()?;
                let resolved = dir.join(s);
                if let Ok(canon) = resolved.canonicalize() {
                    if canon == current {
                        return candidate_root.canonicalize().ok();
                    }
                }
            }
        }
        dir = dir.parent()?;
    }
}

// ---------------------------------------------------------------------------
// Recursive parsing: loads build.toml and all includes
// ---------------------------------------------------------------------------

/// Main entry: parses the build.toml at the given path and recursively
/// loads all submodules to produce a flattened ResolvedProject.
/// If the given path is included by a parent build.toml (workspace root),
/// that root is used instead — single root, CMake-like: build always from root.
/// If `verbose` is true, submodule load messages are printed.
pub fn parse_build_file(path: &Path, verbose: bool) -> Result<ResolvedProject, String> {
    // Resolve to workspace root when this file is included by a parent (single-root build).
    let path_to_load: std::path::PathBuf = find_workspace_root(path)
        .unwrap_or_else(|| path.to_path_buf());
    if verbose && path_to_load != path {
        println!("[CONFIG] Using workspace root: {} (requested: {})", path_to_load.display(), path.display());
    }

    let mut all_targets: HashMap<String, ResolvedTarget> = HashMap::new();
    let mut project_name = default_project_name();
    let mut project_version = default_version();
    let mut project_cxx_standard: Option<u32> = None;

    parse_recursive(&path_to_load, &mut all_targets, &mut project_name, &mut project_version, &mut project_cxx_standard, true, verbose)?;
    
    // Get base_dir for resolving relative paths in fallback include directories
    let base_dir = path_to_load.parent().unwrap_or_else(|| Path::new("."));
    
    // Propagate include directories, libs, and flags from dependencies (CMake INTERFACE propagation)
    propagate_dependency_properties(&mut all_targets, base_dir);

    // Root [project] cxx_standard applies to all targets; child overrides are ignored
    if let Some(std) = project_cxx_standard {
        for target in all_targets.values_mut() {
            target.cxx_standard = Some(std);
        }
    }

    Ok(ResolvedProject {
        name: project_name,
        version: project_version,
        cxx_standard: project_cxx_standard,
        targets: all_targets,
    })
}

/// Recursive parse: reads a build.toml, resolves its targets, and processes includes.
fn parse_recursive(
    path: &Path,
    all_targets: &mut HashMap<String, ResolvedTarget>,
    project_name: &mut String,
    project_version: &mut String,
    project_cxx_standard: &mut Option<u32>,
    is_root: bool,
    verbose: bool,
) -> Result<(), String> {
    // Read file
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;

    // Parse as TOML: root uses [project], included files use [module]
    let toml_value: toml::Value = toml::from_str(&content)
        .map_err(|e| format!("TOML parse error in '{}': {}", path.display(), e))?;

    let (base_dir, targets, includes) = if let toml::Value::Table(ref root) = toml_value {
        let targets_array = root.get("target").cloned().unwrap_or_else(|| toml::Value::Array(vec![]));

        if is_root {
            // Root: require [project]
            let project_table = root.get("project").cloned().unwrap_or_else(|| {
                let mut proj = toml::Value::Table(toml::map::Map::new());
                proj.as_table_mut().unwrap().insert("name".to_string(), toml::Value::String("unnamed_project".to_string()));
                proj.as_table_mut().unwrap().insert("version".to_string(), toml::Value::String("0.1.0".to_string()));
                if let Some(name) = root.get("name") { proj.as_table_mut().unwrap().insert("name".to_string(), name.clone()); }
                if let Some(version) = root.get("version") { proj.as_table_mut().unwrap().insert("version".to_string(), version.clone()); }
                if let Some(includes) = root.get("includes") { proj.as_table_mut().unwrap().insert("includes".to_string(), includes.clone()); }
                if let Some(cxx) = root.get("cxx_standard") { proj.as_table_mut().unwrap().insert("cxx_standard".to_string(), cxx.clone()); }
                proj
            });
            let mut config_table = project_table.as_table().unwrap().clone();
            config_table.insert("target".to_string(), targets_array);
            let config: ProjectConfig = toml::from_str(&toml::to_string(&toml::Value::Table(config_table))
                .map_err(|e| format!("TOML serialize error in '{}': {}", path.display(), e))?)
                .map_err(|e| format!("TOML deserialize error in '{}': {}", path.display(), e))?;
            *project_name = config.name.clone();
            *project_version = config.version.clone();
            if config.cxx_standard.is_some() {
                *project_cxx_standard = config.cxx_standard;
            }
            let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
            (base_dir, config.targets, config.includes)
        } else {
            // Included file: use [module] (fallback to [project] for backward compat)
            let module_table = root.get("module")
                .cloned()
                .or_else(|| root.get("project").cloned())
                .unwrap_or_else(|| {
                    let mut mod_ = toml::Value::Table(toml::map::Map::new());
                    if let Some(name) = root.get("name") { mod_.as_table_mut().unwrap().insert("name".to_string(), name.clone()); }
                    if let Some(includes) = root.get("includes") { mod_.as_table_mut().unwrap().insert("includes".to_string(), includes.clone()); }
                    mod_
                });
            let mut config_table = module_table.as_table().unwrap().clone();
            config_table.insert("target".to_string(), targets_array);
            let config: ModuleConfig = toml::from_str(&toml::to_string(&toml::Value::Table(config_table))
                .map_err(|e| format!("TOML serialize error in '{}': {}", path.display(), e))?)
                .map_err(|e| format!("TOML deserialize error in '{}': {}", path.display(), e))?;
            let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
            (base_dir, config.targets, config.includes)
        }
    } else {
        return Err(format!("Invalid TOML structure in '{}'", path.display()));
    };

    // Resolve and add each target
    for target in &targets {
        let resolved = resolve_target(target, base_dir)?;
        // Allow same target to appear from multiple includes (e.g. root includes utils, and
        // libs/security/build.toml also includes utils when loaded via root). First definition wins.
        if !all_targets.contains_key(&resolved.name) {
            all_targets.insert(resolved.name.clone(), resolved);
        }
    }

    // Debug: check if includes are being parsed
    if verbose && !includes.is_empty() {
        println!(
            "[CONFIG] Found {} include(s) in '{}'",
            includes.len(),
            path.display()
        );
    }

    // Recursively load submodule build.toml files
    for include_path in &includes {
        let full_path = base_dir.join(include_path);
        let canonical = full_path.canonicalize().map_err(|e| {
            format!(
                "Include file not found: '{}' (base dir: '{}'): {}",
                include_path,
                base_dir.display(),
                e
            )
        })?;

        if verbose {
            println!(
                "[CONFIG] Loading submodule: {}",
                canonical.display()
            );
        }

        parse_recursive(&canonical, all_targets, project_name, project_version, project_cxx_standard, false, verbose)?;
    }

    Ok(())
}

/// Propagates include directories, libs, and flags from dependencies (CMake INTERFACE propagation).
/// This simulates CMake's behavior where INTERFACE properties are propagated to dependents.
fn propagate_dependency_properties(all_targets: &mut HashMap<String, ResolvedTarget>, base_dir: &Path) {
    // We need to do this iteratively until no changes occur (transitive dependencies)
    let mut any_changed = true;
    while any_changed {
        any_changed = false;
        let target_names: Vec<String> = all_targets.keys().cloned().collect();
        
        for target_name in target_names {
            let target = all_targets.get(&target_name).unwrap().clone();
            let mut new_include_dirs = target.include_dirs.clone();
            let mut new_libs = target.libs.clone();
            let mut new_flags = target.flags.clone();
            let mut new_compiler_flags = target.compiler_flags.clone();
            let mut new_linker_flags = target.linker_flags.clone();
            let mut target_changed = false;
            
            // For each dependency, add its include_dirs, libs, flags, compiler_flags, linker_flags
            for dep_name in &target.deps {
                if let Some(dep) = all_targets.get(dep_name) {
                    // Add dependency's include directories
                    for include_dir in &dep.include_dirs {
                        if !new_include_dirs.contains(include_dir) {
                            new_include_dirs.push(include_dir.clone());
                            target_changed = true;
                        }
                    }
                    // Add dependency's libs
                    for lib in &dep.libs {
                        if !new_libs.contains(lib) {
                            new_libs.push(lib.clone());
                            target_changed = true;
                        }
                    }
                    // Add dependency's flags
                    for flag in &dep.flags {
                        if !new_flags.contains(flag) {
                            new_flags.push(flag.clone());
                            target_changed = true;
                        }
                    }
                    for flag in &dep.compiler_flags {
                        if !new_compiler_flags.contains(flag) {
                            new_compiler_flags.push(flag.clone());
                            target_changed = true;
                        }
                    }
                    for flag in &dep.linker_flags {
                        if !new_linker_flags.contains(flag) {
                            new_linker_flags.push(flag.clone());
                            target_changed = true;
                        }
                    }
                } else {
                    // Dependency not found - might be an external library or a target created by a function
                    // For known library patterns, add common include directories
                    if dep_name == "bsoncxx_testing" || dep_name.starts_with("bsoncxx") {
                        // bsoncxx libraries typically need src/bsoncxx/include
                        let bsoncxx_include = base_dir.join("src/bsoncxx/include");
                        if !new_include_dirs.contains(&bsoncxx_include) {
                            new_include_dirs.push(bsoncxx_include);
                            target_changed = true;
                        }
                    }
                    if dep_name.starts_with("mongocxx") {
                        // mongocxx libraries typically need src/mongocxx/include
                        let mongocxx_include = base_dir.join("src/mongocxx/include");
                        if !new_include_dirs.contains(&mongocxx_include) {
                            new_include_dirs.push(mongocxx_include);
                            target_changed = true;
                        }
                    }
                }
            }
            
            // Update target if anything changed
            if target_changed {
                any_changed = true;
                if let Some(target_mut) = all_targets.get_mut(&target_name) {
                    target_mut.include_dirs = new_include_dirs;
                    target_mut.libs = new_libs;
                    target_mut.flags = new_flags;
                    target_mut.compiler_flags = new_compiler_flags;
                    target_mut.linker_flags = new_linker_flags;
                }
            }
        }
    }
}

/// Converts a TargetConfig to ResolvedTarget. Expands globs and makes paths absolute.
/// Note: This is called during parsing, before all targets are available, so dependency
/// propagation happens later in a separate pass.
fn resolve_target(target: &TargetConfig, base_dir: &Path) -> Result<ResolvedTarget, String> {
    // Source files: expand glob patterns
    let mut resolved_sources = Vec::new();
    for pattern in &target.sources {
        let full_pattern = base_dir.join(pattern);
        let pattern_str = full_pattern.to_string_lossy().to_string();

        // Check if pattern contains glob characters
        let has_glob = pattern.contains('*') || pattern.contains('?') || pattern.contains('[');
        
        if has_glob {
            // It's a glob pattern
            let entries = glob::glob(&pattern_str)
                .map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;

            let mut found = false;
            for entry in entries {
                match entry {
                    Ok(path) => {
                        resolved_sources.push(path);
                        found = true;
                    }
                    Err(e) => {
                        eprintln!(
                            "[WARN] Glob entry read failed '{}': {}",
                            pattern, e
                        );
                    }
                }
            }

            if !found {
                eprintln!(
                    "[WARN] No source files for pattern '{}' (target: '{}')",
                    pattern, target.name
                );
            }
        } else {
            // It's a direct file path, check if it exists
            if full_pattern.exists() {
                resolved_sources.push(full_pattern);
            } else {
                // Try relative to base_dir as-is
                if base_dir.join(pattern).exists() {
                    resolved_sources.push(base_dir.join(pattern));
                } else {
                    eprintln!(
                        "[WARN] Source file not found: '{}' (target: '{}')",
                        pattern, target.name
                    );
                }
            }
        }
    }

    // Make include dirs absolute
    let resolved_include_dirs: Vec<PathBuf> = target
        .include_dirs
        .iter()
        .map(|d| base_dir.join(d))
        .collect();

    // Make lib dirs absolute
    let resolved_lib_dirs: Vec<PathBuf> = target
        .lib_dirs
        .iter()
        .map(|d| base_dir.join(d))
        .collect();

    // Make output dir absolute
    let resolved_output_dir = base_dir.join(&target.output_dir);

    Ok(ResolvedTarget {
        name: target.name.clone(),
        target_type: target.target_type.clone(),
        sources: resolved_sources,
        include_dirs: resolved_include_dirs,
        lib_dirs: resolved_lib_dirs,
        libs: target.libs.clone(),
        flags: target.flags.clone(),
        cxx_standard: target.cxx_standard,
        compiler_flags: target.compiler_flags.clone(),
        linker_flags: target.linker_flags.clone(),
        deps: target.deps.clone(),
        compiler: target.compiler.clone(),
        output_dir: resolved_output_dir,
    })
}
