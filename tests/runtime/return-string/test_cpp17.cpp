//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` instead of coroutines
#include <test_cpp.h>

std::future<wit::string> exports::my::test::i::ReturnString() {
    std::promise<wit::string> done;
    done.set_value(wit::string::from_view("hello"));
    return done.get_future();
}
