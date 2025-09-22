// config.rs - Büyük proje desteği ile güncellenmiş
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub project: ProjectInfo,
    pub targets: HashMap<String, Target>,
    pub compiler: Option<CompilerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Target {
    pub kind: TargetKind,
    pub sources: Vec<String>,
    pub includes: Vec<String>,
    pub defines: Vec<String>,
    pub libraries: Vec<String>,
    pub library_paths: Vec<String>,
    pub compiler_flags: Vec<String>,
    pub linker_flags: Vec<String>,
    pub output: String,
    pub dependencies: Vec<String>,  // Bağımlılıklar
    pub build_order: Option<usize>, // Build sırası
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum TargetKind {
    #[serde(rename = "executable")]
    Executable,
    #[serde(rename = "static_library")]
    StaticLibrary,
    #[serde(rename = "shared_library")]
    SharedLibrary,
}

impl std::fmt::Display for TargetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetKind::Executable => write!(f, "executable"),
            TargetKind::StaticLibrary => write!(f, "static_library"),
            TargetKind::SharedLibrary => write!(f, "shared_library"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompilerConfig {
    pub cpp_compiler: Option<String>,
    pub c_compiler: Option<String>,
    pub archiver: Option<String>,
    pub linker: Option<String>,
    pub default_flags: Vec<String>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;
        
        // Bağımlılık sırasını hesapla
        config.calculate_build_order()?;
        
        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_target(&self, name: &str) -> Option<&Target> {
        self.targets.get(name)
    }

    pub fn get_all_targets(&self) -> &HashMap<String, Target> {
        &self.targets
    }

    /// Bağımlılık sırasını hesapla (topological sort)
    pub fn calculate_build_order(&mut self) -> anyhow::Result<()> {
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        let mut order = Vec::new();

        fn visit(
            target_name: &str,
            targets: &HashMap<String, Target>,
            visited: &mut HashSet<String>,
            temp_visited: &mut HashSet<String>,
            order: &mut Vec<String>,
        ) -> anyhow::Result<()> {
            if temp_visited.contains(target_name) {
                return Err(anyhow::anyhow!("Circular dependency detected: {}", target_name));
            }
            
            if visited.contains(target_name) {
                return Ok(());
            }

            temp_visited.insert(target_name.to_string());
            
            if let Some(target) = targets.get(target_name) {
                for dep in &target.dependencies {
                    visit(dep, targets, visited, temp_visited, order)?;
                }
            }

            temp_visited.remove(target_name);
            visited.insert(target_name.to_string());
            order.push(target_name.to_string());
            
            Ok(())
        }

        for target_name in self.targets.keys() {
            if !visited.contains(target_name) {
                visit(target_name, &self.targets, &mut visited, &mut temp_visited, &mut order)?;
            }
        }

        // Build sırasını ata
        for (i, target_name) in order.iter().enumerate() {
            if let Some(target) = self.targets.get_mut(target_name) {
                target.build_order = Some(i);
            }
        }

        Ok(())
    }

    /// Build sırasına göre target'ları getir
    pub fn get_targets_in_build_order(&self) -> Vec<(&String, &Target)> {
        let mut targets: Vec<_> = self.targets.iter().collect();
        targets.sort_by_key(|(_, target)| target.build_order.unwrap_or(0));
        targets
    }

    /// Yeni target ekle
    pub fn add_target(&mut self, name: String, target: Target) -> anyhow::Result<()> {
        self.targets.insert(name, target);
        self.calculate_build_order()?;
        Ok(())
    }

    /// Target sil
    pub fn remove_target(&mut self, name: &str) -> anyhow::Result<()> {
        self.targets.remove(name);
        self.calculate_build_order()?;
        Ok(())
    }

    /// Bağımlılık ekle
    pub fn add_dependency(&mut self, target_name: &str, dependency: &str) -> anyhow::Result<()> {
        if let Some(target) = self.targets.get_mut(target_name) {
            if !target.dependencies.contains(&dependency.to_string()) {
                target.dependencies.push(dependency.to_string());
                self.calculate_build_order()?;
            }
        } else {
            return Err(anyhow::anyhow!("Target '{}' not found", target_name));
        }
        Ok(())
    }

    /// Bağımlılık sil
    pub fn remove_dependency(&mut self, target_name: &str, dependency: &str) -> anyhow::Result<()> {
        if let Some(target) = self.targets.get_mut(target_name) {
            target.dependencies.retain(|dep| dep != dependency);
            self.calculate_build_order()?;
        } else {
            return Err(anyhow::anyhow!("Target '{}' not found", target_name));
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut targets = HashMap::new();
        targets.insert("main".to_string(), Target {
            kind: TargetKind::Executable,
            sources: vec!["src/main.cpp".to_string(), "src/hello.cpp".to_string()],
            includes: vec!["include".to_string()],
            defines: vec![],
            libraries: vec![],
            library_paths: vec![],
            compiler_flags: vec!["-std=c++17".to_string(), "-Wall".to_string(), "-Wextra".to_string()],
            linker_flags: vec![],
            output: "bin/main".to_string(),
            dependencies: vec![],
            build_order: Some(0),
        });

        Self {
            project: ProjectInfo {
                name: "myproject".to_string(),
                version: "0.1.0".to_string(),
                description: Some("A C++ project built with Spark".to_string()),
            },
            targets,
            compiler: Some(CompilerConfig {
                cpp_compiler: None,
                c_compiler: None,
                archiver: None,
                linker: None,
                default_flags: vec!["-std=c++17".to_string()],
            }),
        }
    }
}