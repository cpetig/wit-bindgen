use heck::*;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

macro_rules! codegen_test {
    ($id:ident $name:tt $test:tt) => {
        #[test]
        fn $id() {
            test_helpers::run_world_codegen_test(
                "host-cpp",
                $test.as_ref(),
                |resolve, world, files| {
                    wit_bindgen_cpp_host::Opts::default()
                        .build()
                        .generate(resolve, world, files)
                },
                verify,
            );
        }
    };
}

test_helpers::codegen_tests!("*.wit");

fn verify(dir: &Path, name: &str) {
    let path = PathBuf::from(env::var_os("WASI_SDK_PATH").unwrap());
    let mut cmd = Command::new(path.join("bin/clang++"));
    cmd.arg(dir.join(format!("{}_host.cpp", name.to_snake_case())));
    cmd.arg("-I").arg(dir);
    cmd.arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-Wno-unused-parameter");
    cmd.arg("-c");
    cmd.arg("-o").arg(dir.join("obj_host.o"));

    test_helpers::run_command(&mut cmd);
}
