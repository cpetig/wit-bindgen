This folder contains tests for the C++ generator.

The `codegen.rs` test generates bindings for the WIT files of the repository
and (with `WASI_SDK_PATH` set) compiles them.

The `meshless_resources` and `meshless_strings` folders experiment
with directly linking two components in a shared everything environment
(`--symmetric`).

The `symmetric_*` folders (`symmetric_async`, `symmetric_future`,
`symmetric_future_string`, `symmetric_lists`, `symmetric_stream`,
`symmetric_stream_string`, `symmetric_tests`) contain tests for the symmetric
ABI, including async support. They are driven by `symmetric.rs`.

`smoke_details` and `misc_wit` hold small WIT inputs used by the tests.
