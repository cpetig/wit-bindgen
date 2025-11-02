#!/bin/sh
(cd rust-client/src;../../../../target/debug/wit-bindgen rust ../../wit -w executor-import --symmetric --format --link-name symmetric_executor ; cd .. ; cargo fmt)
(cd rust-client/src;../../../../target/debug/wit-bindgen rust ../../wit -w stream-import --symmetric --format --link-name symmetric_stream ; cd .. ; cargo fmt)
(cd src;../../../target/debug/wit-bindgen rust ../wit -w executor --symmetric --format ; cd .. ; cargo fmt)
(cd symmetric_stream/src;../../../../target/debug/wit-bindgen rust ../../wit -w stream-impl --symmetric --format ; cd .. ; cargo fmt)
