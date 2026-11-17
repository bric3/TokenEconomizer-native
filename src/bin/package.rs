use std::{env, path::PathBuf};
use tokeneconomizer::package::{self, Result};

fn run() -> Result<()> {
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binaries = env::current_exe()?
        .parent()
        .ok_or("missing binary directory")?
        .to_path_buf();
    package::cli::run(
        env::args_os().skip(1),
        checkout.join("reference/upstream"),
        Some(binaries),
        checkout.join("dist"),
    )
}

fn main() {
    if let Err(error) = run() {
        eprintln!("te-package: {error}");
        std::process::exit(1);
    }
}
