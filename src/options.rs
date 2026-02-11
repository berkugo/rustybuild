// ============================================================================
// options.rs — Command-line arguments (CLI)
// ============================================================================

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build targets from build.toml
    Build {
        /// Configuration file path (default: build.toml)
        #[arg(short, long, default_value = "build.toml")]
        config: PathBuf,
        
        /// Build only the specified targets (and their dependencies)
        #[arg(short, long)]
        target: Option<Vec<String>>,
        
        /// Clean output directories before building
        #[arg(long)]
        clean: bool,
        
        /// Verbose output (compiler commands etc.)
        #[arg(short, long)]
        verbose: bool,
        
        /// Quiet mode: only errors and summary
        #[arg(short, long)]
        quiet: bool,
        
        /// Do not print LD_LIBRARY_PATH info
        #[arg(long)]
        no_ld_path: bool,
        
        /// Maximum number of targets to build in parallel (default: unlimited per level)
        #[arg(short, long)]
        jobs: Option<usize>,
        /// Ignore errors and continue building remaining targets (like make -i)
        #[arg(short = 'i', long = "ignore-errors")]
        ignore_errors: bool,
    },

    /// Remove build output directories (object files, libraries, executables)
    Clean {
        /// Configuration file path (default: build.toml)
        #[arg(short, long, default_value = "build.toml")]
        config: PathBuf,
        /// Verbose: list each removed directory
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Convert CMakeLists.txt to build.toml
    Convert {
        /// Path to CMakeLists.txt file
        #[arg(short, long)]
        cmake: PathBuf,
        
        /// Output path for build.toml (default: same directory as CMakeLists.txt)
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Initialize a new C++ project
    Init {
        /// Project name (default: current directory name)
        #[arg(short, long)]
        name: Option<String>,
        
        /// C++ standard version (11, 14, 17, 20, 23)
        #[arg(short = 's', long, default_value = "17")]
        cpp_version: String,
        
        /// Project type: executable, library, or mixed
        #[arg(short, long, default_value = "executable")]
        project_type: String,
        
        /// Target directory (default: current directory)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
}

#[derive(Parser, Debug)]
#[command(name = "ngm")]
#[command(about = "ngmake - Modern C++ build tool with TOML configuration (DAG, parallel build)", long_about = None)]
pub struct BuildOptions {
    #[command(subcommand)]
    pub command: Option<Command>,
    
    // Legacy options for backward compatibility (when no subcommand is used)
    /// Configuration file path (default: build.toml)
    #[arg(short, long, value_name = "FILE", default_value = "build.toml")]
    pub config: PathBuf,

    /// Build only the specified targets (and their dependencies)
    #[arg(short = 't', long = "target", value_name = "TARGET", num_args = 1..)]
    pub targets: Option<Vec<String>>,

    /// Clean output directories before building
    #[arg(long)]
    pub clean: bool,

    /// Verbose output (compiler commands etc.)
    #[arg(short, long)]
    pub verbose: bool,

    /// Quiet mode: only errors and summary
    #[arg(short, long)]
    pub quiet: bool,

    /// Do not print LD_LIBRARY_PATH info
    #[arg(long)]
    pub no_ld_path: bool,

    /// Maximum number of targets to build in parallel (default: unlimited per level)
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,
    /// Ignore errors and continue building remaining targets (like make -i)
    #[arg(short = 'i', long = "ignore-errors")]
    pub ignore_errors: bool,
}

impl BuildOptions {
    /// If both verbose and quiet are set, quiet takes precedence
    pub fn show_verbose_output(&self) -> bool {
        self.verbose && !self.quiet
    }

    pub fn show_quiet_output(&self) -> bool {
        self.quiet
    }
}
