use anyhow::{Context, Result, bail};
use std::{env::consts::DLL_EXTENSION, fmt::Write, path::PathBuf, process::Command};
use wit_bindgen_core::{
    Files, WorldGenerator, abi, make_external_symbol, symbol_extensions, uwriteln, wit_parser,
};

#[derive(Default)]
struct ImportLib {
    opts: Opts,
    src: wit_bindgen_core::Source,
    out_dir: Option<PathBuf>,
    /// External symbol names of everything the world exports.
    symbols: Vec<String>,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct Opts {
    /// Generate bindings for symmetric linking support (default).
    #[cfg_attr(feature = "clap", clap(long))]
    pub symmetric: bool,

    /// Additionally generate a forwarding library `lib<PREFIX><world>.so`.
    ///
    /// It defines every symbol the world exports with `PREFIX` prepended and
    /// forwards each call to the original symbol in `lib<world>.so`, which is
    /// looked up (via `dlopen`/`dlsym`) when the library is loaded. This lets
    /// a component that imports and exports the same interface (the bindings
    /// then name the imports `imp_...`) reach the next component in a chain
    /// without clashing with its own exports.
    #[cfg_attr(feature = "clap", clap(long))]
    pub forward_prefix: Option<String>,
}

impl Opts {
    pub fn build(&self, out_dir: &Option<PathBuf>) -> Box<dyn WorldGenerator> {
        let mut r = ImportLib::default();
        r.opts = self.clone();
        r.out_dir = out_dir.clone();
        uwriteln!(r.src, "use std::process::abort;");
        Box::new(r)
    }
}

impl ImportLib {
    fn export_func(
        &mut self,
        resolve: &wit_parser::Resolve,
        func: &wit_parser::Function,
        interface_name: Option<&wit_parser::WorldKey>,
    ) {
        // todo!()
        // println!("{func:?}");
        let core_module_name = interface_name.map(|s| resolve.name_world_key(s));
        //let export_name = func.legacy_core_export_name(core_module_name.as_deref());
        let ext_name = make_external_symbol(
            &core_module_name.unwrap_or("$root".into()),
            &func.name,
            abi::AbiVariant::GuestImport,
        ) + symbol_extensions(func);
        self.symbols.push(ext_name.clone());
        uwriteln!(self.src, "#[no_mangle]");
        uwriteln!(
            self.src,
            "pub extern \"C\" fn {}() {{ abort(); }}",
            ext_name
        );
    }
}

impl WorldGenerator for ImportLib {
    fn import_interface(
        &mut self,
        _resolve: &wit_parser::Resolve,
        _name: &wit_parser::WorldKey,
        _iface: wit_parser::InterfaceId,
        _files: &mut Files,
    ) -> Result<()> {
        Ok(())
    }

    fn export_interface(
        &mut self,
        resolve: &wit_parser::Resolve,
        name: &wit_parser::WorldKey,
        id: wit_parser::InterfaceId,
        _files: &mut Files,
    ) -> Result<()> {
        for (_name, func) in resolve.interfaces[id].functions.iter() {
            self.export_func(resolve, func, Some(name));
        }
        Ok(())
    }

    fn import_funcs(
        &mut self,
        _resolve: &wit_parser::Resolve,
        _world: wit_parser::WorldId,
        _funcs: &[(&str, &wit_parser::Function)],
        _files: &mut Files,
    ) -> Result<()> {
        Ok(())
    }

    fn export_funcs(
        &mut self,
        resolve: &wit_parser::Resolve,
        _world: wit_parser::WorldId,
        funcs: &[(&str, &wit_parser::Function)],
        _files: &mut Files,
    ) -> Result<()> {
        for (_name, func) in funcs.iter() {
            self.export_func(resolve, func, None);
        }
        Ok(())
    }

    fn import_types(
        &mut self,
        _resolve: &wit_parser::Resolve,
        _world: wit_parser::WorldId,
        _types: &[(&str, wit_parser::TypeId)],
        _files: &mut Files,
    ) -> Result<()> {
        Ok(())
    }

