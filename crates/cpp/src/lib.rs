use heck::ToSnakeCase;
use wit_bindgen_core::{
    wit_parser::{Function, InterfaceId, Resolve, TypeId, WorldId, WorldKey},
    Files, Source, WorldGenerator,
};

mod wamr;

#[derive(Default)]
struct Cpp {
    opts: Opts,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct Opts {
    #[cfg_attr(feature = "clap", arg(long, default_value_t = bool::default()))]
    pub host: bool,
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
        // todo!()
        if !self.opts.host {
            files.push(&format!("{snake}.cpp"), c_str.as_bytes());
            files.push(&format!("{snake}_cpp.h"), h_str.as_bytes());
        } else {
            files.push(&format!("{snake}_host.cpp"), c_str.as_bytes());
            files.push(&format!("{snake}_cpp_host.h"), h_str.as_bytes());
        }
    }
}
