//@ wasmtime-flags = '-Wcomponent-model-async'

#include <assert.h>
#include <runner_cpp.h>

wit::Task<void> exports::runner::Run() {
    {
        auto r = co_await test::cross::string_delay::Forward(wit::string::from_view("now"));
        assert(r.get_view() == "directly returned");
    }
    {
        auto task = test::cross::string_delay::Forward(wit::string::from_view("x"));
        assert(!task.done());
        auto r = co_await task;
        assert(r.get_view() == "after sleep: x");
    }
}
