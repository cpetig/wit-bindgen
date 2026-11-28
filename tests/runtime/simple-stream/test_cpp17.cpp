//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` and callbacks instead of
// coroutines. A stream without payload transfers `wit::Void` elements.
#include <assert.h>
#include <memory>
#include <test_cpp.h>
#include <async_support.h>

std::future<void> exports::my::test::i::ReadStream(wit::stream<wit::Void> x) {
    auto done = std::make_shared<std::promise<void>>();
    auto count = std::make_shared<size_t>(0);
    // called when elements arrive (the stream is closed when it returns false)
    // room for the two elements which are written at once
    x.buffering(2);
    std::move(x).set_reader_until([done, count](wit::span<wit::Void> data) {
        *count += data.size();
        assert(!data.empty());
        if (*count == 3) {
            done->set_value();
            return false;
        }
        return true;
    });
    return done->get_future();
}
