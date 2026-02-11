#!/bin/bash
# Test script to convert CMakeLists.txt and build

set -e

cd "$(dirname "$0")"

CMAKE_FILE="mongo-cxx-driver/CMakeLists.txt"
if [ ! -f "$CMAKE_FILE" ]; then
    echo "Error: $CMAKE_FILE not found"
    exit 1
fi

echo "=== Step 1: Converting CMakeLists.txt to build.toml ==="
# We'll use a Rust script to call the converter
cd gui/src-tauri

# Create a simple test binary that calls the converter
cat > src/bin/convert_test.rs << 'RUSTEOF'
mod cmake_converter;
use cmake_converter::{parse_cmake_lists, convert_to_toml};
use std::path::Path;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <cmake_path> <output_path>", args[0]);
        std::process::exit(1);
    }
    
    let cmake_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);
    
    println!("Parsing: {}", cmake_path.display());
    match parse_cmake_lists(cmake_path) {
        Ok(project) => {
            println!("✓ Parsed: {} targets", project.targets.len());
            let base_path = cmake_path.parent().unwrap_or(Path::new("."));
            match convert_to_toml(&project, base_path) {
                Ok(toml) => {
                    fs::write(output_path, toml).expect("Failed to write output");
                    println!("✓ Written to: {}", output_path.display());
                }
                Err(e) => {
                    eprintln!("✗ Conversion error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Parse error: {}", e);
            std::process::exit(1);
        }
    }
}
RUSTEOF

# Try to compile and run (but Tauri might fail, so we'll handle that)
echo "Building converter test..."
cargo build --bin convert_test 2>&1 | tail -20 || {
    echo "Note: Tauri build might fail, but we can test the logic separately"
}

cd ../..

RUSTEOF

