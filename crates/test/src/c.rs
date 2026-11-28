use crate::config::StringList;
use crate::{Compile, LanguageMethods, Runner, Verify};
use anyhow::{Context, Result};
use clap::Parser;
use heck::ToSnakeCase;
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(Default, Debug, Clone, Parser)]
pub struct COpts {
    /// Path to the installation of wasi-sdk
    #[clap(long, env = "WASI_SDK_PATH", value_name = "PATH")]
    pub(crate) wasi_sdk_path: Option<PathBuf>,

    /// Name of the C target to compile for.
    #[clap(long, default_value = "wasm32-wasip2", value_name = "TARGET")]
    c_target: String,
}

pub struct C;

/// C/C++-specific configuration of component files
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LangConfig {
    /// Space-separated list or array of compiler flags to pass.
    #[serde(default)]
    cflags: StringList,
    /// Space-separated list or array of linker flags to pass.
    #[serde(default)]
    ldflags: StringList,
}

fn clang(runner: &Runner) -> PathBuf {
    if runner.is_symmetric() {
        "clang".into()
    } else {
        let target = &runner.opts.c.c_target;
        match &runner.opts.c.wasi_sdk_path {
            Some(path) => path.join(format!("bin/{target}-clang")),
            None => format!("{target}-clang").into(),
        }
    }
}

impl LanguageMethods for C {
    fn display(&self) -> &str {
        "c"
    }

    fn comment_prefix_for_test_config(&self) -> Option<&str> {
        Some("//@")
    }

    fn should_fail_verify(
        &self,
        _runner: &Runner,
        name: &str,
        config: &crate::config::WitConfig,
        _args: &[String],
    ) -> bool {
        config.error_context || name.starts_with("named-fixed-length-list.wit")
    }

    fn codegen_test_variants(&self) -> &[(&str, &[&str])] {
        &[
            ("no-sig-flattening", &["--no-sig-flattening"]),
            ("autodrop", &["--autodrop-borrows=yes"]),
            ("sync", &["--sync=all"]),
        ]
    }

    fn prepare(&self, runner: &mut Runner, _: &str) -> Result<()> {
        prepare(runner, clang(runner))
    }

    fn compile(&self, runner: &Runner, c: &Compile<'_>) -> Result<()> {
        compile(runner, c, clang(runner))
    }

    fn verify(&self, runner: &Runner, v: &Verify<'_>) -> Result<()> {
        verify(runner, v, clang(runner))
    }

    fn should_fail_runtime1(
        &self,
        runner: &Runner,
        test: &crate::Test,
        component: &crate::Component,
    ) -> bool {
        // Resources exported by symmetric components are not supported yet
        // (there are no `resource-new`/`resource-rep` and destructors are
        // not hooked up), these components don't compile.
        let file = component
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        runner.is_symmetric()
            && matches!(
                (test.name.as_str(), file),
                ("accessors", "test")
                    | ("resources", "resources")
                    | ("resource-import-and-export", "intermediate")
                    | ("resource-borrow", "test")
                    | ("autodrop-borrows", "autodropper" | "exporter")
            )
    }

    fn should_fail_runtime2(
        &self,
        runner: &Runner,
        name: &str,
        component: &crate::Component,
    ) -> bool {
        runner.is_symmetric()
            && (
                // The C runner passes a list of lists it never allocated
                // (`a.ptr` is uninitialized), which is harmless in a wasm
                // module but crashes when running natively.
                matches!(component.kind, crate::Kind::Runner) && name == "lists"
                // `cancel-import`: the C callee calls `backpressure_inc`, which
                // the C support of the symmetric ABI doesn't implement (aborts).
                // The C runner expects an import to stay in the "starting"
                // state under backpressure and to keep its arguments when it
                // is cancelled there, but symmetric calls start right away
                // (they are queued by the callee) and own their arguments.
                // Cancelling a running call works in both directions.
                || name == "cancel-import"
            )
    }

