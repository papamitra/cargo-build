#![feature(rustc_private)]
#[allow(unused_extern_crates)]
extern crate rustc_driver;
#[allow(unused_extern_crates)]
extern crate rustc_interface;
#[allow(unused_extern_crates)]
extern crate rustc_save_analysis;

use cargo::core::compiler::{BuildConfig, CompileMode, Executor};
use cargo::core::{PackageId, Shell, Target, Workspace};
use cargo::ops::{compile_with_exec, CompileFilter, CompileOptions, Packages};
use cargo::util::errors::CargoResult;
use cargo::util::{Config as CargoConfig, ProcessBuilder};

use std::env;
use std::ffi::OsString;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use rustc_driver::{run_compiler, Callbacks};
use rustc_interface::interface;
use rustc_save_analysis::CallbackHandler;

use rls_data::Analysis;

struct MyRustcCalls {
    analysis: Arc<Mutex<Option<Analysis>>>,
}

impl Callbacks for MyRustcCalls {
    fn config(&mut self, config: &mut interface::Config) {
        println!("MyRustsCalls::config()");
        config.opts.debugging_opts.save_analysis = true;
    }

    fn after_parsing(&mut self, _compiler: &interface::Compiler) -> bool {
        println!("MyRustcCalls::after_parsing()");
        true
    }

    fn after_analysis(&mut self, compiler: &interface::Compiler) -> bool {
        println!("MyRustcCalls::after_analysis()");

        let sess = compiler.session();
        let input = compiler.input();
        let crate_name = compiler.crate_name().unwrap().peek().clone();

        let expanded_crate = &compiler.expansion().unwrap().peek().0;

        compiler.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            rustc_save_analysis::process_crate(
                tcx,
                &expanded_crate,
                &crate_name,
                &input,
                None,
                CallbackHandler {
                    callback: &mut |a| {
                        let mut analysis = self.analysis.lock().unwrap();
                        let a = unsafe { ::std::mem::transmute(a.clone()) };
                        *analysis = Some(a);
                    },
                },
            );
        });

        println!("{:?}", self.analysis.lock().unwrap());
        true
    }
}

fn current_sysroot() -> Option<String> {
    let home = env::var("RUSTUP_HOME").or_else(|_| env::var("MULTIRUST_HOME"));
    let toolchain = env::var("RUSTUP_TOOLCHAIN").or_else(|_| env::var("MULTIRUST_TOOLCHAIN"));
    if let (Ok(home), Ok(toolchain)) = (home, toolchain) {
        Some(format!("{}/toolchains/{}", home, toolchain))
    } else {
        let rustc_exe = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
        env::var("SYSROOT").ok().or_else(|| {
            Command::new(rustc_exe)
                .arg("--print")
                .arg("sysroot")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_owned())
        })
    }
}

fn parse_arg(args: &[OsString], arg: &str) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == arg {
            return Some(args[i + 1].clone().into_string().unwrap());
        }
    }
    None
}

fn is_primary_package(id: PackageId) -> bool {
    id.source_id().is_path()
    // || self.member_packages.lock().unwrap().contains(&id)
}

struct MyExecutor {
    build_dir: PathBuf,
}

impl Executor for MyExecutor {
    fn exec(
        &self,
        cargo_cmd: ProcessBuilder,
        id: PackageId,
        _target: &Target,
        _mode: CompileMode,
    ) -> CargoResult<()> {
        //cmd.exec()?;

        let cargo_args = cargo_cmd.get_args();
        let out_dir = parse_arg(cargo_args, "--out-dir").expect("no out-dir in rustc command line");
        let _analysis_dir = Path::new(&out_dir).join("save-analysis");

        let mut cmd = cargo_cmd.clone();

        // Add args and envs to cmd.
        let mut args: Vec<_> = cargo_args
            .iter()
            .map(|a| a.clone().into_string().unwrap())
            .collect();
        let _envs = cargo_cmd.get_envs().clone();

        let sysroot = current_sysroot().expect(
            "need to specify `SYSROOT` env var or use rustup or multirust",
        );

        args.push("--sysroot".to_owned());
        args.push(sysroot);

        cmd.args_replace(&args);

        if !is_primary_package(id) {
            cmd.env(
                "RUST_SAVE_ANALYSIS_CONFIG",
                r#"{ "reachable_only": true,
                         "full_docs": true, "pub_only": true,
                         "distro_crate": false,
                         "signatures": false, "borrow_data": false }"#,
            );
            return cmd.exec();
        }

        // Prepare modified cargo-generated args/envs for future rustc calls.
        let rustc = cargo_cmd.get_program().to_owned().into_string().unwrap();
        args.insert(0, rustc);

        let mut callbacks = MyRustcCalls { analysis: Arc::new(Mutex::new(None)) };

        run_compiler(&args, &mut callbacks, None, None);

        Ok(())
    }
}

fn main() {
    let buf: Vec<u8> = Vec::new();
    let cwd = env::current_dir().unwrap();
    let manifest_path = cwd.join("Cargo.toml");
    let build_dir = cwd.join("build_");

    let shell = Shell::from_write(Box::new(BufWriter::new(buf)));
    let config = CargoConfig::new(shell, cwd.to_path_buf(), build_dir.clone());

    let workspace = Workspace::new(&manifest_path, &config).unwrap();

    let compile_opts = CompileOptions {
        spec: Packages::from_flags(false, Vec::new(), Vec::new()).unwrap(),
        filter: CompileFilter::new(
            false, // opts.lib,
            Vec::new(), // opts.bin,
            false, // opts.bins,
            // TODO: support more crate target types.
            Vec::new(),
            // Check all integration tests under `tests/`.
            false, // cfg_test,
            Vec::new(),
            false,
            Vec::new(),
            false,
            false, // opts.all_targets,
        ),
        build_config: BuildConfig::new(
            &config,
            None, // opts.jobs,
            &None, // &opts.target,
            CompileMode::Check { test: false /* cfg_test */ },
        ).unwrap(),
        features: Vec::new(), // opts.features,
        all_features: false, // opts.all_features,
        no_default_features: false, // opts.no_default_features,
        ..CompileOptions::new(&config, CompileMode::Check { test: false /* cfg_test */ }).unwrap()
    };

    let exec = Arc::new(MyExecutor { build_dir }) as Arc<Executor>;
    let _result = compile_with_exec(&workspace, &compile_opts, &exec);

    println!("cwd: {:?}", cwd);
}
