// build.rs - Tam versiyon
use crate::config::{Config, Target};
use crate::utils::{Compiler, CompilerType};
use colored::*;
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct BuildSystem {
    config: Config,
    compiler: Compiler,
    build_cache: Arc<Mutex<HashMap<String, String>>>,
}

impl BuildSystem {
    pub fn new(config: Config) -> Self {
        Self {
            compiler: Compiler::new(),
            build_cache: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    pub fn build(&mut self, target_name: Option<&str>, verbose: bool, _num_jobs: usize) -> anyhow::Result<()> {
        let config = self.config.clone();
        
        let targets = if let Some(name) = target_name {
            vec![(name.to_string(), config.get_target(name).ok_or_else(|| {
                anyhow::anyhow!("Target '{}' not found", name)
            })?.clone())]
        } else {
            // Build sırasına göre tüm target'ları al
            config.get_targets_in_build_order()
                .into_iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        println!("{}", "Building project...".cyan().bold());
        
        for (target_name, target_config) in targets {
            self.build_target(&target_name, &target_config, verbose)?;
        }

        println!("{}", "✓ Build completed successfully!".green().bold());
        Ok(())
    }

    fn build_target(&mut self, target_name: &str, target: &Target, verbose: bool) -> anyhow::Result<()> {
        println!("{}", format!("Building target: {}", target_name).blue().bold());

        // Create build directory
        let build_dir = Path::new("build").join(target_name);
        fs::create_dir_all(&build_dir)?;

        // Collect all source files
        let source_files = self.collect_source_files(&target.sources)?;
        
        // Create build tasks
        let tasks: Vec<(PathBuf, PathBuf)> = source_files
            .iter()
            .map(|source| {
                let object_name = source.file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string() + ".o";
                let object_file = build_dir.join(object_name);
                (source.clone(), object_file)
            })
            .collect();

        // Build object files in parallel
        let progress = ProgressBar::new(tasks.len() as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        let results: Vec<anyhow::Result<()>> = tasks
            .par_iter()
            .map(|(source_file, object_file)| {
                let result = self.compile_source_file(source_file, object_file, target, verbose);
                progress.inc(1);
                result
            })
            .collect();

        progress.finish_with_message("Compilation completed");

        // Check for compilation errors
        for result in results {
            result?;
        }

        // Link the final binary/library
        self.link_target(target_name, target, &build_dir)?;

        Ok(())
    }

    fn compile_source_file(&self, source_file: &Path, object_file: &Path, target: &Target, verbose: bool) -> anyhow::Result<()> {
        // Check if we need to recompile (incremental build)
        if self.needs_recompilation(source_file, object_file)? {
            let compiler_type = if source_file.extension().and_then(|s| s.to_str()) == Some("c") {
                CompilerType::C
            } else {
                CompilerType::Cpp
            };

            let mut cmd = self.compiler.get_compile_command(
                compiler_type,
                source_file,
                object_file,
                target,
            )?;

            if verbose {
                println!("{}", format!("Compiling: {}", source_file.display()).yellow());
                println!("Command: {:?}", cmd);
            }

            let output = cmd.output()?;
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Compilation failed for {}: {}", source_file.display(), error));
            }

            // Update build cache
            self.update_build_cache(source_file)?;
        }

        Ok(())
    }

    fn link_target(&self, _target_name: &str, target: &Target, build_dir: &Path) -> anyhow::Result<()> {
        let object_files: Vec<PathBuf> = fs::read_dir(build_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str() == Some("o") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        let output_path = Path::new(&target.output);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut cmd = self.compiler.get_link_command(
            &target.kind,
            &object_files,
            output_path,
            target,
        )?;

        println!("{}", format!("Linking: {}", target.output).yellow());
        let output = cmd.output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Linking failed: {}", error));
        }

        Ok(())
    }

    fn collect_source_files(&self, patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for pattern in patterns {
            if pattern.contains('*') {
                // Handle glob patterns
                let entries = glob::glob(pattern)?;
                for entry in entries {
                    files.push(entry?);
                }
            } else {
                // Single file
                files.push(PathBuf::from(pattern));
            }
        }

        Ok(files)
    }

    fn needs_recompilation(&self, source_file: &Path, object_file: &Path) -> anyhow::Result<bool> {
        if !object_file.exists() {
            return Ok(true);
        }

        let source_mtime = fs::metadata(source_file)?.modified()?;
        let object_mtime = fs::metadata(object_file)?.modified()?;

        if source_mtime > object_mtime {
            return Ok(true);
        }

        // Check if source file hash changed (for more accurate incremental builds)
        let current_hash = self.calculate_file_hash(source_file)?;
        let cached_hash = self.get_cached_hash(source_file)?;

        Ok(current_hash != cached_hash.unwrap_or_default())
    }

    fn calculate_file_hash(&self, file: &Path) -> anyhow::Result<String> {
        let content = fs::read(file)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(hex::encode(hasher.finalize()))
    }

    fn get_cached_hash(&self, file: &Path) -> anyhow::Result<Option<String>> {
        let cache = self.build_cache.lock().unwrap();
        Ok(cache.get(file.to_str().unwrap()).cloned())
    }

    fn update_build_cache(&self, file: &Path) -> anyhow::Result<()> {
        let hash = self.calculate_file_hash(file)?;
        let mut cache = self.build_cache.lock().unwrap();
        cache.insert(file.to_str().unwrap().to_string(), hash);
        Ok(())
    }

    pub fn clean(&mut self, target_name: Option<&str>) -> anyhow::Result<()> {
        if let Some(name) = target_name {
            let build_dir = Path::new("build").join(name);
            if build_dir.exists() {
                fs::remove_dir_all(&build_dir)?;
            }
            
            if let Some(target) = self.config.get_target(name) {
                let output_path = Path::new(&target.output);
                if output_path.exists() {
                    fs::remove_file(output_path)?;
                }
            }
        } else {
            // Clean all targets
            self.clean_all()?;
        }

        Ok(())
    }

    pub fn clean_all(&mut self) -> anyhow::Result<()> {
        if Path::new("build").exists() {
            fs::remove_dir_all("build")?;
        }

        for target in self.config.get_all_targets().values() {
            let output_path = Path::new(&target.output);
            if output_path.exists() {
                fs::remove_file(output_path)?;
            }
        }

        Ok(())
    }

    /// Sadece değişen target'ları build et
    pub fn build_changed_only(&mut self, verbose: bool, _num_jobs: usize) -> anyhow::Result<()> {
        let config = self.config.clone();
        let targets = config.get_targets_in_build_order();
        
        println!("{}", "Building changed targets only...".cyan().bold());
        
        let mut changed_targets = Vec::new();
        
        for (name, target) in targets {
            if self.target_needs_rebuild(name, target)? {
                changed_targets.push((name.clone(), target.clone()));
            }
        }
        
        if changed_targets.is_empty() {
            println!("{}", "No targets need rebuilding.".yellow());
            return Ok(());
        }
        
        println!("{}", format!("Found {} changed targets", changed_targets.len()).blue());
        
        // Değişen target'ları build et
        for (target_name, target_config) in changed_targets {
            self.build_target(&target_name, &target_config, verbose)?;
        }

        println!("{}", "✓ Changed targets built successfully!".green().bold());
        Ok(())
    }

    /// Target'ın yeniden build edilmesi gerekip gerekmediğini kontrol et
    fn target_needs_rebuild(&self, name: &str, target: &Target) -> anyhow::Result<bool> {
        // Output dosyası var mı?
        let output_path = Path::new(&target.output);
        if !output_path.exists() {
            return Ok(true);
        }

        // Source dosyaları değişmiş mi?
        for source_pattern in &target.sources {
            let source_files = self.collect_source_files(&[source_pattern.clone()])?;
            for source_file in source_files {
                if self.needs_recompilation(&source_file, &Path::new("dummy"))? {
                    return Ok(true);
                }
            }
        }

        // Bağımlılıklar değişmiş mi?
        for dep in &target.dependencies {
            if let Some(dep_target) = self.config.get_target(dep) {
                let dep_output = Path::new(&dep_target.output);
                if !dep_output.exists() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}