This folder contains tests for the C++ generator.

The `codegen.rs` test generates bindings for the WIT files of the repository
and (with `WASI_SDK_PATH` set) compiles them.

The C++ runtime behavior, including the symmetric ABI (`--symmetric`), is
tested by the runtime tests in `tests/runtime` and `tests/runtime-async`,
run via `wit-bindgen test`.

`symmetric_async` is a hand-written example of a C++ async export awaiting
an async import of another component. It will be replaced by a runtime-async
test.
