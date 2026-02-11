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
            int temph = a + b;
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

int main() {
    std::vector<int> vec = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
    process_data(vec);
    int sum = compute_sum(vec);
    double power = compute_power(2.0, 10);
    int fib = fibonacci(15);
    
    std::vector<double> values = {1.1, 2.2, 3.3, 4.4, 5.5};
    double sum2 = compute_sum(values);
    double product = compute_product(values);
    
    for (int i = 0; i < 100; ++i) {
        sum2 += std::sin(i * 0.1);
        product *= std::cos(i * 0.1);
    }
    
    volatile int result = sum + static_cast<int>(power) + fib + static_cast<int>(sum2 + product);
    (void)result;
    
    return 0;
}
