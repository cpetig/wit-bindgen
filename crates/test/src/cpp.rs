use crate::config::StringList;
use crate::{LanguageMethods, Runner};
use anyhow::Context;
use heck::ToSnakeCase;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

// option wasi_sdk_path is inherited from C

pub struct Cpp;

#[derive(Debug)]
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

    /// The C++ standard to compile the bindings and the component with,
    /// `c++20` by default (e.g. `c++17` for `--async-api future`).
    std: Option<String>,
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
        _runner: &Runner,
        _name: &str,
        _config: &crate::config::WitConfig,
        _args: &[String],
    ) -> bool {
        false
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
            // libruntime.a (async/stream support) for the C++ guests
            runner.run_command(
                Command::new("make").current_dir(cwd.join("crates/symmetric_executor/cpp-client")),
            )?;
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
        let std_flag = format!("-std={}", config.std.as_deref().unwrap_or("c++20"));
        // The `expected` test header (tl::expected, C++17 has no std::expected)
        // can't coexist with libstdc++'s deprecated std::unexpected() handler,
        // which is only declared up to C++20 (and in C++20 mode only with
        // _GLIBCXX_USE_DEPRECATED).
        let no_deprecated = config.std.as_deref() == Some("c++17");

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
            .arg(&std_flag)
            .arg("-c")
            .arg("-g")
            .arg("-o")
            .arg(&bindings_object);
        if no_deprecated {
            cmd.arg("-D_GLIBCXX_USE_DEPRECATED=0");
        }
        runner.run_command(&mut cmd)?;

        // Now compile the runner's source code to with the above object and the
        // component-type object into a final component.
        let output = compile.output.with_extension(if runner.is_symmetric() {
            "so"
        } else {
            "core.wasm"
        });
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
            .arg(&std_flag)
            .arg("-g")
            .arg("-o")
            .arg(&output);
        if no_deprecated {
            cmd.arg("-D_GLIBCXX_USE_DEPRECATED=0");
        }
        for flag in Vec::from(config.cflags) {
            cmd.arg(flag);
        }
        if !runner.is_symmetric() {
            cmd.arg("-mexec-model=reactor");
            cmd.arg("-Wl,--skip-wit-component");
        } else {
            let mut version_script = compile
                .bindings_dir
                .join(format!("{}.verscr", compile.component.bindgen.world));
            if compile.component.contents.contains("int main(") {
                // A runner with a `main` instead of the `run` export is started
                // by calling `main`, so export it as well.
                let exported = std::fs::read_to_string(&version_script)?.replacen(
                    "global:",
                    "global:\n    main;",
                    1,
                );
                version_script = version_script.with_extension("main.verscr");
                std::fs::write(&version_script, exported)?;
            }
            cmd.arg("-fPIC").arg(format!(
                "-Wl,--version-script={}",
                version_script.to_str().unwrap()
            ));
            for i in runner.cpp_state.as_ref().unwrap().native_deps.iter() {
                cmd.arg(format!("-L{}", i.as_os_str().to_str().unwrap()));
                // newer cargo keeps cdylibs in build/<crate>/<hash>/out
                for lib in ["libsymmetric_executor.so", "libsymmetric_stream.so"] {
                    if let Some(found) = crate::find_cdylib(i, lib) {
                        cmd.arg(format!("-L{}", found.parent().unwrap().display()));
                    }
                }
            }
            cmd.arg("-shared");
            cmd.arg("-L").arg(compile.artifacts_dir);
            for lib in compile.link_libs {
                cmd.arg(format!("-l{lib}"));
            }
            cmd.arg("-L")
                .arg(helper_dir3.to_str().unwrap().to_string())
                .arg("-lruntime")
                .arg("-lsymmetric_stream")
                .arg("-lsymmetric_executor");
        }
        runner.run_command(&mut cmd)?;

        if !runner.is_symmetric() {
            runner
                .convert_core_module_to_component(&output, compile)
                .with_context(|| format!("failed to convert {output:?}"))?;
        }
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
        runner.run_command(&mut cmd)?;
        Ok(())
    }

    fn default_bindgen_args(&self) -> &[&str] {
        &["--format"]
    }

    fn extra_bindgen_args(&self, runner: &Runner, bindgen: &crate::Bindgen) -> Vec<&'static str> {
        // These tests are written for the `std::future` based API of the
        // symmetric ABI, which is not the default (`wit::FutureReader`).
        let std_future_test = bindgen
            .wit_path
            .parent()
            .and_then(|dir| dir.file_name())
            .is_some_and(|name| name == "future-string" || name == "stream-string");
        if runner.is_symmetric() && std_future_test {
            vec!["--async-api", "future"]
        } else {
            Vec::new()
        }
    }

    fn should_fail_runtime1(
        &self,
        runner: &Runner,
        test: &crate::Test,
        component: &crate::Component,
    ) -> bool {
        runner.is_symmetric()
            && matches!(component.kind, crate::Kind::Test)
            && (test.name == "resources" || test.name == "resource_borrow_in_record")
    }

    fn should_fail_runtime2(
        &self,
        runner: &Runner,
        name: &str,
        component: &crate::Component,
    ) -> bool {
        // dbg!(runner);
        runner.is_symmetric()
            && (matches!(component.kind, crate::Kind::Test) && name == "param-ownership")
    }
}
