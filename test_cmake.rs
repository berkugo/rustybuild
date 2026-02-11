use std::path::Path;
mod cmake_converter;
use cmake_converter::parse_cmake_lists;

fn main() {
    let cmake_path = Path::new("/Users/buguroglu/Desktop/rust_build_tool/mongo-cxx-driver/CMakeLists.txt");
    
    match parse_cmake_lists(cmake_path) {
        Ok(project) => {
            println!("Project name: {}", project.name);
            println!("Project version: {}", project.version);
            println!("Number of targets: {}", project.targets.len());
            println!("Number of subdirectories: {}", project.subdirectories.len());
            println!("\nSubdirectories:");
            for subdir in &project.subdirectories {
                println!("  - {}", subdir);
            }
            println!("\nTargets:");
            for target in &project.targets {
                println!("  - {} ({})", target.name, target.target_type);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

