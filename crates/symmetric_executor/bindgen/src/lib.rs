// this crate tries to minimize dependencies for symmetric bindings

#[cfg(feature = "symmetric")]
pub mod rt {
    pub use dummy_rt::rt::{maybe_link_cabi_realloc, Resource, ResourceRep};
    pub use wit_bindgen_symmetric_rt::{
        async_support, bitflags, run, Cleanup, EventGenerator, EventSubscription,
    };
}

#[cfg(feature = "symmetric")]
pub use dummy_rt::resource;

#[cfg(feature = "canonical")]
pub use original::{generate, resource, rt};

#[cfg(feature = "async")]
pub use rt::async_support::{
    block_on, spawn, FutureReader, FutureWriteCancel, FutureWriter, StreamReader, StreamResult,
    StreamWriter,
};

#[cfg(feature = "symmetric")]
pub use wit_bindgen_rust_macro::generate;
