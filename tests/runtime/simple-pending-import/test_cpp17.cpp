//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: the future is only ready after a thread has
// slept, so the export reports a pending call to its caller
#include <chrono>
#include <test_cpp.h>
#include <thread>

std::future<void> exports::a::b::i::F() {
    return std::async(std::launch::async, [] {
        for (int i = 0; i < 10; ++i) {
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
    });
}
