//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` instead of coroutines
#include <test_cpp.h>

template <class T> static std::future<T> ready(T value) {
    std::promise<T> done;
    done.set_value(std::move(value));
    return done.get_future();
}

std::future<std::expected<void, wit::Void>> exports::my::test::i::UnitOk() {
    return ready(std::expected<void, wit::Void>());
}

std::future<std::expected<void, wit::string>> exports::my::test::i::UnitErr() {
    return ready(std::expected<void, wit::string>(std::unexpected(wit::string::from_view("unit"))));
}

std::future<std::expected<uint32_t, wit::string>> exports::my::test::i::Halve(uint32_t x) {
    if (x % 2 != 0) {
        return ready(std::expected<uint32_t, wit::string>(std::unexpected(wit::string::from_view("odd"))));
    }
    return ready(std::expected<uint32_t, wit::string>(x / 2));
}
