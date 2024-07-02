# Introduction

The native-mesh example contains two components, `component_a` and `component_b`.
Both these components communicate to each other using `mesh`.

This example has been implemented in C++ as well as Rust.

# Building

To build the C++ version, use the make file.

Since the `wit-bindgen` does not create the complete code for `Rust`, the bindings are
generated, hand patched and stored with the code.

To build each and every individual components, use `cargo build` inside that specific directory.
Or
Just do `cargo build component_a`, will build all the dependent components and the binary.

Build the `component_b`, `mesh` and `component_a` in that order.

The actual binary is `component_a` and will be present in `wit-bindgen/target/debug`.

# Running

To run the application use `caro run` inside the `component_a` 
Or
once the build is successful, run the binary from `./wit-bindgen/target/debug/component_a`

```

The code directory structure looks as below

├── Makefile
├── cpp
│   ├── component_a
│   │   ├── Makefile
│   │   ├── a.cpp
│   │   ├── a_cpp.h
│   │   ├── component_a
│   │   ├── libcomponent_b.so -> ../component_b/libcomponent_b.so
│   │   ├── libmesh.so -> ../mesh/libmesh.so
│   │   └── main.cpp
│   ├── component_b
│   │   ├── Makefile
│   │   ├── b.cpp
│   │   ├── b_cpp.h
│   │   ├── exports-foo-foo-resources-R.h
│   │   ├── impl.cpp
│   │   └── libcomponent_b.so
│   └── mesh
│       ├── Makefile
│       ├── impl.cpp
│       ├── libcomponent_b.so -> ../component_b/libcomponent_b.so
│       ├── libmesh.so
│       ├── mesh-exports-foo-foo-resources-R.h
│       ├── mesh-exports-foo-foo-resources.h
│       ├── mesh_cpp_native.h
│       └── mesh_native.cpp
├── rust
│   ├── README.md
│   ├── component_a
│   │   ├── Cargo.lock
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── a.rs
│   │       ├── a_impl.rs
│   │       └── main.rs
│   ├── component_b
│   │   ├── Cargo.lock
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── b.rs
│   │       ├── b_impl.rs
│   │       ├── lib.rs
│   │       └── resource_table.rs
│   └── mesh
│       ├── Cargo.lock
│       ├── Cargo.toml
│       └── src
│           ├── impl_mesh.rs
│           ├── lib.rs
│           └── mesh.rs
└── wit
    └── resources_simple.wit ( Contains the wit interface definitions for the components and the mesh)

```

# Internal working

`component_b` export three interfaces `create`, `add` and `consume`. `component_a` uses those `interfaces`.

The `component_a` askes for creating a resource in `component_b` address space through the `mesh`.
So, `component_a` a receive the created object through `mesh`.
The `mesh` acts like a transport between the two components.
`mesh` stores the `resource` in its table and pass the `reference` to that object.

The same functionality happens for all the three apis that are exported by the `component_b`.


Note: Rust does not build the .so objects for the libraries, it uses the `crates` mechanism to share the interfaces 
with other components.







