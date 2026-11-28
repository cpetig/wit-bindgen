# Type mapping

| Code | Environment |
| --- | --- |
| G-- | guest side |
| H-- | host side |
| -I- | guest-import (guest calls) |
| -E- | guest-export (host calls) |
| --A | argument |
| --R | result |
| --S | in struct |

| mode | |
| --- | --- |
| v | passed by value |
| t | owernership transferred |
| p | cabi_post_ cleans up |

| API | | | ABI | |
| --- | --- | --- | --- | --- |
| 🌓 | asymmetric | | 📘 | canonical |
| ⚖️ | symmetric | | 🪞 | symmetric |

| Code | mode | WIT Type | Rust type | C++ Type | Lower | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| GIA | v | string | &str[^1] | string_view (17) | addr, len | |
| | | list | &[T] | std::span [^5] | addr, len | |
| | | tuple | (...) | std::tuple | 0, 1, ...| |
| | | tuple<string, list> | (&str, &[T]) | std::tuple<...> | a,l,a,l |
| | | record{string, list} | &T | T const& | a,l,a,l |
| | | large-struct (>16 args) | &T | T const& | &t |
| | | result<string,list> | Result<&str, &[]> | std::expected<string_view, span> | d,a,l |
| | | option\<string> | Option\<&str> | optional<string_view> const& | d,a,l|
| | | list\<resrc> | &[\&Resrc]? | vector<string_view> const& | a,l|
| GIR | t | string | String | wit::string[^2] | &(addr, len) [^7] | |
| | | list | Vec | wit::vector | &(a,l) |
| | | result<string,list> | Result<String, Vec> | std::expected<wit::string, wit::vector> | &(d,a,l) |
| GEA | t | string | String | 🌓 wit::string | addr, len |
| | | | | ⚖️ string_view | |
| | | result<string,list> | Result<String, Vec> | 🌓 std::expected<wit::string, wit::vector> | d,a,l |
| | | | | ⚖️ std::expected<string_view, wit::span> | |
| GER | p | string | String | wit::string (or std?) | 📘 -> &(a,l) cabi_post_N:P/I#F [^6] |
| | | | | | 🪞 &(a,l) |
| | | result<string,list> | Result<String, Vec> | std::expected<wit::string, wit::vector> | 📘 -> &(d,a,l) cabi_post |
| --S | ? | string | String | wit::string | addr, len |
| HIA | v | string | | string_view | a,l |
| HIR | t | string | | wit::string[^3] | &(a,l) |
| HEA | t | string | | 🌓 wit::string[^4] | a,l |
| | | | | ⚖️ string_view [^5] | |
| HER | p | string | | 🌓 wit::guest_owned<string_view> | 📘 -> &(a,l) |
| | | | | ⚖️ wit::string [^5] | 🪞 &(a,l) |

[^1]: The host never frees memory (is never passed ownership)!

[^2]: A wit::string is identical to the canonical representation, so it can be part of structures. On the guest a wit::string owns the memory and frees it after use.
On the host a wit::string can be constructed(=allocated) with an exec_env argument. Thus, without an exec_env a wit::string on the host is inaccessible.
Complex (non-POD) struct elements on the host will need exec_env to decode or construct.

[^3]: A wit::string requires exec_env inside the host implementation. ~~Perhaps a flexible type (either std::string or wit::string would be possible), or make this a generation option?~~ std::string requires a copy, wit::string requires passing exec_env to the method (which is necessary for methods anyway).

[^4]: A host side wit::string doesn't own the data (not free in dtor), thus no move semantics.

[^5]: Not implemented, for now symmetric is priority

[^6]: Here the callee (guest) allocates the memory for the set on its side

[^7]: Caller passes address of the return object as argument

## [Symmetric ABI](https://github.com/WebAssembly/component-model/issues/386)

The idea is to directly connect (link) components to each other.

Thus imported and exported functions and resources need to be compatible
at the ABI level.

