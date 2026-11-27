#include <leaf_cpp.h>

// Completes only after the event loop ran `rounds` times.
wit::Task<void> exports::test::cross::wait::Sleep(uint32_t rounds) {
    for (uint32_t i = 0; i < rounds; ++i) {
        co_await wit::yield_async();
    }
}
