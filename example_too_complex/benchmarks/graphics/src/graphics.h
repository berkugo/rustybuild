#ifndef EXAMPLE_TOO_COMPLEX_BENCHMARKS_GRAPHICS_SRC_GRAPHICS_H
#define EXAMPLE_TOO_COMPLEX_BENCHMARKS_GRAPHICS_SRC_GRAPHICS_H

#include <string>
#include <vector>
#include <memory>

// Forward declarations
class BaseClass {
public:
    virtual ~BaseClass() = default;
    virtual void process() = 0;
    virtual int calculate(int value) const = 0;
};

class DerivedClass : public BaseClass {
private:
    int m_value;
    std::string m_name;
    
public:
    DerivedClass(int value, const std::string& name)
        : m_value(value), m_name(name) {}
    
    void process() override {
        m_value *= 2;
    }
    
    int calculate(int value) const override {
        return m_value + value;
    }
    
    const std::string& getName() const { return m_name; }
    int getValue() const { return m_value; }
};

// Utility functions
void initialize();
void process();
void cleanup();

// Template functions
template<typename T>
T multiply(T a, T b) {
    return a * b;
}

template<typename T>
T add(T a, T b) {
    return a + b;
}

// Inline functions
inline int square(int x) {
    return x * x;
}

inline double cube(double x) {
    return x * x * x;
}

#endif // EXAMPLE_TOO_COMPLEX_BENCHMARKS_GRAPHICS_SRC_GRAPHICS_H
