//! For build scripts of crates linking against the symmetric_executor / symmetric_stream cdylibs.

use std::{env, fs, path::PathBuf, time::SystemTime};

/// Print the cargo link-search directives for the symmetric runtime libraries.
///
/// The libraries are built as cdylib dependencies; older cargo put them into `deps/`, newer
/// versions keep them in `build/<crate>/<hash>/out`, where the newest one is used.
pub fn emit() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("called from a build script"));
    println!("cargo:rustc-link-search={}/../../../deps", out.display());
    let build = out.join("../../..");
    for name in ["symmetric_executor", "symmetric_stream"] {
        let Ok(entries) = fs::read_dir(build.join(name)) else {
            continue;
        };
        let newest = entries
            .flatten()
            .map(|e| e.path().join("out"))
            .filter_map(|dir| {
                let modified = fs::metadata(dir.join(format!("lib{name}.so")))
                    .ok()?
                    .modified()
                    .ok()?;
                Some((modified, dir))
            })
            .max_by_key(|(modified, _): &(SystemTime, PathBuf)| *modified);
        if let Some((_, dir)) = newest {
            println!("cargo:rustc-link-search={}", dir.display());
        }
    }
}
