cmake_minimum_required(VERSION 3.10)
project(SimpleProject VERSION 1.0.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Common library
add_library(common STATIC
    src/common/logger.cpp
    src/common/config.cpp
)

target_include_directories(common PUBLIC
    include/common
)

# Core library
add_library(core STATIC
    src/core/engine.cpp
    src/core/manager.cpp
)

target_include_directories(core PUBLIC
    include/core
)

target_link_libraries(core PUBLIC common)

# Math library
add_library(math STATIC
    src/math/calculator.cpp
    src/math/vector.cpp
)

target_include_directories(math PUBLIC
    include/math
)

target_link_libraries(math PUBLIC common)

# Utils library
add_library(utils STATIC
    src/utils/string_util.cpp
    src/utils/file_util.cpp
)

target_include_directories(utils PUBLIC
    include/utils
)

target_link_libraries(utils PUBLIC common)

# IO library (shared)
add_library(io SHARED
    src/io/reader.cpp
    src/io/writer.cpp
)

target_include_directories(io PUBLIC
    include/io
)

target_link_libraries(io PUBLIC utils)

# Network library
add_library(network STATIC
    src/network/client.cpp
    src/network/server.cpp
)

target_include_directories(network PUBLIC
    include/network
)

target_link_libraries(network PUBLIC core io)

# Main application
add_executable(main_app
    src/main.cpp
    src/app.cpp
)

target_include_directories(main_app PRIVATE
    include
)

target_link_libraries(main_app PRIVATE
    network
    math
    utils
)

# Test executables
add_executable(test_utils
    tests/test_utils.cpp
)

target_include_directories(test_utils PRIVATE
    include
)

target_link_libraries(test_utils PRIVATE utils)

add_executable(test_math
    tests/test_math.cpp
)

target_include_directories(test_math PRIVATE
    include
)

target_link_libraries(test_math PRIVATE math)

# Benchmark
add_executable(benchmark
    benchmarks/main.cpp
    benchmarks/perf_test.cpp
)

target_include_directories(benchmark PRIVATE
    include
)

target_link_libraries(benchmark PRIVATE
    math
    utils
    core
)

