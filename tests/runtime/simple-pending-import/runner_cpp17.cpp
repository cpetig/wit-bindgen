//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    std::future<void> pending = a::b::i::F();
    pending.get();
    return std::async(std::launch::deferred, [] {});
}
