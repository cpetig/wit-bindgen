#include "./cpp_host/the_world_cpp_native.h"
#include <iostream>

using namespace std;

void foo::foo::strings::A(std::string_view x)
{
    std::cout << x << std::endl;
}
wit::string foo::foo::strings::B()
{
    wit::string b = wit::string::from_view(std::string_view("hello B"));
    return b;
}
wit::string foo::foo::strings::C(std::string_view a, std::string_view b)
{
    std::cout << a << '|' << b << std::endl;
    wit::string c = wit::string::from_view(std::string_view("hello C"));
    return c;
}