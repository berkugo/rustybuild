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

int main() {
    return 0;
}
