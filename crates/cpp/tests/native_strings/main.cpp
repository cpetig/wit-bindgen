
#include "the_world_cpp_native.h"
#include <iostream>

int main() {
    wit::string a = wit::string::from_view(std::string_view("hello A"));
    exports::foo::foo::strings::A(a);

    {
        auto b = exports::foo::foo::strings::B();
        std::cout << b.inner() << std::endl;
        // make sure that b's result is destructed before calling C
    }

    wit::string c1 = wit::string::from_view(std::string_view("hello C1"));
    wit::string c2 = wit::string::from_view(std::string_view("hello C2"));
    auto c = exports::foo::foo::strings::C(c1, c2);
    std::cout << c.inner() << std::endl;
    return 0;
}
