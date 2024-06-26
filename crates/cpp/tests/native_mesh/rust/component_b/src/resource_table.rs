use wasmtime::component::ResourceTable::{self, *};

struct RT {
    rt: ResourceTable,
}

impl RT {
    fn create_rt() -> RT {
        Self {
            rt: ResourceTable::new(),
        }
    }

    fn insert_resource(&mut self, item: T) {}
}
