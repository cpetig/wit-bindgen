//! For build scripts of crates linking against cdylib dependencies like symmetric_executor.

use std::{collections::BTreeMap, collections::BTreeSet, env, fs, path::PathBuf, time::SystemTime};

/// Print the cargo link-search directives for the cdylib dependencies of this workspace.
///
/// Older cargo put the built libraries into `deps/`, newer versions keep them in
/// `build/<crate>/<hash>/out`, where the newest build of each library is used.
pub fn emit() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("called from a build script"));
    println!("cargo:rustc-link-search={}/../../../deps", out.display());
    let Ok(crates) = fs::read_dir(out.join("../../..")) else {
        return;
    };
    let mut newest: BTreeMap<String, (SystemTime, PathBuf)> = BTreeMap::new();
    for hashes in crates.flatten().filter_map(|c| fs::read_dir(c.path()).ok()) {
        for dir in hashes.flatten().map(|h| h.path().join("out")) {
            let Ok(files) = fs::read_dir(&dir) else {
                continue;
            };
            for file in files.flatten() {
                let name = file.file_name().to_string_lossy().into_owned();
                if !(name.starts_with("lib") && name.ends_with(".so")) {
                    continue;
                }
                let Some(modified) = file.metadata().ok().and_then(|m| m.modified().ok()) else {
                    continue;
                };
                if newest.get(&name).is_none_or(|(t, _)| *t < modified) {
                    newest.insert(name, (modified, dir.clone()));
                }
            }
        }
    }
    let dirs: BTreeSet<_> = newest.into_values().map(|(_, dir)| dir).collect();
    for dir in dirs {
        println!("cargo:rustc-link-search={}", dir.display());
    }
}
