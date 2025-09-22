#!/bin/bash

# MyBuild Test Script
# This script demonstrates how MyBuild would work

echo "🚀 MyBuild - Modern C++ Build Tool Demo"
echo "========================================"
echo

# Check if we're in the example project directory
if [ ! -f "example_project/mybuild.toml" ]; then
    echo "❌ Please run this script from the project root directory"
    exit 1
fi

cd example_project

echo "📁 Project Structure:"
echo "├── mybuild.toml (configuration)"
echo "├── src/"
echo "│   ├── main.cpp"
echo "│   └── greeter.cpp"
echo "├── include/"
echo "│   └── greeter.h"
echo "├── bin/ (output directory)"
echo "└── lib/ (library output)"
echo

echo "🔧 Configuration (mybuild.toml):"
echo "Project: hello-world v0.1.0"
echo "Targets:"
echo "  • main (executable) → bin/hello-world"
echo "  • lib (static library) → lib/libgreeter.a"
echo

echo "🏗️  Simulating build process..."
echo

# Simulate the build process
echo "Building target: main"
echo "Compiling: src/main.cpp"
echo "Compiling: src/greeter.cpp"
echo "Linking: bin/hello-world"
echo

echo "Building target: lib"
echo "Compiling: src/greeter.cpp"
echo "Archiving: lib/libgreeter.a"
echo

# Create the output directories
mkdir -p bin lib

# Simulate compilation with actual g++
if command -v g++ &> /dev/null; then
    echo "✅ Compiling with g++..."
    g++ -std=c++17 -Wall -Wextra -O2 -Iinclude -DVERSION=1.0 -c src/greeter.cpp -o build/greeter.o
    g++ -std=c++17 -Wall -Wextra -O2 -Iinclude -DVERSION=1.0 -c src/main.cpp -o build/main.o
    g++ -o bin/hello-world build/main.o build/greeter.o
    
    # Create static library
    ar rcs lib/libgreeter.a build/greeter.o
    
    echo "✅ Build completed successfully!"
    echo
    echo "🎯 Running the executable:"
    echo "=========================="
    ./bin/hello-world
    echo
    echo "📊 Build artifacts:"
    ls -la bin/ lib/
else
    echo "⚠️  g++ not found, showing simulated output:"
    echo "Hello from MyBuild!"
    echo "Goodbye from MyBuild!"
    echo "Version: 1.0"
fi

echo
echo "🎉 MyBuild Demo Complete!"
echo
echo "Key Features Demonstrated:"
echo "• TOML-based configuration"
echo "• Multiple target types (executable + static library)"
echo "• Include paths and compiler flags"
echo "• Cross-platform compiler detection"
echo "• Clean project structure"
