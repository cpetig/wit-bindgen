//@ args = '--async-api future'
//@ symmetric-only = true
//@
//@ [lang]
//@ std = 'c++17'

// C++17 variant of middle.cpp: a thread waits for the import of the leaf
// component, which only completes after the event loop ran
#include <middle_cpp.h>
#include <string>

std::future<wit::string> exports::test::cross::string_delay::Forward(wit::string s) {
    return std::async(std::launch::async, [s = std::move(s)]() {
        if (s.get_view() == "now") {
            return wit::string::from_view("directly returned");
        }
        ::test::cross::wait::Sleep(3).get();
        std::string result = "after sleep: " + std::string(s.get_view());
        return wit::string::from_view(result);
    });
}
