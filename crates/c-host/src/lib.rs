// mod component_type_object;

use heck::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::mem;
use wit_bindgen_core::{
    uwrite, uwriteln, wit_parser::*, Files, InterfaceGenerator as _, Ns, WorldGenerator,
};
use wit_component::StringEncoding;

#[derive(Default)]
struct CHost {
    src: Source,
    opts: Opts,
    includes: Vec<String>,
    // return_pointer_area_size: usize,
    // return_pointer_area_align: usize,
    names: Ns,
    // needs_string: bool,
    world: String,
    sizes: SizeAlign,

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
}

impl Opts {
    pub fn build(&self) -> Box<dyn WorldGenerator> {
        let mut r = CHost::default();
        r.opts = self.clone();
        Box::new(r)
    }
}

#[derive(Debug, Default)]
struct Return {
    scalar: Option<Scalar>,
    retptrs: Vec<Type>,
}

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
    OptionBool(Type),
    ResultBool(Option<Type>, Option<Type>),
    Type(Type),
}

impl WorldGenerator for CHost {
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
        let prev = self.interface_names.insert(id, name.clone());
        assert!(prev.is_none());
        let mut gen = self.interface(resolve, true);
        gen.interface = Some(id);
        if gen.gen.interfaces_with_types_printed.insert(id) {
            gen.types(id);
        }

        for (i, (_name, func)) in resolve.interfaces[id].functions.iter().enumerate() {
            if i == 0 {
                let name = resolve.name_world_key(name);
                uwriteln!(gen.src.h_fns, "\n// Imported Functions from `{name}`");
            }
            gen.import(Some(name), func);
        }

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
    ) {
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
    }

    fn export_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        _files: &mut Files,
    ) {
        let name = &resolve.worlds[world].name;
        let mut gen = self.interface(resolve, false);

        for (i, (_name, func)) in funcs.iter().enumerate() {
            if i == 0 {
                uwriteln!(gen.src.h_fns, "\n// Exported Functions from `{name}`");
            }
            gen.export(func, None);
        }

        gen.gen.src.append(&gen.src);
    }

    fn export_types(
        &mut self,
        resolve: &Resolve,
        _world: WorldId,
        types: &[(&str, TypeId)],
        _files: &mut Files,
    ) {
        let mut gen = self.interface(resolve, false);
        for (name, id) in types {
            gen.define_type(name, *id);
        }
        gen.gen.src.append(&gen.src);
    }

    fn finish(&mut self, resolve: &Resolve, id: WorldId, files: &mut Files) {
        self.finish_types(resolve);
        let world = &resolve.worlds[id];
        self.include("<stdlib.h>");
        let snake = world.name.to_snake_case();

        self.print_intrinsics();

        let version = env!("CARGO_PKG_VERSION");
        let mut h_str = wit_bindgen_core::Source::default();
        uwriteln!(
            h_str,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );

        uwrite!(
            h_str,
            "#ifndef __HOST_BINDINGS_{0}_H
            #define __HOST_BINDINGS_{0}_H
            #ifdef __cplusplus
            extern \"C\" {{",
            world.name.to_shouty_snake_case(),
        );

        // Deindent the extern C { declaration
        h_str.deindent(1);
        uwriteln!(h_str, "\n#endif\n");

        self.include("<stdint.h>");
        self.include("<stdbool.h>");

        for include in self.includes.iter() {
            uwriteln!(h_str, "#include {include}");
        }

        let mut c_str = wit_bindgen_core::Source::default();
        uwriteln!(
            c_str,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );
        uwriteln!(c_str, "#include \"{snake}_host.h\"");
        if c_str.len() > 0 {
            c_str.push_str("\n");
        }
        c_str.push_str(&self.src.c_defs);
        c_str.push_str(&self.src.c_fns);

        if self.src.h_defs.len() > 0 {
            h_str.push_str(&self.src.h_defs);
        }

        h_str.push_str(&self.src.h_fns);

        uwriteln!(c_str, "\n// Component Adapters");

        c_str.push_str(&self.src.c_adapters);

        uwriteln!(
            h_str,
            "
            #ifdef __cplusplus
            }}
            #endif
            #endif"
        );

        files.push(&format!("{snake}_host.c"), c_str.as_bytes());
        files.push(&format!("{snake}_host.h"), h_str.as_bytes());
    }
}

