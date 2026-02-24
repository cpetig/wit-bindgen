use crate::config::StringList;
use crate::{Kind, LanguageMethods, Runner};
use anyhow::Context;
use heck::ToSnakeCase;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

// option wasi_sdk_path is inherited from C

pub struct Cpp;

pub struct State {
    native_deps: Vec<PathBuf>,
}

/// C/C++-specific configuration of component files
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LangConfig {
    /// Space-separated list or array of compiler flags to pass.
    #[serde(default)]
    cflags: StringList,
}

fn clangpp(runner: &Runner) -> PathBuf {
    if runner.is_symmetric() {
        "clang++".into()
    } else {
        match &runner.opts.c.wasi_sdk_path {
            Some(path) => path.join("bin/wasm32-wasip2-clang++"),
            None => "wasm32-wasip2-clang++".into(),
        }
    }
}

impl LanguageMethods for Cpp {
    fn display(&self) -> &str {
        "cpp"
    }

    fn comment_prefix_for_test_config(&self) -> Option<&str> {
        Some("//@")
    }

    fn bindgen_name(&self) -> Option<&str> {
        Some("cpp")
    }

    fn should_fail_verify(
        &self,
        name: &str,
        config: &crate::config::WitConfig,
        _args: &[String],
    ) -> bool {
        return match name {
            "issue1514-6.wit" | "named-fixed-length-list.wit" => true,
            _ => false,
        } || config.async_;
    }

    fn prepare(&self, runner: &mut crate::Runner, test_name: &str) -> anyhow::Result<()> {
        let compiler = clangpp(runner);
        let cwd = std::env::current_dir()?;
        let dir = cwd.join(&runner.opts.artifacts).join("cpp");

        super::write_if_different(&dir.join("test.cpp"), "int main() { return 0; }")?;

        println!("Testing if `{}` works...", compiler.display());
        runner
            .run_command(Command::new(&compiler).current_dir(&dir).arg("test.cpp"))
            .inspect_err(|_| {
                eprintln!(
                    "Error: failed to find `{}`. Hint: pass `--wasi-sdk-path` or set `WASI_SDK_PATH`",
                    compiler.display()
                );
            })?;

        let mut native_deps = Vec::new();
        if runner.is_symmetric() {
            let cwd = std::env::current_dir()?;
            let dir = cwd.join(&runner.opts.artifacts).join("rust");
            let wit_bindgen = dir.join("wit-bindgen");
            let mut target_out_dir = wit_bindgen.join("target");
            target_out_dir.push("debug");

            native_deps.push(target_out_dir);
            let root_dir = runner.opts.artifacts.join(test_name);
            native_deps.push(root_dir);
        }

        runner.cpp_state = Some(State { native_deps });
        Ok(())
    }

    fn generate_bindings_prepare(
        &self,
        _runner: &Runner,
        bindgen: &crate::Bindgen,
        dir: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut export_header_dir = bindgen.wit_path.clone();
        export_header_dir.pop();
        export_header_dir.push("cpp");

        // copy resource implementation in header files to target dir
        if export_header_dir.is_dir() {
            if !dir.exists() {
                std::fs::create_dir_all(dir).context("failed to create bindings dir")?;
            }
            for entry in export_header_dir
                .read_dir()
                .context("failed to read test header directory")?
            {
                let entry = entry.context("failed to read test header directory entry")?;
                let path = entry.path();
                let mut dest = PathBuf::from(dir);
                dest.push(path.file_name().unwrap());
                std::fs::copy(path, dest).context("failed to copy header file")?;
            }
        }
        Ok(())
    }