For now for functions the guest import convention is used in both directions:

- The imported function ABI is used with the following properties

  - (unchanged) List and string arguments are passed as Views, no free
    required, lifetime is constrained until the end of the call

  - (unchanged) Owned resources in arguments or results pass ownership
    to the callee

  - (unchanged) If there are too many (>1) flat results, a local
    uninitialized ret_area is passed via the last argument

  - (unchanged) Returned objects are owned.
    For functional safety, i.e. avoiding all
    allocations in the hot path, the hope is with [#385](https://github.com/WebAssembly/component-model/issues/385).

- The imported resource ABI is used also for exporting
  with one modification:

   Resource IDs become usize, so you can optimize the resource table away.

### Async functions (WASI 0.3)

- The *exported* ABI guides the symmetric choice, arguments are directly 
  passed like with synchronous function calls, if a return value is used a 
  return pointer is added. The returned value is either nullptr (finished)
  or the handle/address of an event subscription (see symmetric_executor crate).

  No need for externally visible callbacks as the code directly registers 
  its callbacks with the executor. No need for set_results.

- Stream and future handles become pointer size, the internal API is described via
  a WIT file (symmetric_stream)

### Canonical Options

The component binary format defines several options for lifting
and lowering. As they create binary incompatibility for symmetric
bindings they become part of the encoded function name. As mixed case
symbols are invalid in WIT we use upper case letters to encode the
options:

- String Encoding: "" utf-8, "W" utf-16 (wide), "L" latin1+utf-16
- "A" async
- memory isn't meaningful for symmetric
- realloc can't be specified by the current Rust tooling
- post_return and callback already have a naming convention

## Structs proposal

See also the explanation of ownership at https://docs.rs/wit-bindgen/0.42.1/wit_bindgen/macro.generate.html

```
resource r; 
record d { s: string, l: list<r> }
arg: func(d: d);
result: func() -> d;
```

```
struct DResult {
  wit::string s;
  wit::list<R> l;
}
struct DParam {
  std::string_view s;
  std::span<R> l;
}
```

|direction|style|
|---|---|
|GIA|void arg(DParam d);|
|GIR|DResult result();|
|GEA|void arg(DResult d);|
|GER|DResult result();|

## C++20 Coroutine & Async Support

The C++ backend targets C++20 (`-std=c++20`).

Asynchronous functions in WIT are mapped to idiomatic C++20 coroutines:
- Functions defined as `async func(...) -> T` or filtered via `--async`/`--sync` return `wit::Task<T>` (or `wit::Task<void>` for functions returning void).
- Callers can `co_await` imported async functions or inspect completion with `.done()`, `.get()`.
- Export implementations use `co_return` (or `co_return value;`) within a coroutine body returning `wit::Task<T>`.
- WIT `future<T>` is mapped to `wit::FutureReader<T>`.
- WIT `stream<T>` is mapped to `wit::StreamReader<T>`.
- WIT `error-context` is mapped to `wit::ErrorContext`.

With the symmetric ABI, destroying the `wit::Task` of an async import while the
call is still in progress cancels the call (`cancel-task` of the executor, it
returns once the callee no longer uses the arguments and the result area). An
async export implemented as a coroutine supports this: when its caller
cancels it, the coroutine frame is destroyed (dropping the arguments) and the
caller is told that no results were written (`activate-cancelled`).

## C++17 async API (`--async-api future`, symmetric only)

MISRA C++:2023 is based on C++17, which has no coroutines. For the symmetric
ABI (native code, threads are available) the generator can therefore map
async functions, futures and streams to `std::future` and threads instead of
coroutines:

| WIT | C++17 (`--async-api future`) | C++20 (default, `coroutine`) |
|---|---|---|
| `async func(...) -> T` (import) | returns `std::future<T>` | returns `wit::Task<T>` |
| `async func(...) -> T` (export) | implemented by returning `std::future<T>` | coroutine returning `wit::Task<T>` |
| `future<T>` | `std::future<T>` | `wit::FutureReader<T>` |
| `stream<T>` | `wit::stream<T>` (callback reader, `stream_writer<T>`) | `wit::StreamReader<T>` |
| `list<T>` view | `wit::span<T>` | `std::span<T>` |

With `--autosar --symmetric` this mode is implied, and `std::future` /
`std::promise` become `ara::core::Future` / `ara::core::Promise`
(`future_compat.h`, `WIT_ARA_FUTURE`). The option is rejected for the
canonical (wasm) ABI, as it needs threads.

The runtime part lives in `crates/symmetric_executor/cpp-client`
(`async_support.h`, `stream_support.h`).

### How it works

The executor is not thread safe in the sense of polling a task from two
threads, so everything blocking is moved to a helper thread which reports back
through an executor event:

- **Async import:** if the callee did not finish synchronously, a helper thread
  blocks on the returned event (which also runs the executor) and then lifts
  the results into the `std::future`. The result area (`RetArea`) is owned by
  that thread, so dropping the `std::future` early is memory safe.
- **Async export:** the user returns a `std::future`. If it is not ready yet, a
  helper thread waits for it, writes the results and activates the completion
  event of the caller.
- **`future<T>` written by C++17:** a ready value is copied into the stream at
  once (`write-detached`). Otherwise a helper thread waits for the
  `std::future` and writes it afterwards.
- **`future<T>` read by C++17:** the `std::promise` is fulfilled from an
  executor callback. If the writer goes away without a value, `get()` throws
  `std::future_error(broken_promise)`; with ara::core the error is
  `future_errc::broken_promise` in the `Result`.
- **`stream<T>`:** the reader registers a callback (`set_reader` /
  `set_reader_until`) which is called with the elements as they arrive.
  `stream_writer<T>::write` blocks until the reader took all elements or went
  away; `write_nb` only hands over what a waiting reader takes right now.

### Limitations

These differ from the canonical ABI and from the C++20 mode. The test
harness lists the combinations they break in `SYMMETRIC_EXPECTED_FAILURES`
(`crates/test/src/lib.rs`).

- **No cancellation.** Dropping a `std::future` does not cancel the async
  import or tell the writer of a `future<T>` that the reader is gone. The
  helper thread runs the call to completion and the value is discarded.
  Writers which check whether their value was received see it as delivered.
  An async export implemented in C++17 cannot be cancelled by its caller: a
  cancelling caller waits until the `std::future` is ready.
- **Asynchronous even when ready-ish.** An export returns synchronously
  only if its `std::future` is already ready when the function returns.
  Otherwise completion is reported later through the helper thread, even if
  the value becomes available a moment later. The same holds for a
  `future<T>` passed to a callee: if the `std::future` is not ready yet, its
  value arrives after the call returned. Peers which expect a synchronous
  result (as a canonical writer which wrote before the call would deliver it)
  see a pending operation instead.
- **Blocking writes.** `stream_writer<T>::write` blocks the calling thread
  until the reader took everything. A peer that expects the write to be in
  flight (pending) before it starts reading can deadlock or observe a
  different order of events. Use `write_nb` from code running on the
  executor.
- **Small read buffer.** The stream reader offers a buffer of
  `buffering(n)` elements, 1 by default. As in the canonical ABI, a larger
  write is then consumed in several steps. A writer which expects one write
  to transfer everything at once sees a partial transfer.
- **One thread per pending operation.** Every pending import, export or
  `future<T>` write occupies a thread until it completes. Threads which have
  finished are joined by an executor callback; the remaining ones are joined
  when the library is unloaded. This is not suited for many concurrent
  operations or for systems which forbid dynamic thread creation.
- **ara::core.** `ara::core::Future` has no exceptions, so `wit::future_get`
  aborts on an error result. There is no `std::async`, so plain
  `std::thread`s are used.
