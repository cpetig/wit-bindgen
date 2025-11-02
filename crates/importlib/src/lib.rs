use anyhow::{bail, Context, Result};
use std::{env::consts::DLL_EXTENSION, fmt::Write, path::PathBuf, process::Command};
use wit_bindgen_core::{abi, make_external_symbol, uwriteln, wit_parser, Files, WorldGenerator};

#[derive(Default)]
struct ImportLib {
    opts: Opts,
    src: wit_bindgen_core::Source,
    out_dir: Option<PathBuf>,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct Opts {
    /// Generate bindings for symmetric linking support (default).
    #[cfg_attr(feature = "clap", clap(long))]
    pub symmetric: bool,
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
        );
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
    ) {
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
    ) {
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

        let mut cmd = Command::new("rustc");
        cmd.arg("--crate-type=cdylib");
        cmd.arg(&rustpath);
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
}
