// mod component_type_object;

use heck::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::mem;
use wit_bindgen_core::abi::{AbiVariant, Bindgen, Instruction, LiftLower, WasmSignature, WasmType};
use wit_bindgen_core::{
    uwrite, uwriteln, wit_parser::*, Files, InterfaceGenerator as _, Ns, TypeInfo, Types,
    WorldGenerator,
};
use wit_bindgen_rust_lib::{
    dealias, FnSig, Ownership, RustFlagsRepr, RustFunctionGenerator, RustGenerator, TypeMode,
};
use wit_component::StringEncoding;

pub const RESOURCE_BASE_CLASS_NAME: &str = "ResourceBase";
pub const OWNED_CLASS_NAME: &str = "Owned";

// follows https://google.github.io/styleguide/cppguide.html

#[derive(Default)]
struct Includes {
    needs_vector: bool,
    needs_expected: bool,
    needs_string: bool,
    needs_string_view: bool,
    needs_optional: bool,
    needs_cstring: bool,
    needs_guest_alloc: bool,
    needs_resources: bool,
}

#[derive(Clone)]
struct HostFunction {
    wasm_name: String,
    wamr_signature: String,
    host_name: String,
}

enum Context {
    Argument,
    ReturnValue,
    InStruct,
}

#[derive(Default, Copy, Clone, PartialEq, Eq)]
enum Direction {
    #[default]
    Import,
    Export,
}

#[derive(Default)]
struct ResourceInfo {
    direction: Direction,
    owned: bool,
    docs: Docs,
}

#[derive(Default)]
struct CppHost {
    src: Source,
    opts: Opts,
    dependencies: Includes,
    includes: Vec<String>,
    names: Ns,
    world: String,
    sizes: SizeAlign,

    // per module
    host_functions: HashMap<String, Vec<HostFunction>>,

    // Known names for interfaces as they're seen in imports and exports.
    //
    // This is subsequently used to generate a namespace for each type that's
    // used, but only in the case that the interface itself doesn't already have
    // an original name.
    interface_names: HashMap<InterfaceId, WorldKey>,

    // Interfaces who have had their types printed.
    //
    // This is used to guard against printing the types for an interface twice.
    // The same interface can be both imported and exported in which case only
    // one set of types is generated and all bindings for both imports and
    // exports use that set of types.
    interfaces_with_types_printed: HashSet<InterfaceId>,

    // Type definitions for the given `TypeId`. This is printed topologically
    // at the end.
    typedefs: HashMap<TypeId, wit_bindgen_core::Source>,
    types: Types,

    // The set of types that are considered public (aka need to be in the
    // header file) which are anonymous and we're effectively monomorphizing.
    // This is discovered lazily when printing type names.
    public_anonymous_types: BTreeSet<TypeId>,

    // This is similar to `public_anonymous_types` where it's discovered
    // lazily, but the set here are for private types only used in the
    // implementation of functions. These types go in the implementation file,
    // not the header file.
    private_anonymous_types: BTreeSet<TypeId>,

    resources: HashMap<TypeId, ResourceInfo>,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct Opts {
    /// Set component string encoding
    #[cfg_attr(feature = "clap", arg(long, default_value_t = StringEncoding::default()))]
    pub string_encoding: StringEncoding,
    /// Create guest header
    #[cfg_attr(feature = "clap", arg(long, default_value_t = bool::default()))]
    pub guest_header: bool,
    /// Generate code for directly linking to guest code
    #[cfg_attr(feature = "clap", arg(long, default_value_t = bool::default()))]
    pub short_cut: bool,
}

impl Opts {
    pub fn build(&self) -> Box<dyn WorldGenerator> {
        let mut r = CppHost::default();
        r.opts = self.clone();
        Box::new(r)
    }
}

#[derive(Debug, Default)]
struct Return {
    scalar: Option<Scalar>,
    retptrs: Vec<Type>,
}

#[derive(Debug)]
struct CSig {
    name: String,
    // sig: String,
    params: Vec<(bool, String)>,
    ret: Return,
    retptrs: Vec<String>,
}

#[derive(Debug)]
enum Scalar {
    Void,
    // OptionBool(Type),
    // ResultBool(Option<Type>, Option<Type>),
    Type(Type),
}

// fn owner_namespace_g(resolve: &Resolve, ty: &TypeDef, world: &str) -> String {
//     match &ty.owner {
//         // If this type belongs to an interface, then use that interface's
//         // original name if it's listed in the source. Otherwise if it's an
//         // "anonymous" interface as part of a world use the name of the
//         // import/export in the world which would have been stored in
//         // `interface_names`.
//         TypeOwner::Interface(owner) => match resolve.interfaces[*owner].name {
//             Some(_) => todo!(),
//             None => todo!(),
//             // WorldKey::Name(name) => name.to_snake_case(),
//             // WorldKey::Interface(id) => {
//             //     let mut ns = String::new();
//             //     let iface = &resolve.interfaces[*id];
//             //     let pkg = &resolve.packages[iface.package.unwrap()];
//             //     ns.push_str(&pkg.name.namespace.to_snake_case());
//             //     ns.push_str("::");
//             //     ns.push_str(&pkg.name.name.to_snake_case());
//             //     ns.push_str("::");
//             //     ns.push_str(&iface.name.as_ref().unwrap().to_snake_case());
//             //     ns
//             // }
//         },

//         TypeOwner::World(owner) => resolve.worlds[*owner].name.to_snake_case(),

//         // Namespace everything else under the "default" world being
//         // generated to avoid putting too much into the root namespace in C.
//         TypeOwner::None => world.to_snake_case(),
//     }
// }

impl WorldGenerator for CppHost {
    fn preprocess(&mut self, resolve: &Resolve, world: WorldId) {
        let name = &resolve.worlds[world].name;
        self.world = name.to_string();
        self.sizes.fill(resolve);
    }

    fn import_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        id: InterfaceId,
        _files: &mut Files,
    ) {
        // println!("import interface(,{name:?},{id:?})");
        // uwriteln!(self.src.h_defs, "namespace {name} {{");
        // for i in resolve.interfaces[id].types.iter() {
        //     let typename = self.type_reference(resolve, i.1);
        //     uwriteln!(self.src.h_defs, "  using {} = {typename};", i.0);
        //     // println!("{} {:?}", i.0, i.1);
        //     // println!("{:?}", resolve.types.get(*i.1));
        // }
        let prev = self.interface_names.insert(id, name.clone());
        assert!(prev.is_none());
        let mut gen = self.interface(resolve, true, Identifier::Interface(id, name));
        gen.interface = Some(id);
        // uwriteln!(gen.src.h_defs, "namespace {name} {{");
        // uwriteln!(gen.src.h_helpers, "namespace {name} {{");
        if gen.gen.interfaces_with_types_printed.insert(id) {
            gen.types(id);
        }

        // FIXME: resources should go after types?
        Self::declare_resources(&mut gen, resolve, name, id);

        let (enter, leave) = match name {
            WorldKey::Name(n) => (n.to_snake_case(), "}".to_string()),
            WorldKey::Interface(i) => {
                fn namespace(name: &str, enter: &mut String, leave: &mut String) {
                    enter.push_str(&format!("namespace {} {{ ", name.to_snake_case()));
                    leave.push('}')
                }
                let mut enter = String::new();
                let mut leave = String::new();
                let iface = &resolve.interfaces[*i];
                if let Some(pkg) = iface.package.map(|id| &resolve.packages[id]) {
                    namespace(&pkg.name.namespace, &mut enter, &mut leave);
                    namespace(&pkg.name.name, &mut enter, &mut leave);
                }
                if let Some(name) = resolve.interfaces[*i].name.as_ref() {
                    namespace(name, &mut enter, &mut leave);
                }
                (enter, leave)
            }
        };
        uwriteln!(gen.src.h_fns, "{enter}");
        for (_i, (_name, func)) in resolve.interfaces[id].functions.iter().enumerate() {
            if matches!(&func.kind, FunctionKind::Freestanding) {
                gen.import(Some(name), func);
            }
        }
        uwriteln!(gen.src.h_fns, "{leave}");

        // uwriteln!(gen.src.h_defs, "}}");
        // uwriteln!(gen.src.h_helpers, "}}");
        gen.gen.src.append(&gen.src);
    }

    fn import_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        _files: &mut Files,
    ) {
        let name = &resolve.worlds[world].name;
        let mut gen = self.interface(resolve, true, Identifier::World(world));

        for (i, (_name, func)) in funcs.iter().enumerate() {
            if i == 0 {
                uwriteln!(gen.src.h_fns, "\n// Imported Functions from `{name}`");
            }
            gen.import(None, func);
        }

        gen.gen.src.append(&gen.src);
    }

    fn export_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        id: InterfaceId,
        _files: &mut Files,
    ) -> std::result::Result<(), anyhow::Error> {
        self.interface_names.insert(id, name.clone());
        let mut gen = self.interface(resolve, false, Identifier::Interface(id, name));
        gen.interface = Some(id);
        if gen.gen.interfaces_with_types_printed.insert(id) {
            gen.types(id);
        }

        for (i, (_name, func)) in resolve.interfaces[id].functions.iter().enumerate() {
            if i == 0 {
                let name = resolve.name_world_key(name);
                uwriteln!(gen.src.h_fns, "\n// Exported Functions from `{name}`");
            }
            gen.export(func, Some(name));
        }

        gen.gen.src.append(&gen.src);
        Ok(())
    }

    fn export_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        _files: &mut Files,
    ) -> std::result::Result<(), anyhow::Error> {
        let name = &resolve.worlds[world].name;
        let mut gen = self.interface(resolve, false, Identifier::World(world));

        for (i, (_name, func)) in funcs.iter().enumerate() {
            if i == 0 {
                uwriteln!(gen.src.h_fns, "\n// Exported Functions from `{name}`");
            }
            gen.export(func, None);
        }

        gen.gen.src.append(&gen.src);
        Ok(())
    }

    // fn export_types(
    //     &mut self,
    //     resolve: &Resolve,
    //     _world: WorldId,
    //     types: &[(&str, TypeId)],
    //     _files: &mut Files,
    // ) {
    //     let mut gen = self.interface(resolve, false);
    //     for (name, id) in types {
    //         gen.define_type(name, *id);
    //     }
    //     gen.gen.src.append(&gen.src);
    // }

    fn finish(&mut self, resolve: &Resolve, id: WorldId, files: &mut Files) {
        self.finish_types(resolve);
        let world = &resolve.worlds[id];
        // self.include("<stdlib.h>");
        let snake = world.name.to_snake_case();

        self.print_intrinsics();

        let version = env!("CARGO_PKG_VERSION");
        let mut h_str = wit_bindgen_core::Source::default();
        uwriteln!(
            h_str,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );

        if self.opts.guest_header {
            uwrite!(
                h_str,
                "#ifndef __CPP_GUEST_BINDINGS_{0}_H
            #define __CPP_GUEST_BINDINGS_{0}_H\n",
                world.name.to_shouty_snake_case(),
            );
        } else {
            uwrite!(
                h_str,
                "#ifndef __CPP_HOST_BINDINGS_{0}_H
            #define __CPP_HOST_BINDINGS_{0}_H\n",
                world.name.to_shouty_snake_case(),
            );
        }

        self.include("<cstdint>");
        if self.dependencies.needs_string {
            self.include("<string>");
        }
        if self.dependencies.needs_string_view {
            self.include("<string_view>");
        }
        if self.dependencies.needs_vector {
            self.include("<vector>");
        }
        if self.dependencies.needs_expected {
            self.include("<expected>");
        }
        if self.dependencies.needs_optional {
            self.include("<optional>");
        }
        if self.dependencies.needs_cstring {
            self.include("<cstring>");
        }

        for include in self.includes.iter() {
            uwriteln!(h_str, "#include {include}");
        }

        let mut c_str = wit_bindgen_core::Source::default();
        uwriteln!(
            c_str,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );
        uwriteln!(c_str, "#include \"{snake}_cpp_host.h\"");
        if !self.opts.short_cut {
            uwriteln!(
                c_str,
                "#include <wasm_export.h> // wasm-micro-runtime header"
            );

            if c_str.len() > 0 {
                c_str.push_str("\n");
            }
            if self.dependencies.needs_guest_alloc {
                uwriteln!(
                    c_str,
                    "int32_t guest_alloc(wasm_exec_env_t exec_env, uint32_t size);"
                );
            }

            if self.dependencies.needs_resources {
                let (_, ns_enter, ns_leave, _) =
                    self.surround_with_namespace2(resolve, TypeOwner::World(id));
                if !self.opts.guest_header {
                    uwriteln!(
                        h_str,
                        "{ns_enter} class {RESOURCE_BASE_CLASS_NAME} {{
                        public:
                        int32_t id;
                        virtual ~{RESOURCE_BASE_CLASS_NAME}();
                        {RESOURCE_BASE_CLASS_NAME}();
                        static {RESOURCE_BASE_CLASS_NAME}* lookup_resource(int32_t id);
                      }}; 
                      template <typename T> struct {OWNED_CLASS_NAME} {{
                        T *ptr;
                      }}; {ns_leave}"
                    );
                } else {
                    uwriteln!(
                        h_str,
                        "{ns_enter} class {RESOURCE_BASE_CLASS_NAME} {{
                        protected:
                        int32_t handle;
                        public:
                        {RESOURCE_BASE_CLASS_NAME}() : handle() {{}}
                        void set_handle(int32_t h) {{ handle=h; }}
                        int32_t get_handle() const {{ return handle; }}
                      }}; {ns_leave}"
                    );
                }
            }
        }

        c_str.push_str(&self.src.c_defs);
        c_str.push_str(&self.src.c_fns);

        if self.src.h_defs.len() > 0 {
            h_str.push_str(&self.src.h_defs);
        }

        h_str.push_str(&self.src.h_fns);

        uwriteln!(c_str, "\n// Component Adapters");

        c_str.push_str(&self.src.c_adapters);

        if !self.opts.short_cut && !self.opts.guest_header {
            uwriteln!(
                h_str,
                "extern \"C\" void register_{}();",
                world.name.to_snake_case()
            );
            uwriteln!(c_str, "void register_{}() {{", world.name.to_snake_case());
            for i in self.host_functions.iter() {
                uwriteln!(
                    c_str,
                    "  static NativeSymbol {}_funs[] = {{",
                    i.0.replace(":", "_").to_snake_case()
                );
                for f in i.1.iter() {
                    uwriteln!(
                        c_str,
                        "    {{ \"{}\", (void*){}, \"{}\", nullptr }},",
                        f.wasm_name,
                        f.host_name,
                        f.wamr_signature
                    );
                }
                uwriteln!(c_str, "  }};");
            }
            for i in self.host_functions.iter() {
                uwriteln!(c_str, "  wasm_runtime_register_natives(\"{}\", {1}_funs, sizeof({1}_funs)/sizeof(NativeSymbol));", i.0, i.0.replace(":", "_").to_snake_case());
            }
            uwriteln!(c_str, "}}");
        }

        uwriteln!(
            h_str,
            "
            #endif"
        );

        if self.opts.guest_header {
            files.push(&format!("{snake}_cpp.h"), h_str.as_bytes());
        } else {
            files.push(&format!("{snake}_host.cpp"), c_str.as_bytes());
            files.push(&format!("{snake}_cpp_host.h"), h_str.as_bytes());
        }
    }

    fn import_types(
        &mut self,
        _resolve: &Resolve,
        _world: WorldId,
        _types: &[(&str, TypeId)],
        _files: &mut Files,
    ) {
        todo!()
    }
}

