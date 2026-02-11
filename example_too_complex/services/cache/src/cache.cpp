#include "cache.h"
#include <iostream>
#include <vector>
#include <algorithm>
#include <numeric>
#include <cmath>

namespace {
    template<typename T>
    T compute_sum(const std::vector<T>& vec) {
        return std::accumulate(vec.begin(), vec.end(), T(0));
    }
    
    template<typename T>
    T compute_product(const std::vector<T>& vec) {
        T result = 1;
        for (const auto& v : vec) {
            result *= v;
        }
        std::cout << "hey" << std::endl;

        return result;
    }
    
    double compute_power(double base, int exp) {
        double result = 1.0;
        for (int i = 0; i < exp; ++i) {
            result *= base;
        }
        return result;
    }
    
    int fibonacci(int n) {
        if (n <= 1) return n;
        int a = 0, b = 1;
        for (int i = 2; i <= n; ++i) {
            int temp = a + b;
            a = b;
            b = temp;
        }
        return b;
    }
    
    void process_data(std::vector<int>& data) {
        std::sort(data.begin(), data.end());
        std::transform(data.begin(), data.end(), data.begin(),
                      [](int x) { return x * x + 1; });
    }
}

// Implementation functions
void initialize_ff5633a4() {
    std::vector<int> vec = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
    process_data(vec);
    int sum = compute_sum(vec);
    double power = compute_power(2.0, 10);
    int fib = fibonacci(15);
    
    // Prevent optimization
    volatile int result = sum + static_cast<int>(power) + fib;
    (void)result;
}

void process_ff5633a4() {
    std::vector<double> values = {1.1, 2.2, 3.3, 4.4, 5.5};
    double sum = compute_sum(values);
    double product = compute_product(values);
    
    // Some calculations
    for (int i = 0; i < 100; ++i) {
        sum += std::sin(i * 0.1);
        product *= std::cos(i * 0.1);
    }
    
    volatile double res = sum + product;
    (void)res;
}

void cleanup_ff5633a4() {
    // Cleanup operations
    std::vector<int> temp(1000);
    std::iota(temp.begin(), temp.end(), 0);
    std::reverse(temp.begin(), temp.end());
    
    volatile int check = temp[0];
    (void)check;
}

namespace cache_heavy {
    template<int N> struct Fib { static const int value = Fib<N-1>::value + Fib<N-2>::value; };
    template<> struct Fib<0> { static const int value = 0; };
    template<> struct Fib<1> { static const int value = 1; };
    template<int N> int pow2() { return 2 * pow2<N-1>(); }
    template<> int pow2<0>() { return 1; }
}
void cache_compile_heavy_anchor() {
    (void)cache_heavy::Fib<20>::value;
    (void)cache_heavy::pow2<12>();
}
