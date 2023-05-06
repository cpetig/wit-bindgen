// mod component_type_object;

use heck::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::mem;
use wit_bindgen_core::wit_parser::abi::{AbiVariant, Bindgen};
use wit_bindgen_core::{
    uwrite, uwriteln, wit_parser::*, Files, InterfaceGenerator as _, Ns, WorldGenerator,
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
    types: HashMap<TypeId, wit_bindgen_core::Source>,

    // The set of types that are considered public (aka need to be in the
    // header file) which are anonymous and we're effectively monomorphizing.
    // This is discovered lazily when printing type names.
    public_anonymous_types: BTreeSet<TypeId>,

    // This is similar to `public_anonymous_types` where it's discovered
    // lazily, but the set here are for private types only used in the
    // implementation of functions. These types go in the implementation file,
    // not the header file.
    private_anonymous_types: BTreeSet<TypeId>,
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
        let mut gen = self.interface(resolve, true);
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
        let mut gen = self.interface(resolve, true);

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
        let mut gen = self.interface(resolve, false);
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
        let mut gen = self.interface(resolve, false);

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
                        int32_t id;
                        public:
                        virtual ~{RESOURCE_BASE_CLASS_NAME}() {{}}
                        {RESOURCE_BASE_CLASS_NAME}() : id() {{}}
                        void register_resource();
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
                        bool owned;
                        public:
                        {RESOURCE_BASE_CLASS_NAME}() : handle(), owned() {{}}
                        void set_handle(int32_t h, bool o) {{ handle=h; owned=o; }}
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
        _in_import: bool,
    ) -> InterfaceGenerator<'a> {
        InterfaceGenerator {
            src: Source::default(),
            gen: self,
            resolve,
            interface: None,
            // in_import,
        }
    }

    fn include(&mut self, s: &str) {
        self.includes.push(s.to_string());
    }

    fn finish_types(&mut self, resolve: &Resolve) {
        for (id, _) in resolve.types.iter() {
            if let Some(ty) = self.types.get(&id) {
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
                for func in funcs {
                    gen.import(Some(name), func);
                }
                if gen.gen.opts.guest_header {
                    // destructor
                    gen.src.h_defs(&format!("~{pascal}();\n"));
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

struct InterfaceGenerator<'a> {
    src: Source,
    // in_import: bool,
    gen: &'a mut CppHost,
    resolve: &'a Resolve,
    interface: Option<InterfaceId>,
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
            | TypeDefKind::Variant(_)
            | TypeDefKind::Union(_) => {
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

    fn type_union(&mut self, _id: TypeId, _name: &str, _union: &Union, _docs: &Docs) {
        todo!();
        // let prev = mem::take(&mut self.src.h_defs);
        // self.src.h_defs("\n");
        // self.docs(docs, SourceType::HDefs);
        // self.src.h_defs("typedef struct {\n");
        // self.src.h_defs(int_repr(union.tag()));
        // self.src.h_defs(" tag;\n");
        // self.src.h_defs("union {\n");
        // for (i, case) in union.cases.iter().enumerate() {
        //     self.docs(&case.docs, SourceType::HDefs);
        //     self.print_ty(SourceType::HDefs, &case.ty, None, Context::InStruct);
        //     uwriteln!(self.src.h_defs, " f{i};");
        // }
        // self.src.h_defs("} val;\n");
        // self.src.h_defs("} ");
        // self.print_typedef_target(id, name);

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
    c_args: Vec<(String, String)>, // name, type
    wamr_types: String,
    c_result: String,
    wamr_result: String,
}

impl Default for WamrSig {
    fn default() -> Self {
        Self {
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
                TypeDefKind::Union(_) => todo!(),
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

    const RESULT_NAME: &str = "result_out";

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
                TypeDefKind::Enum(_) => todo!(),
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
                TypeDefKind::Union(_) => todo!(),
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
        let mut result = WamrSig::default();
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
                    TypeDefKind::Union(_) => todo!(),
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
            TypeDefKind::Enum(_) => todo!(),
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
            TypeDefKind::Union(_) => todo!(),
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
            .types
            .insert(id, mem::replace(&mut self.src.h_defs, orig_h_defs));
        assert!(prev.is_none());
    }
}

struct FunctionBindgen<'a, 'b> {
    gen: &'a mut InterfaceGenerator<'b>,
    func: &'a Function,
    //    size: &'a SizeAlign,
}

// impl<'a, 'b> FunctionBindgen<'a, 'b> {
//     fn new(gen: &'a mut InterfaceGenerator<'b>, func: &'a Function) -> Self {
//         Self { gen, func }
//     }
// }

impl Bindgen for FunctionBindgen<'_, '_> {
    type Operand = String;

    fn emit(
        &mut self,
        _resolve: &Resolve,
        inst: &abi::Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        println!("{inst:?}({operands:?}) -> {results:?}");
        match inst {
            abi::Instruction::GetArg { nth } => results.push(self.func.params[*nth].0.clone()),
            abi::Instruction::I32Const { .. } => todo!(),
            abi::Instruction::Bitcasts { .. } => todo!(),
            abi::Instruction::ConstZero { .. } => todo!(),
            abi::Instruction::I32Load { .. } => todo!(),
            abi::Instruction::I32Load8U { .. } => todo!(),
            abi::Instruction::I32Load8S { .. } => todo!(),
            abi::Instruction::I32Load16U { .. } => todo!(),
            abi::Instruction::I32Load16S { .. } => todo!(),
            abi::Instruction::I64Load { .. } => todo!(),
            abi::Instruction::F32Load { .. } => todo!(),
            abi::Instruction::F64Load { .. } => todo!(),
            abi::Instruction::I32Store { .. } => todo!(),
            abi::Instruction::I32Store8 { .. } => todo!(),
            abi::Instruction::I32Store16 { .. } => todo!(),
            abi::Instruction::I64Store { .. } => todo!(),
            abi::Instruction::F32Store { .. } => todo!(),
            abi::Instruction::F64Store { .. } => todo!(),
            abi::Instruction::I32FromChar => todo!(),
            abi::Instruction::I64FromU64 => todo!(),
            abi::Instruction::I64FromS64 => todo!(),
            abi::Instruction::I32FromU32 => todo!(),
            abi::Instruction::I32FromS32 => results.push(format!("(int32_t)({})", operands[0])),
            abi::Instruction::I32FromU16 => todo!(),
            abi::Instruction::I32FromS16 => todo!(),
            abi::Instruction::I32FromU8 => todo!(),
            abi::Instruction::I32FromS8 => todo!(),
            abi::Instruction::F32FromFloat32 => todo!(),
            abi::Instruction::F64FromFloat64 => todo!(),
            abi::Instruction::S8FromI32 => todo!(),
            abi::Instruction::U8FromI32 => todo!(),
            abi::Instruction::S16FromI32 => todo!(),
            abi::Instruction::U16FromI32 => todo!(),
            abi::Instruction::S32FromI32 => todo!(),
            abi::Instruction::U32FromI32 => todo!(),
            abi::Instruction::S64FromI64 => todo!(),
            abi::Instruction::U64FromI64 => todo!(),
            abi::Instruction::CharFromI32 => todo!(),
            abi::Instruction::Float32FromF32 => todo!(),
            abi::Instruction::Float64FromF64 => todo!(),
            abi::Instruction::BoolFromI32 => todo!(),
            abi::Instruction::I32FromBool => todo!(),
            abi::Instruction::ListCanonLower { .. } => todo!(),
            abi::Instruction::StringLower { .. } => todo!(),
            abi::Instruction::ListLower { .. } => todo!(),
            abi::Instruction::ListCanonLift { .. } => todo!(),
            abi::Instruction::StringLift => todo!(),
            abi::Instruction::ListLift { .. } => todo!(),
            abi::Instruction::IterElem { .. } => todo!(),
            abi::Instruction::IterBasePointer => todo!(),
            abi::Instruction::RecordLower { .. } => todo!(),
            abi::Instruction::RecordLift { .. } => todo!(),
            abi::Instruction::TupleLower { .. } => todo!(),
            abi::Instruction::TupleLift { .. } => todo!(),
            abi::Instruction::FlagsLower { .. } => todo!(),
            abi::Instruction::FlagsLift { .. } => todo!(),
            abi::Instruction::VariantPayloadName => todo!(),
            abi::Instruction::VariantLower { .. } => todo!(),
            abi::Instruction::VariantLift { .. } => todo!(),
            abi::Instruction::UnionLower { .. } => todo!(),
            abi::Instruction::UnionLift { .. } => todo!(),
            abi::Instruction::EnumLower { .. } => todo!(),
            abi::Instruction::EnumLift { .. } => todo!(),
            abi::Instruction::OptionLower { .. } => todo!(),
            abi::Instruction::OptionLift { .. } => todo!(),
            abi::Instruction::ResultLower { .. } => todo!(),
            abi::Instruction::ResultLift { .. } => todo!(),
            abi::Instruction::CallWasm { .. } => todo!(),
            abi::Instruction::CallInterface { .. } => todo!(),
            abi::Instruction::Return { .. } => todo!(),
            abi::Instruction::Malloc { .. } => todo!(),
            abi::Instruction::GuestDeallocate { .. } => todo!(),
            abi::Instruction::GuestDeallocateString => todo!(),
            abi::Instruction::GuestDeallocateList { .. } => todo!(),
            abi::Instruction::GuestDeallocateVariant { .. } => todo!(),
            abi::Instruction::HandleLower { .. } => todo!(),
            abi::Instruction::HandleLift { .. } => todo!(),
        }
    }

    fn return_pointer(&mut self, _size: usize, _align: usize) -> Self::Operand {
        //println!("")
        todo!()
    }

    fn push_block(&mut self) {
        todo!()
    }

    fn finish_block(&mut self, _operand: &mut Vec<Self::Operand>) {
        todo!()
    }

    fn sizes(&self) -> &SizeAlign {
        &self.gen.gen.sizes
    }

    fn is_list_canonical(&self, _resolve: &Resolve, _element: &Type) -> bool {
        todo!()
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
                | TypeDefKind::Variant(_)
                | TypeDefKind::Union(_) => {
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
