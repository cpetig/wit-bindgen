//! For build scripts of crates linking against cdylib dependencies like symmetric_executor.
//!
//! Depending on this crate makes cargo build the symmetric runtime libraries before the build
//! script runs, so that they can be found.

include!("scan.rs");
