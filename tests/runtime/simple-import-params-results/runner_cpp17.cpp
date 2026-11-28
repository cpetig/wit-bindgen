//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    a::b::i::OneArgument(1).get();
    assert(a::b::i::OneResult().get() == 2);
    assert(a::b::i::OneArgumentAndResult(3).get() == 4);
    a::b::i::TwoArguments(5, 6).get();
    assert(a::b::i::TwoArgumentsAndResult(7, 8).get() == 9);
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
