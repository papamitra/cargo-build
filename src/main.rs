use cargo::core::compiler::{BuildConfig, CompileMode, Context, Executor, Unit};
use cargo::core::{Shell, Workspace};
use cargo::ops::{compile_with_exec, CompileFilter, CompileOptions, Packages};
use cargo::util::Config as CargoConfig;

use std::env;
use std::io::{self, BufWriter};

fn main() {
    let cwd = env::current_dir().unwrap();
    let build_dir = cwd.join("build_");

    let shell = Shell::from_write(Box::new(BufWriter::new(std::io::stdout())));
    let config = CargoConfig::new(shell, cwd.to_path_buf(), build_dir);

    let workspace = Workspace::new(&cwd, &config).unwrap();

    println!("cwd: {:?}", cwd);
}
