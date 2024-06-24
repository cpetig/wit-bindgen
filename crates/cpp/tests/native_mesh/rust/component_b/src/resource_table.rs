use std::collections::HashMap;

#[derive(Debug)]
pub struct ResTable {
    resources: HashMap<i32, *mut u8>,
}

impl ResTable {
    pub fn create_res_table() -> Self {
        let mut info: HashMap<i32, *mut u8> = HashMap::new();
        Self { resources: info }
    }

    pub fn store_resource(&mut self, res: *mut u8) -> i32 {
        let idx = (self.resources.len() + 1) as i32;
        self.resources.insert(idx, res);
        idx
    }

    pub fn lookup_resource(&mut self, idx: i32) -> *mut u8 {
        *self.resources.get(&idx).unwrap()
    }
}
