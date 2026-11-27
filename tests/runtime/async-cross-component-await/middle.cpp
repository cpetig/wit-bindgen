#include <middle_cpp.h>

wit::Task<wit::string> exports::test::cross::string_delay::Forward(wit::string s) {
    if (s.get_view() == "now") {
        co_return wit::string::from_view("directly returned");
    }
    co_await ::test::cross::wait::Sleep(3);
    std::string result = "after sleep: " + std::string(s.get_view());
    co_return wit::string::from_view(result);
}
