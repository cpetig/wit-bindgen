use heck::{ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use std::{collections::HashMap, fmt::Write};
use wit_bindgen_core::{
    abi::{Bindgen, WasmType},
    abi::{self, AbiVariant, LiftLower},
    uwrite, uwriteln,
    wit_parser::{
        Function, FunctionKind, Handle, InterfaceId, Resolve, TypeDefKind, TypeId, TypeOwner,
        WorldId, WorldKey,
    },
    Files, InterfaceGenerator, Source, WorldGenerator,
};

mod wamr;

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

#[derive(Default)]
struct SourceWithState {
    src: Source,
    namespace: Vec<String>,
}

#[derive(Default)]
struct Cpp {
    opts: Opts,
    c_src: SourceWithState,
    h_src: SourceWithState,
    dependencies: Includes,
    includes: Vec<String>,
    host_functions: HashMap<String, Vec<HostFunction>>,
    world: String,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct Opts {
    /// Generate host bindings
    #[cfg_attr(feature = "clap", arg(long, default_value_t = bool::default()))]
    pub host: bool,
    /// Generate code for directly linking to guest code
    #[cfg_attr(feature = "clap", arg(long, default_value_t = bool::default()))]
    pub short_cut: bool,
}

impl Opts {
    pub fn build(self) -> Box<dyn WorldGenerator> {
        let mut r = Cpp::new();
        r.opts = self;
        Box::new(r)
    }
}

impl Cpp {
    fn new() -> Cpp {
        Cpp::default()
    }

    fn include(&mut self, s: &str) {
        self.includes.push(s.to_string());
    }

    fn interface<'a>(
        &'a mut self,
        resolve: &'a Resolve,
        name: &'a Option<&'a WorldKey>,
        in_import: bool,
    ) -> CppInterfaceGenerator<'a> {
        CppInterfaceGenerator {
            src: Source::default(),
            gen: self,
            resolve,
            interface: None,
            name,
            // public_anonymous_types: BTreeSet::new(),
            in_import,
            // export_funcs: Vec::new(),
        }
    }
}

impl WorldGenerator for Cpp {
    fn preprocess(&mut self, resolve: &Resolve, world: WorldId) {
        let name = &resolve.worlds[world].name;
        self.world = name.to_string();
        //        self.sizes.fill(resolve);
    }

