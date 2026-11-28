//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <runner_cpp.h>
#include <async_support.h>

std::future<void> exports::runner::Run() {
    std::future<void> read_task;
    {
        auto streams = create_wasi_stream<wit::Void>();
        read_task = my::test::i::ReadStream(std::move(streams.second));
        // blocks until the reader has taken the elements
        streams.first.write(std::vector<wit::Void>(1));
        streams.first.write(std::vector<wit::Void>(2));
        // dropping the writer ends the stream
    }
    read_task.get();
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
