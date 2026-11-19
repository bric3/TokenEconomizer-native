use std::{env, error::Error, process::Command};
use tokeneconomizer::experiments::{aggregate, sdk};

fn run() -> Result<i32, Box<dyn Error>> {
    if env::args_os().nth(1).is_some_and(|arg| arg == "aggregate") {
        let args = env::args_os()
            .skip(2)
            .map(|arg| {
                arg.into_string()
                    .map_err(|_| "aggregate arguments must be UTF-8")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output = aggregate::command("te-bench aggregate", &args)?;
        print!("{}", output.stdout);
        eprint!("{}", output.stderr);
        return Ok(output.exit.unwrap_or(0));
    }
    let executable = env::current_exe()?;
    let bin = executable
        .parent()
        .ok_or("missing benchmark binary directory")?;
    let (sdk, root) = sdk::script_and_root()?;
    if !sdk.is_file() {
        return Err("benchmark SDK binding is missing".into());
    }
    let mut command =
        Command::new(env::var_os("TE_BENCH_PYTHON").unwrap_or_else(|| "python3".into()));
    command
        .arg("-B")
        .arg(sdk)
        .arg(bin.join(format!("te-bridge{}", env::consts::EXE_SUFFIX)))
        .arg(root)
        .arg(
            executable
                .file_name()
                .ok_or("missing benchmark program name")?,
        )
        .args(env::args_os().skip(1));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().into())
    }
    #[cfg(not(unix))]
    {
        Ok(command.status()?.code().unwrap_or(1))
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("te-bench: {error}");
            std::process::exit(1);
        }
    }
}
