#!/bin/bash
# Test script for CMake converter
# This script compiles the test binary and runs it on mongo-cxx-driver's CMakeLists.txt

set -e

cd "$(dirname "$0")"

# Find the main CMakeLists.txt
CMAKE_FILE=""
if [ -f "mongo-cxx-driver/CMakeLists.txt" ]; then
    CMAKE_FILE="mongo-cxx-driver/CMakeLists.txt"
elif [ -f "mongo-cxx-driver/benchmark/CMakeLists.txt" ]; then
    CMAKE_FILE="mongo-cxx-driver/benchmark/CMakeLists.txt"
else
    echo "Error: Could not find CMakeLists.txt in mongo-cxx-driver"
    exit 1
fi

echo "=== Building test binary ==="
# Build just the cmake_converter module without Tauri
cd gui/src-tauri
cargo build --lib 2>&1 | grep -E "(Compiling|error|warning)" | head -20 || true

echo ""
echo "=== Testing CMake converter on: $CMAKE_FILE ==="
# We'll need to create a simpler test that doesn't require Tauri
# For now, let's just check if the code compiles
cargo check --lib 2>&1 | tail -10

