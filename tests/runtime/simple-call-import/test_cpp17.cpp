//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` instead of coroutines
#include <test_cpp.h>

std::future<void> exports::a::b::i::F() {
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
