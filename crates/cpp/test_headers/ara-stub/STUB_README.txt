These headers (ara-stub/ara/core/*.h) are minimal test STUBS of the AUTOSAR
Adaptive ara::core API (a subset of R22-11, kept in line with the AUTOSAR CAPI
headers, https://github.com/AUTOSAR/capi), only used to compile the
`--autosar` bindings without a real AUTOSAR implementation. They are not
conforming implementations.

This directory is deliberately NOT on any default include path (the cpp-client
Makefile and the test harness only add crates/cpp/test_headers), so the stubs
can never shadow real ara headers. Opt in explicitly with
    -I crates/cpp/test_headers/ara-stub
and otherwise point -I to your AUTOSAR installation (or the CAPI directory
core/src/core-types/include).
