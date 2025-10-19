use anyhow::Result;
use wit_bindgen_core::WorldGenerator;

#[derive(Default)]
struct ImportLib {
    opts: Opts,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct Opts {
    /// Generate bindings for symmetric linking support (default).
    #[cfg_attr(feature = "clap", clap(long))]
    pub symmetric: bool,
}

impl Opts {
    pub fn build(&self) -> Box<dyn WorldGenerator> {
        let mut r = ImportLib::default();
        r.opts = self.clone();
        Box::new(r)
    }
}

impl WorldGenerator for ImportLib {
    // type Options = Opts;

    // fn generate(
    //     &self,
    //     _world: &wit_parser::World,
    //     _opts: &Self::Opts,
    //     _files: &mut wit_bindgen_core::Files,
    // ) -> anyhow::Result<()> {
    //     Ok(())
    // }

    fn import_interface(
        &mut self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _name: &wit_bindgen_core::wit_parser::WorldKey,
        _iface: wit_bindgen_core::wit_parser::InterfaceId,
        _files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }

    fn export_interface(
        &mut self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _name: &wit_bindgen_core::wit_parser::WorldKey,
        _iface: wit_bindgen_core::wit_parser::InterfaceId,
        _files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }

    fn import_funcs(
        &mut self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _world: wit_bindgen_core::wit_parser::WorldId,
        _funcs: &[(&str, &wit_bindgen_core::wit_parser::Function)],
        _files: &mut wit_bindgen_core::Files,
    ) {
        todo!()
    }

    fn export_funcs(
        &mut self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _world: wit_bindgen_core::wit_parser::WorldId,
        _funcs: &[(&str, &wit_bindgen_core::wit_parser::Function)],
        _files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }

    fn import_types(
        &mut self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _world: wit_bindgen_core::wit_parser::WorldId,
        _types: &[(&str, wit_bindgen_core::wit_parser::TypeId)],
        _files: &mut wit_bindgen_core::Files,
    ) {
        todo!()
    }

    fn finish(
        &mut self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _world: wit_bindgen_core::wit_parser::WorldId,
        _files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }
}