impl CppHost {
    fn interface<'a>(
        &'a mut self,
        resolve: &'a Resolve,
        in_import: bool,
        //        world: WorldId,
        identifier: Identifier<'a>,
    ) -> InterfaceGenerator<'a> {
        InterfaceGenerator {
            src: Source::default(),
            gen: self,
            resolve,
            interface: None,
            identifier,
            in_import,
            wasm_import_module: None,
        }
    }

    fn include(&mut self, s: &str) {
        self.includes.push(s.to_string());
    }

    fn finish_types(&mut self, resolve: &Resolve) {
        for (id, _) in resolve.types.iter() {
            if let Some(ty) = self.typedefs.get(&id) {
                self.src.h_defs(ty);
            }
        }
    }

    // fn print_anonymous_type(&mut self, _resolve: &Resolve, _ty: TypeId) {
    //     todo!();
    // }

    // returns namespace prefix, enter, and leave code
    fn surround_with_namespace(
        &mut self,
        resolve: &Resolve,
        id: TypeId,
    ) -> (String, String, String, TypeOwner) {
        self.surround_with_namespace2(resolve, resolve.types[id].owner.clone())
    }

    fn surround_with_namespace2(
        &mut self,
        resolve: &Resolve,
        owner: TypeOwner,
        //        id: TypeId,
    ) -> (String, String, String, TypeOwner) {
        fn namespace(name: &str, ns: &mut String, enter: &mut String, leave: &mut String) {
            ns.push_str(&format!("{name}::"));
            enter.push_str(&format!("namespace {name} {{ "));
            leave.push('}');
        }

        //        let owner: TypeOwner = resolve.types[id].owner.clone();
        let mut ns = String::new();
        let mut enter = String::new();
        let mut leave = String::new();

        match owner {
            TypeOwner::World(w) => namespace(
                &resolve.worlds[w].name.to_snake_case(),
                &mut ns,
                &mut enter,
                &mut leave,
            ),
            TypeOwner::Interface(i) => {
                let iface = &resolve.interfaces[i];
                let pkg = &resolve.packages[iface.package.unwrap()];
                namespace(
                    &pkg.name.namespace.to_snake_case(),
                    &mut ns,
                    &mut enter,
                    &mut leave,
                );
                namespace(
                    &pkg.name.name.to_snake_case(),
                    &mut ns,
                    &mut enter,
                    &mut leave,
                );
                if let Some(name) = &iface.name {
                    namespace(&name.to_snake_case(), &mut ns, &mut enter, &mut leave);
                }
            }
            TypeOwner::None => (),
        }
        (ns, enter, leave, owner)
    }

    fn owner_namespace(&mut self, resolve: &Resolve, id: TypeId) -> String {
        let ty = &resolve.types[id];
        match ty.owner {
            // If this type belongs to an interface, then use that interface's
            // original name if it's listed in the source. Otherwise if it's an
            // "anonymous" interface as part of a world use the name of the
            // import/export in the world which would have been stored in
            // `interface_names`.
            TypeOwner::Interface(owner) => match &self.interface_names[&owner] {
                WorldKey::Name(name) => name.to_snake_case(),
                WorldKey::Interface(id) => {
                    let mut ns = String::new();
                    let iface = &resolve.interfaces[*id];
                    let pkg = &resolve.packages[iface.package.unwrap()];
                    ns.push_str(&pkg.name.namespace.to_snake_case());
                    ns.push_str("::");
                    ns.push_str(&pkg.name.name.to_snake_case());
                    ns.push_str("::");
                    ns.push_str(&iface.name.as_ref().unwrap().to_snake_case());
                    ns
                }
            },

            TypeOwner::World(owner) => resolve.worlds[owner].name.to_snake_case(),

            // Namespace everything else under the "default" world being
            // generated to avoid putting too much into the root namespace in C.
            TypeOwner::None => self.world.to_snake_case(),
        }
    }

    // fn type_name(
    //     &mut self,
    //     resolve: &Resolve,
    //     ty: &Type,
    //     parent: Option<&str>,
    //     ctx: Context,
    // ) -> String {
    //     let mut name = String::new();
    //     self.push_type_name(resolve, ty, &mut name, parent, ctx);
    //     name
    // }

    fn push_type_name(
        &mut self,
        resolve: &Resolve,
        ty: &Type,
        dst: &mut String,
        parent: Option<&str>,
        ctx: Context,
    ) {
        match ty {
            Type::Bool => dst.push_str("bool"),
            Type::Char => dst.push_str("uint32_t"), // TODO: better type?
            Type::U8 => dst.push_str("uint8_t"),
            Type::S8 => dst.push_str("int8_t"),
            Type::U16 => dst.push_str("uint16_t"),
            Type::S16 => dst.push_str("int16_t"),
            Type::U32 => dst.push_str("uint32_t"),
            Type::S32 => dst.push_str("int32_t"),
            Type::U64 => dst.push_str("uint64_t"),
            Type::S64 => dst.push_str("int64_t"),
            Type::Float32 => dst.push_str("float"),
            Type::Float64 => dst.push_str("double"),
            Type::String => {
                if matches!(ctx, Context::Argument) {
                    dst.push_str("std::string_view");
                    self.dependencies.needs_string_view = true;
                } else {
                    dst.push_str("std::string");
                    self.dependencies.needs_string = true;
                }
            }
            Type::Id(id) => {
                let ty = &resolve.types[*id];
                let ns = self.owner_namespace(resolve, *id);
                match &ty.name {
                    Some(name) => {
                        if ns != parent.unwrap_or_default() {
                            dst.push_str("::");
                            dst.push_str(&ns);
                            dst.push_str("::");
                        }
                        dst.push_str(&name.to_pascal_case());
                    }
                    None => match &ty.kind {
                        TypeDefKind::Type(t) => self.push_type_name(resolve, t, dst, parent, ctx),
                        _ => {
                            // self.public_anonymous_types.insert(*id);
                            // self.private_anonymous_types.remove(id);
                            // dst.push_str("::");
                            // dst.push_str(&ns);
                            // dst.push_str("::");
                            push_ty_name(
                                resolve,
                                &Type::Id(*id),
                                dst,
                                &self.world,
                                parent,
                                &mut self.dependencies,
                                &self.opts,
                            );
                        }
                    },
                }
            }
        }
    }

    fn declare_resources(
        gen: &mut InterfaceGenerator<'_>,
        resolve: &Resolve,
        name: &WorldKey,
        id: InterfaceId,
    ) {
        let funcs = resolve.interfaces[id].functions.values();
        let by_resource = group_by_resource(funcs);
        let mut world_name = gen.gen.world.to_snake_case();
        world_name.push_str("::");

        for (resource, funcs) in by_resource {
            if let Some(resource) = resource {
                gen.gen.dependencies.needs_resources = true;
                let (_, ns_enter, ns_leave, _) = gen.gen.surround_with_namespace(resolve, resource);
                gen.src.h_defs(&ns_enter);
                let pascal = resolve.types[resource]
                    .name
                    .as_deref()
                    .unwrap()
                    .to_pascal_case();
                let derive = format!(" : {world_name}{RESOURCE_BASE_CLASS_NAME}");
                gen.src.h_defs(&format!("class {pascal}{derive} {{\n"));
                if gen.gen.opts.guest_header {
                    //gen.src.h_defs("  int32_t handle;\nbool owned;\n");
                } else {
                    gen.src.h_defs(
                        "  // private implementation data\n  struct pImpl;\n  pImpl * p_impl;\n",
                    );
                }
                gen.src.h_defs(&format!("public:\n"));
                // destructor
                gen.src.h_defs(&format!("~{pascal}();\n"));
                for func in funcs {
                    gen.import(Some(name), func);
                }
                if gen.gen.opts.guest_header {
                    // consuming constructor from handle (bindings)
                    gen.src.h_defs(&format!(
                        "{pascal}({world_name}{RESOURCE_BASE_CLASS_NAME}&&);\n"
                    ));
                }
                gen.src.h_defs(&format!("}}; {ns_leave}\n"));
            }
        }
    }
}

fn group_by_resource<'a>(
    funcs: impl Iterator<Item = &'a Function>,
) -> BTreeMap<Option<TypeId>, Vec<&'a Function>> {
    let mut by_resource = BTreeMap::<_, Vec<_>>::new();
    for func in funcs {
        match &func.kind {
            FunctionKind::Freestanding => by_resource.entry(None).or_default().push(func),
            FunctionKind::Method(ty) | FunctionKind::Static(ty) | FunctionKind::Constructor(ty) => {
                by_resource.entry(Some(*ty)).or_default().push(func);
            }
        }
    }
    by_resource
}

#[derive(Clone)]
enum Identifier<'a> {
    World(WorldId),
    Interface(InterfaceId, &'a WorldKey),
}

struct InterfaceGenerator<'a> {
    src: Source,
    identifier: Identifier<'a>,
    // in_import: bool,
    gen: &'a mut CppHost,
    resolve: &'a Resolve,
    interface: Option<InterfaceId>,
    in_import: bool,
    wasm_import_module: Option<&'a str>,
}

impl CppHost {
    fn print_intrinsics(&mut self) {}
}

impl Return {
    fn return_single(&mut self, resolve: &Resolve, ty: &Type, orig_ty: &Type) {
        let id = match ty {
            Type::Id(id) => *id,
            // Type::String => {
            //     self.retptrs.push(*orig_ty);
            //     return;
            // }
            _ => {
                self.scalar = Some(Scalar::Type(*orig_ty));
                return;
            }
        };
        match &resolve.types[id].kind {
            TypeDefKind::Type(t) => return self.return_single(resolve, t, orig_ty),

            // Flags are returned as their bare values, and enums are scalars
            TypeDefKind::Flags(_) | TypeDefKind::Enum(_) => {
                self.scalar = Some(Scalar::Type(*orig_ty));
                return;
            }

            // Unpack optional returns where a boolean discriminant is
            // returned and then the actual type returned is returned
            // through a return pointer.
            TypeDefKind::Option(_ty) => {
                self.scalar = Some(Scalar::Type(*orig_ty));
                return;
            }

            // Unpack a result as a boolean return type, with two
            // return pointers for ok and err values
            TypeDefKind::Result(_r) => {
                self.scalar = Some(Scalar::Type(*orig_ty));
                return;
            }

            // These types are always returned indirectly.
            TypeDefKind::Tuple(_)
            | TypeDefKind::Record(_)
            | TypeDefKind::List(_)
            | TypeDefKind::Variant(_) => {
                self.scalar = Some(Scalar::Type(*orig_ty));
                return;
            }

            TypeDefKind::Future(_) => todo!("return_single for future"),
            TypeDefKind::Stream(_) => todo!("return_single for stream"),
            TypeDefKind::Unknown => unreachable!(),
            TypeDefKind::Resource => todo!(),
            TypeDefKind::Handle(_h) => {
                self.scalar = Some(Scalar::Type(Type::U32));
                return;
            }
        }

        // self.retptrs.push(*orig_ty);
    }
}

