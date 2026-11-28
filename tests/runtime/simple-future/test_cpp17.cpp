//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` instead of coroutines. A future
// becomes ready when the executor has delivered the value, so a thread waits
// for it instead of blocking the caller.
#include <test_cpp.h>

std::future<void> exports::my::test::i::ReadFuture(std::future<void> x) {
    return std::async(std::launch::async, [x = std::move(x)]() mutable { x.get(); });
}

std::future<void> exports::my::test::i::DropFuture(std::future<void> x) {
    // there is no way to tell the writer that the future has been dropped,
    // the value will just be discarded
    (void)x;
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
