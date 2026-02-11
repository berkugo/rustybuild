<p align="center">
  <img src="assets/logo.png" width="320" alt="ngmake" />
</p>

# ngmake - Make Builds Great Again

ngmake is a modern, DAG-based dependency-resolving, multi-threaded C++ build tool. Written in Rust.

## Build pipeline: CMake vs ngmake

### CMake approach

1. **CMakeLists.txt** — You describe targets, sources, and dependencies in CMake’s language (macros, generator expressions, platform checks).
2. **CMake (configure)** — CMake runs and *generates* build files: Makefile or Ninja build rules for your platform.
3. **Makefile / build.ninja** — Generated text files that describe every compile and link command.
4. **Make / Ninja (build)** — A separate process reads those files and runs the compiler/linker.

```
CMakeLists.txt  →  CMake (configure)  →  Makefile / build.ninja  →  Make / Ninja  →  Build
      ↑                    ↑                          ↑                    ↑
   Your config      Generates rules            Generated file         Runs compiler
```

So you always have **two stages**: configure (CMake) and build (Make/Ninja), and a layer of generated build files in between.

### Our approach

1. **build.toml** — You describe targets, sources, and dependencies in TOML (simple, declarative).
2. **ngmake** — Reads the config, resolves the dependency DAG, and **directly** runs the compiler and linker in parallel. No intermediate Makefile or Ninja file is generated.

```
build.toml  →  ngmake  →  Build
      ↑            ↑
  Your config   Compiles & links directly (Ninja-style parallelism)
```

So you have **one stage**: ngmake both “configures” and “builds” in a single run, with the same kind of parallel job scheduling as Ninja, but without generating any build script.

### Why it matters

- **Fewer moving parts** — No CMake binary, no Make/Ninja, no generated build directory to manage.
- **Faster iteration** — Change `build.toml` and run `ngm` again; no configure step.
- **Same parallelism** — ngmake uses a Ninja-style job queue and `-j N`, so you get similar parallel build behavior without the extra toolchain.

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

Binary: `target/release/ngm`

## Usage

```bash
# Build with build.toml in current directory
ngm

# Specify config file (-c / --config)
ngm -c build.toml
ngm --config /path/build.toml

# Build only specific targets (and their dependencies)
ngm --target app
ngm -t app -t benchmark

# Clean output directories before building
ngm --clean

# Quiet mode (errors and short summary only)
ngm -q

# Verbose output (including compiler commands)
ngm -v

# Do not print LD_LIBRARY_PATH info
ngm --no-ld-path

# Build at most 4 targets in parallel (-j / --jobs)
ngm -j 4

# All options
ngm --help
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
../target/release/ngm build.toml
LD_LIBRARY_PATH=build:$LD_LIBRARY_PATH ./build/app
```

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
- Example: `ngm build -j 4` limits to 4 parallel jobs

See **Build pipeline: CMake vs ngmake** at the top of this README for how this compares to the CMake → Make/Ninja workflow.

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
