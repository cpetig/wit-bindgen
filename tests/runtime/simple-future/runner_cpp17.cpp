//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    {
        std::promise<void> tx;
        std::future<void> read_task = my::test::i::ReadFuture(tx.get_future());
        tx.set_value();
        read_task.get();
    }
    {
        std::promise<void> tx;
        std::future<void> drop_task = my::test::i::DropFuture(tx.get_future());
        tx.set_value();
        drop_task.get();
    }
    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
