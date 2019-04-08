use cargo::core::compiler::{BuildConfig, CompileMode, Executor};
use cargo::core::{PackageId, Shell, Target, Workspace};
use cargo::ops::{compile_with_exec, CompileFilter, CompileOptions, Packages};
use cargo::util::errors::CargoResult;
use cargo::util::{Config as CargoConfig, ProcessBuilder};

use std::env;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct MyExecutor;

fn parse_arg(args: &[OsString], arg: &str) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == arg {
            return Some(args[i + 1].clone().into_string().unwrap());
        }
    }
    None
}

impl Executor for MyExecutor {
    fn exec(
        &self,
        cargo_cmd: ProcessBuilder,
        _id: PackageId,
        _target: &Target,
        _mode: CompileMode,
    ) -> CargoResult<()> {
        //cmd.exec()?;

        let cargo_args = cargo_cmd.get_args();
        let out_dir = parse_arg(cargo_args, "--out-dir").expect("no out-dir in rustc command line");
        let analysis_dir = Path::new(&out_dir).join("save-analysis");

        println!("{:?}", cargo_cmd);
        Ok(())
    }
}

fn main() {
    let buf: Vec<u8> = Vec::new();
    let cwd = env::current_dir().unwrap();
    let manifest_path = cwd.join("Cargo.toml");
    let build_dir = cwd.join("build_");

    let shell = Shell::from_write(Box::new(BufWriter::new(buf)));
    let config = CargoConfig::new(shell, cwd.to_path_buf(), build_dir);

    let workspace = Workspace::new(&manifest_path, &config).unwrap();

    let compile_opts = CompileOptions {
        spec: Packages::from_flags(false, Vec::new(), Vec::new()).unwrap(),
        filter: CompileFilter::new(
            false,      // opts.lib,
            Vec::new(), // opts.bin,
            false,      // opts.bins,
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
            None,  // opts.jobs,
            &None, // &opts.target,
            CompileMode::Check {
                test: false, /* cfg_test */
            },
        )
        .unwrap(),
        features: Vec::new(),       // opts.features,
        all_features: false,        // opts.all_features,
        no_default_features: false, // opts.no_default_features,
        ..CompileOptions::new(
            &config,
            CompileMode::Check {
                test: false, /* cfg_test */
            },
        )
        .unwrap()
    };

    let exec = Arc::new(MyExecutor {}) as Arc<dyn Executor>;
    let _result = compile_with_exec(&workspace, &compile_opts, &exec);

    println!("cwd: {:?}", cwd);
}
