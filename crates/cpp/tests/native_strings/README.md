# Native string example

In this example, there is a guest, a native host (cpp_host) and an application.
The `guest + native host` together form the actual `guest` code, which exports some APIs.
Now the application which is `host`, will use the guest exported calls.
The application/host also imports some calls, those are part of the `guest_imported_functions.cpp`

The directory strucutre is as below:
```
├── cpp_host (Native host code)
├── guest    (Guest code)
├── guest_imported_fns.cpp ( host application imported calls)
├── main.cpp (Application)
└── wit (Wit file that is used for generating the code)

```

# Call-graph for a function

This is how the example works, call graph for the function `A` (communication between the guest_1 and guest_2 using the mesh/native host code)

guest_1->exports::foo::foo::strings::A(a){native host export call}->fooX3AfooX2FstringsX23a(){native host export binding(lifting)}
-> exports::foo::foo::strings::A(wit::string &&x){guest_1 export implementation}
-> foo::foo::strings::A(std::string_view x){guest import call}->fooX3AfooX2FstringsX00a() {native host import binding(lowering)}
-> foo::foo::strings::A(std::string_view x) { guest import functions implementation}
