# Symmetric executor

The executor coordinates the async parts of the symmetric ABI: components are
native shared objects calling each other directly, an async call returns
either NULL (done) or an `event-subscription` which becomes ready when the
callee's task completes. The interface is `wit/executor.wit` (package
`symmetric:runtime`, version history in `wit/world.wit`), the streams are
described in `symmetric_stream/DESIGN.md`.

Runtimes using it: `rust-client` (Rust guests), `c-client`, `cpp-client` (C++17
`--async-api future`) and the `WIT_SYMMETRIC` part of
`crates/cpp/helper-types/wit.h` (C++20 coroutines).

## Callbacks and registrations (0.6.0)

- `register` / `register-unique` queue a callback for an event. Callbacks never
  run while the task list is locked, so a callback may register, cancel,
  `block-on` or `cancel-task` (a nested loop runs all registered callbacks).
- `callback-registration.cancel` returns `some(data)` if the callback will not
  run any more and did not free its data. It returns `none` if the callback
  returned `ready` (it freed the data, even while `cancel` waited for it on
  another thread), if it completed inside `register`, or if it is running on
  the calling thread right now. After `cancel` returns the callback is never
  called again. `is-pending` tells whether it may still fire. Dropping the
  handle without `cancel` keeps the registration.

## Task cancellation (0.5.0)

- Caller: `cancel-task(subscription)` returns `false` at once if the task
  completed already. Otherwise it activates the cancel generator the callee
  linked (`event-generator.link-cancel`) and runs the executor until the task
  completes. The result is `true` if the callee ended with
  `activate-cancelled` (results not written), `false` if it completed normally.
  Afterwards the callee no longer touches arguments or results.
- Callee: registers on its cancel event; on cancellation it drops its state
  (arguments included) and calls `activate-cancelled`. A callee without
  `link-cancel` runs to completion while the caller waits.
- Rust: dropping a pending import future cancels; export tasks (also those
  queued by backpressure) are cancellable. C: `subtask_cancel` /
  `task_cancel` / `EVENT_CANCEL`. C++20: destroying a pending `wit::Task` of
  an import cancels, exported coroutines are destroyed on cancellation.
  C++17: no cancellation (see crates/cpp/DESIGN.md).

## Open problems

Executor and protocol:
- A caller which drops the completion subscription of a pending call without
  `cancel-task` leaves the callee running: Rust export tasks stay registered
  on their cancel event, so `run()` waits for a task which may never end. The
  callee can't tell that nobody holds a subscription any more; fixing it needs
  the executor to count user held subscriptions of the completion event and
  not to count registrations on a linked cancel event as keeping `run()` alive.
- `cancel` of a registration from inside a callback, on another thread's
  callback which itself waits for the canceller, deadlocks (inherent to the
  "callback is not called after cancel returns" guarantee).
- A registration cancelled while another thread's tick is checking it is
  dropped by that tick, so its eventfd is released shortly after `cancel`
  returns instead of before.
- Cancelling the import tasks of the callee (canonical `subtask.cancel`
  returning `BLOCKED`, i.e. asynchronous cancellation) is not available:
  `cancel-task` always waits.

Runtimes:
- Rust: import arguments kept in `_deallocate` (lists, owned strings) leak when
  the import future is dropped mid-call; fixing it needs a drop guard in the
  generated code. When `cancel-task` reports a normal completion the results
  written by the callee are not lifted, so owned results (strings, lists,
  resources) leak.
- C: `backpressure_inc`/`backpressure_dec` are not supported in symmetric mode
  (abort); `cancel-import` with a C component is an expected failure for this
  and because the C runner asserts the canonical STARTING state, while
  symmetric calls start right away. The global wake generator of
  `wit_async_support.c` is never dropped.
- C++17 (`--async-api future`): no cancellation; `lift_event` keeps its
  promise until the event fires.
- Linking chains of components uses `imp_` forwarding trampolines
  (`--forward-prefix`, x86_64 and aarch64), a workaround.

Tests and build:
- `resource-import-and-export` needs the renaming done by its `compose.wac`.
- C++ symmetric: `accessors/test.cpp` does not compile, `lists` crashes
  (SIGSEGV) in mixed Rust/C++ combinations, four mixed `strings` combinations
  with C fail, `param-ownership | runner-owning.cpp | test.cpp` passes despite
  its expected-failure entry.
- `cargo test -p wit-bindgen-symmetric-rt` fails to link and the `bindgen`
  workspace member does not build; cargo runs in this workspace rewrite
  `Cargo.lock`.
- The hand written tests in `crates/cpp/tests` are outdated (e.g.
  `symmetric_async/sleep` still uses runtime 0.3.0) and superseded by
  `tests/runtime`.