    fn finish(
        &mut self,
        resolve: &wit_parser::Resolve,
        world: wit_parser::WorldId,
        _files: &mut Files,
    ) -> Result<()> {
        let worldname = resolve
            .worlds
            .get(world)
            .map(|w| w.name.clone())
            .unwrap_or("world".to_string());
        let rustname = format!("{worldname}_importlib.rs",);
        let rustpath = match &self.out_dir {
            Some(path) => path.join(rustname),
            None => rustname.into(),
        };
        let libname = format!("lib{worldname}.{}", DLL_EXTENSION);
        let libpath = match &self.out_dir {
            Some(path) => path.join(libname),
            None => libname.into(),
        };
        eprintln!("Generating {:?}", libpath);
        std::fs::write(&rustpath, self.src.as_bytes())
            .with_context(|| format!("failed to write {:?}", rustpath))?;

        compile_cdylib(&rustpath, &libpath)?;

        if let Some(prefix) = &self.opts.forward_prefix {
            let fwdname = format!("{prefix}{worldname}_forward.rs");
            let fwdpath = match &self.out_dir {
                Some(path) => path.join(fwdname),
                None => fwdname.into(),
            };
            let fwdlib = format!("lib{prefix}{worldname}.{}", DLL_EXTENSION);
            let fwdlibpath = match &self.out_dir {
                Some(path) => path.join(fwdlib),
                None => fwdlib.into(),
            };
            let target = format!("lib{worldname}.{DLL_EXTENSION}");
            std::fs::write(&fwdpath, forward_source(&self.symbols, prefix, &target))
                .with_context(|| format!("failed to write {:?}", fwdpath))?;
            compile_cdylib(&fwdpath, &fwdlibpath)?;
        }
        Ok(())
    }
}

fn compile_cdylib(rustpath: &std::path::Path, libpath: &std::path::Path) -> Result<()> {
    let mut cmd = Command::new("rustc");
    cmd.arg("--crate-type=cdylib");
    cmd.arg(rustpath);
    cmd.arg("-O");
    cmd.arg("-o");
    cmd.arg(libpath);
    cmd.arg("-C");
    cmd.arg("strip=debuginfo");
    let output = cmd.output().with_context(|| "failed to run rustc")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("rustc failed:\n{}", stderr);
    }
    Ok(())
}

/// Source of a library defining `<prefix><symbol>` as a tail call to `symbol`
/// of the library `target` (resolved with `dlsym` on that library only).
fn forward_source(symbols: &[String], prefix: &str, target: &str) -> String {
    let mut src = String::new();
    let n = symbols.len();
    uwriteln!(src, "use std::ffi::{{c_char, c_int, c_void}};");
    uwriteln!(
        src,
        "static mut SLOTS: [usize; {}] = [0; {}];",
        n.max(1),
        n.max(1)
    );
    uwriteln!(src, "static NAMES: [&[u8]; {}] = [", n);
    for s in symbols {
        uwriteln!(src, "    b\"{s}\\0\",");
    }
    uwriteln!(src, "];");
    uwriteln!(src, "#[link(name = \"dl\")]");
    uwriteln!(src, "unsafe extern \"C\" {{");
    uwriteln!(
        src,
        "    fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;"
    );
    uwriteln!(
        src,
        "    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;"
    );
    uwriteln!(src, "}}");
    uwriteln!(src, "extern \"C\" fn init() {{");
    uwriteln!(src, "    unsafe {{");
    uwriteln!(src, "        // RTLD_LAZY | RTLD_LOCAL");
    uwriteln!(
        src,
        "        let h = dlopen(b\"{target}\\0\".as_ptr().cast(), 1);"
    );
    uwriteln!(src, "        if h.is_null() {{ std::process::abort(); }}");
    uwriteln!(src, "        for (i, name) in NAMES.iter().enumerate() {{");
    uwriteln!(src, "            let p = dlsym(h, name.as_ptr().cast());");
    uwriteln!(
        src,
        "            if p.is_null() {{ std::process::abort(); }}"
    );
    uwriteln!(
        src,
        "            (&raw mut SLOTS as *mut usize).add(i).write(p as usize);"
    );
    uwriteln!(src, "        }}");
    uwriteln!(src, "    }}");
    uwriteln!(src, "}}");
    uwriteln!(src, "#[used]");
    uwriteln!(src, "#[unsafe(link_section = \".init_array\")]");
    uwriteln!(src, "static INIT: extern \"C\" fn() = init;");
    for (i, s) in symbols.iter().enumerate() {
        let off = i * 8;
        // naked functions so that the symbols are exported like any
        // `#[no_mangle]` function
        uwriteln!(src, "#[unsafe(no_mangle)]");
        uwriteln!(src, "#[unsafe(naked)]");
        uwriteln!(src, "pub extern \"C\" fn {prefix}{s}() {{");
        uwriteln!(src, "    #[cfg(target_arch = \"x86_64\")]");
        uwriteln!(
            src,
            "    std::arch::naked_asm!(\"jmp *{{slots}}+{off}(%rip)\", slots = sym SLOTS, options(att_syntax));"
        );
        uwriteln!(src, "    #[cfg(target_arch = \"aarch64\")]");
        uwriteln!(
            src,
            "    std::arch::naked_asm!(\"adrp x16, {{slots}}+{off}\", \"ldr x16, [x16, :lo12:{{slots}}+{off}]\", \"br x16\", slots = sym SLOTS);"
        );
        uwriteln!(src, "}}");
    }
    src
}
