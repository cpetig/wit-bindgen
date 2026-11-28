//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    {
        auto r = test::cross::string_delay::Forward(wit::string::from_view("now")).get();
        assert(r.get_view() == "directly returned");
    }
    {
        auto task = test::cross::string_delay::Forward(wit::string::from_view("x"));
        // The middle component waits for the leaf component, but whether the
        // future is already ready here can't be asserted: a helper thread of
        // the import blocks on the executor and may well finish before this
        // thread looks (runner.cpp asserts `!task.done()`, which is
        // deterministic for coroutines).
        auto r = task.get();
        assert(r.get_view() == "after sleep: x");
    }
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
