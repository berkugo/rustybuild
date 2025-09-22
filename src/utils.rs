use crate::config::{Target, TargetKind};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub enum CompilerType {
    C,
    Cpp,
}

pub struct Compiler {
    cpp_compiler: String,
    c_compiler: String,
    archiver: String,
    linker: String,
}

impl Compiler {
    pub fn new() -> Self {
        let (cpp_compiler, c_compiler, archiver, linker) = Self::detect_compilers();
        
        Self {
            cpp_compiler,
            c_compiler,
            archiver,
            linker,
        }
    }

    fn detect_compilers() -> (String, String, String, String) {
        let mut cpp_compiler = "g++".to_string();
        let mut c_compiler = "gcc".to_string();
        let mut archiver = "ar".to_string();
        let mut linker = "g++".to_string();

        // Try to detect available compilers
        if Self::command_exists("clang++") {
            cpp_compiler = "clang++".to_string();
            linker = "clang++".to_string();
        }
        
        if Self::command_exists("clang") {
            c_compiler = "clang".to_string();
        }

        // On Windows, try MSVC
        #[cfg(target_os = "windows")]
        {
            if Self::command_exists("cl") {
                cpp_compiler = "cl".to_string();
                c_compiler = "cl".to_string();
                linker = "link".to_string();
                archiver = "lib".to_string();
            }
        }

        (cpp_compiler, c_compiler, archiver, linker)
    }

    fn command_exists(command: &str) -> bool {
        Command::new(command)
            .arg("--version")
            .output()
            .is_ok()
    }

    pub fn get_compile_command(
        &self,
        compiler_type: CompilerType,
        source_file: &Path,
        object_file: &Path,
        target: &Target,
    ) -> anyhow::Result<Command> {
        let compiler = match compiler_type {
            CompilerType::Cpp => &self.cpp_compiler,
            CompilerType::C => &self.c_compiler,
        };

        let mut cmd = Command::new(compiler);
        
        // Add compiler flags
        for flag in &target.compiler_flags {
            cmd.arg(flag);
        }

        // Add include paths
        for include in &target.includes {
            cmd.arg("-I").arg(include);
        }

        // Add defines
        for define in &target.defines {
            cmd.arg("-D").arg(define);
        }

        // Add output file
        cmd.arg("-c").arg(source_file).arg("-o").arg(object_file);

        Ok(cmd)
    }

    pub fn get_link_command(
        &self,
        target_kind: &TargetKind,
        object_files: &[PathBuf],
        output_path: &Path,
        target: &Target,
    ) -> anyhow::Result<Command> {
        let mut cmd = match target_kind {
            TargetKind::Executable => {
                let mut cmd = Command::new(&self.linker);
                cmd.arg("-o").arg(output_path);
                cmd
            }
            TargetKind::StaticLibrary => {
                let mut cmd = Command::new(&self.archiver);
                cmd.arg("rcs").arg(output_path);
                cmd
            }
            TargetKind::SharedLibrary => {
                let mut cmd = Command::new(&self.linker);
                cmd.arg("-shared").arg("-o").arg(output_path);
                cmd
            }
        };

        // Add object files
        for obj_file in object_files {
            cmd.arg(obj_file);
        }

        // Add linker flags
        for flag in &target.linker_flags {
            cmd.arg(flag);
        }

        // Add library paths
        for lib_path in &target.library_paths {
            cmd.arg("-L").arg(lib_path);
        }

        // Add libraries
        for lib in &target.libraries {
            cmd.arg("-l").arg(lib);
        }

        Ok(cmd)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

pub fn ensure_directory_exists(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn get_file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_string())
}

pub fn is_source_file(path: &Path) -> bool {
    if let Some(ext) = get_file_extension(path) {
        matches!(ext.as_str(), "cpp" | "cxx" | "cc" | "c" | "c++")
    } else {
        false
    }
}

pub fn is_header_file(path: &Path) -> bool {
    if let Some(ext) = get_file_extension(path) {
        matches!(ext.as_str(), "h" | "hpp" | "hxx" | "hh")
    } else {
        false
    }
}

pub fn format_duration(seconds: f64) -> String {
    if seconds < 1.0 {
        format!("{:.0}ms", seconds * 1000.0)
    } else if seconds < 60.0 {
        format!("{:.1}s", seconds)
    } else {
        let minutes = (seconds / 60.0) as u64;
        let remaining_seconds = seconds % 60.0;
        format!("{}m {:.1}s", minutes, remaining_seconds)
    }
}

pub fn get_system_info() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("{} ({})", os, arch)
}
