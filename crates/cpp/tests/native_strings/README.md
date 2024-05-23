# Native string example

In this example there are two guests, guest_1 and guest_2
They use the mesh/native-host code to communicate to each other

The wit-bindgen creates the guest bindings and native-host code

This is how the example works, call graph for the function `A` (communication between the guest_1 and guest_2 using the mesh/native host code)

guest_1->exports::foo::foo::strings::A(a){native host export call}->fooX3AfooX2FstringsX23a(){guest export binding}
-> exports::foo::foo::strings::A(wit::string &&x){guest_1 export implementation}
-> foo::foo::strings::A(std::string_view x){guest import call}->fooX3AfooX2FstringsX00a() {guest import binding}
-> foo::foo::strings::A(std::string_view x) { guest_2 import implementation}