impl CHost {
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
        // Continuously generate anonymous types while we continue to find more
        //
        // First we take care of the public set of anonymous types. This will
        // iteratively print them and also remove any references from the
        // private set if we happen to also reference them.
        while !self.public_anonymous_types.is_empty() {
            for ty in mem::take(&mut self.public_anonymous_types) {
                self.print_anonymous_type(resolve, ty);
            }
        }

        // Next we take care of private types. To do this we have basically the
        // same loop as above, after we switch the sets. We record, however,
        // all private types in a local set here to later determine if the type
        // needs to be in the C file or the H file.
        //
        // Note though that we don't re-print a type (and consider it private)
        // if we already printed it above as part of the public set.
        let mut private_types = HashSet::new();
        self.public_anonymous_types = mem::take(&mut self.private_anonymous_types);
        while !self.public_anonymous_types.is_empty() {
            for ty in mem::take(&mut self.public_anonymous_types) {
                if self.types.contains_key(&ty) {
                    continue;
                }
                private_types.insert(ty);
                self.print_anonymous_type(resolve, ty);
            }
        }

        for (id, _) in resolve.types.iter() {
            if let Some(ty) = self.types.get(&id) {
                if private_types.contains(&id) {
                    // It's private; print it in the .c file.
                    self.src.c_defs(ty);
                } else {
                    // It's public; print it in the .h file.
                    self.src.h_defs(ty);
                    // self.print_dtor(resolve, id);
                }
            }
        }
    }

    fn print_anonymous_type(&mut self, resolve: &Resolve, ty: TypeId) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\ntypedef ");
        let kind = &resolve.types[ty].kind;
        match kind {
            TypeDefKind::Type(_)
            | TypeDefKind::Flags(_)
            | TypeDefKind::Record(_)
            | TypeDefKind::Enum(_)
            | TypeDefKind::Variant(_)
            | TypeDefKind::Union(_) => {
                unreachable!()
            }
            TypeDefKind::Tuple(t) => {
                self.src.h_defs("struct {\n");
                for (i, t) in t.types.iter().enumerate() {
                    let ty = self.type_name(resolve, t);
                    uwriteln!(self.src.h_defs, "{ty} f{i};");
                }
                self.src.h_defs("}");
            }
            TypeDefKind::Option(t) => {
                self.src.h_defs("struct {\n");
                self.src.h_defs("bool is_some;\n");
                if !is_empty_type(resolve, t) {
                    let ty = self.type_name(resolve, t);
                    uwriteln!(self.src.h_defs, "{ty} val;");
                }
                self.src.h_defs("}");
            }
            TypeDefKind::Result(r) => {
                self.src.h_defs(
                    "struct {
                    bool is_err;
                    union {
                ",
                );
                if let Some(ok) = get_nonempty_type(resolve, r.ok.as_ref()) {
                    let ty = self.type_name(resolve, ok);
                    uwriteln!(self.src.h_defs, "{ty} ok;");
                }
                if let Some(err) = get_nonempty_type(resolve, r.err.as_ref()) {
                    let ty = self.type_name(resolve, err);
                    uwriteln!(self.src.h_defs, "{ty} err;");
                }
                self.src.h_defs("} val;\n");
                self.src.h_defs("}");
            }
            TypeDefKind::List(t) => {
                self.src.h_defs("struct {\n");
                let ty = self.type_name(resolve, t);
                uwriteln!(self.src.h_defs, "{ty} *ptr;");
                self.src.h_defs("size_t len;\n");
                self.src.h_defs("}");
            }
            TypeDefKind::Future(_) => todo!("print_anonymous_type for future"),
            TypeDefKind::Stream(_) => todo!("print_anonymous_type for stream"),
            TypeDefKind::Unknown => unreachable!(),
        }
        self.src.h_defs(" ");
        let ns = self.owner_namespace(resolve, ty);
        self.src.h_defs(&ns);
        self.src.h_defs("_");
        self.src.h_defs.print_ty_name(resolve, &Type::Id(ty));
        self.src.h_defs("_t;\n");
        let type_source = mem::replace(&mut self.src.h_defs, prev);
        self.types.insert(ty, type_source);
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
                    ns.push_str("_");
                    ns.push_str(&pkg.name.name.to_snake_case());
                    ns.push_str("_");
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

    fn type_name(&mut self, resolve: &Resolve, ty: &Type) -> String {
        let mut name = String::new();
        self.push_type_name(resolve, ty, &mut name);
        name
    }

    fn push_type_name(&mut self, resolve: &Resolve, ty: &Type, dst: &mut String) {
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
                dst.push_str(&self.world.to_snake_case());
                dst.push_str("_");
                dst.push_str("string_t");
                // self.needs_string = true;
            }
            Type::Id(id) => {
                let ty = &resolve.types[*id];
                let ns = self.owner_namespace(resolve, *id);
                match &ty.name {
                    Some(name) => {
                        dst.push_str(&ns);
                        dst.push_str("_");
                        dst.push_str(&name.to_snake_case());
                        dst.push_str("_t");
                    }
                    None => match &ty.kind {
                        TypeDefKind::Type(t) => self.push_type_name(resolve, t, dst),
                        _ => {
                            self.public_anonymous_types.insert(*id);
                            self.private_anonymous_types.remove(id);
                            dst.push_str(&ns);
                            dst.push_str("_");
                            push_ty_name(resolve, &Type::Id(*id), dst);
                            dst.push_str("_t");
                        }
                    },
                }
            }
        }
    }
}

