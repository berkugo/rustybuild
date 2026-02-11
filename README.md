# OxiMake — Modern C++ Build Tool

OxiMake is a modern, DAG-based dependency-resolving, multi-threaded C++ build tool. Written in Rust.

## Features

- **TOML-based configuration** — Project definition via `build.toml`, nested submodule support (`includes`)
- **DAG dependency resolution** — Manual dependencies between targets via `deps`, topological order via Kahn's algorithm
- **Multi-threaded parallel build** — Independent targets build concurrently (level-by-level or make-style with `-j N`)
- **Incremental build** — Only recompiles changed source files (like Make/Ninja)
- **Multiple compiler support** — GCC, G++, Clang
- **Multiple target types** — Executable, Static Library (`.a`), Shared Library (`.so`)
- **Glob support** — Collect source files with patterns like `src/**/*.cpp`
- **LD_LIBRARY_PATH handling** — Shared library resolution
- **Per-target flags** — Separate compiler flags per target
- **CMake converter (BETA)** — Convert CMakeLists.txt to build.toml format

## Installation

```bash
cargo build --release
```

Binary: `target/release/oximake`

## Usage

```bash
# Build with build.toml in current directory
oximake

# Specify config file (-c / --config)
oximake -c build.toml
oximake --config /path/build.toml

# Build only specific targets (and their dependencies)
oximake --target app
oximake -t app -t benchmark

# Clean output directories before building
oximake --clean

# Quiet mode (errors and short summary only)
oximake -q

# Verbose output (including compiler commands)
oximake -v

# Do not print LD_LIBRARY_PATH info
oximake --no-ld-path

# Build at most 4 targets in parallel (-j / --jobs)
oximake -j 4

# All options
oximake --help
```

### Command-line options

| Option | Short | Description |
|--------|-------|-------------|
| `--config <file>` | `-c` | Configuration file (default: `build.toml`) |
| `--target <target>...` | `-t` | Build only these targets and their dependencies |
| `--clean` | — | Remove output directories before building |
| `--verbose` | `-v` | Verbose output (command lines etc.) |
| `--quiet` | `-q` | Quiet: only errors and short summary |
| `--no-ld-path` | — | Do not print LD_LIBRARY_PATH info |
| `--jobs <N>` | `-j` | Max targets to build in parallel (default: unlimited) |
| `--help` | `-h` | Help |

## build.toml structure

```toml
name = "project_name"
version = "1.0.0"
includes = ["libs/alt_modul/build.toml"]

[[target]]
name = "mylib"
type = "static_lib"          # executable | static_lib | shared_lib
sources = ["src/**/*.cpp"]   # Glob-supported source files
include_dirs = ["include"]   # -I flags
lib_dirs = ["/usr/local/lib"] # -L flags
libs = ["pthread", "m"]      # -l flags
flags = ["-O2", "-Wall", "-std=c++17"]
deps = []                    # Other targets this one depends on
compiler = "g++"             # gcc | g++ | clang
output_dir = "build"

[[target]]
name = "app"
type = "executable"
sources = ["src/**/*.cpp"]
include_dirs = ["include"]
deps = ["mylib"]
compiler = "g++"
```

## Example project

A full example lives in `example/`:

```
example/
├── build.toml                    # Main config
├── src/main.cpp                  # Main app
├── include/app.h
└── libs/
    ├── mathlib/                  # Static lib submodule
    │   ├── build.toml
    │   ├── src/math.cpp
    │   └── include/mathlib/math.h
    └── strutil/                  # Shared lib submodule
        ├── build.toml
        ├── src/strutil.cpp
        └── include/strutil/strutil.h
```

To run:

```bash
cd example
../target/release/oximake build.toml
LD_LIBRARY_PATH=build:$LD_LIBRARY_PATH ./build/app
```

## Architecture

| Module | Description |
|--------|-------------|
| `options.rs` | CLI options (clap: `--config`, `--target`, `--clean`, `-q`, `-v`, `-j`) |
| `config.rs` | TOML parsing, recursive include loading, glob expansion |
| `dag.rs` | DAG construction, topological sort, target filtering (`filter_order_for_targets`) |
| `compiler.rs` | Compiler command building, compile + link, incremental build support |
| `builder.rs` | Parallel build manager (level-by-level or make-style with `-j N`) |
| `main.rs` | CLI entry point, orchestration |

## Parallel Build System

The build tool uses **Ninja-style parallel execution** by default:

### Default Mode (Ninja-Style)
- **Global job pool** with automatic CPU count detection
- When a job finishes, any ready target (dependencies satisfied) starts immediately
- No waiting for entire levels to complete
- Works exactly like Ninja's default behavior
- Maximum parallelism: automatically uses all available CPU cores

### Manual Job Control (`-j N`)
- Override automatic detection with `-j N` to limit concurrent jobs
- Useful for resource-constrained environments
- Example: `oximake build -j 4` limits to 4 parallel jobs

### Comparison with CMake

**CMake approach:**
```
CMakeLists.txt → CMake → Makefile/Ninja → Make/Ninja → Build
```

**Our approach:**
```
build.toml → oximake → Build (directly)
```

We combine CMake's configuration parsing with Ninja/Make's parallel execution in a single tool, eliminating the intermediate build file generation step.

## GUI (Tauri + React + Tailwind)

A desktop UI lives in `gui/`:

- Load by entering a **build.toml** path
- **Dependency graph** (DAG) visualization (ReactFlow)
- **build.toml** text editor and save
- **Build** / **Clean + Build** to see output in the window
- **CMake Converter** (BETA) — Convert CMakeLists.txt to build.toml format

```bash
cd gui && npm install && npm run tauri dev
```

On Linux, Tauri may require extra system libraries; see `gui/README.md` for details.

### CMake Converter (BETA)

The CMake converter can convert CMakeLists.txt files to build.toml format. This feature is currently in **beta** and may not support all CMake features. Complex CMake projects with advanced macros, generator expressions, or conditional logic may require manual adjustments after conversion.

## License

MIT