    fn import_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        id: InterfaceId,
        _files: &mut Files,
    ) {
        let binding = Some(name);
        let mut gen = self.interface(resolve, &binding, true);
        gen.interface = Some(id);
        // if self.gen.interfaces_with_types_printed.insert(id) {
        gen.types(id);
        // }

        for (_name, func) in resolve.interfaces[id].functions.iter() {
            gen.generate_guest_import(func);
        }
        // gen.finish();
    }

    fn export_interface(
        &mut self,
        _resolve: &Resolve,
        _name: &WorldKey,
        _iface: InterfaceId,
        _files: &mut Files,
    ) -> anyhow::Result<()> {
        todo!()
    }

    fn import_funcs(
        &mut self,
        _resolve: &Resolve,
        _world: WorldId,
        _funcs: &[(&str, &Function)],
        _files: &mut Files,
    ) {
        todo!()
    }

    fn export_funcs(
        &mut self,
        _resolve: &Resolve,
        _world: WorldId,
        _funcs: &[(&str, &Function)],
        _files: &mut Files,
    ) -> anyhow::Result<()> {
        todo!()
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

    fn finish(&mut self, resolve: &Resolve, world_id: WorldId, files: &mut Files) {
        let world = &resolve.worlds[world_id];
        let snake = world.name.to_snake_case();

        let mut h_str = SourceWithState::default();
        let mut c_str = SourceWithState::default();

        let version = env!("CARGO_PKG_VERSION");
        uwriteln!(
            h_str.src,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );

        if !self.opts.host {
            uwrite!(
                h_str.src,
                "#ifndef __CPP_GUEST_BINDINGS_{0}_H
                #define __CPP_GUEST_BINDINGS_{0}_H\n",
                world.name.to_shouty_snake_case(),
            );
        } else {
            uwrite!(
                h_str.src,
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
        if !self.opts.host && self.dependencies.needs_resources {
            self.include("<cassert>");
        }

        for include in self.includes.iter() {
            uwriteln!(h_str.src, "#include {include}");
        }

        uwriteln!(
            c_str.src,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );
        if !self.opts.host {
            uwriteln!(c_str.src, "#include \"{snake}_cpp.h\"");
        } else {
            uwriteln!(c_str.src, "#include \"{snake}_cpp_host.h\"");
            if !self.opts.short_cut {
                uwriteln!(
                    c_str.src,
                    "#include <wasm_export.h> // wasm-micro-runtime header"
                );

                if c_str.src.len() > 0 {
                    c_str.src.push_str("\n");
                }
                if self.dependencies.needs_guest_alloc {
                    uwriteln!(
                        c_str.src,
                        "int32_t guest_alloc(wasm_exec_env_t exec_env, uint32_t size);"
                    );
                }
            }
        }

        if self.dependencies.needs_resources {
            let namespace = namespace(resolve, &TypeOwner::World(world_id));
            h_str.change_namespace(&namespace);
            if self.opts.host {
                uwriteln!(
                    h_str.src,
                    "class {RESOURCE_BASE_CLASS_NAME} {{
                            public:
                            int32_t id;
                            virtual ~{RESOURCE_BASE_CLASS_NAME}();
                            {RESOURCE_BASE_CLASS_NAME}();
                            static {RESOURCE_BASE_CLASS_NAME}* lookup_resource(int32_t id);
                        }}; 
                        template <typename T> struct {OWNED_CLASS_NAME} {{
                            T *ptr;
                        }};"
                );
            } else {
                // somehow spaces get removed, newlines remain (problem occurs before const&)
                uwriteln!(
                    h_str.src,
                    "class {RESOURCE_BASE_CLASS_NAME} {{
                            static const int32_t invalid = -1;
                            protected:
                            int32_t handle;
                            public:
                            {RESOURCE_BASE_CLASS_NAME}(int32_t h=invalid) : handle(h) {{}}
                            {RESOURCE_BASE_CLASS_NAME}({RESOURCE_BASE_CLASS_NAME}&&r) 
                                : handle(r.handle) {{ 
                                    r.handle=invalid; 
                            }}
                            {RESOURCE_BASE_CLASS_NAME}({RESOURCE_BASE_CLASS_NAME} 
                                const&) = delete;
                            void set_handle(int32_t h) {{ handle=h; }}
                            int32_t get_handle() const {{ return handle; }}
                            int32_t into_raw() {{
                                int32_t h= handle;
                                handle= invalid;
                                return h;
                            }}
                            {RESOURCE_BASE_CLASS_NAME}& operator=({RESOURCE_BASE_CLASS_NAME}&&r) {{
                                assert(handle<0);
                                handle= r.handle;
                                r.handle= invalid;
                                return *this;
                            }}
                            {RESOURCE_BASE_CLASS_NAME}& operator=({RESOURCE_BASE_CLASS_NAME} 
                                const&r) = delete;
                            }};"
                );
            }
        }
        h_str.change_namespace(&Vec::default());

        self.c_src.change_namespace(&Vec::default());
        c_str.src.push_str(&self.c_src.src);
        self.h_src.change_namespace(&Vec::default());
        h_str.src.push_str(&self.h_src.src);
        // c_str.push_str(&self.src.c_fns);

        // if self.src.h_defs.len() > 0 {
        //     h_str.push_str(&self.src.h_defs);
        // }

        // h_str.push_str(&self.src.h_fns);

        uwriteln!(c_str.src, "\n// Component Adapters");

        // c_str.push_str(&self.src.c_adapters);

        if !self.opts.short_cut && self.opts.host {
            uwriteln!(
                h_str.src,
                "extern \"C\" void register_{}();",
                world.name.to_snake_case()
            );
            uwriteln!(
                c_str.src,
                "void register_{}() {{",
                world.name.to_snake_case()
            );
            for i in self.host_functions.iter() {
                uwriteln!(
                    c_str.src,
                    "  static NativeSymbol {}_funs[] = {{",
                    i.0.replace(":", "_").to_snake_case()
                );
                for f in i.1.iter() {
                    uwriteln!(
                        c_str.src,
                        "    {{ \"{}\", (void*){}, \"{}\", nullptr }},",
                        f.wasm_name,
                        f.host_name,
                        f.wamr_signature
                    );
                }
                uwriteln!(c_str.src, "  }};");
            }
            for i in self.host_functions.iter() {
                uwriteln!(c_str.src, "  wasm_runtime_register_natives(\"{}\", {1}_funs, sizeof({1}_funs)/sizeof(NativeSymbol));", i.0, i.0.replace(":", "_").to_snake_case());
            }
            uwriteln!(c_str.src, "}}");
        }

        uwriteln!(
            h_str.src,
            "
            #endif"
        );

        if !self.opts.host {
            files.push(&format!("{snake}.cpp"), c_str.src.as_bytes());
            files.push(&format!("{snake}_cpp.h"), h_str.src.as_bytes());
        } else {
            files.push(&format!("{snake}_host.cpp"), c_str.src.as_bytes());
            files.push(&format!("{snake}_cpp_host.h"), h_str.src.as_bytes());
        }
    }
}