impl<'a> wit_bindgen_core::InterfaceGenerator<'a> for InterfaceGenerator<'a> {
    fn resolve(&self) -> &'a Resolve {
        self.resolve
    }

    fn type_record(&mut self, id: TypeId, name: &str, record: &Record, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        // let ns = self.gen.owner_namespace(self.resolve, id).to_snake_case();
        let (ns, ns_enter, ns_leave, _owner) = self.gen.surround_with_namespace(self.resolve, id);
        self.src.h_defs(&format!("{ns_enter}\n"));
        self.docs(docs, SourceType::HDefs);
        let pascal = name.to_pascal_case();
        self.src.h_defs(&format!("struct {pascal} {{\n"));
        for field in record.fields.iter() {
            self.docs(&field.docs, SourceType::HDefs);
            self.print_ty(SourceType::HDefs, &field.ty, Some(&ns), Context::InStruct);
            self.src.h_defs(" ");
            self.src.h_defs(&to_c_ident(&field.name));
            self.src.h_defs(";\n");
        }
        self.src.h_defs("};\n");
        self.src.h_defs(&format!("{ns_leave}\n"));
        self.finish_ty(id, prev);
    }

    fn type_tuple(&mut self, id: TypeId, name: &str, tuple: &Tuple, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        let (ns, ns_enter, ns_leave, _owner) = self.gen.surround_with_namespace(self.resolve, id);
        self.src.h_defs(&format!("{ns_enter}\n"));
        self.docs(docs, SourceType::HDefs);
        let pascal = name.to_pascal_case();
        self.src.h_defs(&format!("struct {pascal} {{\n"));
        for (i, ty) in tuple.types.iter().enumerate() {
            //self.docs(&field.docs, SourceType::HDefs);
            self.print_ty(SourceType::HDefs, ty, Some(&ns), Context::InStruct);
            self.src.h_defs(&format!(" f{i};\n"));
        }
        self.src.h_defs("};\n");
        self.src.h_defs(&format!("{ns_leave}\n"));
        self.finish_ty(id, prev);
    }

    fn type_flags(&mut self, _id: TypeId, _name: &str, _flags: &Flags, _docs: &Docs) {
        todo!();
        // let prev = mem::take(&mut self.src.h_defs);
        // self.src.h_defs("\n");
        // self.docs(docs, SourceType::HDefs);
        // self.src.h_defs("typedef ");
        // let repr = flags_repr(flags);
        // self.src.h_defs(int_repr(repr));
        // self.src.h_defs(" ");
        // self.print_typedef_target(id, name);

        // if flags.flags.len() > 0 {
        //     self.src.h_defs("\n");
        // }
        // let ns = self
        //     .gen
        //     .owner_namespace(self.resolve, id)
        //     .to_shouty_snake_case();
        // for (i, flag) in flags.flags.iter().enumerate() {
        //     self.docs(&flag.docs, SourceType::HDefs);
        //     uwriteln!(
        //         self.src.h_defs,
        //         "#define {ns}_{}_{} (1 << {i})",
        //         name.to_shouty_snake_case(),
        //         flag.name.to_shouty_snake_case(),
        //     );
        // }

        // self.finish_ty(id, prev);
    }

    fn type_variant(&mut self, _id: TypeId, _name: &str, _variant: &Variant, _docs: &Docs) {
        todo!();
        // let prev = mem::take(&mut self.src.h_defs);
        // self.src.h_defs("\n");
        // self.docs(docs, SourceType::HDefs);
        // self.src.h_defs("typedef struct {\n");
        // self.src.h_defs(int_repr(variant.tag()));
        // self.src.h_defs(" tag;\n");
        // self.src.h_defs("union {\n");
        // for case in variant.cases.iter() {
        //     if let Some(ty) = get_nonempty_type(self.resolve, case.ty.as_ref()) {
        //         self.print_ty(SourceType::HDefs, ty, None, Context::InStruct);
        //         self.src.h_defs(" ");
        //         self.src.h_defs(&to_c_ident(&case.name));
        //         self.src.h_defs(";\n");
        //     }
        // }
        // self.src.h_defs("} val;\n");
        // self.src.h_defs("} ");
        // self.print_typedef_target(id, name);

        // if variant.cases.len() > 0 {
        //     self.src.h_defs("\n");
        // }
        // let ns = self
        //     .gen
        //     .owner_namespace(self.resolve, id)
        //     .to_shouty_snake_case();
        // for (i, case) in variant.cases.iter().enumerate() {
        //     self.docs(&case.docs, SourceType::HDefs);
        //     uwriteln!(
        //         self.src.h_defs,
        //         "#define {ns}_{}_{} {i}",
        //         name.to_shouty_snake_case(),
        //         case.name.to_shouty_snake_case(),
        //     );
        // }

        // self.finish_ty(id, prev);
    }

    fn type_option(&mut self, _id: TypeId, _name: &str, _payload: &Type, _docs: &Docs) {
        todo!();
        // let prev = mem::take(&mut self.src.h_defs);
        // self.src.h_defs("\n");
        // self.docs(docs, SourceType::HDefs);
        // self.src.h_defs("typedef struct {\n");
        // self.src.h_defs("bool is_some;\n");
        // if !is_empty_type(self.resolve, payload) {
        //     self.print_ty(SourceType::HDefs, payload, None, Context::InStruct);
        //     self.src.h_defs(" val;\n");
        // }
        // self.src.h_defs("} ");
        // self.print_typedef_target(id, name);

        // self.finish_ty(id, prev);
    }

    fn type_result(&mut self, _id: TypeId, _name: &str, _result: &Result_, _docs: &Docs) {
        todo!();
        // let prev = mem::take(&mut self.src.h_defs);
        // self.src.h_defs("\n");
        // self.docs(docs, SourceType::HDefs);
        // self.src.h_defs("typedef struct {\n");
        // self.src.h_defs("bool is_err;\n");
        // self.src.h_defs("union {\n");
        // if let Some(ok) = get_nonempty_type(self.resolve, result.ok.as_ref()) {
        //     self.print_ty(SourceType::HDefs, ok, None, Context::InStruct);
        //     self.src.h_defs(" ok;\n");
        // }
        // if let Some(err) = get_nonempty_type(self.resolve, result.err.as_ref()) {
        //     self.print_ty(SourceType::HDefs, err, None, Context::InStruct);
        //     self.src.h_defs(" err;\n");
        // }
        // self.src.h_defs("} val;\n");
        // self.src.h_defs("} ");
        // self.print_typedef_target(id, name);

        // self.finish_ty(id, prev);
    }

    fn type_enum(&mut self, id: TypeId, name: &str, enum_: &Enum, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        let (_ns, ns_enter, ns_leave, _owner) = self.gen.surround_with_namespace(self.resolve, id);
        let pascal = name.to_pascal_case();
        self.src.h_defs(&format!("{ns_enter}\n"));
        self.docs(docs, SourceType::HDefs);
        let int_t = int_repr(enum_.tag());
        uwriteln!(self.src.h_defs, "enum class {pascal} : {int_t} {{");
        for (i, case) in enum_.cases.iter().enumerate() {
            self.docs(&case.docs, SourceType::HDefs);
            uwriteln!(self.src.h_defs, " k{} = {i},", case.name.to_pascal_case(),);
        }
        uwriteln!(self.src.h_defs, "}};\n{ns_leave}");

        self.finish_ty(id, prev);
    }

    fn type_alias(&mut self, id: TypeId, name: &str, ty: &Type, _docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        let (ns, ns_enter, ns_leave, _owner) = self.gen.surround_with_namespace(self.resolve, id);
        let pascal = name.to_pascal_case();
        // let tns = self.docs(docs, SourceType::HDefs);
        self.src.h_defs(&format!("{ns_enter} using {pascal} = "));
        self.print_ty(SourceType::HDefs, ty, Some(&ns), Context::InStruct);
        self.src.h_defs(&format!("; {ns_leave}\n"));
        self.gen.names.insert(&format!("{ns}{pascal}")).unwrap();
        self.finish_ty(id, prev);
    }

    fn type_list(&mut self, _id: TypeId, _name: &str, _ty: &Type, _docs: &Docs) {
        todo!();
        // let prev = mem::take(&mut self.src.h_defs);
        // self.src.h_defs("\n");
        // self.docs(docs, SourceType::HDefs);
        // self.src.h_defs("typedef struct {\n");
        // self.print_ty(SourceType::HDefs, ty, None, Context::InStruct);
        // self.src.h_defs(" *ptr;\n");
        // self.src.h_defs("size_t len;\n");
        // self.src.h_defs("} ");
        // self.print_typedef_target(id, name);
        // self.finish_ty(id, prev);
    }

    fn type_builtin(&mut self, _id: TypeId, name: &str, ty: &Type, docs: &Docs) {
        drop((_id, name, ty, docs));
    }

    fn type_resource(&mut self, id: TypeId, name: &str, docs: &Docs) {
        if false {
            self.src.h_defs("\n");
            let mut world_name = self.gen.world.clone();
            world_name.push_str("::");
            let (ns, ns_enter, ns_leave, _owner) =
                self.gen.surround_with_namespace(self.resolve, id);
            let pascal = name.to_pascal_case();
            self.src.h_defs(&ns_enter);
            self.docs(docs, SourceType::HDefs);
            self.gen.dependencies.needs_resources = true;
            self.src.h_defs(&format!(
                "class {pascal} : {world_name}{RESOURCE_BASE_CLASS_NAME} {{\n"
            ));

            for intf in self.resolve.interfaces.iter() {
                for (_name, func) in intf.1.functions.iter() {
                    if !matches!(&func.kind, FunctionKind::Freestanding) {
                        //gen.import(Some(name), func);
                    }
                }
            }
            // self.print_ty(SourceType::HDefs, id, None, Context::InStruct);
            self.src.h_defs(&format!("}}; {ns_leave}\n"));
            self.gen.names.insert(&format!("{ns}{pascal}")).unwrap();
            // self.finish_ty(id, prev);
        }
    }
}

#[derive(Debug)]
struct WamrSig {
    wasm: WasmSignature,
    c_args: Vec<(String, String)>, // name, type
    wamr_types: String,
    c_result: String,
    wamr_result: String,
}

impl Default for WamrSig {
    fn default() -> Self {
        Self {
            wasm: WasmSignature {
                params: Default::default(),
                results: Default::default(),
                indirect_params: Default::default(),
                retptr: Default::default(),
            },
            c_args: Default::default(),
            wamr_types: Default::default(),
            c_result: "void".into(),
            wamr_result: Default::default(),
        }
    }
}

