# MyBuild Project Structure

## Overview
This is a complete Rust-based build tool for C++ projects, designed as a modern alternative to CMake/Ninja.

## Project Files

```
mybuild/
├── Cargo.toml                 # Rust project configuration
├── README.md                  # Project documentation
├── PROJECT_STRUCTURE.md       # This file
├── test_build.sh             # Demo script
├── src/                      # Rust source code
│   ├── main.rs               # CLI entry point
│   ├── config.rs             # TOML configuration parser
│   ├── build.rs              # Build system with incremental builds
│   ├── init.rs               # Project initialization
│   └── utils.rs              # Utilities and compiler detection
└── example_project/          # Example C++ project
    ├── mybuild.toml          # Example configuration
    ├── src/                  # C++ source files
    │   ├── main.cpp
    │   └── greeter.cpp
    ├── include/              # Header files
    │   └── greeter.h
    ├── bin/                  # Output directory
    ├── lib/                  # Library output
    └── .gitignore           # Git ignore file
```

## Key Components

### 1. CLI Interface (`src/main.rs`)
- Uses `clap` for command-line argument parsing
- Commands: `init`, `build`, `clean`, `info`
- Supports verbose output, parallel jobs, target selection

### 2. Configuration System (`src/config.rs`)
- TOML-based project configuration
- Support for multiple target types (executable, static lib, shared lib)
- Compiler flags, include paths, libraries, etc.

### 3. Build System (`src/build.rs`)
- Incremental builds using file hashing
- Parallel compilation with `rayon`
- Progress bars with `indicatif`
- Cross-platform compiler detection

### 4. Project Initialization (`src/init.rs`)
- Creates project structure
- Generates example files
- Sets up `.gitignore`

### 5. Utilities (`src/utils.rs`)
- Compiler detection (g++, clang++, MSVC)
- File type detection
- Duration formatting
- System information

## Features Implemented

✅ **CLI Interface**
- `mgr init` - Initialize new project
- `mgr build` - Build project with options
- `mgr clean` - Clean build artifacts
- `mgr info` - Show project information

✅ **Configuration**
- TOML-based configuration
- Multiple target support
- Compiler flags and options
- Include paths and libraries

✅ **Incremental Builds**
- File change detection
- SHA256 hashing for accuracy
- Only recompiles changed files

✅ **Parallel Compilation**
- Multi-core build support
- Configurable job count
- Progress tracking

✅ **Cross-Platform**
- Windows (MSVC, MinGW)
- macOS (clang++, g++)
- Linux (g++, clang++)

✅ **User Experience**
- Colored output
- Progress bars
- Verbose logging
- Error handling

## Dependencies

- `clap` - CLI argument parsing
- `serde` + `toml` - Configuration serialization
- `colored` - Colored terminal output
- `rayon` - Parallel processing
- `indicatif` - Progress bars
- `sha2` - File hashing for incremental builds
- `glob` - File pattern matching
- `anyhow` - Error handling

## Usage Examples

### Initialize Project
```bash
mgr init my-project
cd my-project
```

### Build Project
```bash
mgr build                    # Build all targets
mgr build main              # Build specific target
mgr build --clean           # Clean build
mgr build --verbose --jobs 8 # Verbose with 8 parallel jobs
```

### Clean Build
```bash
mgr clean                   # Clean all
mgr clean main              # Clean specific target
```

### Show Info
```bash
mgr info                    # Show project information
```

## Future Enhancements

🔄 **Planned Features**
- Tauri-based UI for visual dependency graphs
- AI-powered build optimization suggestions
- IDE integration (VS Code, CLion)
- Package manager integration
- Build analytics and metrics

## Testing

Run the demo script to see MyBuild in action:
```bash
./test_build.sh
```

This will:
1. Show the project structure
2. Display configuration
3. Simulate the build process
4. Actually compile the example project (if g++ is available)
5. Run the resulting executable

