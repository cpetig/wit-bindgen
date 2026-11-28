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
    {
        // the writer is dropped: the reader sees the end of the stream
        auto streams = create_wasi_stream<uint8_t>();
        {
            auto drop_tx = std::move(streams.first);
            (void)drop_tx;
        }
        auto ended = std::make_shared<std::promise<void>>();
        std::move(streams.second).set_reader([ended](wit::span<uint8_t> data) {
            assert(data.empty());
            ended->set_value();
        });
        ended->get_future().get();
    }
    {
        // the reader is dropped: the writer returns with the data unsent
        auto streams = create_wasi_stream<uint8_t>();
        {
            auto drop_rx = std::move(streams.second);
            (void)drop_rx;
        }
        streams.first.write(std::vector<uint8_t>{42});
    }
    {
        std::future<void> read_task;
        {
            auto streams = create_wasi_stream<uint8_t>();
            read_task = my::test::i::ReadStream(std::move(streams.second));
            streams.first.write(std::vector<uint8_t>{0});
            streams.first.write(std::vector<uint8_t>{1, 2});
            streams.first.write(std::vector<uint8_t>{3, 4});
        }
        read_task.get();
    }
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