impl InterfaceGenerator<'_> {
    fn cpp_func_name(&self, interface_name: Option<&WorldKey>, func: &Function) -> String {
        let ns = match interface_name {
            Some(WorldKey::Name(k)) => k.to_snake_case(),
            Some(WorldKey::Interface(id)) => {
                let iface = &self.resolve.interfaces[*id];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                let mut name = pkg.name.namespace.to_snake_case();
                name.push_str("::");
                name.push_str(&pkg.name.name.to_snake_case());
                name.push_str("::");
                name.push_str(&iface.name.as_ref().unwrap().to_snake_case());
                name
            }
            None => self.gen.world.to_snake_case(),
        };
        // let ns = interface_name.unwrap_or(&self.gen.world);
        format!("{}::{}", ns, func.name.to_pascal_case())
    }

    fn c_func_name(&self, interface_name: Option<&WorldKey>, func: &Function) -> String {
        let ns = match interface_name {
            Some(WorldKey::Name(k)) => k.to_snake_case(),
            Some(WorldKey::Interface(id)) => {
                let iface = &self.resolve.interfaces[*id];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                let mut name = pkg.name.namespace.to_snake_case();
                name.push_str("_");
                name.push_str(&pkg.name.name.to_snake_case());
                name.push_str("_");
                name.push_str(&iface.name.as_ref().unwrap().to_snake_case());
                name
            }
            None => self.gen.world.to_snake_case(),
        };
        // let ns = interface_name.unwrap_or(&self.gen.world);
        format!("{}_{}", ns, func.name.to_snake_case())
    }

    fn push_wamr(
        &mut self,
        variant: AbiVariant,
        name: &str,
        ty: &Type,
        result: &mut Vec<(String, String)>,
        params_str: &mut String,
    ) {
        match ty {
            Type::Bool
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::S8
            | Type::S16
            | Type::S32
            | Type::Char => {
                let mut res = String::default();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut res,
                    "",
                    None,
                    &mut self.gen.dependencies,
                    &self.gen.opts,
                );
                result.push((name.to_snake_case(), res));
                params_str.push('i');
            }
            Type::U64 | Type::S64 => {
                let mut res = String::default();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut res,
                    "",
                    None,
                    &mut self.gen.dependencies,
                    &self.gen.opts,
                );
                result.push((name.to_snake_case(), res));
                params_str.push('I');
            }
            Type::Float32 => {
                let mut res = String::default();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut res,
                    "",
                    None,
                    &mut self.gen.dependencies,
                    &self.gen.opts,
                );
                result.push((name.to_snake_case(), res));
                params_str.push('f');
            }
            Type::Float64 => {
                let mut res = String::default();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut res,
                    "",
                    None,
                    &mut self.gen.dependencies,
                    &self.gen.opts,
                );
                result.push((name.to_snake_case(), res));
                params_str.push('F');
            }
            Type::String => {
                result.push((
                    format!("{}_ptr", name.to_snake_case()),
                    "char const*".to_string(),
                ));
                result.push((
                    format!("{}_len", name.to_snake_case()),
                    "uint32_t".to_string(),
                ));
                params_str.push_str("$~");
            }
            Type::Id(id) => match &self.resolve.types[*id].kind {
                TypeDefKind::Type(t) => self.push_wamr(variant, name, t, result, params_str),
                TypeDefKind::Record(_r) => {
                    result.push((name.to_snake_case(), "int32_t".to_string()));
                    params_str.push_str("?");
                }
                TypeDefKind::Flags(_) => todo!(),
                TypeDefKind::Tuple(_) => todo!(),
                TypeDefKind::Variant(_) => todo!(),
                TypeDefKind::Enum(_e) => {
                    result.push((name.to_snake_case(), "int32_t".to_string()));
                    params_str.push_str("i");
                }
                TypeDefKind::Option(_) => todo!(),
                TypeDefKind::Result(_) => todo!(),
                TypeDefKind::List(_t) => {
                    let mut res = String::default();
                    push_ty_name(
                        self.resolve,
                        ty,
                        &mut res,
                        "",
                        None,
                        &mut self.gen.dependencies,
                        &self.gen.opts,
                    );
                    res.push('*');
                    result.push((format!("{}_ptr", name.to_snake_case()), res));
                    result.push((
                        format!("{}_len", name.to_snake_case()),
                        "uint32_t".to_string(),
                    ));
                    params_str.push_str("*~");
                }
                TypeDefKind::Future(_) => todo!(),
                TypeDefKind::Stream(_) => todo!(),
                TypeDefKind::Unknown => todo!(),
                TypeDefKind::Resource => todo!(),
                TypeDefKind::Handle(_h) => {
                    result.push((name.to_snake_case(), "int32_t".to_string()));
                    params_str.push('i');
                }
            },
        }
    }

    const RESULT_NAME: &'static str = "result_out";

    fn wamr_add_result(&mut self, sig: &mut WamrSig, name: &str, ty: &Type) {
        let mut dummy = Includes::default();
        match ty {
            Type::Bool
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::S8
            | Type::S16
            | Type::S32
            | Type::Char => {
                sig.c_result.clear();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut sig.c_result,
                    "",
                    None,
                    &mut dummy,
                    &self.gen.opts,
                );
                sig.wamr_result = "i".into();
            }
            Type::S64 | Type::U64 => {
                sig.c_result.clear();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut sig.c_result,
                    "",
                    None,
                    &mut dummy,
                    &self.gen.opts,
                );
                sig.wamr_result = "I".into();
            }
            Type::Float32 => {
                sig.c_result.clear();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut sig.c_result,
                    "",
                    None,
                    &mut dummy,
                    &self.gen.opts,
                );
                sig.wamr_result = "f".into();
            }
            Type::Float64 => {
                sig.c_result.clear();
                push_ty_name(
                    self.resolve,
                    ty,
                    &mut sig.c_result,
                    "",
                    None,
                    &mut dummy,
                    &self.gen.opts,
                );
                sig.wamr_result = "F".into();
            }
            Type::String => {
                sig.c_args
                    .push((Self::RESULT_NAME.into(), "uint32_t *".into()));
                sig.wamr_types.push('*');
            }
            Type::Id(id) => match &self.resolve.types[*id].kind {
                TypeDefKind::Record(_) => todo!(),
                TypeDefKind::Flags(_) => todo!(),
                TypeDefKind::Tuple(_) => todo!(),
                TypeDefKind::Variant(_) => todo!(),
                TypeDefKind::Enum(_e) => {
                    sig.c_args
                        .push((Self::RESULT_NAME.into(), "/* enum */".into()));
                    sig.wamr_types.push('*');
                }
                TypeDefKind::Option(_o) => {
                    sig.c_args
                        .push((Self::RESULT_NAME.into(), "/* option */".into()));
                    sig.wamr_types.push('*');
                }
                TypeDefKind::Result(_) => {
                    sig.c_args
                        .push((Self::RESULT_NAME.into(), "uint8_t *".into()));
                    sig.wamr_types.push('*');
                }
                TypeDefKind::List(_) => {
                    sig.c_args
                        .push((Self::RESULT_NAME.into(), "uint32_t *".into()));
                    sig.wamr_types.push('*');
                }
                TypeDefKind::Future(_) => todo!(),
                TypeDefKind::Stream(_) => todo!(),
                TypeDefKind::Type(ty) => self.wamr_add_result(sig, name, ty),
                TypeDefKind::Unknown => todo!(),
                TypeDefKind::Resource => todo!(),
                TypeDefKind::Handle(_h) => {
                    sig.c_result.clear();
                    push_ty_name(
                        self.resolve,
                        ty,
                        &mut sig.c_result,
                        "",
                        None,
                        &mut dummy,
                        &self.gen.opts,
                    );
                    sig.wamr_result = "i".into();
                }
            },
        }
    }

    fn wamr_signature(&mut self, variant: AbiVariant, func: &Function) -> WamrSig {
        let wasm_sig = self.resolve.wasm_signature(variant, func);
        let mut result = WamrSig::default();
        result.wasm = wasm_sig;
        // let mut params = Vec::new();
        // let mut params_str = String::new();
        // let mut result = String::from("void");
        // let mut result_str = String::new();
        for (name, param) in func.params.iter() {
            self.push_wamr(
                variant,
                name,
                param,
                &mut result.c_args,
                &mut result.wamr_types,
            );
        }
        match &func.results {
            Results::Named(p) => {
                if !p.is_empty() {
                    dbg!(p);
                    todo!()
                }
            }
            Results::Anon(e) => self.wamr_add_result(&mut result, "result", e),
        }
        result
    }

    fn add_parameter(&mut self, result: &mut String, nm: &str, ty: &Type) {
        match ty {
            Type::Bool
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::S8
            | Type::S16
            | Type::S32
            | Type::S64
            | Type::Float32
            | Type::Float64
            | Type::Char => {
                if !result.is_empty() {
                    result.push_str(", ");
                };
                result.push_str(&nm.to_snake_case());
            }
            Type::String => {
                self.src.c_adapters(&format!(
                    "  std::string_view {0} = std::string_view{{{0}_ptr, {0}_len}};\n",
                    nm.to_snake_case()
                ));
                if !result.is_empty() {
                    result.push_str(", ");
                };
                result.push_str(&nm.to_snake_case());
            }
            Type::Id(id) => {
                let ty = &self.resolve.types[*id];
                match &ty.kind {
                    TypeDefKind::Record(_r) => result.push_str("/* TODO record 1246 */"),
                    TypeDefKind::Flags(_) => todo!(),
                    TypeDefKind::Tuple(_) => todo!(),
                    TypeDefKind::Variant(_) => todo!(),
                    TypeDefKind::Enum(_e) => {
                        let ns = self.gen.owner_namespace(self.resolve, *id);
                        let typename = format!(
                            "{}::{}",
                            ns,
                            ty.name.clone().unwrap_or_default().to_pascal_case()
                        );
                        if !result.is_empty() {
                            result.push_str(", ");
                        };
                        result.push_str(&format!(
                            "static_cast<{}>({})",
                            typename,
                            nm.to_snake_case()
                        ));
                    }
                    TypeDefKind::Option(_) => todo!(),
                    TypeDefKind::Result(_) => todo!(),
                    TypeDefKind::List(_l) => result.push_str("/* TODO list 1245 */"),
                    TypeDefKind::Future(_) => todo!(),
                    TypeDefKind::Stream(_) => todo!(),
                    TypeDefKind::Type(t) => {
                        self.add_parameter(result, nm, t);
                    }
                    TypeDefKind::Unknown => todo!(),
                    TypeDefKind::Resource => todo!(),
                    TypeDefKind::Handle(_h) => result.push_str("/* TODO handle 1265 */"),
                }
            }
        }
    }

    fn create_parameters(&mut self, func: &Function) -> String {
        let mut result = String::new();
        for (nm, ty) in func.params.iter() {
            self.add_parameter(&mut result, nm, ty);
        }
        result
    }

    // TODO: This information has to be calculated somewhere inside the normal c bindgen as well
    // fn alignment(ty: &Type, resolve: &Resolve, sizes: &SizeAlign) -> usize {
    //     sizes.align(ty)
    // match ty {
    //     Type::Bool | Type::S8 | Type::U8 => 1,
    //     Type::S16 | Type::U16 => 2,
    //     Type::U32 | Type::S32 | Type::Char | Type::Float32 => 4,
    //     Type::U64 | Type::S64 | Type::Float64 => 8,
    //     Type::String => 4,
    //     Type::Id(id) => {
    //         let td = &resolve.types[*id];
    //         match &td.kind {
    //             TypeDefKind::Record(r) => r
    //                 .fields
    //                 .iter()
    //                 .map(|f| Self::alignment(&f.ty, resolve))
    //                 .max()
    //                 .unwrap_or(1),
    //             TypeDefKind::Flags(_) => todo!(),
    //             TypeDefKind::Tuple(_) => todo!(),
    //             TypeDefKind::Variant(_) => todo!(),
    //             TypeDefKind::Enum(_) => todo!(),
    //             TypeDefKind::Option(_) => todo!(),
    //             TypeDefKind::Result(_) => todo!(),
    //             TypeDefKind::Union(_) => todo!(),
    //             TypeDefKind::List(_) => todo!(),
    //             TypeDefKind::Future(_) => todo!(),
    //             TypeDefKind::Stream(_) => todo!(),
    //             TypeDefKind::Type(ty) => Self::alignment(&ty, resolve),
    //             TypeDefKind::Unknown => todo!(),
    //         }
    //     }
    // }
    // }

    fn deep_copy_out2(&mut self, ty: &Type, src_var: &str, target_ptr: &str) {
        match ty {
            Type::Bool
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::S8
            | Type::S16
            | Type::S32
            | Type::S64
            | Type::Float32
            | Type::Float64
            | Type::Char => {
                let mut typename = String::new();
                self.gen.push_type_name(
                    self.resolve,
                    ty,
                    &mut typename,
                    None,
                    Context::ReturnValue,
                );
                self.src
                    .c_adapters(&format!("    *(({typename}*){target_ptr}) = {src_var};\n"));
            }
            Type::String => todo!(),
            Type::Id(id) => {
                let td = &self.resolve.types[*id];
                self.deep_copy_out(td, src_var, target_ptr);
            }
        }
    }

    fn deep_copy_out(&mut self, ty: &TypeDef, src_var: &str, target_ptr: &str) {
        //self.src.c_adapters(&format!("  // TODO {ty:?}\n"));
        match &ty.kind {
            TypeDefKind::Record(r) => {
                let mut offset = 0;
                self.src.c_adapters("  {\n");
                self.src.c_adapters(&format!(
                    "    uint8_t *record_ptr = (uint8_t*)({target_ptr});\n"
                ));
                // self.src.c_adapters(&format!("  // TODO record {ty:?}\n"));
                for f in r.fields.iter() {
                    let align = self.gen.sizes.align(&f.ty);
                    if align > 1 {
                        offset = (offset + align - 1) & !(align - 1);
                    }
                    self.deep_copy_out2(
                        &f.ty,
                        &format!("{src_var}.{}", f.name),
                        &format!("(record_ptr+{offset})"),
                    );
                    let size = self.gen.sizes.size(&f.ty);
                    offset += size;
                    // self.src
                    //     .c_adapters(&format!("    (){}\n", &f.ty, f.name));
                }
                self.src.c_adapters("  }\n");
            }
            TypeDefKind::Flags(_) => todo!(),
            TypeDefKind::Tuple(_) => todo!(),
            TypeDefKind::Variant(_) => todo!(),
            TypeDefKind::Enum(_e) => {
                self.src.c_adapters("  /* TODO enum */\n");
            }
            TypeDefKind::Option(_) => {
                self.src.c_adapters("  /* TODO option */\n");
            }
            TypeDefKind::Result(r) => {
                self.src
                    .c_adapters(&format!("  if ({src_var}.has_value()) {{\n"));
                self.src.c_adapters(&format!("    {target_ptr}[0]=0;\n"));
                if let Some(ok) = &r.ok {
                    let alignment = self.gen.sizes.align(ok);
                    self.deep_copy_out2(
                        ok,
                        &format!("({src_var}.value())"),
                        &format!("({target_ptr}+{alignment})"),
                    );
                }
                self.src.c_adapters("  } else {\n");
                self.src.c_adapters(&format!("    {target_ptr}[0]=1;\n"));
                if let Some(err) = &r.err {
                    let alignment = self.gen.sizes.align(err);
                    self.deep_copy_out2(
                        err,
                        &format!("({src_var}.error())"),
                        &format!("({target_ptr}+{alignment})"),
                    );
                }
                self.src.c_adapters("  }\n");
            }
            TypeDefKind::List(l) => {
                self.gen.dependencies.needs_guest_alloc = true;
                self.src.c_adapters("  {\n");
                self.src
                    .c_adapters(&format!("    uint32_t list_elems = {src_var}.size();\n"));
                let element_size = self.gen.sizes.size(l);
                let cast = if !self.gen.opts.short_cut {
                    self.src.c_adapters(&format!(
                    "    uint32_t list_addr = guest_alloc(exec_env, list_elems*{element_size});\n"
                ));
                    self.src.c_adapters(
                        "    // TODO make this work for non-trivially-copyable types\n",
                    );
                    self.src.c_adapters(&format!("    memcpy(wasm_runtime_addr_app_to_native(wasm_runtime_get_module_inst(exec_env), list_addr), {src_var}.data(), list_elems*{element_size});\n"));
                    ""
                } else {
                    self.src.c_adapters(&format!(
                        "    void* list_addr = malloc(list_elems*{element_size});\n"
                    ));
                    self.src.c_adapters(&format!(
                        "    memcpy(list_addr, {src_var}.data(), list_elems*{element_size});\n"
                    ));
                    "(long long)"
                };
                // self.src.c_adapters("    // TODO make this work for non-32bit types\n");
                // self.src.c_adapters(&format!(
                //     "    uint32_t* elements = (uint32_t*)wasm_runtime_addr_app_to_native(wasm_runtime_get_module_inst(exec_env), list_addr);\n"
                // ));
                // self.src
                //     .c_adapters("    for (uint32_t i=0; i<list_elems; ++i) {\n");
                // self.deep_copy_out2(l, &format!("({src_var}[i])"), &format!("(elements+i)"));
                // self.src.c_adapters("    }\n");
                self.src
                    .c_adapters(&format!("    {target_ptr}[0]={cast}list_addr;\n"));
                self.src
                    .c_adapters(&format!("    {target_ptr}[1]=list_elems;\n"));
                self.src.c_adapters("  }\n");
            }
            TypeDefKind::Future(_) => todo!(),
            TypeDefKind::Stream(_) => todo!(),
            TypeDefKind::Type(td) => {
                self.deep_copy_out2(td, src_var, target_ptr);
            }
            TypeDefKind::Unknown => todo!(),
            TypeDefKind::Resource => todo!(),
            TypeDefKind::Handle(_h) => self.src.c_adapters("   /* TODO handle 1460 */\n"),
        }
    }

    fn store_or_return_result2(&mut self, result: &str, ty: &Type) {
        match ty {
            Type::Bool
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::S8
            | Type::S16
            | Type::S32
            | Type::U64
            | Type::S64
            | Type::Float32
            | Type::Float64
            | Type::Char => {
                self.src.c_adapters(&format!("  return {result};\n"));
            }
            Type::String => {
                self.gen.dependencies.needs_cstring = true;
                self.gen.dependencies.needs_guest_alloc = true;
                let cast = if !self.gen.opts.short_cut {
                    self.src.c_adapters(&format!(
                        "  int32_t {result}_addr = guest_alloc(exec_env, {result}.size());\n"
                    ));
                    self.src.c_adapters(&format!("  memcpy(wasm_runtime_addr_app_to_native(wasm_runtime_get_module_inst(exec_env), {result}_addr), {result}.data(), {result}.size());\n"));
                    ""
                } else {
                    self.src.c_adapters(&format!(
                        "  void* {result}_addr = malloc({result}.size());\n"
                    ));
                    self.src.c_adapters(&format!(
                        "  memcpy({result}_addr, {result}.data(), {result}.size());\n"
                    ));
                    "(long long)"
                };
                self.src
                    .c_adapters(&format!("  result_out[0] = {cast}{result}_addr;\n"));
                self.src
                    .c_adapters(&format!("  result_out[1] = {result}.size();\n"));
            }
            Type::Id(id) => {
                let ty = &self.resolve.types[*id];
                match &ty.kind {
                    // TypeDefKind::Result(r) => {
                    //     self.deep_copy_out(ty, result, Self::RESULT_NAME);
                    // }
                    TypeDefKind::Type(td) => self.store_or_return_result2(result, &td),
                    _ => {
                        self.deep_copy_out(ty, result, Self::RESULT_NAME);
                        // dbg!(&ty);
                        //                        self.deep_copy_out(ty, result, &format!("{result}_out"));
                    }
                }
            }
        }
    }

    fn store_or_return_result(&mut self, result: &str, kind: &Results) {
        match kind {
            Results::Named(_) => todo!(),
            Results::Anon(ty) => self.store_or_return_result2(result, ty),
        }
    }

    fn import(&mut self, interface_name: Option<&WorldKey>, func: &Function) {
        let wamr_sig = self.wamr_signature(AbiVariant::GuestImport, func);
        // dbg!(&wamr_sig);
        self.docs(&func.docs, SourceType::HFns);
        let _sig = self.resolve.wasm_signature(AbiVariant::GuestImport, func);
        // dbg!(sig);

        // self.src.c_fns("\n");

        let name = self.c_func_name(interface_name, func);
        let host_name = self.gen.names.tmp(&format!("host_{name}"));
        let _c_sig = self.print_sig(interface_name, func);
        // dbg!(c_sig);
        if self.gen.opts.short_cut {
            self.src
                .c_adapters(&format!("__attribute__((export_name(\"{}\")))\n", name));
            self.src.c_adapters("extern \"C\"\n");
            self.src
                .c_adapters(&format!("{} {host_name}(", wamr_sig.c_result));
        } else {
            self.src.c_adapters(&format!(
                "static {} {host_name}(wasm_exec_env_t exec_env",
                wamr_sig.c_result
            ));
        }
        for (n, (nm, ty)) in wamr_sig.c_args.iter().enumerate() {
            if !self.gen.opts.short_cut || n > 0 {
                self.src.c_adapters(", ");
            }
            self.src.c_adapters(ty);
            self.src.c_adapters(" ");
            self.src.c_adapters(nm);
        }
        self.src.c_adapters(") {\n");
        let param = self.create_parameters(func);
        let func_ns = self.cpp_func_name(interface_name, func);
        if func.results.len() > 0 {
            self.src
                .c_adapters(&format!("  auto call_result = ::{func_ns}({param});\n"));
            self.store_or_return_result("call_result", &func.results);
        } else {
            self.src.c_adapters(&format!("  ::{func_ns}({param});\n"));
        }
        self.src.c_adapters("}\n");

        let module_name = match interface_name {
            Some(name) => self.resolve.name_world_key(name),
            None => "$root".to_string(),
        };
        let remember = HostFunction {
            wasm_name: func.name.clone(),
            wamr_signature: format!("({}){}", wamr_sig.wamr_types, wamr_sig.wamr_result),
            host_name,
        };
        self.gen
            .host_functions
            .entry(module_name)
            .and_modify(|v| v.push(remember.clone()))
            .or_insert(vec![remember]);
        //self.src.c_adapters(&format!(" [ \"{}\", (void*){host_name}, \"({}){}\", nullptr ],\n", func.name, wamr_sig.wamr_types, wamr_sig.wamr_result));

        // this doesn't seem to work as needed
        // let resolver = self.resolve;
        // let mut interface = self.gen.interface(resolver, true);
        // let mut bindgen = FunctionBindgen::new(&mut interface, func);
        // resolver.call(
        //     AbiVariant::GuestImport,
        //     abi::LiftLower::LowerArgsLiftResults,
        //     func,
        //     &mut bindgen,
        // );
    }

    fn export(&mut self, _func: &Function, _interface_name: Option<&WorldKey>) {
        todo!("function exports");
    }

    fn print_sig(&mut self, interface_name: Option<&WorldKey>, func: &Function) -> CSig {
        let name = self.cpp_func_name(interface_name, func);
        self.gen.names.insert(&name).expect("duplicate symbols");

        let result_rets = false;
        let result_rets_has_ok_type = false;

        let ret = self.classify_ret(func);
        let parent_namespace: Option<String> = interface_name.map(|i| match i {
            WorldKey::Name(n) => n.to_snake_case(),
            WorldKey::Interface(i) => self.resolve.interfaces[*i]
                .name
                .clone()
                .unwrap_or("".into())
                .to_snake_case(),
        });
        let parent_namespace_ref: Option<&str> = parent_namespace.as_ref().map(|i| i.as_str());
        let target = match func.kind {
            FunctionKind::Freestanding => self.src.h_fns.as_mut_string(),
            // member functions need to go to definitions
            _ => self.src.h_defs.as_mut_string(),
        };
        if matches!(func.kind, FunctionKind::Static(_)) {
            target.push_str("static ");
        }
        if !matches!(func.kind, FunctionKind::Constructor(_)) {
            match &ret.scalar {
                None | Some(Scalar::Void) => target.push_str("void"),
                // Some(Scalar::OptionBool(_id)) => target.push_str("bool"),
                // Some(Scalar::ResultBool(ok, _err)) => {
                //     result_rets = true;
                //     result_rets_has_ok_type = ok.is_some();
                //     target.push_str("bool");
                // }
                Some(Scalar::Type(ty)) => self.gen.push_type_name(
                    self.resolve,
                    ty,
                    target,
                    parent_namespace_ref,
                    Context::ReturnValue,
                ),
            }
            target.push_str(" ");
        }
        let (func_name, skip_args) = match func.kind {
            FunctionKind::Freestanding => (func.name.to_pascal_case(), 0),
            FunctionKind::Method(_) => (func.item_name().to_pascal_case(), 1),
            FunctionKind::Static(_) => (func.item_name().to_pascal_case(), 0),
            FunctionKind::Constructor(_) => (
                func.name[func.name.find(']').unwrap() + 1..].to_pascal_case(),
                0,
            ),
        };
        target.push_str(&func_name);
        target.push_str("(");
        let mut params = Vec::new();
        for (i, (name, ty)) in func.params.iter().enumerate() {
            if i > skip_args {
                target.push_str(", ");
            }
            let pointer = is_arg_by_pointer(self.resolve, ty);
            let (print_ty, print_name) = (ty, to_c_ident(name));
            if i >= skip_args {
                self.gen.push_type_name(
                    self.resolve,
                    print_ty,
                    target,
                    parent_namespace_ref,
                    Context::Argument,
                );
                target.push_str(" ");
                if pointer {
                    target.push_str("const& ");
                }
                target.push_str(&print_name);
            }
            params.push((true && pointer, to_c_ident(name)));
        }
        let mut retptrs = Vec::new();
        let single_ret = ret.retptrs.len() == 1;
        for (i, ty) in ret.retptrs.iter().enumerate() {
            if i > 0 || func.params.len() > 0 {
                target.push_str(", ");
            }
            self.gen.push_type_name(
                self.resolve,
                ty,
                target,
                parent_namespace_ref,
                Context::ReturnValue,
            );
            target.push_str(" *");
            let name: String = if result_rets {
                assert!(i <= 1);
                if i == 0 && result_rets_has_ok_type {
                    "ret".into()
                } else {
                    "err".into()
                }
            } else if single_ret {
                "ret".into()
            } else {
                format!("ret{}", i)
            };
            target.push_str(&name);
            retptrs.push(name);
        }
        if func.params.len() == 0 && ret.retptrs.len() == 0 {
            target.push_str("void");
        }
        target.push_str(")");

        target.push_str(";\n");

        CSig {
            name,
            params,
            ret,
            retptrs,
        }
    }

    fn classify_ret(&mut self, func: &Function) -> Return {
        let mut ret = Return::default();
        match func.results.len() {
            0 => ret.scalar = Some(Scalar::Void),
            1 => {
                let ty = func.results.iter_types().next().unwrap();
                ret.return_single(self.resolve, ty, ty);
            }
            _ => todo!(),
            // {
            //     ret.retptrs.extend(func.results.iter_types().cloned());
            // }
        }
        return ret;
    }

    // fn print_typedef_target(&mut self, id: TypeId, name: &str) {
    //     let ns = self.gen.owner_namespace(self.resolve, id).to_snake_case();
    //     let snake = name.to_snake_case();
    //     self.src.h_defs(&ns);
    //     self.src.h_defs("_");
    //     self.src.h_defs(&snake);
    //     self.src.h_defs("_t;\n");
    //     self.gen.names.insert(&format!("{ns}_{snake}_t")).unwrap();
    // }

    fn print_ty(&mut self, stype: SourceType, ty: &Type, parent: Option<&str>, ctx: Context) {
        self.gen.push_type_name(
            self.resolve,
            ty,
            self.src.src(stype).as_mut_string(),
            parent,
            ctx,
        );
    }

    fn docs(&mut self, docs: &Docs, stype: SourceType) {
        let docs = match &docs.contents {
            Some(docs) => docs,
            None => return,
        };
        let src = self.src.src(stype);
        for line in docs.trim().lines() {
            src.push_str("// ");
            src.push_str(line);
            src.push_str("\n");
        }
    }

    // this feels like some sort of re-ordering ???
    fn finish_ty(&mut self, id: TypeId, orig_h_defs: wit_bindgen_core::Source) {
        let prev = self
            .gen
            .typedefs
            .insert(id, mem::replace(&mut self.src.h_defs, orig_h_defs));
        assert!(prev.is_none());
    }

    fn make_export_name(input: &str) -> String {
        input
            .chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' => c,
                _ => '_',
            })
            .collect()
    }

    fn export_name2(module_name: &str, name: &str) -> String {
        let mut res = Self::make_export_name(module_name);
        res.push('_');
        res.push_str(&Self::make_export_name(name));
        res
    }

    fn declare_import2(
        module_name: &str,
        name: &str,
        args: &str,
        result: &str,
    ) -> (String, String) {
        let extern_name = Self::export_name2(module_name, name);
        let import = format!("extern __attribute__((import_module(\"{module_name}\")))\n __attribute__((import_name(\"{name}\")))\n {result} {extern_name}({args});\n");
        (extern_name, import)
    }
}

