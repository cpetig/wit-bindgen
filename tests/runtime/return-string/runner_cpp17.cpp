//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    auto got = my::test::i::ReturnString().get();
    assert(got.get_view() == "hello");
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
