//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <test_cpp.h>

template <class T> static std::future<T> ready(T value) {
    std::promise<T> done;
    done.set_value(std::move(value));
    return done.get_future();
}

static std::future<void> ready() {
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}

std::future<void> exports::a::b::i::OneArgument(uint32_t x) {
    assert(x == 1);
    return ready();
}

std::future<uint32_t> exports::a::b::i::OneResult() {
    return ready<uint32_t>(2);
}

std::future<uint32_t> exports::a::b::i::OneArgumentAndResult(uint32_t x) {
    assert(x == 3);
    return ready<uint32_t>(4);
}

std::future<void> exports::a::b::i::TwoArguments(uint32_t x, uint32_t y) {
    assert(x == 5);
    assert(y == 6);
    return ready();
}

std::future<uint32_t> exports::a::b::i::TwoArgumentsAndResult(uint32_t x, uint32_t y) {
    assert(x == 7);
    assert(y == 8);
    return ready<uint32_t>(9);
}
