# Spark - Modern C++ Build Tool

A fast, modern build tool for C++ projects written in Rust, designed as an alternative to CMake/Ninja with incremental build support and parallel compilation.

## Features

- 🚀 **Fast Incremental Builds**: Only recompiles changed files
- ⚡ **Parallel Compilation**: Multi-core build support
- 🎨 **Colored Output**: Beautiful, informative build logs
- 🔧 **Cross-Platform**: Works on Windows, macOS, and Linux
- 📝 **Simple Configuration**: TOML-based project configuration
- 🎯 **Multiple Target Types**: Executables, static libraries, shared libraries
- 🔍 **Auto-Detection**: Automatically detects available compilers
- 🔗 **Dependency Management**: Handle complex target dependencies

## Installation

### From Source

```bash
git clone <repository-url>
cd spark
cargo build --release
cargo install --path .
```

## Quick Start

### 1. Initialize a New Project

```bash
spark init my-awesome-project
cd my-awesome-project
```

### 2. Build Your Project

```bash
# Build all targets
spark build

# Build specific target
spark build main

# Clean build (rebuild everything)
spark build --clean

# Verbose output
spark build --verbose

# Parallel build with specific job count
spark build --jobs 8
```

### 3. Clean Build Artifacts

```bash
# Clean all targets
spark clean

# Clean specific target
spark clean main
```

### 4. Show Project Information

```bash
spark info
```

### 5. Manage Targets

```bash
# Add new target
spark add-target utils --kind shared_library --deps core_lib

# Remove target
spark remove-target utils

# Add dependency
spark add-dependency main_app utils_lib

# Show dependency graph
spark deps

# Build only changed targets
spark build --changed
```

## Example Large Project

```bash
# Create project
spark init big-project

# Add core library
spark add-target core_lib --kind shared_library

# Add utils library (depends on core)
spark add-target utils_lib --kind shared_library --deps core_lib

# Add network library (depends on both)
spark add-target network_lib --kind shared_library --deps core_lib,utils_lib

# Add main application (depends on all)
spark add-target main_app --kind executable --deps core_lib,utils_lib,network_lib

# Show dependency graph
spark deps

# Build with automatic dependency resolution
spark build
```

## Configuration

The `mybuild.toml` file defines your project configuration:

```toml
[project]
name = "my-awesome-project"
version = "0.1.0"
description = "A C++ project built with Spark"

[targets.main]
kind = "executable"
sources = ["src/main.cpp", "src/utils.cpp"]
includes = ["include"]
dependencies = ["core_lib"]
compiler_flags = ["-std=c++17", "-Wall", "-Wextra"]
output = "bin/my-awesome-project"

[targets.core_lib]
kind = "shared_library"
sources = ["src/core/*.cpp"]
includes = ["include/core"]
dependencies = []
output = "lib/core_lib.so"
```

## Why Spark?

- **Faster than CMake**: Incremental builds and parallel compilation
- **Simpler Configuration**: TOML vs CMakeLists.txt
- **Better UX**: Interactive CLI, colored output, progress bars
- **Modern Design**: Built with Rust for performance and reliability
- **Dependency Management**: Automatic build order resolution
- **Cross-Platform**: Works everywhere C++ works

## License

This project is licensed under the MIT License.