    fn generate_bindings_prepare(
        &self,
        _runner: &Runner,
        bindgen: &crate::Bindgen,
        dir: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut export_header_dir = bindgen.wit_path.clone();
        export_header_dir.pop();
        export_header_dir.push("c");

        // copy custom header files to target dir
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
}

/// Where the support library for async in symmetric C components lives.
fn c_client_dir() -> Result<PathBuf> {
    Ok(env::current_dir()?.join("crates/symmetric_executor/c-client"))
}

fn prepare(runner: &mut Runner, compiler: PathBuf) -> Result<()> {
    let cwd = env::current_dir()?;
    let dir = cwd.join(&runner.opts.artifacts).join("c");

    super::write_if_different(&dir.join("test.c"), "int main() { return 0; }")?;

    println!("Testing if `{}` works...", compiler.display());
    runner
        .run_command(Command::new(&compiler).current_dir(&dir).arg("test.c"))
        .inspect_err(|_| {
            eprintln!(
                "Error: failed to find `{}`. Hint: pass `--wasi-sdk-path` or set `WASI_SDK_PATH`",
                compiler.display()
            );
        })?;

    if runner.is_symmetric() {
        // libruntime.a (async/stream support) for the C components
        runner.run_command(
            Command::new("make")
                .current_dir(c_client_dir()?)
                .arg(format!("CC={}", compiler.display())),
        )?;
    }

    Ok(())
}

fn compile(runner: &Runner, compile: &Compile<'_>, compiler: PathBuf) -> Result<()> {
    let config = compile.component.deserialize_lang_config::<LangConfig>()?;

    // Compile the C-based bindings to an object file.
    let bindings_object = compile.output.with_extension("bindings.o");
    let mut cmd = Command::new(clang(runner));
    cmd.arg(
        compile
            .bindings_dir
            .join(format!("{}.c", compile.component.bindgen.world)),
    )
    .arg("-I")
    .arg(&compile.bindings_dir)
    .arg("-Wall")
    .arg("-Wextra")
    .arg("-Werror")
    .arg("-Wno-unused-parameter")
    .arg("-c")
    .arg("-o")
    .arg(&bindings_object);
    if runner.is_symmetric() {
        // for wit_async_support.h
        cmd.arg("-I").arg(c_client_dir()?).arg("-fPIC");
    }
    for flag in Vec::from(config.cflags.clone()) {
        cmd.arg(flag);
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
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-Wc++-compat")
        .arg("-Wno-unused-parameter")
        .arg("-g")
        .arg("-o")
        .arg(&output);
    for flag in Vec::from(config.cflags) {
        cmd.arg(flag);
    }
    for flag in Vec::from(config.ldflags) {
        cmd.arg(flag);
    }
    if !runner.is_symmetric() {
        cmd.arg("-mexec-model=reactor");
        if produces_component(runner) {
            cmd.arg("-Wl,--skip-wit-component");
        }
    } else {
        cmd.arg("--shared").arg("-fPIC");
        cmd.arg("-L").arg(compile.artifacts_dir);
        for lib in compile.link_libs {
            cmd.arg(format!("-l{lib}"));
        }
        // the executor and stream implementation, and the async support
        // built on top of it
        let debug_dir = env::current_dir()?
            .join(&runner.opts.artifacts)
            .join("rust")
            .join("wit-bindgen")
            .join("target")
            .join("debug");
        cmd.arg("-I").arg(c_client_dir()?);
        cmd.arg("-L").arg(c_client_dir()?).arg("-lruntime");
        for lib in ["libsymmetric_executor.so", "libsymmetric_stream.so"] {
            if let Some(found) = crate::find_cdylib(&debug_dir, lib) {
                cmd.arg("-L").arg(found.parent().unwrap());
            }
        }
        cmd.arg("-lsymmetric_stream").arg("-lsymmetric_executor");
    }
    runner.run_command(&mut cmd)?;

    if !runner.is_symmetric() {
        runner
            .convert_core_module_to_component(&output, compile)
            .with_context(|| format!("failed to convert {output:?}"))?;
    }
    Ok(())
}

fn produces_component(runner: &Runner) -> bool {
    match runner.opts.c.c_target.as_str() {
        "wasm32-wasip1" => false,
        _ => true,
    }
}

fn verify(runner: &Runner, verify: &Verify<'_>, compiler: PathBuf) -> Result<()> {
    let mut cmd = Command::new(compiler);
    cmd.arg(
        verify
            .bindings_dir
            .join(format!("{}.c", verify.world.to_snake_case())),
    )
    .arg("-I")
    .arg(&verify.bindings_dir)
    .arg("-Wall")
    .arg("-Wextra")
    .arg("-Werror")
    .arg("-Wc++-compat")
    .arg("-Wno-unused-parameter")
    .arg("-c")
    .arg("-o")
    .arg(verify.artifacts_dir.join("tmp.o"));
    runner.run_command(&mut cmd)?;
    Ok(())
}
