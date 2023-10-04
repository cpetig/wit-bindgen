use heck::{ToShoutySnakeCase, ToSnakeCase};
use std::{collections::HashMap, fmt::Write};
use wit_bindgen_core::{
    uwrite, uwriteln,
    wit_parser::{Function, InterfaceId, Resolve, TypeId, TypeOwner, WorldId, WorldKey},
    Files, Source, WorldGenerator,
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
struct Cpp {
    opts: Opts,
    c_src: Source,
    h_src: Source,
    dependencies: Includes,
    includes: Vec<String>,
    host_functions: HashMap<String, Vec<HostFunction>>,
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
}

impl WorldGenerator for Cpp {
    fn import_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        iface: InterfaceId,
        files: &mut Files,
    ) {
        //todo!()
    }

    fn export_interface(
        &mut self,
        resolve: &Resolve,
        name: &WorldKey,
        iface: InterfaceId,
        files: &mut Files,
    ) -> anyhow::Result<()> {
        todo!()
    }

    fn import_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        files: &mut Files,
    ) {
        todo!()
    }

    fn export_funcs(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        funcs: &[(&str, &Function)],
        files: &mut Files,
    ) -> anyhow::Result<()> {
        todo!()
    }

    fn import_types(
        &mut self,
        resolve: &Resolve,
        world: WorldId,
        types: &[(&str, TypeId)],
        files: &mut Files,
    ) {
        todo!()
    }

    fn finish(&mut self, resolve: &Resolve, world_id: WorldId, files: &mut Files) {
        let world = &resolve.worlds[world_id];
        let snake = world.name.to_snake_case();

        let mut h_str = Source::default();
        let mut c_str = Source::default();
        let mut h_namespace = Vec::new();

        let version = env!("CARGO_PKG_VERSION");
        uwriteln!(
            h_str,
            "// Generated by `wit-bindgen` {version}. DO NOT EDIT!"
        );

        if !self.opts.host {
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
        if !self.opts.host && self.dependencies.needs_resources {
            self.include("<cassert>");
        }

        for include in self.includes.iter() {
            uwriteln!(h_str, "#include {include}");
        }

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
                let namespace = namespace(resolve, TypeOwner::World(world_id));
                change_namespace(&mut h_namespace, &namespace, &mut h_str);
                if self.opts.host {
                    uwriteln!(
                        h_str,
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
                        h_str,
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
        }
        change_namespace(&mut h_namespace, &Vec::default(), &mut h_str);

        c_str.push_str(&self.c_src);
        h_str.push_str(&self.h_src);
        // c_str.push_str(&self.src.c_fns);

        // if self.src.h_defs.len() > 0 {
        //     h_str.push_str(&self.src.h_defs);
        // }

        // h_str.push_str(&self.src.h_fns);

        uwriteln!(c_str, "\n// Component Adapters");

        // c_str.push_str(&self.src.c_adapters);

        if !self.opts.short_cut && self.opts.host {
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

        if !self.opts.host {
            files.push(&format!("{snake}.cpp"), c_str.as_bytes());
            files.push(&format!("{snake}_cpp.h"), h_str.as_bytes());
        } else {
            files.push(&format!("{snake}_host.cpp"), c_str.as_bytes());
            files.push(&format!("{snake}_cpp_host.h"), h_str.as_bytes());
        }
    }
}

// determine namespace
fn namespace(resolve: &Resolve, owner: TypeOwner) -> Vec<String> {
    let mut result = Vec::default();
    match owner {
        TypeOwner::World(w) => result.push(resolve.worlds[w].name.to_snake_case()),
        TypeOwner::Interface(i) => {
            let iface = &resolve.interfaces[i];
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

fn change_namespace(current: &mut Vec<String>, target: &Vec<String>, output: &mut Source) {
    let mut same = 0;
    // itertools::fold_while?
    for (a, b) in current.iter().zip(target.iter()) {
        if a == b {
            same += 1;
        } else {
            break;
        }
    }
    for i in same..current.len() {
        uwrite!(output, "}}");
    }
    current.truncate(same);
    for i in target.iter().skip(same) {
        uwrite!(output, "namespace {} {{", i);
        current.push(i.clone());
    }
}
