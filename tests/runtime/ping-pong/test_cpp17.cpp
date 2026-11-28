//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <string>
#include <test_cpp.h>

std::future<std::future<wit::string>> exports::my::test::i::Ping(
    std::future<wit::string> x, wit::string y) {
    return std::async(std::launch::async, [x = std::move(x), y = std::move(y)]() mutable {
        auto first = x.get();
        std::string combined(first.get_view());
        combined.append(y.get_view());
        std::promise<wit::string> tx;
        tx.set_value(wit::string::from_view(combined));
        return tx.get_future();
    });
}

std::future<wit::string> exports::my::test::i::Pong(std::future<wit::string> x) {
    return std::async(std::launch::async, [x = std::move(x)]() mutable { return x.get(); });
}