// determine namespace
fn namespace(resolve: &Resolve, owner: &TypeOwner) -> Vec<String> {
    let mut result = Vec::default();
    match owner {
        TypeOwner::World(w) => result.push(resolve.worlds[*w].name.to_snake_case()),
        TypeOwner::Interface(i) => {
            let iface = &resolve.interfaces[*i];
            let pkg = &resolve.packages[iface.package.unwrap()];
            result.push(pkg.name.namespace.to_snake_case());
            result.push(pkg.name.name.to_snake_case());
            if let Some(name) = &iface.name {
                result.push(name.to_snake_case());
            }
        }
        TypeOwner::None => (),
    }
    result
}

impl SourceWithState {
    fn change_namespace(&mut self, target: &Vec<String>) {
        let mut same = 0;
        // itertools::fold_while?
        for (a, b) in self.namespace.iter().zip(target.iter()) {
            if a == b {
                same += 1;
            } else {
                break;
            }
        }
        for _i in same..self.namespace.len() {
            uwrite!(self.src, "}}");
        }
        if same != self.namespace.len() {
            // finish closing brackets by a newline
            uwriteln!(self.src, "");
        }
        self.namespace.truncate(same);
        for i in target.iter().skip(same) {
            uwrite!(self.src, "namespace {} {{", i);
            self.namespace.push(i.clone());
        }
    }
}

struct CppInterfaceGenerator<'a> {
    src: Source,
    gen: &'a mut Cpp,
    resolve: &'a Resolve,
    interface: Option<InterfaceId>,
    name: &'a Option<&'a WorldKey>,
    //    public_anonymous_types: BTreeSet<TypeId>,
    in_import: bool,
    //    export_funcs: Vec<(String, String)>,
}

impl CppInterfaceGenerator<'_> {
    fn types(&mut self, iface: InterfaceId) {
        let iface = &self.resolve().interfaces[iface];
        for (name, id) in iface.types.iter() {
            self.define_type(name, *id);
        }
    }

    fn define_type(&mut self, name: &str, id: TypeId) {
        let ty = &self.resolve().types[id];
        match &ty.kind {
            TypeDefKind::Record(record) => self.type_record(id, name, record, &ty.docs),
            TypeDefKind::Resource => self.type_resource(id, name, &ty.docs),
            TypeDefKind::Flags(flags) => self.type_flags(id, name, flags, &ty.docs),
            TypeDefKind::Tuple(tuple) => self.type_tuple(id, name, tuple, &ty.docs),
            TypeDefKind::Enum(enum_) => self.type_enum(id, name, enum_, &ty.docs),
            TypeDefKind::Variant(variant) => self.type_variant(id, name, variant, &ty.docs),
            TypeDefKind::Option(t) => self.type_option(id, name, t, &ty.docs),
            TypeDefKind::Result(r) => self.type_result(id, name, r, &ty.docs),
            TypeDefKind::List(t) => self.type_list(id, name, t, &ty.docs),
            TypeDefKind::Type(t) => self.type_alias(id, name, t, &ty.docs),
            TypeDefKind::Future(_) => todo!("generate for future"),
            TypeDefKind::Stream(_) => todo!("generate for stream"),
            TypeDefKind::Handle(_) => todo!("generate for handle"),
            TypeDefKind::Unknown => unreachable!(),
        }
    }

    fn print_signature(&mut self, func: &Function) -> Vec<String> {
        // Vec::default()
        uwriteln!(self.gen.h_src.src, "void {}(...);", func.name);
        uwriteln!(self.gen.c_src.src, "void {}(...)", func.name);
        vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()]
    }

    fn generate_guest_import(&mut self, func: &Function) {
        //        let mut sig = FnSig::default();
        let params = self.print_signature(func);
        self.gen.c_src.src.push_str("{\n");
        let mut f = FunctionBindgen::new(self, params);
        abi::call(
            f.gen.resolve,
            AbiVariant::GuestImport,
            LiftLower::LowerArgsLiftResults,
            func,
            &mut f,
        );
        self.gen.c_src.src.push_str("}\n");
    }
}

