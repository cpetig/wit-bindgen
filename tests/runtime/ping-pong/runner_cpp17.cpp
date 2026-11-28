//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    std::promise<wit::string> tx1;
    auto f1 = my::test::i::Ping(tx1.get_future(), wit::string::from_view("world"));
    tx1.set_value(wit::string::from_view("hello"));
    std::future<wit::string> rx2 = f1.get();
    auto m2 = rx2.get();
    assert(m2.get_view() == "helloworld");

    std::promise<wit::string> tx2;
    auto f3 = my::test::i::Pong(tx2.get_future());
    tx2.set_value(std::move(m2));
    auto m3 = f3.get();
    assert(m3.get_view() == "helloworld");

    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