struct InterfaceGenerator<'a> {
    src: Source,
    // in_import: bool,
    gen: &'a mut CHost,
    resolve: &'a Resolve,
    interface: Option<InterfaceId>,
}

impl CHost {
    fn print_intrinsics(&mut self) {}
}

impl Return {
    fn return_single(&mut self, resolve: &Resolve, ty: &Type, orig_ty: &Type) {
        let id = match ty {
            Type::Id(id) => *id,
            Type::String => {
                self.retptrs.push(*orig_ty);
                return;
            }
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
            TypeDefKind::Option(_ty) => {}

            // Unpack a result as a boolean return type, with two
            // return pointers for ok and err values
            TypeDefKind::Result(_r) => {}

            // These types are always returned indirectly.
            TypeDefKind::Tuple(_)
            | TypeDefKind::Record(_)
            | TypeDefKind::List(_)
            | TypeDefKind::Variant(_)
            | TypeDefKind::Union(_) => {}

            TypeDefKind::Future(_) => todo!("return_single for future"),
            TypeDefKind::Stream(_) => todo!("return_single for stream"),
            TypeDefKind::Unknown => unreachable!(),
        }

        self.retptrs.push(*orig_ty);
    }
}

impl<'a> wit_bindgen_core::InterfaceGenerator<'a> for InterfaceGenerator<'a> {
    fn resolve(&self) -> &'a Resolve {
        self.resolve
    }

    fn type_record(&mut self, id: TypeId, name: &str, record: &Record, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        for field in record.fields.iter() {
            self.docs(&field.docs, SourceType::HDefs);
            self.print_ty(SourceType::HDefs, &field.ty);
            self.src.h_defs(" ");
            self.src.h_defs(&to_c_ident(&field.name));
            self.src.h_defs(";\n");
        }
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);

        self.finish_ty(id, prev);
    }

    fn type_tuple(&mut self, id: TypeId, name: &str, tuple: &Tuple, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        for (i, ty) in tuple.types.iter().enumerate() {
            self.print_ty(SourceType::HDefs, ty);
            uwriteln!(self.src.h_defs, " f{i};");
        }
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);

        self.finish_ty(id, prev);
    }

    fn type_flags(&mut self, id: TypeId, name: &str, flags: &Flags, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef ");
        let repr = flags_repr(flags);
        self.src.h_defs(int_repr(repr));
        self.src.h_defs(" ");
        self.print_typedef_target(id, name);

        if flags.flags.len() > 0 {
            self.src.h_defs("\n");
        }
        let ns = self
            .gen
            .owner_namespace(self.resolve, id)
            .to_shouty_snake_case();
        for (i, flag) in flags.flags.iter().enumerate() {
            self.docs(&flag.docs, SourceType::HDefs);
            uwriteln!(
                self.src.h_defs,
                "#define {ns}_{}_{} (1 << {i})",
                name.to_shouty_snake_case(),
                flag.name.to_shouty_snake_case(),
            );
        }

        self.finish_ty(id, prev);
    }

    fn type_variant(&mut self, id: TypeId, name: &str, variant: &Variant, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        self.src.h_defs(int_repr(variant.tag()));
        self.src.h_defs(" tag;\n");
        self.src.h_defs("union {\n");
        for case in variant.cases.iter() {
            if let Some(ty) = get_nonempty_type(self.resolve, case.ty.as_ref()) {
                self.print_ty(SourceType::HDefs, ty);
                self.src.h_defs(" ");
                self.src.h_defs(&to_c_ident(&case.name));
                self.src.h_defs(";\n");
            }
        }
        self.src.h_defs("} val;\n");
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);

        if variant.cases.len() > 0 {
            self.src.h_defs("\n");
        }
        let ns = self
            .gen
            .owner_namespace(self.resolve, id)
            .to_shouty_snake_case();
        for (i, case) in variant.cases.iter().enumerate() {
            self.docs(&case.docs, SourceType::HDefs);
            uwriteln!(
                self.src.h_defs,
                "#define {ns}_{}_{} {i}",
                name.to_shouty_snake_case(),
                case.name.to_shouty_snake_case(),
            );
        }

        self.finish_ty(id, prev);
    }

    fn type_union(&mut self, id: TypeId, name: &str, union: &Union, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        self.src.h_defs(int_repr(union.tag()));
        self.src.h_defs(" tag;\n");
        self.src.h_defs("union {\n");
        for (i, case) in union.cases.iter().enumerate() {
            self.docs(&case.docs, SourceType::HDefs);
            self.print_ty(SourceType::HDefs, &case.ty);
            uwriteln!(self.src.h_defs, " f{i};");
        }
        self.src.h_defs("} val;\n");
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);

        self.finish_ty(id, prev);
    }

    fn type_option(&mut self, id: TypeId, name: &str, payload: &Type, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        self.src.h_defs("bool is_some;\n");
        if !is_empty_type(self.resolve, payload) {
            self.print_ty(SourceType::HDefs, payload);
            self.src.h_defs(" val;\n");
        }
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);

        self.finish_ty(id, prev);
    }

    fn type_result(&mut self, id: TypeId, name: &str, result: &Result_, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        self.src.h_defs("bool is_err;\n");
        self.src.h_defs("union {\n");
        if let Some(ok) = get_nonempty_type(self.resolve, result.ok.as_ref()) {
            self.print_ty(SourceType::HDefs, ok);
            self.src.h_defs(" ok;\n");
        }
        if let Some(err) = get_nonempty_type(self.resolve, result.err.as_ref()) {
            self.print_ty(SourceType::HDefs, err);
            self.src.h_defs(" err;\n");
        }
        self.src.h_defs("} val;\n");
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);

        self.finish_ty(id, prev);
    }

    fn type_enum(&mut self, id: TypeId, name: &str, enum_: &Enum, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        uwrite!(self.src.h_defs, "\n");
        self.docs(docs, SourceType::HDefs);
        let int_t = int_repr(enum_.tag());
        uwrite!(self.src.h_defs, "typedef {int_t} ");
        self.print_typedef_target(id, name);

        if enum_.cases.len() > 0 {
            self.src.h_defs("\n");
        }
        let ns = self
            .gen
            .owner_namespace(self.resolve, id)
            .to_shouty_snake_case();
        for (i, case) in enum_.cases.iter().enumerate() {
            self.docs(&case.docs, SourceType::HDefs);
            uwriteln!(
                self.src.h_defs,
                "#define {ns}_{}_{} {i}",
                name.to_shouty_snake_case(),
                case.name.to_shouty_snake_case(),
            );
        }

        self.finish_ty(id, prev);
    }

    fn type_alias(&mut self, id: TypeId, name: &str, ty: &Type, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef ");
        self.print_ty(SourceType::HDefs, ty);
        self.src.h_defs(" ");
        self.print_typedef_target(id, name);
        self.finish_ty(id, prev);
    }

    fn type_list(&mut self, id: TypeId, name: &str, ty: &Type, docs: &Docs) {
        let prev = mem::take(&mut self.src.h_defs);
        self.src.h_defs("\n");
        self.docs(docs, SourceType::HDefs);
        self.src.h_defs("typedef struct {\n");
        self.print_ty(SourceType::HDefs, ty);
        self.src.h_defs(" *ptr;\n");
        self.src.h_defs("size_t len;\n");
        self.src.h_defs("} ");
        self.print_typedef_target(id, name);
        self.finish_ty(id, prev);
    }

    fn type_builtin(&mut self, _id: TypeId, name: &str, ty: &Type, docs: &Docs) {
        drop((_id, name, ty, docs));
    }
}

