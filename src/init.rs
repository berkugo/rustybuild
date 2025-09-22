use crate::config::Config;
use colored::*;
use dialoguer::{Input, Select, Confirm};
use std::fs;
use std::path::Path;

pub struct ProjectInitializer;

impl ProjectInitializer {
    pub fn new() -> Self {
        Self
    }

    pub fn init_project(&self, name: Option<&str>, target_dir: Option<&Path>) -> anyhow::Result<()> {
        println!("{}", "🚀 Spark Project Initializer".cyan().bold());
        println!("{}", "================================".cyan());
        println!();

        // Get project name interactively
        let project_name = if let Some(name) = name {
            name.to_string()
        } else {
            let default_name = std::env::current_dir()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            
            Input::<String>::new()
                .with_prompt("Project name")
                .default(default_name)
                .interact()?
        };

        // Get project description
        let description = Input::<String>::new()
            .with_prompt("Project description")
            .default("A C++ project built with Spark".to_string())
            .interact()?;

        // Get project version
        let version = Input::<String>::new()
            .with_prompt("Project version")
            .default("0.1.0".to_string())
            .interact()?;

        // Get target directory
        let target_dir = if let Some(dir) = target_dir {
            dir.to_path_buf()
        } else {
            let current_dir = std::env::current_dir()?;
            let project_dir = current_dir.join(&project_name);
            
            if project_dir.exists() {
                let overwrite = Confirm::new()
                    .with_prompt(format!("Directory '{}' already exists. Overwrite?", project_name))
                    .default(false)
                    .interact()?;
                
                if !overwrite {
                    return Err(anyhow::anyhow!("Project initialization cancelled"));
                }
            }
            
            project_dir
        };

        // Get C++ standard
        let cpp_std = Select::new()
            .with_prompt("C++ Standard")
            .default(0)
            .items(&["C++17", "C++20", "C++23"])
            .interact()?;

        let cpp_std_flag = match cpp_std {
            0 => "-std=c++17",
            1 => "-std=c++20", 
            2 => "-std=c++23",
            _ => "-std=c++17",
        };

        // Get build type
        let build_type = Select::new()
            .with_prompt("Build type")
            .default(0)
            .items(&["Executable", "Static Library", "Shared Library"])
            .interact()?;

        let target_kind = match build_type {
            0 => crate::config::TargetKind::Executable,
            1 => crate::config::TargetKind::StaticLibrary,
            2 => crate::config::TargetKind::SharedLibrary,
            _ => crate::config::TargetKind::Executable,
        };

        // Get output name
        let output_name = Input::<String>::new()
            .with_prompt("Output name")
            .default(project_name.clone())
            .interact()?;

        // Determine output file extension based on target type and platform
        let output_extension = match target_kind {
            crate::config::TargetKind::Executable => {
                if cfg!(target_os = "windows") { ".exe" } else { "" }
            },
            crate::config::TargetKind::StaticLibrary => {
                if cfg!(target_os = "windows") { ".lib" } else { ".a" }
            },
            crate::config::TargetKind::SharedLibrary => {
                if cfg!(target_os = "windows") { ".dll" } else if cfg!(target_os = "macos") { ".dylib" } else { ".so" }
            },
        };

        let final_output_name = format!("{}{}", output_name, output_extension);

        println!();
        println!("{}", "📁 Creating project structure...".yellow());

        // Create basic directory structure
        let dirs = ["src", "include", "bin", "build"];
        for dir in &dirs {
            fs::create_dir_all(target_dir.join(dir))?;
        }
        
        // Create mybuild.toml configuration file
        let mut config = Config::default();
        config.project.name = project_name.clone();
        config.project.version = version;
        config.project.description = Some(description);
        
        // Update target configuration
        if let Some(target) = config.targets.get_mut("main") {
            target.kind = target_kind;
            target.compiler_flags = vec![cpp_std_flag.to_string(), "-Wall".to_string(), "-Wextra".to_string()];
            target.output = format!("bin/{}", final_output_name);
        }
        
        config.save(target_dir.join("mybuild.toml"))?;
        
        // Create example source files
        self.create_example_files(&target_dir, &project_name)?;
        
        println!("{}", "✅ Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".cyan().bold());
        println!("  cd {}", project_name);
        println!("  mgr build");
        println!("  ./bin/{}", final_output_name);
        
        Ok(())
    }

    fn create_example_files(&self, base_path: &Path, project_name: &str) -> anyhow::Result<()> {
        // Create main.cpp
        let main_cpp = format!(
            r#"#include <iostream>
#include "hello.h"

int main() {{
    std::cout << "Hello from {}!" << std::endl;
    
    HelloWorld greeter;
    greeter.sayHello();
    greeter.sayGoodbye();
    
    return 0;
}}
"#,
            project_name
        );
        fs::write(base_path.join("src").join("main.cpp"), main_cpp)?;

        // Create hello.h
        let header_content = format!(
            r#"#pragma once
#include <iostream>

class HelloWorld {{
public:
    void sayHello();
    void sayGoodbye();
}};
"#
        );
        fs::write(base_path.join("include").join("hello.h"), header_content)?;

        // Create hello.cpp
        let impl_content = r#"#include "hello.h"
#include <iostream>

void HelloWorld::sayHello() {
    std::cout << "Hello, World!" << std::endl;
}

void HelloWorld::sayGoodbye() {
    std::cout << "Goodbye, World!" << std::endl;
}
"#;
        fs::write(base_path.join("src").join("hello.cpp"), impl_content)?;

        Ok(())
    }
}