impl<'a> wit_bindgen_core::InterfaceGenerator<'a> for CppInterfaceGenerator<'a> {
    fn resolve(&self) -> &'a Resolve {
        self.resolve
    }

    fn type_record(
        &mut self,
        _id: TypeId,
        _name: &str,
        _record: &wit_bindgen_core::wit_parser::Record,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_resource(&mut self, id: TypeId, name: &str, docs: &wit_bindgen_core::wit_parser::Docs) {
        let type_ = &self.resolve.types[id];
        if let TypeOwner::Interface(intf) = type_.owner {
            let mut world_name = self.gen.world.to_snake_case();
            world_name.push_str("::");
            let funcs = self.resolve.interfaces[intf].functions.values();
            let namespc = namespace(self.resolve, &type_.owner);
            self.gen.h_src.change_namespace(&namespc);

            self.gen.dependencies.needs_resources = true;
            let pascal = name.to_upper_camel_case();

            let derive = format!(" : public {world_name}{RESOURCE_BASE_CLASS_NAME}");
            uwriteln!(self.gen.h_src.src, "class {pascal}{derive} {{\n");
            if self.gen.opts.host {
                uwriteln!(
                    self.gen.h_src.src,
                    "  // private implementation data\n  struct pImpl;\n  pImpl * p_impl;\n"
                );
            } else {
                //gen.src.h_defs("  int32_t handle;\nbool owned;\n");
            }
            uwriteln!(self.gen.h_src.src, "public:\n");
            // destructor
            uwriteln!(self.gen.h_src.src, "~{pascal}();\n");
            for func in funcs {
                // Some(name),
                self.generate_guest_import(func);
            }

            if !self.gen.opts.host {
                // consuming constructor from handle (bindings)
                uwriteln!(
                    self.gen.h_src.src,
                    "{pascal}({world_name}{RESOURCE_BASE_CLASS_NAME}&&);\n"
                );
                uwriteln!(self.gen.h_src.src, "{pascal}({pascal}&&) = default;\n");
            }
            uwriteln!(self.gen.h_src.src, "}};\n");

            if self.gen.opts.host {
                let iface = &self.resolve.interfaces[intf];
                let pkg = &self.resolve.packages[iface.package.unwrap()];
                let mut interface_name = pkg.name.namespace.to_snake_case();
                interface_name.push_str("_");
                interface_name.push_str(&pkg.name.name.to_snake_case());
                interface_name.push_str("_");
                interface_name.push_str(&iface.name.as_ref().unwrap().to_snake_case());
                let resource = self.resolve.types[id].name.as_deref().unwrap();
                let resource_snake = resource.to_snake_case();
                let host_name = format!("host_{interface_name}_resource_drop_{resource_snake}");
                let wasm_name = format!("[resource-drop]{resource}");
                uwriteln!(self.gen.c_src.src, "static void {host_name}(wasm_exec_env_t exec_env, int32_t self) {{\n  delete {world_name}{RESOURCE_BASE_CLASS_NAME}::lookup_resource(self);\n}}\n", );
                // let remember = HostFunction {
                //     wasm_name,
                //     wamr_signature: "(i)".into(),
                //     host_name,
                // };
                // let module_name = self.resolve.name_world_key(name);
                // self.gen
                //     .host_functions
                //     .entry(module_name)
                //     .and_modify(|v| v.push(remember.clone()))
                //     .or_insert(vec![remember]);
            }

            // let entry = self
            //     .gen
            //     .resources
            //     .entry(dealias(self.resolve, id))
            //     .or_default();
            // if !self.in_import {
            //     entry.direction = Direction::Export;
            // }
            // entry.docs = docs.clone();
        }
    }

    fn type_flags(
        &mut self,
        _id: TypeId,
        _name: &str,
        _flags: &wit_bindgen_core::wit_parser::Flags,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_tuple(
        &mut self,
        _id: TypeId,
        _name: &str,
        _flags: &wit_bindgen_core::wit_parser::Tuple,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_variant(
        &mut self,
        _id: TypeId,
        _name: &str,
        _variant: &wit_bindgen_core::wit_parser::Variant,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_option(
        &mut self,
        _id: TypeId,
        _name: &str,
        _payload: &wit_bindgen_core::wit_parser::Type,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_result(
        &mut self,
        _id: TypeId,
        _name: &str,
        _result: &wit_bindgen_core::wit_parser::Result_,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_enum(
        &mut self,
        _id: TypeId,
        _name: &str,
        _enum_: &wit_bindgen_core::wit_parser::Enum,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_alias(
        &mut self,
        _id: TypeId,
        _name: &str,
        _ty: &wit_bindgen_core::wit_parser::Type,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_list(
        &mut self,
        _id: TypeId,
        _name: &str,
        _ty: &wit_bindgen_core::wit_parser::Type,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }

    fn type_builtin(
        &mut self,
        _id: TypeId,
        _name: &str,
        _ty: &wit_bindgen_core::wit_parser::Type,
        _docs: &wit_bindgen_core::wit_parser::Docs,
    ) {
        todo!()
    }
}

struct FunctionBindgen<'a, 'b> {
    gen: &'b mut CppInterfaceGenerator<'a>,
    params: Vec<String>,
}

impl<'a, 'b> FunctionBindgen<'a, 'b> {
    fn new(gen: &'b mut CppInterfaceGenerator<'a>, params: Vec<String>) -> Self {
        Self { gen, params }
    }
}

impl<'a, 'b> Bindgen for FunctionBindgen<'a, 'b> {
    type Operand = String;

    fn emit(
        &mut self,
        resolve: &Resolve,
        inst: &wit_bindgen_core::abi::Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        let mut top_as = |cvt: &str| {
            results.push(format!("({cvt}({}))", operands.pop().unwrap()));
        };

        //todo!()
        match inst {
            abi::Instruction::GetArg { nth } => results.push(self.params[*nth].clone()),
            abi::Instruction::I32Const { val } => results.push(format!("(int32_t({}))", val)),
            abi::Instruction::Bitcasts { casts } => todo!(),
            abi::Instruction::ConstZero { tys } => {
                for ty in tys.iter() {
                    match ty {
                        WasmType::I32 => results.push("int32_t(0)".to_string()),
                        WasmType::I64 => results.push("int64_t(0)".to_string()),
                        WasmType::F32 => results.push("0.0f".to_string()),
                        WasmType::F64 => results.push("0.0".to_string()),
                    }
                }
            }
            abi::Instruction::I32Load { offset } => todo!(),
            abi::Instruction::I32Load8U { offset } => todo!(),
            abi::Instruction::I32Load8S { offset } => todo!(),
            abi::Instruction::I32Load16U { offset } => todo!(),
            abi::Instruction::I32Load16S { offset } => todo!(),
            abi::Instruction::I64Load { offset } => todo!(),
            abi::Instruction::F32Load { offset } => todo!(),
            abi::Instruction::F64Load { offset } => todo!(),
            abi::Instruction::I32Store { offset } => todo!(),
            abi::Instruction::I32Store8 { offset } => todo!(),
            abi::Instruction::I32Store16 { offset } => todo!(),
            abi::Instruction::I64Store { offset } => todo!(),
            abi::Instruction::F32Store { offset } => todo!(),
            abi::Instruction::F64Store { offset } => todo!(),
            abi::Instruction::I32FromChar
            | abi::Instruction::I32FromBool
            | abi::Instruction::I32FromU8
            | abi::Instruction::I32FromS8
            | abi::Instruction::I32FromU16
            | abi::Instruction::I32FromS16
            | abi::Instruction::I32FromU32
            | abi::Instruction::I32FromS32 => top_as("int32_t"),
            abi::Instruction::I64FromU64 | abi::Instruction::I64FromS64 => top_as("int64_t"),
            abi::Instruction::F32FromFloat32 => todo!(),
            abi::Instruction::F64FromFloat64 => todo!(),
            abi::Instruction::S8FromI32 => todo!(),
            abi::Instruction::U8FromI32 => todo!(),
            abi::Instruction::S16FromI32 => todo!(),
            abi::Instruction::U16FromI32 => todo!(),
            abi::Instruction::S32FromI32 => todo!(),
            abi::Instruction::U32FromI32 => top_as("uint32_t"),
            abi::Instruction::S64FromI64 => todo!(),
            abi::Instruction::U64FromI64 => todo!(),
            abi::Instruction::CharFromI32 => todo!(),
            abi::Instruction::Float32FromF32 => todo!(),
            abi::Instruction::Float64FromF64 => todo!(),
            abi::Instruction::BoolFromI32 => top_as("bool"),
            abi::Instruction::ListCanonLower { element, realloc } => todo!(),
            abi::Instruction::StringLower { realloc } => todo!(),
            abi::Instruction::ListLower { element, realloc } => todo!(),
            abi::Instruction::ListCanonLift { element, ty } => todo!(),
            abi::Instruction::StringLift => todo!(),
            abi::Instruction::ListLift { element, ty } => todo!(),
            abi::Instruction::IterElem { element } => todo!(),
            abi::Instruction::IterBasePointer => todo!(),
            abi::Instruction::RecordLower { record, name, ty } => todo!(),
            abi::Instruction::RecordLift { record, name, ty } => todo!(),
            abi::Instruction::HandleLower { handle, name, ty } => {
                let op = &operands[0];
                results.push(format!("({op}).into_handle()"));
            }
            abi::Instruction::HandleLift { handle, name, ty } => {
                let op = &operands[0];
                // let (prefix, resource, _owned) = match handle {
                //     Handle::Borrow(resource) => ("&", resource, false),
                //     Handle::Own(resource) => ("", resource, true),
                // };
                // let resource = dealias(resolve, *resource);

                results.push(op.clone());
            }
            abi::Instruction::TupleLower { tuple, ty } => todo!(),
            abi::Instruction::TupleLift { tuple, ty } => todo!(),
            abi::Instruction::FlagsLower { flags, name, ty } => todo!(),
            abi::Instruction::FlagsLift { flags, name, ty } => todo!(),
            abi::Instruction::VariantPayloadName => todo!(),
            abi::Instruction::VariantLower {
                variant,
                name,
                ty,
                results,
            } => todo!(),
            abi::Instruction::VariantLift { variant, name, ty } => todo!(),
            abi::Instruction::EnumLower { enum_, name, ty } => todo!(),
            abi::Instruction::EnumLift { enum_, name, ty } => todo!(),
            abi::Instruction::OptionLower {
                payload,
                ty,
                results,
            } => todo!(),
            abi::Instruction::OptionLift { payload, ty } => todo!(),
            abi::Instruction::ResultLower {
                result,
                ty,
                results,
            } => todo!(),
            abi::Instruction::ResultLift { result, ty } => todo!(),
            abi::Instruction::CallWasm { name, sig } => {
                let func = "test"; //self.declare_import(
                                   //                    self.gen.wasm_import_module.unwrap(),
                                   //     name,
                                   //     &sig.params,
                                   //     &sig.results,
                                   // );

                // ... then call the function with all our operands
                if sig.results.len() > 0 {
                    self.gen.gen.c_src.src.push_str("auto ret = ");
                    results.push("ret".to_string());
                }
                self.gen.gen.c_src.src.push_str(&func);
                self.gen.gen.c_src.src.push_str("(");
                self.gen.gen.c_src.src.push_str(&operands.join(", "));
                self.gen.gen.c_src.src.push_str(");\n");
            }
            abi::Instruction::CallInterface { func } => todo!(),
            abi::Instruction::Return { amt, func } => {
                match amt {
                    0 => {}
                    1 => {
                        match &func.kind {
                            FunctionKind::Constructor(_) => {
                                // strange but works
                                self.gen.gen.c_src.src.push_str("this->handle = ");
                            }
                            _ => self.gen.gen.c_src.src.push_str("return "),
                        }
                        self.gen.gen.c_src.src.push_str(&operands[0]);
                        self.gen.gen.c_src.src.push_str(";\n");
                    }
                    _ => todo!(),
                }
            }
            abi::Instruction::Malloc {
                realloc,
                size,
                align,
            } => todo!(),
            abi::Instruction::GuestDeallocate { size, align } => todo!(),
            abi::Instruction::GuestDeallocateString => todo!(),
            abi::Instruction::GuestDeallocateList { element } => todo!(),
            abi::Instruction::GuestDeallocateVariant { blocks } => todo!(),
        }
    }

    fn return_pointer(&mut self, size: usize, align: usize) -> Self::Operand {
        todo!()
    }

    fn push_block(&mut self) {
        todo!()
    }

    fn finish_block(&mut self, operand: &mut Vec<Self::Operand>) {
        todo!()
    }

    fn sizes(&self) -> &wit_bindgen_core::wit_parser::SizeAlign {
        todo!()
    }

    fn is_list_canonical(
        &self,
        resolve: &Resolve,
        element: &wit_bindgen_core::wit_parser::Type,
    ) -> bool {
        todo!()
    }
}