impl<'a> RustGenerator<'a> for InterfaceGenerator<'a> {
    fn resolve(&self) -> &'a Resolve {
        self.resolve
    }

    fn ownership(&self) -> Ownership {
        Ownership::Owning
    }

    fn path_to_interface(&self, interface: InterfaceId) -> Option<String> {
        let mut path = String::new();
        if let Identifier::Interface(cur, name) = self.identifier {
            if cur == interface {
                return None;
            }
            // if !self.in_import {
            //path.push_str("super::");
            // }
            match name {
                WorldKey::Name(_) => {
                    //path.push_str("super::");
                }
                WorldKey::Interface(_) => {
                    //path.push_str("super::super::super::");
                }
            }
        }
        let name = &self.gen.interface_names[&interface];
        match name {
            WorldKey::Name(n) => path.push_str(n),
            WorldKey::Interface(_i) => todo!(),
        }
        Some(path)
    }

    fn is_exported_resource(&self, ty: TypeId) -> bool {
        matches!(
            self.gen
                .resources
                .get(&dealias(self.resolve, ty))
                .map(|info| info.direction),
            Some(Direction::Export)
        )
    }

    // fn add_own(&mut self, resource: TypeId, handle: TypeId) {
    //     self.gen
    //         .resources
    //         .entry(dealias(self.resolve, resource))
    //         .or_default()
    //         .own = Some(handle);
    // }

    fn push_str(&mut self, s: &str) {
        self.src.push_str(s);
    }

    fn info(&self, ty: TypeId) -> TypeInfo {
        self.gen.types.get(ty)
    }

    fn types_mut(&mut self) -> &mut Types {
        &mut self.gen.types
    }

    fn print_borrowed_slice(
        &mut self,
        mutbl: bool,
        ty: &Type,
        lifetime: &'static str,
        mode: TypeMode,
    ) {
        self.print_rust_slice(mutbl, ty, lifetime, mode);
    }

    fn print_borrowed_str(&mut self, _lifetime: &'static str) {
        self.push_str("&");
        // if self.gen.opts.raw_strings {
        //     self.push_str("[u8]");
        // } else {
        self.push_str("str");
        // }
    }

    fn push_vec_name(&mut self) {
        self.push_str("std::vector");
    }

    fn push_string_name(&mut self) {
        self.push_str("std::string");
    }

    fn mark_resource_owned(&mut self, resource: TypeId) {
        self.gen
            .resources
            .entry(dealias(self.resolve, resource))
            .or_default()
            .owned = true;
    }

    fn print_signature(
        &mut self,
        func: &Function,
        param_mode: TypeMode,
        sig: &FnSig,
    ) -> Vec<String> {
        if !matches!(func.kind, FunctionKind::Constructor(_)) {
            self.print_results(&func.results, TypeMode::Owned);
            self.push_str(" ");
        }
        let params = self.print_docs_and_params(func, param_mode, &sig);
        params
    }

    fn print_docs_and_params(
        &mut self,
        func: &Function,
        _param_mode: TypeMode,
        sig: &FnSig,
    ) -> Vec<String> {
        // self.rustdoc(&func.docs);
        // self.rustdoc_params(&func.params, "Parameters");
        // TODO: re-add this when docs are back
        // self.rustdoc_params(&func.results, "Return");

        let object = match &func.kind {
            FunctionKind::Freestanding => None,
            FunctionKind::Method(i) => Some(i),
            FunctionKind::Static(i) => Some(i),
            FunctionKind::Constructor(i) => Some(i),
        }
        .map(|i| {
            self.resolve.types[*i]
                .name
                .as_ref()
                .unwrap()
                .to_pascal_case()
        })
        .unwrap_or_default();
        let func_name = if sig.use_item_name {
            if let FunctionKind::Constructor(_i) = &func.kind {
                format!("{object}::{object}")
            } else {
                format!("{object}::{}", func.item_name().to_pascal_case())
            }
        } else {
            func.name.to_pascal_case()
        };
        self.push_str(&func_name);
        if let Some(generics) = &sig.generics {
            self.push_str(generics);
        }
        self.push_str("(");
        if let Some(arg) = &sig.self_arg {
            self.push_str(arg);
            self.push_str(",");
        }
        let mut params = Vec::new();
        for (i, (name, param)) in func.params.iter().enumerate() {
            params.push(to_rust_ident(name));
            if i == 0 && sig.self_is_first_param {
                // params.push("self".to_string());
                continue;
            }
            if i == 0 && name == "self" {
                continue;
            }
            let name = to_rust_ident(name);
            self.print_ty(SourceType::HDefs, param, None, Context::Argument);
            self.push_str(" ");
            self.push_str(&name);
            if i + 1 != func.params.len() {
                self.push_str(",");
            }
        }
        self.push_str(")");
        params
    }

    fn print_tyid(&mut self, id: TypeId, mode: TypeMode) {
        let info = self.info(id);
        let lt = self.lifetime_for(&info, mode);
        let ty = &RustGenerator::resolve(self).types[id];
        if ty.name.is_some() {
            // If this type has a list internally, no lifetime is being printed,
            // but we're in a borrowed mode, then that means we're in a borrowed
            // context and don't want ownership of the type but we're using an
            // owned type definition. Inject a `&` in front to indicate that, at
            // the API level, ownership isn't required.
            if info.has_list && lt.is_none() {
                if let TypeMode::AllBorrowed(lt) | TypeMode::LeafBorrowed(lt) = mode {
                    self.push_str("&");
                    if lt != "'_" {
                        self.push_str(lt);
                        self.push_str(" ");
                    }
                }
            }
            let name = self.type_path(id, lt.is_none());
            self.push_str(&name);

            // If the type recursively owns data and it's a
            // variant/record/list, then we need to place the
            // lifetime parameter on the type as well.
            if info.has_list && needs_generics(RustGenerator::resolve(self), &ty.kind) {
                self.print_generics(lt);
            }

            return;

            fn needs_generics(resolve: &Resolve, ty: &TypeDefKind) -> bool {
                match ty {
                    TypeDefKind::Variant(_)
                    | TypeDefKind::Record(_)
                    | TypeDefKind::Option(_)
                    | TypeDefKind::Result(_)
                    | TypeDefKind::Future(_)
                    | TypeDefKind::Stream(_)
                    | TypeDefKind::List(_)
                    | TypeDefKind::Flags(_)
                    | TypeDefKind::Enum(_)
                    | TypeDefKind::Tuple(_) => true,
                    TypeDefKind::Type(Type::Id(t)) => {
                        needs_generics(resolve, &resolve.types[*t].kind)
                    }
                    TypeDefKind::Type(Type::String) => true,
                    TypeDefKind::Resource | TypeDefKind::Handle(_) | TypeDefKind::Type(_) => false,
                    TypeDefKind::Unknown => unreachable!(),
                }
            }
        }

        match &ty.kind {
            TypeDefKind::List(t) => self.print_list(t, mode),

            TypeDefKind::Option(t) => {
                self.push_str("std::option<");
                self.print_ty(SourceType::HDefs, t, None, Context::Argument);
                self.push_str(">");
            }

            TypeDefKind::Result(r) => {
                self.push_str("std::expected<");
                self.print_optional_ty(r.ok.as_ref(), mode);
                self.push_str(",");
                self.print_optional_ty(r.err.as_ref(), mode);
                self.push_str(">");
            }

            TypeDefKind::Variant(_) => panic!("unsupported anonymous variant"),

            // Tuple-like records are mapped directly to Rust tuples of
            // types. Note the trailing comma after each member to
            // appropriately handle 1-tuples.
            TypeDefKind::Tuple(t) => {
                self.push_str("(");
                for ty in t.types.iter() {
                    self.print_ty(SourceType::HDefs, ty, None, Context::Argument);
                    self.push_str(",");
                }
                self.push_str(")");
            }
            TypeDefKind::Resource => {
                panic!("unsupported anonymous type reference: resource")
            }
            TypeDefKind::Record(_) => {
                panic!("unsupported anonymous type reference: record")
            }
            TypeDefKind::Flags(_) => {
                panic!("unsupported anonymous type reference: flags")
            }
            TypeDefKind::Enum(_) => {
                panic!("unsupported anonymous type reference: enum")
            }
            TypeDefKind::Future(ty) => {
                self.push_str("Future<");
                self.print_optional_ty(ty.as_ref(), mode);
                self.push_str(">");
            }
            TypeDefKind::Stream(stream) => {
                self.push_str("Stream<");
                self.print_optional_ty(stream.element.as_ref(), mode);
                self.push_str(",");
                self.print_optional_ty(stream.end.as_ref(), mode);
                self.push_str(">");
            }

            TypeDefKind::Handle(Handle::Own(ty)) => {
                self.mark_resource_owned(*ty);
                self.print_ty(SourceType::HDefs, &Type::Id(*ty), None, Context::Argument);
            }

            TypeDefKind::Handle(Handle::Borrow(ty)) => {
                self.push_str("&");
                self.print_ty(SourceType::HDefs, &Type::Id(*ty), None, Context::Argument);
            }

            TypeDefKind::Type(t) => self.print_ty(SourceType::HDefs, t, None, Context::Argument),

            // TypeDefKind::Resource => {
            //     todo!("implement resources")
            // }
            TypeDefKind::Unknown => unreachable!(),
        }
    }

    fn print_ty(&mut self, ty: &Type, mode: TypeMode) {
        match ty {
            Type::Id(t) => self.print_tyid(*t, mode),
            Type::Bool => self.push_str("bool"),
            Type::U8 => self.push_str("uint8_t"),
            Type::U16 => self.push_str("uint16_t"),
            Type::U32 => self.push_str("uint32_t"),
            Type::U64 => self.push_str("uint64_t"),
            Type::S8 => self.push_str("int8_t"),
            Type::S16 => self.push_str("int16_t"),
            Type::S32 => self.push_str("int32_t"),
            Type::S64 => self.push_str("int64_t"),
            Type::Float32 => self.push_str("float"),
            Type::Float64 => self.push_str("double"),
            Type::Char => self.push_str("int32_t"),
            Type::String => match mode {
                TypeMode::AllBorrowed(_lt) | TypeMode::LeafBorrowed(_lt) => {
                    self.push_str("std::string_view");
                }
                TypeMode::Owned => {
                    self.push_str("std::string");
                }
                TypeMode::HandlesBorrowed(_) => todo!(),
            },
        }
    }

    fn print_optional_ty(&mut self, ty: Option<&Type>, _mode: TypeMode) {
        match ty {
            Some(ty) => self.print_ty(SourceType::HDefs, ty, None, Context::Argument),
            None => self.push_str("void"),
        }
    }

    fn print_results(&mut self, results: &Results, mode: TypeMode) {
        match results.len() {
            0 | 1 => self.print_optional_ty(results.iter_types().next(), mode),
            _ => todo!(),
        }
    }

    fn wasm_type(&mut self, ty: WasmType) {
        self.push_str(wasm_type(ty));
    }

    fn print_list(&mut self, ty: &Type, mode: TypeMode) {
        let next_mode = if matches!(self.ownership(), Ownership::Owning) {
            TypeMode::Owned
        } else {
            mode
        };
        match mode {
            TypeMode::AllBorrowed(lt) => {
                self.print_borrowed_slice(false, ty, lt, next_mode);
            }
            TypeMode::LeafBorrowed(lt) => {
                if RustGenerator::resolve(self).all_bits_valid(ty) {
                    self.print_borrowed_slice(false, ty, lt, next_mode);
                } else {
                    self.push_vec_name();
                    self.push_str("<");
                    self.print_ty(SourceType::HDefs, ty, None, Context::Argument);
                    self.push_str(">");
                }
            }
            TypeMode::Owned => {
                self.push_vec_name();
                self.push_str("<");
                self.print_ty(SourceType::HDefs, ty, None, Context::Argument);
                self.push_str(">");
            }
            TypeMode::HandlesBorrowed(_) => todo!(),
        }
    }

    fn print_rust_slice(
        &mut self,
        mutbl: bool,
        ty: &Type,
        _lifetime: &'static str,
        _mode: TypeMode,
    ) {
        self.push_str("std::vector<");
        self.print_ty(SourceType::HDefs, ty, None, Context::Argument);
        self.push_str(">");
        if !mutbl {
            self.push_str(" const ");
        }
        self.push_str("&");
    }
}

