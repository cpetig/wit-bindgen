use anyhow::Result;
use wit_bindgen_core::WorldGenerator;

#[derive(Default)]
struct ImportLib {
    opts: Opts,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct Opts {}

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
        name: &wit_bindgen_core::wit_parser::WorldKey,
        iface: wit_bindgen_core::wit_parser::InterfaceId,
        files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }

    fn export_interface(
        &mut self,
        resolve: &wit_bindgen_core::wit_parser::Resolve,
        name: &wit_bindgen_core::wit_parser::WorldKey,
        iface: wit_bindgen_core::wit_parser::InterfaceId,
        files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }

    fn import_funcs(
        &mut self,
        resolve: &wit_bindgen_core::wit_parser::Resolve,
        world: wit_bindgen_core::wit_parser::WorldId,
        funcs: &[(&str, &wit_bindgen_core::wit_parser::Function)],
        files: &mut wit_bindgen_core::Files,
    ) {
        todo!()
    }

    fn export_funcs(
        &mut self,
        resolve: &wit_bindgen_core::wit_parser::Resolve,
        world: wit_bindgen_core::wit_parser::WorldId,
        funcs: &[(&str, &wit_bindgen_core::wit_parser::Function)],
        files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }

    fn import_types(
        &mut self,
        resolve: &wit_bindgen_core::wit_parser::Resolve,
        world: wit_bindgen_core::wit_parser::WorldId,
        types: &[(&str, wit_bindgen_core::wit_parser::TypeId)],
        files: &mut wit_bindgen_core::Files,
    ) {
        todo!()
    }

    fn finish(
        &mut self,
        resolve: &wit_bindgen_core::wit_parser::Resolve,
        world: wit_bindgen_core::wit_parser::WorldId,
        files: &mut wit_bindgen_core::Files,
    ) -> Result<()> {
        todo!()
    }
}
