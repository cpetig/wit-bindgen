//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of runner.cpp: `std::future` instead of coroutines
#include <assert.h>
#include <runner_cpp.h>

std::future<void> exports::runner::Run() {
    auto r1 = my::test::i::UnitOk().get();
    assert(r1.has_value());

    auto r2 = my::test::i::UnitErr().get();
    assert(!r2.has_value());
    assert(r2.error().get_view() == "unit");

    auto r3 = my::test::i::Halve(8).get();
    assert(r3.has_value());
    assert(*r3 == 4);

    auto r4 = my::test::i::Halve(7).get();
    assert(!r4.has_value());
    assert(r4.error().get_view() == "odd");

    std::promise<void> done;
    done.set_value();
    return done.get_future();
}
