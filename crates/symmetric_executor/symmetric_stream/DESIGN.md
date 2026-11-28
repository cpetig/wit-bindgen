# Symmetric stream detailed design

The protocol (version 0.4.0 of `symmetric:runtime`) is specified at the top of
`../wit/stream.wit`, the state machine of the implementation is documented at
the top of `src/lib.rs`. This file only summarizes the pieces.

## Elements (per stream, all under one mutex)
- `read_offer`: an empty buffer offered by the reader, waiting for a writer
- `write_offer`: a filled buffer offered by the writer, waiting for a reader
- `read_lent`: the reader's offer while a writer fills it directly
  (`start-writing` .. `finish-writing`, a short section; `cancel-read` and
  closing the reader wait for it, using a condition variable)
- `read_done`, `write_done`: completed operations (the buffer with the number
  of elements transferred), collected by `read-result` / `write-result`
- `readers`, `writers`: number of open handles, 0 means closed
- `read_event` (reader subscribes, activated on completion/EOF),
  `write_event` (writer subscribes, activated on completion, when a reader
  offers a buffer, and on close of the reader)

## Rendezvous
Whoever arrives second performs the transfer inside its call (`start-reading`
or `offer-write`), `min(capacity, size)` elements are copied, both operations
complete with that count. Writers of the same process may also borrow the
reader's offer (`start-writing`/`finish-writing`) to fill it without an
intermediate copy.

## Cancellation / close
`cancel-read`/`cancel-write` withdraw an offer or return the already completed
buffer (with its count), after that the runtime does not reference the memory.
The last handle of a direction going away withdraws the offers of that side and
wakes the other side (EOF).

## Potential extensions
- extend towards queueing multiple buffers in flight
