//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    a::b::i::F().get();
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