    fn compile(&self, runner: &Runner, compile: &crate::Compile) -> anyhow::Result<()> {
        let compiler = clangpp(runner);
        let config = compile.component.deserialize_lang_config::<LangConfig>()?;

        let cwd = std::env::current_dir()?;
        let mut helper_dir = cwd.clone();
        helper_dir.push("crates");
        helper_dir.push("cpp");
        helper_dir.push("helper-types");
        // for expected
        let mut helper_dir2 = cwd.clone();
        helper_dir2.push("crates");
        helper_dir2.push("cpp");
        helper_dir2.push("test_headers");
        // for async_support.h
        let mut helper_dir3 = cwd.clone();
        helper_dir3.push("crates");
        helper_dir3.push("symmetric_executor");
        helper_dir3.push("cpp-client");

        // Compile the C-based bindings to an object file.
        let bindings_object = compile.output.with_extension("bindings.o");
        let mut cmd = Command::new(clangpp(runner));
        cmd.arg(
            compile
                .bindings_dir
                .join(format!("{}.cpp", compile.component.bindgen.world)),
        )
        .arg("-I")
        .arg(&compile.bindings_dir)
        .arg("-I")
        .arg(helper_dir.to_str().unwrap())
        .arg("-I")
        .arg(helper_dir2.to_str().unwrap());
        if runner.is_symmetric() {
            cmd.arg("-I")
                .arg(helper_dir3.to_str().unwrap())
                .arg("-fPIC");
        }
        cmd.arg("-fno-exceptions")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-Werror")
            .arg("-Wno-unused-parameter")
            .arg("-std=c++20")
            .arg("-c")
            .arg("-g")
            .arg("-o")
            .arg(&bindings_object);
        runner.run_command(&mut cmd)?;

        // Now compile the runner's source code to with the above object and the
        // component-type object into a final component.
        let mut cmd = Command::new(compiler);
        cmd.arg(&compile.component.path).arg(&bindings_object);
        if !runner.is_symmetric() {
            cmd.arg(compile.bindings_dir.join(format!(
                "{}_component_type.o",
                compile.component.bindgen.world
            )));
        }
        cmd.arg("-I")
            .arg(&compile.bindings_dir)
            .arg("-I")
            .arg(helper_dir.to_str().unwrap())
            .arg("-I")
            .arg(helper_dir2.to_str().unwrap());
        if !runner.is_symmetric() {
            cmd.arg("-fno-exceptions");
        } else {
            cmd.arg("-I").arg(helper_dir3.to_str().unwrap());
        }
        cmd.arg("-Wall")
            .arg("-Wextra")
            .arg("-Werror")
            .arg("-Wc++-compat")
            .arg("-Wno-unused-parameter")
            .arg("-std=c++20")
            .arg("-g")
            .arg("-o")
            .arg(&compile.output);
        for flag in Vec::from(config.cflags) {
            cmd.arg(flag);
        }
        if !runner.is_symmetric() {
            cmd.arg("-mexec-model=reactor");
        } else {
            cmd.arg("-fPIC").arg(format!(
                "-Wl,--version-script={}",
                compile
                    .bindings_dir
                    .join(format!("{}.verscr", compile.component.bindgen.world))
                    .to_str()
                    .unwrap() // .to_string(),
            ));
            for i in runner.cpp_state.as_ref().unwrap().native_deps.iter() {
                cmd.arg(format!("-L{}", i.as_os_str().to_str().unwrap()));
            }
            cmd.arg("-shared");
            if matches!(compile.component.kind, Kind::Runner) {
                let mut bindings_parent: PathBuf = compile.bindings_dir.into();
                bindings_parent.pop();
                cmd.arg("-L")
                    .arg(bindings_parent.to_str().unwrap().to_string());
                cmd.arg("-ltest");
            }
            cmd.arg("-L")
                .arg(helper_dir3.to_str().unwrap().to_string())
                .arg("-lruntime")
                .arg("-lsymmetric_stream")
                .arg("-lsymmetric_executor");
        }
        runner.run_command(&mut cmd)?;
        Ok(())
    }

    fn verify(&self, runner: &Runner, verify: &crate::Verify) -> anyhow::Result<()> {
        // for expected
        let cwd = std::env::current_dir()?;
        let mut helper_dir2 = cwd;
        helper_dir2.push("crates");
        helper_dir2.push("cpp");
        helper_dir2.push("test_headers");

        let compiler = clangpp(runner);
        let mut cmd = Command::new(compiler);
        cmd.arg(
            verify
                .bindings_dir
                .join(format!("{}.cpp", verify.world.to_snake_case())),
        )
        .arg("-I")
        .arg(&verify.bindings_dir)
        .arg("-I")
        .arg(helper_dir2.to_str().unwrap())
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-Wc++-compat")
        .arg("-Wno-unused-parameter")
        .arg("-std=c++20")
        .arg("-c")
        .arg("-o")
        .arg(verify.artifacts_dir.join("tmp.o"));
        runner.run_command(&mut cmd)
    }

    fn default_bindgen_args(&self) -> &[&str] {
        &["--format"]
    }

    fn should_fail_runtime1(
        &self,
        runner: &Runner,
        test: &crate::Test,
        component: &crate::Component,
    ) -> bool {
        runner.is_symmetric()
            && ((matches!(component.kind, crate::Kind::Test)
                && (test.name == "resources"
                    || test.name == "resource_borrow_in_record"
                    || test.name == /*cpp*/"param-ownership"))
                || (matches!(component.kind, crate::Kind::Runner)
                    && (test.name == "results"
                        || test.name == "common-types"
                        || test.name == /*async*/"future-string"
                        || test.name == /*async*/"stream-string")))
    }
}
