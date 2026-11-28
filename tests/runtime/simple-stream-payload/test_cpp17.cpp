//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of test.cpp: `std::future` and callbacks instead of coroutines
#include <assert.h>
#include <memory>
#include <test_cpp.h>
#include <async_support.h>

std::future<void> exports::my::test::i::ReadStream(wit::stream<uint8_t> x) {
    auto done = std::make_shared<std::promise<void>>();
    auto received = std::make_shared<std::vector<uint8_t>>();
    // called when elements arrive (the stream is closed when it returns false)
    std::move(x).set_reader_until([done, received](wit::span<uint8_t> data) {
        assert(!data.empty());
        received->insert(received->end(), data.begin(), data.end());
        if (received->size() == 5) {
            assert((*received == std::vector<uint8_t>{0, 1, 2, 3, 4}));
            done->set_value();
            return false;
        }
        return true;
    });
    return done->get_future();
}
