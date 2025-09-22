// main.rs - Yeni komutlar ile güncellenmiş
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

mod config;
mod build;
mod init;
mod utils;

use config::Config;
use build::BuildSystem;
use init::ProjectInitializer;

#[derive(Parser)]
#[command(name = "spark")]
#[command(about = "Spark - A modern, fast build tool for C++ projects")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new MyBuild project
    Init {
        /// Project name (defaults to current directory name)
        name: Option<String>,
        /// Target directory (defaults to current directory)
        #[arg(short, long)]
        target: Option<PathBuf>,
    },
    /// Build the project
    Build {
        /// Target to build (defaults to all targets)
        target: Option<String>,
        /// Clean build (rebuild everything)
        #[arg(short, long)]
        clean: bool,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
        /// Number of parallel jobs
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
        /// Build only changed targets
        #[arg(long)]
        changed: bool,
    },
    /// Clean build artifacts
    Clean {
        /// Target to clean (defaults to all targets)
        target: Option<String>,
    },
    /// Show project information
    Info,
    /// Add a new target
    AddTarget {
        /// Target name
        name: String,
        /// Target type
        #[arg(long)]
        kind: Option<String>,
        /// Dependencies (comma-separated)
        #[arg(long)]
        deps: Option<String>,
    },
    /// Remove a target
    RemoveTarget {
        /// Target name
        name: String,
    },
    /// Add dependency to target
    AddDependency {
        /// Target name
        target: String,
        /// Dependency name
        dependency: String,
    },
    /// Remove dependency from target
    RemoveDependency {
        /// Target name
        target: String,
        /// Dependency name
        dependency: String,
    },
    /// Show dependency graph
    Deps,
    /// Build only changed targets
    BuildChanged,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { name, target } => {
            let initializer = ProjectInitializer::new();
            initializer.init_project(name.as_deref(), target.as_deref())?;
        }
        Commands::Build { target, clean, verbose, jobs, changed } => {
            let config = Config::load("mybuild.toml")?;
            let mut build_system = BuildSystem::new(config);
            
            if *clean {
                build_system.clean_all()?;
                println!("{}", "✓ Cleaned build artifacts".yellow());
            }
            
            let num_jobs = jobs.unwrap_or_else(|| num_cpus::get());
            
            if *changed {
                build_system.build_changed_only(*verbose, num_jobs)?;
            } else {
                build_system.build(target.as_deref(), *verbose, num_jobs)?;
            }
        }
        Commands::Clean { target } => {
            let config = Config::load("mybuild.toml")?;
            let mut build_system = BuildSystem::new(config);
            build_system.clean(target.as_deref())?;
            println!("{}", "✓ Cleaned build artifacts".yellow());
        }
        Commands::Info => {
            let config = Config::load("mybuild.toml")?;
            println!("{}", "MyBuild Project Information".cyan().bold());
            println!("Project: {}", config.project.name.bright_blue());
            println!("Version: {}", config.project.version.bright_blue());
            println!("Description: {}", config.project.description.as_deref().unwrap_or("None").bright_blue());
            println!();
            println!("Targets ({}):", config.targets.len());
            for (name, target) in config.get_targets_in_build_order() {
                let deps = if target.dependencies.is_empty() {
                    "no dependencies".to_string()
                } else {
                    target.dependencies.join(", ")
                };
                println!("  • {} ({}) - depends on: {}", 
                    name.bright_green(), 
                    target.kind, 
                    deps.yellow()
                );
            }
        }
        Commands::AddTarget { name, kind, deps } => {
            let config = Config::load("mybuild.toml")?;
            let mut config = config;
            
            let target_kind = match kind.as_deref() {
                Some("executable") => config::TargetKind::Executable,
                Some("static_library") => config::TargetKind::StaticLibrary,
                Some("shared_library") => config::TargetKind::SharedLibrary,
                _ => {
                    println!("{}", "Invalid target kind. Using 'executable' as default.".yellow());
                    config::TargetKind::Executable
                }
            };
            
            let dependencies = deps.as_ref()
                .map(|d| d.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            
            let target = config::Target {
                kind: target_kind,
                sources: vec![],
                includes: vec!["include".to_string()],
                defines: vec![],
                libraries: vec![],
                library_paths: vec![],
                compiler_flags: vec!["-std=c++17".to_string(), "-Wall".to_string(), "-Wextra".to_string()],
                linker_flags: vec![],
                output: format!("bin/{}", name),
                dependencies,
                build_order: None,
            };
            
            config.add_target(name.clone(), target)?;
            config.save("mybuild.toml")?;
            println!("{}", format!("✓ Target '{}' added successfully!", name).green());
        }
        Commands::RemoveTarget { name } => {
            let config = Config::load("mybuild.toml")?;
            let mut config = config;
            config.remove_target(name)?;
            config.save("mybuild.toml")?;
            println!("{}", format!("✓ Target '{}' removed successfully!", name).green());
        }
        Commands::AddDependency { target, dependency } => {
            let config = Config::load("mybuild.toml")?;
            let mut config = config;
            config.add_dependency(target, dependency)?;
            config.save("mybuild.toml")?;
            println!("{}", format!("✓ Dependency '{}' added to '{}'", dependency, target).green());
        }
        Commands::RemoveDependency { target, dependency } => {
            let config = Config::load("mybuild.toml")?;
            let mut config = config;
            config.remove_dependency(target, dependency)?;
            config.save("mybuild.toml")?;
            println!("{}", format!("✓ Dependency '{}' removed from '{}'", dependency, target).green());
        }
        Commands::Deps => {
            let config = Config::load("mybuild.toml")?;
            println!("{}", "Dependency Graph:".cyan().bold());
            println!("==================");
            for (name, target) in config.get_targets_in_build_order() {
                if target.dependencies.is_empty() {
                    println!("  {} (no dependencies)", name.green());
                } else {
                    println!("  {} depends on: {}", name.green(), target.dependencies.join(", ").yellow());
                }
            }
        }
        Commands::BuildChanged => {
            let config = Config::load("mybuild.toml")?;
            let mut build_system = BuildSystem::new(config);
            build_system.build_changed_only(false, num_cpus::get())?;
        }
    }

    Ok(())
}