struct FunctionBindgen<'a, 'b> {
    gen: &'a mut InterfaceGenerator<'b>,
    func: &'a Function,
    src: Source,
    blocks: Vec<String>,
    block_storage: Vec<(Source, Vec<(String, String)>)>,
    tmp: usize,
    needs_cleanup_list: bool,
    cleanup: Vec<(String, String)>,
    import_return_pointer_area_size: usize,
    import_return_pointer_area_align: usize,
    params: Vec<String>,
    trait_name: Option<&'b str>,
    //    size: &'a SizeAlign,
}

impl<'a, 'b> FunctionBindgen<'a, 'b> {
    //     fn new(gen: &'a mut InterfaceGenerator<'b>, func: &'a Function) -> Self {
    //         Self { gen, func }
    //     }

    fn emit_cleanup(&mut self) {}

    fn wasm_type_cpp(ty: WasmType) -> &'static str {
        wit_bindgen_c::wasm_type(ty)
    }

    fn declare_import(
        &mut self,
        module_name: &str,
        name: &str,
        params: &[WasmType],
        results: &[WasmType],
    ) -> String {
        let mut args = String::default();
        for (n, param) in params.iter().enumerate() {
            args.push_str(Self::wasm_type_cpp(*param));
            if n + 1 != params.len() {
                args.push_str(", ");
            }
        }
        let result = if results.is_empty() {
            "void"
        } else {
            Self::wasm_type_cpp(results[0])
        };
        let (name, code) = InterfaceGenerator::declare_import2(module_name, name, &args, result);
        self.src.push_str(&code);
        name
        // Define the actual function we're calling inline
        //todo!();
        // let mut sig = "(".to_owned();
        // for param in params.iter() {
        //     sig.push_str("_: ");
        //     sig.push_str(wasm_type(*param));
        //     sig.push_str(", ");
        // }
        // sig.push_str(")");
        // assert!(results.len() < 2);
        // for result in results.iter() {
        //     sig.push_str(" -> ");
        //     sig.push_str(wasm_type(*result));
        // }
        // uwriteln!(
        //     self.src,
        //     "
        //         #[cfg(target_arch = \"wasm32\")]
        //         #[link(wasm_import_module = \"{module_name}\")]
        //         extern \"C\" {{
        //             #[link_name = \"{name}\"]
        //             fn wit_import{sig};
        //         }}

        //         #[cfg(not(target_arch = \"wasm32\"))]
        //         fn wit_import{sig} {{ unreachable!() }}
        //     "
        // );
        // "wit_import".to_string()
    }
}

impl RustFunctionGenerator for FunctionBindgen<'_, '_> {
    fn push_str(&mut self, s: &str) {
        self.src.push_str(s);
    }

    fn tmp(&mut self) -> usize {
        let ret = self.tmp;
        self.tmp += 1;
        ret
    }

    fn rust_gen(&self) -> &dyn RustGenerator {
        self.gen
    }

    fn lift_lower(&self) -> LiftLower {
        if self.gen.in_import {
            LiftLower::LowerArgsLiftResults
        } else {
            LiftLower::LiftArgsLowerResults
        }
    }
}

impl Bindgen for FunctionBindgen<'_, '_> {
    type Operand = String;

    fn push_block(&mut self) {
        let prev_src = mem::take(&mut self.src);
        let prev_cleanup = mem::take(&mut self.cleanup);
        self.block_storage.push((prev_src, prev_cleanup));
    }

    fn finish_block(&mut self, operands: &mut Vec<String>) {
        if self.cleanup.len() > 0 {
            self.needs_cleanup_list = true;
            self.push_str("cleanup_list.extend_from_slice(&[");
            for (ptr, layout) in mem::take(&mut self.cleanup) {
                self.push_str("(");
                self.push_str(&ptr);
                self.push_str(", ");
                self.push_str(&layout);
                self.push_str("),");
            }
            self.push_str("]);\n");
        }
        let (prev_src, prev_cleanup) = self.block_storage.pop().unwrap();
        let src = mem::replace(&mut self.src, prev_src);
        self.cleanup = prev_cleanup;
        let expr = match operands.len() {
            0 => "()".to_string(),
            1 => operands[0].clone(),
            _ => format!("({})", operands.join(", ")),
        };
        if src.c_defs.is_empty() {
            self.blocks.push(expr);
        } else if operands.is_empty() {
            self.blocks.push(format!("{{\n{}\n}}", &src.c_defs[..]));
        } else {
            self.blocks
                .push(format!("{{\n{}\n{}\n}}", &src.c_defs[..], expr));
        }
    }

    fn return_pointer(&mut self, size: usize, align: usize) -> String {
        let tmp = self.tmp();

        // Imports get a per-function return area to facilitate using the
        // stack whereas exports use a per-module return area to cut down on
        // stack usage. Note that for imports this also facilitates "adapter
        // modules" for components to not have data segments.
        if true {
            //self.gen.in_import {
            self.import_return_pointer_area_size = self.import_return_pointer_area_size.max(size);
            self.import_return_pointer_area_align =
                self.import_return_pointer_area_align.max(align);
            uwriteln!(self.src.c_defs, "auto ptr{tmp} = (int32_t)&ret_area;");
        } else {
            todo!();
            // self.gen.return_pointer_area_size = self.gen.return_pointer_area_size.max(size);
            // self.gen.return_pointer_area_align = self.gen.return_pointer_area_align.max(align);
            // uwriteln!(self.src, "auto ptr{tmp} = _RET_AREA.0.as_mut_ptr() as i32;");
        }
        format!("ptr{}", tmp)
    }

    fn sizes(&self) -> &SizeAlign {
        &self.gen.gen.sizes
    }

    fn is_list_canonical(&self, resolve: &Resolve, ty: &Type) -> bool {
        resolve.all_bits_valid(ty)
    }

    fn emit(
        &mut self,
        resolve: &Resolve,
        inst: &Instruction<'_>,
        operands: &mut Vec<String>,
        results: &mut Vec<String>,
    ) {
        let mut top_as = |cvt: &str| {
            results.push(format!("({cvt})({})", operands.pop().unwrap()));
        };

        // work around the fact that some functions only push
        fn print_to_result<'a, 'b, 'c, T: FnOnce(&mut InterfaceGenerator<'a>)>(
            slf: &'a mut FunctionBindgen<'b, 'c>,
            resolve: &'a Resolve,
            f: T,
        ) -> String {
            let mut sizes = SizeAlign::default();
            sizes.fill(resolve);
            let mut gen = InterfaceGenerator {
                identifier: slf.gen.identifier.clone(),
                // wasm_import_module: slf.gen.wasm_import_module.clone(),
                src: Source::default(),
                in_import: slf.gen.in_import.clone(),
                gen: slf.gen.gen,
                // sizes,
                resolve,
                interface: slf.gen.interface,
                wasm_import_module: slf.gen.wasm_import_module,
                // return_pointer_area_size: 0,
                // return_pointer_area_align: 0,
            };
            f(&mut gen);
            //gen.print_optional_ty(result.ok.as_ref(), TypeMode::Owned);
            let mut ok_type = String::default();
            std::mem::swap(gen.src.c_defs.as_mut_string(), &mut ok_type);
            ok_type
        }