impl InterfaceGenerator<'_> {
    fn c_func_name(&self, interface_name: Option<&WorldKey>, func: &Function) -> String {
        let mut name = String::new();
        match interface_name {
            Some(WorldKey::Name(k)) => name.push_str(&k.to_snake_case()),
            Some(WorldKey::Interface(id)) => {
                // if !self.in_import {
                //     name.push_str("exports_");
                // }
                let iface = &self.resolve.interfaces[*id];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                name.push_str(&pkg.name.namespace.to_snake_case());
                name.push_str("_");
                name.push_str(&pkg.name.name.to_snake_case());
                name.push_str("_");
                name.push_str(&iface.name.as_ref().unwrap().to_snake_case());
            }
            None => name.push_str(&self.gen.world.to_snake_case()),
        }
        name.push_str("_");
        name.push_str(&func.name.to_snake_case());
        name
    }

    fn import(&mut self, interface_name: Option<&WorldKey>, func: &Function) {
        self.docs(&func.docs, SourceType::HFns);
        // let sig = self.resolve.wasm_signature(AbiVariant::GuestImport, func);

        self.src.c_fns("\n");

        let name = self.c_func_name(interface_name, func);
        let host_name = self.gen.names.tmp(&format!("host_{name}"));
        let _c_sig = self.print_sig(interface_name, func);
        self.src.c_adapters(&format!(
            r#"
                static wasm_trap_t *
                {host_name}(const wasm_val_vec_t *args, wasm_val_vec_t *results)
                {{
                    check_i_(args, results);
                    {name}();
                    return NULL;
                }}
                "#
        ));
        if false {
            // for later
            self.src.c_adapters(
                r#"
                static void check_i_(const wasm_val_vec_t *args, wasm_val_vec_t *results)
                {
                    assert(args->num_elems == 1);
                    assert(args->data[0].kind == WASM_I32);
                }
                "#,
            );
            self.src.c_adapters(
                r#"
                static wasm_functype_t *type_i_()
                {
                    return wasm_functype_new_1_0(wasm_valtype_new_i32());
                }
                "#,
            );
        }
        self.src.c_adapters(&format!(
            r#"
            static const struct function_register_t exports[] = {{
                {{"{:?}", "{}", &type_i_, {host_name}}},
            }};
            "#,
            interface_name.unwrap_or(&WorldKey::Name("env".into())),
            func.name
        ));
    }

    fn export(&mut self, _func: &Function, _interface_name: Option<&WorldKey>) {
        todo!("function exports");
    }

    fn print_sig(&mut self, interface_name: Option<&WorldKey>, func: &Function) -> CSig {
        let name = self.c_func_name(interface_name, func);
        self.gen.names.insert(&name).expect("duplicate symbols");

        let mut result_rets = false;
        let mut result_rets_has_ok_type = false;

        let ret = self.classify_ret(func);
        match &ret.scalar {
            None | Some(Scalar::Void) => self.src.h_fns("void"),
            Some(Scalar::OptionBool(_id)) => self.src.h_fns("bool"),
            Some(Scalar::ResultBool(ok, _err)) => {
                result_rets = true;
                result_rets_has_ok_type = ok.is_some();
                self.src.h_fns("bool");
            }
            Some(Scalar::Type(ty)) => self.print_ty(SourceType::HFns, ty),
        }
        self.src.h_fns(" ");
        self.src.h_fns(&name);
        self.src.h_fns("(");
        let mut params = Vec::new();
        for (i, (name, ty)) in func.params.iter().enumerate() {
            if i > 0 {
                self.src.h_fns(", ");
            }
            let pointer = is_arg_by_pointer(self.resolve, ty);
            let (print_ty, print_name) = (ty, to_c_ident(name));
            self.print_ty(SourceType::HFns, print_ty);
            self.src.h_fns(" ");
            if pointer {
                self.src.h_fns("*");
            }
            self.src.h_fns(&print_name);
            params.push((true && pointer, to_c_ident(name)));
        }
        let mut retptrs = Vec::new();
        let single_ret = ret.retptrs.len() == 1;
        for (i, ty) in ret.retptrs.iter().enumerate() {
            if i > 0 || func.params.len() > 0 {
                self.src.h_fns(", ");
            }
            self.print_ty(SourceType::HFns, ty);
            self.src.h_fns(" *");
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
            self.src.h_fns(&name);
            retptrs.push(name);
        }
        if func.params.len() == 0 && ret.retptrs.len() == 0 {
            self.src.h_fns("void");
        }
        self.src.h_fns(")");

        self.src.h_fns(";\n");

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
            _ => {
                ret.retptrs.extend(func.results.iter_types().cloned());
            }
        }
        return ret;
    }

    fn print_typedef_target(&mut self, id: TypeId, name: &str) {
        let ns = self.gen.owner_namespace(self.resolve, id).to_snake_case();
        let snake = name.to_snake_case();
        self.src.h_defs(&ns);
        self.src.h_defs("_");
        self.src.h_defs(&snake);
        self.src.h_defs("_t;\n");
        self.gen.names.insert(&format!("{ns}_{snake}_t")).unwrap();
    }

    fn print_ty(&mut self, stype: SourceType, ty: &Type) {
        self.gen
            .push_type_name(self.resolve, ty, self.src.src(stype).as_mut_string());
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

    fn finish_ty(&mut self, id: TypeId, orig_h_defs: wit_bindgen_core::Source) {
        let prev = self
            .gen
            .types
            .insert(id, mem::replace(&mut self.src.h_defs, orig_h_defs));
        assert!(prev.is_none());
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
    fn h_fns(&mut self, s: &str) {
        self.h_fns.push_str(s);
    }
    fn h_helpers(&mut self, s: &str) {
        self.h_helpers.push_str(s);
    }
    fn c_defs(&mut self, s: &str) {
        self.c_defs.push_str(s);
    }
    fn c_fns(&mut self, s: &str) {
        self.c_fns.push_str(s);
    }
    fn c_helpers(&mut self, s: &str) {
        self.c_helpers.push_str(s);
    }
    fn c_adapters(&mut self, s: &str) {
        self.c_adapters.push_str(s);
    }
}

trait SourceExt {
    fn as_source(&mut self) -> &mut wit_bindgen_core::Source;

    fn print_ty_name(&mut self, resolve: &Resolve, ty: &Type) {
        push_ty_name(resolve, ty, self.as_source().as_mut_string());
    }
}

impl SourceExt for wit_bindgen_core::Source {
    fn as_source(&mut self) -> &mut wit_bindgen_core::Source {
        self
    }
}

fn push_ty_name(resolve: &Resolve, ty: &Type, src: &mut String) {
    match ty {
        Type::Bool => src.push_str("bool"),
        Type::Char => src.push_str("char32"),
        Type::U8 => src.push_str("u8"),
        Type::S8 => src.push_str("s8"),
        Type::U16 => src.push_str("u16"),
        Type::S16 => src.push_str("s16"),
        Type::U32 => src.push_str("u32"),
        Type::S32 => src.push_str("s32"),
        Type::U64 => src.push_str("u64"),
        Type::S64 => src.push_str("s64"),
        Type::Float32 => src.push_str("float32"),
        Type::Float64 => src.push_str("float64"),
        Type::String => src.push_str("string"),
        Type::Id(id) => {
            let ty = &resolve.types[*id];
            if let Some(name) = &ty.name {
                return src.push_str(&name.to_snake_case());
            }
            match &ty.kind {
                TypeDefKind::Type(t) => push_ty_name(resolve, t, src),
                TypeDefKind::Record(_)
                | TypeDefKind::Flags(_)
                | TypeDefKind::Enum(_)
                | TypeDefKind::Variant(_)
                | TypeDefKind::Union(_) => {
                    unimplemented!()
                }
                TypeDefKind::Tuple(t) => {
                    src.push_str("tuple");
                    src.push_str(&t.types.len().to_string());
                    for ty in t.types.iter() {
                        src.push_str("_");
                        push_ty_name(resolve, ty, src);
                    }
                }
                TypeDefKind::Option(ty) => {
                    src.push_str("option_");
                    push_ty_name(resolve, ty, src);
                }
                TypeDefKind::Result(r) => {
                    src.push_str("result_");
                    push_optional_ty_name(resolve, r.ok.as_ref(), src);
                    src.push_str("_");
                    push_optional_ty_name(resolve, r.err.as_ref(), src);
                }
                TypeDefKind::List(t) => {
                    src.push_str("list_");
                    push_ty_name(resolve, t, src);
                }
                TypeDefKind::Future(t) => {
                    src.push_str("future_");
                    push_optional_ty_name(resolve, t.as_ref(), src);
                }
                TypeDefKind::Stream(s) => {
                    src.push_str("stream_");
                    push_optional_ty_name(resolve, s.element.as_ref(), src);
                    src.push_str("_");
                    push_optional_ty_name(resolve, s.end.as_ref(), src);
                }
                TypeDefKind::Unknown => unreachable!(),
            }
        }
    }

    fn push_optional_ty_name(resolve: &Resolve, ty: Option<&Type>, dst: &mut String) {
        match ty {
            Some(ty) => push_ty_name(resolve, ty, dst),
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

pub fn owns_anything(resolve: &Resolve, ty: &Type) -> bool {
    wit_bindgen_c::owns_anything(resolve, ty)
}

pub fn optional_owns_anything(resolve: &Resolve, ty: Option<&Type>) -> bool {
    wit_bindgen_c::optional_owns_anything(resolve, ty)
}

pub fn to_c_ident(name: &str) -> String {
    wit_bindgen_c::to_c_ident(name)
}
