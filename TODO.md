# All generators

* handle support in exports

* push/pull-buffer support in exports

# wasmtime

* buffer-in-buffer doesn't work. Doesn't work because we can't get a re-access
  of the transaction to add more buffers into it after-the-fact.

* Needs more testing on big-endian.

* Features from wiggle:
  * use `GuestError::InFunc` more liberally
    - stores/loads
    - `try_from` conversions
  * generate just the trait (??? what to do about `wasmtime` dep ???)

# JS

* Is there a better representation for general `variant` types? Currently it's
  `{ tag: string, val: T }` but that seems like it's probably sub-par. There's
  specializations for `option<T>` and `enum` variants, but that's it.

* Is there a better representation for flags than simply an integer?

* Should functions returning `result<T, E>` get translated in JS to functions
  that return `T` and throw `E`?

* Adding imports to an import object is clunky because you need to also pass in
  a closure which extracts values from the raw instance. Unsure how to make this
  less clunky though.

* Needs more testing on big-endian. Specifically slice copies are probably not
  correct.

* Style with names needs to be respected, currently things are using
  `to_snake_case` but I think JS prefers camelCase?

* The `bigint` type is strict in that it does not accept plain `number` types to
  work with it. Should generated bindings be more flexible though and work with
  `number` in addition to `bigint`?

* Host-handle types are always ascribed as `any` but ideally we'd do better than
  that and assign them types. Maybe the type should be imported from somewhere
  else?

* Lifting/lowering of variants can almost surely use a more compressed technique
  which generates less code.

* Enums are handled in lowering as either strings or numbers, but should only
  numbers be handled here? Does anyone pass around strings as enum values?

* Exported handle types in JS aren't nominal. As of this writing they all only
  have a `drop` and a `clone` method so they're interchangeable from `tsc`'s
  perspective. Ideally these would be nominal separate types.

* Imported handle types show up as `any` in TS, unsure how to plumb through
  actual types to get that actually typed.

# Cpp

* Nested lists
* Host: Strings inside records
* Strings test: return-unicode should get out parameter

# Tests

- async: 8 PASS  11 FAIL
------ Failure: cancel-import --------
    missing features
------ Failure: future-cancel-read --------
------ Failure: future-cancel-write --------
------ Failure: future-cancel-write-then-read --------
------ Failure: future-closes-with-error --------
------ Failure: pending-import --------
------ Failure: ping-pong --------
------ Failure: simple-import-params-results --------
------ Failure: simple-pending-import --------
------ Failure: simple-stream --------
------ Failure: simple-stream-payload --------
    
- normal: 22 PASS  30 FAIL
------ Failure: demo --------
    two test impls
------ Failure: flavorful --------
    memory allocation mismatch
------ Failure: lists --------
    memory allocation mismatch
------ Failure: resource_floats --------
------ Failure: resource-import-and-export --------
------ Failure: resources --------
------ Failure: resource_with_lists --------
------ Failure: results --------
    complex so layout
------ Failure: ownership --------
------ Failure: xcrate --------
------ Failure: alternative-bitflags --------
------ Failure: with-and-resources --------
------ Failure: with-option-generate --------
------ Failure: resource_into_inner --------
------ Failure: type_section_suffix --------
------ Failure: with --------
------ Failure: raw-strings --------
------ Failure: skip --------
------ Failure: run-ctors-once-workaround --------
------ Failure: other-dependencies --------
------ Failure: disable-custom-section-link-helpers --------
------ Failure: owned-resource-deref-mut --------
------ Failure: with-types --------
# ------ Failure: strings-simple --------