        match inst {
            Instruction::GetArg { nth } => results.push(self.params[*nth].clone()),
            Instruction::I32Const { val } => results.push(format!("(int32_t){}", val)),
            Instruction::ConstZero { tys } => {
                for ty in tys.iter() {
                    match ty {
                        WasmType::I32 => results.push("(int32_t)0".to_string()),
                        WasmType::I64 => results.push("(int64_t)0".to_string()),
                        WasmType::F32 => results.push("0.0f".to_string()),
                        WasmType::F64 => results.push("0.0".to_string()),
                    }
                }
            }

            Instruction::I64FromU64 | Instruction::I64FromS64 => {
                let s = operands.pop().unwrap();
                results.push(format!("(int64_t)({})", s));
            }
            Instruction::I32FromChar
            | Instruction::I32FromU8
            | Instruction::I32FromS8
            | Instruction::I32FromU16
            | Instruction::I32FromS16
            | Instruction::I32FromU32
            | Instruction::I32FromS32 => {
                let s = operands.pop().unwrap();
                results.push(format!("(int32_t)({})", s));
            }

            Instruction::F32FromFloat32 => {
                let s = operands.pop().unwrap();
                results.push(format!("(float)({})", s));
            }
            Instruction::F64FromFloat64 => {
                let s = operands.pop().unwrap();
                results.push(format!("(double)({})", s));
            }
            Instruction::Float32FromF32
            | Instruction::Float64FromF64
            | Instruction::S32FromI32
            | Instruction::S64FromI64 => {
                results.push(operands.pop().unwrap());
            }
            Instruction::S8FromI32 => top_as("int8_t"),
            Instruction::U8FromI32 => top_as("uint8_t"),
            Instruction::S16FromI32 => top_as("int16_t"),
            Instruction::U16FromI32 => top_as("uint16_t"),
            Instruction::U32FromI32 => top_as("uint32_t"),
            Instruction::U64FromI64 => top_as("uint64_t"),
            Instruction::CharFromI32 => {
                todo!();
                // results.push(format!(
                //     "{{
                //         #[cfg(not(debug_assertions))]
                //         {{ ::core::char::from_u32_unchecked({} as u32) }}
                //         #[cfg(debug_assertions)]
                //         {{ ::core::char::from_u32({} as u32).unwrap() }}
                //     }}",
                //     operands[0], operands[0]
                // ));
            }

            Instruction::Bitcasts { casts } => {
                wit_bindgen_rust_lib::bitcast(casts, operands, results)
            }

            Instruction::I32FromBool => {
                results.push(format!("(int32_t)({})", operands[0]));
            }
            Instruction::BoolFromI32 => {
                results.push(format!("{}!=0", operands[0]));
            }

            Instruction::FlagsLower { flags, .. } => {
                let tmp = self.tmp();
                self.push_str(&format!("auto flags{} = {};\n", tmp, operands[0]));
                for i in 0..flags.repr().count() {
                    results.push(format!("(flags{}.bits() >> {}) as i32", tmp, i * 32));
                }
            }
            Instruction::FlagsLift { flags, ty, .. } => {
                let repr = RustFlagsRepr::new(flags);
                let name = self.gen.type_path(*ty, true);
                let mut result = format!("{name}::empty()");
                for (i, op) in operands.iter().enumerate() {
                    result.push_str(&format!(
                        " | {name}::from_bits_retain((({op} as {repr}) << {}) as _)",
                        i * 32
                    ));
                }
                results.push(result);
            }

            Instruction::HandleLower {
                handle: Handle::Own(_),
                ..
            } => {
                let op = &operands[0];
                results.push(format!("({op}).into_handle()"))
            }

            Instruction::HandleLower {
                handle: Handle::Borrow(_),
                ..
            } => {
                let op = &operands[0];
                if op == "self" {
                    results.push("this->handle".into());
                } else {
                    results.push(format!("({op}).handle"));
                }
            }

            Instruction::HandleLift { handle, .. } => {
                let op = &operands[0];
                let (prefix, resource, _owned) = match handle {
                    Handle::Borrow(resource) => ("&", resource, false),
                    Handle::Own(resource) => ("", resource, true),
                };
                let resource = dealias(resolve, *resource);

                results.push(
                    if let Direction::Export = self.gen.gen.resources[&resource].direction {
                        match handle {
                            Handle::Borrow(_) => {
                                let name = resolve.types[resource]
                                    .name
                                    .as_deref()
                                    .unwrap()
                                    .to_upper_camel_case();
                                format!(
                                    "::core::mem::transmute::<isize, &Rep{name}>\
                                     ({op}.try_into().unwrap())"
                                )
                            }
                            Handle::Own(_) => {
                                let name = self.gen.type_path(resource, true);
                                format!("{name}::from_handle({op})")
                            }
                        }
                    } else {
                        let name = self.gen.type_path(resource, true);
                        let world = &self.gen.gen.world; // .map(|w| &resolve.worlds[w].name).unwrap();
                        format!("{prefix}{name}{{std::move({world}::{RESOURCE_BASE_CLASS_NAME}({op}))}}")
                    },
                );
            }

            Instruction::RecordLower { ty, record, .. } => {
                self.record_lower(*ty, record, &operands[0], results);
            }
            Instruction::RecordLift { ty, record, .. } => {
                let mut result = self.typename_lift(*ty);
                result.push_str("{");
                for (_field, val) in record.fields.iter().zip(operands) {
                    // result.push_str(&to_rust_ident(&field.name));
                    // result.push_str(":");
                    result.push_str(&val);
                    result.push_str(", ");
                }
                result.push_str("}");
                results.push(result);
            }

            Instruction::TupleLower { tuple, .. } => {
                self.tuple_lower(tuple, &operands[0], results);
            }
            Instruction::TupleLift { .. } => {
                self.tuple_lift(operands, results);
            }

            Instruction::VariantPayloadName => results.push("e".to_string()),

            Instruction::VariantLower {
                variant,
                results: result_types,
                ty,
                ..
            } => {
                let blocks = self
                    .blocks
                    .drain(self.blocks.len() - variant.cases.len()..)
                    .collect::<Vec<_>>();
                self.let_results(result_types.len(), results);
                let op0 = &operands[0];
                self.push_str(&format!("match {op0} {{\n"));
                let name = self.typename_lower(*ty);
                for (case, block) in variant.cases.iter().zip(blocks) {
                    let case_name = case.name.to_upper_camel_case();
                    self.push_str(&format!("{name}::{case_name}"));
                    if case.ty.is_some() {
                        self.push_str(&format!("(e) => {block},\n"));
                    } else {
                        self.push_str(&format!(" => {{\n{block}\n}}\n"));
                    }
                }
                self.push_str("};\n");
            }

            Instruction::VariantLift { variant, ty, .. } => {
                let mut result = String::new();
                result.push_str("{");

                let named_enum = variant.cases.iter().all(|c| c.ty.is_none());
                let blocks = self
                    .blocks
                    .drain(self.blocks.len() - variant.cases.len()..)
                    .collect::<Vec<_>>();
                let op0 = &operands[0];

                if named_enum {
                    // In unchecked mode when this type is a named enum then we know we
                    // defined the type so we can transmute directly into it.
                    // result.push_str("#[cfg(not(debug_assertions))]");
                    // result.push_str("{");
                    // result.push_str("::core::mem::transmute::<_, ");
                    // result.push_str(&name.to_upper_camel_case());
                    // result.push_str(">(");
                    // result.push_str(op0);
                    // result.push_str(" as ");
                    // result.push_str(int_repr(variant.tag()));
                    // result.push_str(")");
                    // result.push_str("}");
                }

                // if named_enum {
                //     result.push_str("#[cfg(debug_assertions)]");
                // }
                result.push_str("{");
                result.push_str(&format!("match {op0} {{\n"));
                let name = self.typename_lift(*ty);
                for (i, (case, block)) in variant.cases.iter().zip(blocks).enumerate() {
                    let pat = i.to_string();
                    let block = if case.ty.is_some() {
                        format!("({block})")
                    } else {
                        String::new()
                    };
                    let case = case.name.to_upper_camel_case();
                    // if i == variant.cases.len() - 1 {
                    //     result.push_str("#[cfg(debug_assertions)]");
                    //     result.push_str(&format!("{pat} => {name}::{case}{block},\n"));
                    //     result.push_str("#[cfg(not(debug_assertions))]");
                    //     result.push_str(&format!("_ => {name}::{case}{block},\n"));
                    // } else {
                    result.push_str(&format!("{pat} => {name}::{case}{block},\n"));
                    // }
                }
                // result.push_str("#[cfg(debug_assertions)]");
                // result.push_str("_ => panic!(\"invalid enum discriminant\"),\n");
                result.push_str("}");
                result.push_str("}");

                result.push_str("}");
                results.push(result);
            }

            Instruction::OptionLower {
                results: _result_types,
                ..
            } => {
                todo!();
                // let some = self.blocks.pop().unwrap();
                // let none = self.blocks.pop().unwrap();
                // self.let_results(result_types.len(), results);
                // let operand = &operands[0];
                // self.push_str(&format!(
                //     "match {operand} {{
                //         Some(e) => {some},
                //         None => {{\n{none}\n}},
                //     }};"
                // ));
            }

            Instruction::OptionLift { .. } => {
                let some = self.blocks.pop().unwrap();
                let none = self.blocks.pop().unwrap();
                assert_eq!(none, "()");
                let operand = &operands[0];
                results.push(format!(
                    "{operand}==1 ? std::optional<>(std::move({some})) : std::optional()"
                ));
            }

            Instruction::ResultLower {
                results: _result_types,
                // result,
                ..
            } => {
                todo!();
                // let err = self.blocks.pop().unwrap();
                // let ok = self.blocks.pop().unwrap();
                // self.let_results(result_types.len(), results);
                // let operand = &operands[0];
                // let ok_binding = if result.ok.is_some() { "e" } else { "_" };
                // let err_binding = if result.err.is_some() { "e" } else { "_" };
                // self.push_str(&format!(
                //     "match {operand} {{
                //         Ok({ok_binding}) => {{ {ok} }},
                //         Err({err_binding}) => {{ {err} }},
                //     }};"
                // ));
            }

            Instruction::ResultLift { result, .. } => {
                let mut err = self.blocks.pop().unwrap();
                let mut ok = self.blocks.pop().unwrap();
                if result.ok.is_none() {
                    ok.clear();
                } else {
                    ok = format!("std::move({ok})");
                }
                if result.err.is_none() {
                    err.clear();
                } else {
                    err = format!("std::move({err})");
                }
                let ok_type = print_to_result(self, resolve, |gen| {
                    gen.print_optional_ty(result.ok.as_ref(), TypeMode::Owned)
                });
                let err_type = print_to_result(self, resolve, |gen| {
                    gen.print_optional_ty(result.err.as_ref(), TypeMode::Owned)
                });
                let type_name = format!("std::expected<{ok_type}, {err_type}>",);
                let err_type = "std::unexpected";
                let operand = &operands[0];
                results.push(format!(
                    "{operand}==0 \n? {type_name}({ok}) \n: {type_name}({err_type}({err}))"
                ));
            }

            Instruction::EnumLower { enum_, ty, .. } => {
                let mut result = format!("match {} {{\n", operands[0]);
                let name = self.gen.type_path(*ty, true);
                for (i, case) in enum_.cases.iter().enumerate() {
                    let case = case.name.to_upper_camel_case();
                    result.push_str(&format!("{name}::{case} => {i},\n"));
                }
                result.push_str("}");
                results.push(result);
            }

            Instruction::EnumLift { .. } => {
                todo!();
                // let mut result = String::new();
                // result.push_str("{");

                // // In checked mode do a `match`.
                // result.push_str("#[cfg(debug_assertions)]");
                // result.push_str("{");
                // result.push_str("match ");
                // result.push_str(&operands[0]);
                // result.push_str(" {\n");
                // let name = self.gen.type_path(*ty, true);
                // for (i, case) in enum_.cases.iter().enumerate() {
                //     let case = case.name.to_upper_camel_case();
                //     result.push_str(&format!("{i} => {name}::{case},\n"));
                // }
                // result.push_str("_ => panic!(\"invalid enum discriminant\"),\n");
                // result.push_str("}");
                // result.push_str("}");

                // // In unchecked mode when this type is a named enum then we know we
                // // defined the type so we can transmute directly into it.
                // result.push_str("#[cfg(not(debug_assertions))]");
                // result.push_str("{");
                // result.push_str("::core::mem::transmute::<_, ");
                // result.push_str(&self.gen.type_path(*ty, true));
                // result.push_str(">(");
                // result.push_str(&operands[0]);
                // result.push_str(" as ");
                // result.push_str(int_repr(enum_.tag()));
                // result.push_str(")");
                // result.push_str("}");

                // result.push_str("}");
                // results.push(result);
            }

            Instruction::ListCanonLower { realloc, .. } => {
                let tmp = self.tmp();
                let val = format!("vec{}", tmp);
                let ptr = format!("ptr{}", tmp);
                let len = format!("len{}", tmp);
                //                if realloc.is_none() {
                self.push_str(&format!("auto& {} = {};\n", val, operands[0]));
                // } else {
                //     let op0 = operands.pop().unwrap();
                //     self.push_str(&format!("auto {} = ({}).into_boxed_slice();\n", val, op0));
                // }
                self.push_str(&format!("auto {} = (int32_t)({}.data());\n", ptr, val));
                self.push_str(&format!("auto {} = (int32_t)({}.size());\n", len, val));
                if realloc.is_some() {
                    todo!();
                    // self.push_str(&format!("::core::mem::forget({});\n", val));
                }
                results.push(ptr);
                results.push(len);
            }

            Instruction::ListCanonLift { .. } => {
                let tmp = self.tmp();
                let len = format!("len{}", tmp);
                self.push_str(&format!("auto {} = {};\n", len, operands[1]));
                let result = format!("std::vector((?*)({}), {len})", operands[0]);
                results.push(result);
            }

            Instruction::StringLower { realloc } => {
                let tmp = self.tmp();
                let val = format!("vec{}", tmp);
                let ptr = format!("ptr{}", tmp);
                let len = format!("len{}", tmp);
                if realloc.is_none() {
                    self.push_str(&format!("auto {} = {};\n", val, operands[0]));
                } else {
                    todo!();
                    // let op0 = format!("{}.into_bytes()", operands[0]);
                    // self.push_str(&format!("let {} = ({}).into_boxed_slice();\n", val, op0));
                }
                self.push_str(&format!("auto {} = (int32_t)({}.data());\n", ptr, val));
                self.push_str(&format!("auto {} = (int32_t)({}.size());\n", len, val));
                if realloc.is_some() {
                    todo!();
                    //                    self.push_str(&format!("::core::mem::forget({});\n", val));
                }
                results.push(ptr);
                results.push(len);
            }

            Instruction::StringLift => {
                let tmp = self.tmp();
                let len = format!("len{}", tmp);
                self.push_str(&format!("auto {} = {};\n", len, operands[1]));
                let result = format!("std::string((char const*)({}), {len})", operands[0]);
                results.push(result);
            }

            Instruction::ListLower { element, realloc } => {
                let body = self.blocks.pop().unwrap();
                let tmp = self.tmp();
                let vec = format!("vec{tmp}");
                let result = format!("result{tmp}");
                let layout = format!("layout{tmp}");
                let len = format!("len{tmp}");
                self.push_str(&format!(
                    "let {vec} = {operand0};\n",
                    operand0 = operands[0]
                ));
                self.push_str(&format!("let {len} = {vec}.len() as i32;\n"));
                let size = self.gen.gen.sizes.size(element);
                let align = self.gen.gen.sizes.align(element);
                self.push_str(&format!(
                    "let {layout} = alloc::Layout::from_size_align_unchecked({vec}.len() * {size}, {align});\n",
                ));
                self.push_str(&format!(
                    "let {result} = if {layout}.size() != 0\n{{\nlet ptr = alloc::alloc({layout});\n",
                ));
                self.push_str(&format!(
                    "if ptr.is_null()\n{{\nalloc::handle_alloc_error({layout});\n}}\nptr\n}}",
                ));
                self.push_str(&format!("else {{\n::core::ptr::null_mut()\n}};\n",));
                self.push_str(&format!("for (i, e) in {vec}.into_iter().enumerate() {{\n",));
                self.push_str(&format!(
                    "let base = {result} as i32 + (i as i32) * {size};\n",
                ));
                self.push_str(&body);
                self.push_str("}\n");
                results.push(format!("{result} as i32"));
                results.push(len);

                if realloc.is_none() {
                    // If an allocator isn't requested then we must clean up the
                    // allocation ourselves since our callee isn't taking
                    // ownership.
                    self.cleanup.push((result, layout));
                }
            }

            Instruction::ListLift { element, .. } => {
                let body = self.blocks.pop().unwrap();
                let tmp = self.tmp();
                let size = self.gen.gen.sizes.size(element);
                let _align = self.gen.gen.sizes.align(element);
                let len = format!("len{tmp}");
                let base = format!("base{tmp}");
                let result = format!("result{tmp}");
                self.push_str(&format!(
                    "auto {base} = {operand0};\n",
                    operand0 = operands[0]
                ));
                self.push_str(&format!(
                    "auto {len} = {operand1};\n",
                    operand1 = operands[1]
                ));
                let elemtype = print_to_result(self, resolve, |gen| {
                    gen.print_ty(SourceType::HDefs, element, None, Context::Argument)
                });
                self.push_str(&format!("auto {result} = std::vector<{elemtype}>();\n"));
                self.push_str(&format!("{result}.reserve({len});\n"));
                self.push_str(&format!("for (unsigned i=0;i<{len};++i) {{\n"));
                self.push_str(&format!("auto base = {base} + i * {size};\n"));
                self.push_str(&format!("{result}.push_back({body});\n"));
                self.push_str("}\n");
                results.push(result);
                self.push_str(&format!("free((void*){base});\n"));
            }

            Instruction::IterElem { .. } => results.push("e".to_string()),

            Instruction::IterBasePointer => results.push("base".to_string()),

            Instruction::CallWasm { name, sig, .. } => {
                let func = self.declare_import(
                    self.gen.wasm_import_module.unwrap(),
                    name,
                    &sig.params,
                    &sig.results,
                );

                // ... then call the function with all our operands
                if sig.results.len() > 0 {
                    self.push_str("auto ret = ");
                    results.push("ret".to_string());
                }
                self.push_str(&func);
                self.push_str("(");
                self.push_str(&operands.join(", "));
                self.push_str(");\n");
            }

            Instruction::CallInterface { func, .. } => {
                self.let_results(func.results.len(), results);
                match &func.kind {
                    FunctionKind::Freestanding => {
                        self.push_str(&format!(
                            "<{0}Impl as {0}>::{1}",
                            self.trait_name.unwrap(),
                            to_rust_ident(&func.name)
                        ));
                    }
                    FunctionKind::Method(ty) | FunctionKind::Static(ty) => {
                        self.push_str(&format!(
                            "<Rep{0} as {0}>::{1}",
                            resolve.types[*ty]
                                .name
                                .as_deref()
                                .unwrap()
                                .to_upper_camel_case(),
                            to_rust_ident(func.item_name())
                        ));
                    }
                    FunctionKind::Constructor(ty) => {
                        self.push_str(&format!(
                            "Own{0}::new(<Rep{0} as {0}>::new",
                            resolve.types[*ty]
                                .name
                                .as_deref()
                                .unwrap()
                                .to_upper_camel_case()
                        ));
                    }
                }
                self.push_str("(");
                self.push_str(&operands.join(", "));
                self.push_str(")");
                if let FunctionKind::Constructor(_) = &func.kind {
                    self.push_str(")");
                }
                self.push_str(";\n");
            }

            Instruction::Return { amt, func, .. } => {
                self.emit_cleanup();
                match amt {
                    0 => {}
                    1 => {
                        match &func.kind {
                            FunctionKind::Constructor(_) => {
                                // strange but works
                                self.push_str("*this = ");
                            }
                            _ => self.push_str("return "),
                        }
                        self.push_str(&operands[0]);
                        self.push_str(";\n");
                    }
                    _ => todo!(),
                }
            }

            Instruction::I32Load { offset } => {
                results.push(format!("*((int32_t const*)({} + {}))", operands[0], offset));
            }
            Instruction::I32Load8U { offset } => {
                results.push(format!(
                    "(int32_t)(*((uint8_t const*)({} + {})))",
                    operands[0], offset
                ));
            }
            Instruction::I32Load8S { offset } => {
                results.push(format!(
                    "(int32_t)(*((int8_t const*)({} + {})))",
                    operands[0], offset
                ));
            }
            Instruction::I32Load16U { offset } => {
                results.push(format!(
                    "(int32_t)(*((uint16_t const*)({} + {})))",
                    operands[0], offset
                ));
            }
            Instruction::I32Load16S { offset } => {
                results.push(format!(
                    "(int32_t)(*((int16_t const*)({} + {})))",
                    operands[0], offset
                ));
            }
            Instruction::I64Load { offset } => {
                results.push(format!("*((int64_t const*)({} + {}))", operands[0], offset));
            }
            Instruction::F32Load { offset } => {
                results.push(format!("*((float const*)({} + {}))", operands[0], offset));
            }
            Instruction::F64Load { offset } => {
                results.push(format!("*((double const*)({} + {}))", operands[0], offset));
            }
            Instruction::I32Store { offset } => {
                self.push_str(&format!(
                    "*((int32_t*)({} + {})) = {};\n",
                    operands[1], offset, operands[0]
                ));
            }
            Instruction::I32Store8 { offset } => {
                self.push_str(&format!(
                    "*((int8_t*)({} + {})) = int8_t({});\n",
                    operands[1], offset, operands[0]
                ));
            }
            Instruction::I32Store16 { offset } => {
                self.push_str(&format!(
                    "*((uint16_t*)({} + {})) = uint16_t({});\n",
                    operands[1], offset, operands[0]
                ));
            }
            Instruction::I64Store { offset } => {
                self.push_str(&format!(
                    "*((int64_t*)({} + {})) = {};\n",
                    operands[1], offset, operands[0]
                ));
            }
            Instruction::F32Store { offset } => {
                self.push_str(&format!(
                    "*((float*)({} + {})) = {};\n",
                    operands[1], offset, operands[0]
                ));
            }
            Instruction::F64Store { offset } => {
                self.push_str(&format!(
                    "*((double*)({} + {})) = {};\n",
                    operands[1], offset, operands[0]
                ));
            }

            Instruction::Malloc { .. } => unimplemented!(),

            Instruction::GuestDeallocate { size, align } => {
                self.push_str(&format!(
                    "wit_bindgen::rt::dealloc({}, {}, {});\n",
                    operands[0], size, align
                ));
            }

            Instruction::GuestDeallocateString => {
                self.push_str(&format!(
                    "wit_bindgen::rt::dealloc({}, ({}) as usize, 1);\n",
                    operands[0], operands[1],
                ));
            }

            Instruction::GuestDeallocateVariant { blocks } => {
                let max = blocks - 1;
                let blocks = self
                    .blocks
                    .drain(self.blocks.len() - blocks..)
                    .collect::<Vec<_>>();
                let op0 = &operands[0];
                self.src.push_str(&format!("match {op0} {{\n"));
                for (i, block) in blocks.into_iter().enumerate() {
                    let pat = if i == max {
                        String::from("_")
                    } else {
                        i.to_string()
                    };
                    self.src.push_str(&format!("{pat} => {block},\n"));
                }
                self.src.push_str("}\n");
            }

            Instruction::GuestDeallocateList { element } => {
                let body = self.blocks.pop().unwrap();
                let tmp = self.tmp();
                let size = self.gen.gen.sizes.size(element);
                let align = self.gen.gen.sizes.align(element);
                let len = format!("len{tmp}");
                let base = format!("base{tmp}");
                self.push_str(&format!(
                    "let {base} = {operand0};\n",
                    operand0 = operands[0]
                ));
                self.push_str(&format!(
                    "let {len} = {operand1};\n",
                    operand1 = operands[1]
                ));

                if body != "()" {
                    self.push_str("for i in 0..");
                    self.push_str(&len);
                    self.push_str(" {\n");
                    self.push_str("let base = ");
                    self.push_str(&base);
                    self.push_str(" + i *");
                    self.push_str(&size.to_string());
                    self.push_str(";\n");
                    self.push_str(&body);
                    self.push_str("\n}\n");
                }
                self.push_str(&format!(
                    "wit_bindgen::rt::dealloc({base}, ({len} as usize) * {size}, {align});\n",
                ));
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
enum SourceType {
    #[default]
    HDefs,
    HFns,
    // HHelpers,
    // CDefs,
    // CFns,
    // CHelpers,
    // CAdapters,
}

#[derive(Default)]
struct Source {
    h_defs: wit_bindgen_core::Source,
    h_fns: wit_bindgen_core::Source,
    h_helpers: wit_bindgen_core::Source,
    c_defs: wit_bindgen_core::Source,
    c_fns: wit_bindgen_core::Source,
    c_helpers: wit_bindgen_core::Source,
    c_adapters: wit_bindgen_core::Source,
}

impl Source {
    fn push_str(&mut self, src: &str) {
        self.c_fns.push_str(src);
    }

    fn src(&mut self, stype: SourceType) -> &mut wit_bindgen_core::Source {
        match stype {
            SourceType::HDefs => &mut self.h_defs,
            SourceType::HFns => &mut self.h_fns,
        }
    }
    fn append(&mut self, append_src: &Source) {
        self.h_defs.push_str(&append_src.h_defs);
        self.h_fns.push_str(&append_src.h_fns);
        self.h_helpers.push_str(&append_src.h_helpers);
        self.c_defs.push_str(&append_src.c_defs);
        self.c_fns.push_str(&append_src.c_fns);
        self.c_helpers.push_str(&append_src.c_helpers);
        self.c_adapters.push_str(&append_src.c_adapters);
    }
    fn h_defs(&mut self, s: &str) {
        self.h_defs.push_str(s);
    }
    // fn h_fns(&mut self, s: &str) {
    //     self.h_fns.push_str(s);
    // }
    // fn h_helpers(&mut self, s: &str) {
    //     self.h_helpers.push_str(s);
    // }
    // fn c_defs(&mut self, s: &str) {
    //     self.c_defs.push_str(s);
    // }
    // fn c_fns(&mut self, s: &str) {
    //     self.c_fns.push_str(s);
    // }
    // fn c_helpers(&mut self, s: &str) {
    //     self.c_helpers.push_str(s);
    // }
    fn c_adapters(&mut self, s: &str) {
        self.c_adapters.push_str(s);
    }
}

trait SourceExt {
    fn as_source(&mut self) -> &mut wit_bindgen_core::Source;

    fn print_ty_name(
        &mut self,
        resolve: &Resolve,
        ty: &Type,
        world: &str,
        parent: Option<&str>,
        dependencies: &mut Includes,
        opts: &Opts,
    ) {
        push_ty_name(
            resolve,
            ty,
            self.as_source().as_mut_string(),
            world,
            parent,
            dependencies,
            opts,
        );
    }
}

impl SourceExt for wit_bindgen_core::Source {
    fn as_source(&mut self) -> &mut wit_bindgen_core::Source {
        self
    }
}

fn push_ty_name(
    resolve: &Resolve,
    ty: &Type,
    src: &mut String,
    world: &str,
    parent: Option<&str>,
    dependencies: &mut Includes,
    opts: &Opts,
) {
    fn owner_namespace(resolve: &Resolve, id: TypeId, world: &str) -> String {
        let ty = &resolve.types[id];
        match ty.owner {
            // If this type belongs to an interface, then use that interface's
            // original name if it's listed in the source. Otherwise if it's an
            // "anonymous" interface as part of a world use the name of the
            // import/export in the world which would have been stored in
            // `interface_names`.
            TypeOwner::Interface(owner) => resolve.interfaces[owner]
                .name
                .as_ref()
                .map(|s| s.to_snake_case())
                .unwrap_or_else(|| world.to_snake_case()),

            TypeOwner::World(owner) => resolve.worlds[owner].name.to_snake_case(),

            // Namespace everything else under the "default" world being
            // generated to avoid putting too much into the root namespace in C.
            TypeOwner::None => world.to_snake_case(),
        }
    }

    match ty {
        Type::Bool => src.push_str("bool"),
        Type::Char => src.push_str("uint32_t"),
        Type::U8 => src.push_str("uint8_t"),
        Type::S8 => src.push_str("int8_t"),
        Type::U16 => src.push_str("uint16_t"),
        Type::S16 => src.push_str("int16_t"),
        Type::U32 => src.push_str("uint32_t"),
        Type::S32 => src.push_str("int32_t"),
        Type::U64 => src.push_str("uint64_t"),
        Type::S64 => src.push_str("int64_t"),
        Type::Float32 => src.push_str("float"),
        Type::Float64 => src.push_str("double"),
        Type::String => {
            src.push_str("std::string_view");
            dependencies.needs_string_view = true;
        }
        Type::Id(id) => {
            let ty = &resolve.types[*id];
            if let Some(name) = &ty.name {
                let ns = owner_namespace(resolve, *id, world).to_snake_case();
                if ns != parent.unwrap_or_default() {
                    src.push_str("::");
                    src.push_str(&ns);
                    src.push_str("::");
                }
                return src.push_str(&name.to_pascal_case());
            }
            match &ty.kind {
                TypeDefKind::Type(t) => {
                    push_ty_name(resolve, t, src, world, parent, dependencies, opts)
                }
                TypeDefKind::Record(_)
                | TypeDefKind::Flags(_)
                | TypeDefKind::Enum(_)
                | TypeDefKind::Variant(_) => {
                    unimplemented!()
                }
                TypeDefKind::Tuple(t) => {
                    src.push_str("std::tuple<");
                    for (i, ty) in t.types.iter().enumerate() {
                        if i != 0 {
                            src.push_str(", ");
                        }
                        push_ty_name(resolve, ty, src, world, parent, dependencies, opts);
                    }
                    src.push_str(">");
                }
                TypeDefKind::Option(ty) => {
                    dependencies.needs_optional = true;
                    src.push_str("std::optional<");
                    push_ty_name(resolve, ty, src, world, parent, dependencies, opts);
                    src.push_str(">");
                }
                TypeDefKind::Result(r) => {
                    dependencies.needs_expected = true;
                    // src.push_str(&world.to_snake_case());
                    src.push_str("std::expected<");
                    push_optional_ty_name(
                        resolve,
                        r.ok.as_ref(),
                        src,
                        world,
                        parent,
                        dependencies,
                        opts,
                    );
                    src.push_str(", ");
                    push_optional_ty_name(
                        resolve,
                        r.err.as_ref(),
                        src,
                        world,
                        parent,
                        dependencies,
                        opts,
                    );
                    src.push_str(">");
                }
                TypeDefKind::List(t) => {
                    dependencies.needs_vector = true;
                    src.push_str("std::vector<");
                    push_ty_name(resolve, t, src, world, parent, dependencies, opts);
                    src.push_str(">");
                }
                TypeDefKind::Future(t) => {
                    src.push_str("std::future<");
                    push_optional_ty_name(
                        resolve,
                        t.as_ref(),
                        src,
                        world,
                        parent,
                        dependencies,
                        opts,
                    );
                    src.push_str(">");
                }
                TypeDefKind::Stream(s) => {
                    src.push_str("stream_");
                    push_optional_ty_name(
                        resolve,
                        s.element.as_ref(),
                        src,
                        world,
                        parent,
                        dependencies,
                        opts,
                    );
                    src.push_str("_");
                    push_optional_ty_name(
                        resolve,
                        s.end.as_ref(),
                        src,
                        world,
                        parent,
                        dependencies,
                        opts,
                    );
                }
                TypeDefKind::Unknown => unreachable!(),
                TypeDefKind::Resource => todo!(),
                TypeDefKind::Handle(h) => {
                    let (id, ownership, closing) = match h {
                        Handle::Own(id) if opts.guest_header => (id, "".into(), " "),
                        Handle::Borrow(id) if opts.guest_header => (id, "".into(), " "),
                        Handle::Own(id) => (id, format!("{world}::{OWNED_CLASS_NAME}<"), ">"),
                        Handle::Borrow(id) => (id, "".into(), "*"),
                    };
                    let ty: &TypeDef = &resolve.types[*id];
                    let ns = owner_namespace(resolve, *id, world);
                    let name = ty.name.as_ref().map_or("?".into(), |n| n.to_pascal_case());
                    src.push_str(&format!("{ownership}{ns}::{name}{closing}"));
                }
            }
        }
    }

    fn push_optional_ty_name(
        resolve: &Resolve,
        ty: Option<&Type>,
        dst: &mut String,
        world: &str,
        parent: Option<&str>,
        dependencies: &mut Includes,
        opts: &Opts,
    ) {
        match ty {
            Some(ty) => push_ty_name(resolve, ty, dst, world, parent, dependencies, opts),
            None => dst.push_str("void"),
        }
    }
}

pub fn int_repr(ty: Int) -> &'static str {
    wit_bindgen_c::int_repr(ty)
}

pub fn flags_repr(f: &Flags) -> Int {
    wit_bindgen_c::flags_repr(f)
}

pub fn is_arg_by_pointer(resolve: &Resolve, ty: &Type) -> bool {
    wit_bindgen_c::is_arg_by_pointer(resolve, ty)
}

pub fn is_empty_type(resolve: &Resolve, ty: &Type) -> bool {
    wit_bindgen_c::is_empty_type(resolve, ty)
}

pub fn get_nonempty_type<'o>(resolve: &Resolve, ty: Option<&'o Type>) -> Option<&'o Type> {
    wit_bindgen_c::get_nonempty_type(resolve, ty)
}

pub fn owns_anything(
    resolve: &Resolve,
    ty: &Type,
    is_local_resource: &dyn Fn(&Resolve, TypeId) -> bool,
) -> bool {
    wit_bindgen_c::owns_anything(resolve, ty, is_local_resource)
}

pub fn optional_owns_anything(
    resolve: &Resolve,
    ty: Option<&Type>,
    is_local_resource: &dyn Fn(&Resolve, TypeId) -> bool,
) -> bool {
    wit_bindgen_c::optional_owns_anything(resolve, ty, is_local_resource)
}

pub fn to_c_ident(name: &str) -> String {
    wit_bindgen_c::to_c_ident(name)
}

fn to_rust_ident(name: &str) -> String {
    match name {
        // Escape C++ keywords.
        // Source: https://doc.rust-lang.org/reference/keywords.html
        "this" => "this_".into(),
        _ => wit_bindgen_c::to_c_ident(name),
    }
}

fn wasm_type(ty: WasmType) -> &'static str {
    match ty {
        WasmType::I32 => "int32_t",
        WasmType::I64 => "int64_t",
        WasmType::F32 => "float",
        WasmType::F64 => "double",
    }
}
