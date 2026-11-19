use std::{env, error::Error, process::Command};
use tokeneconomizer::experiments::{aggregate, datasets, sdk, sweep, workloads};

fn run() -> Result<i32, Box<dyn Error>> {
    if let Some(command) = env::args_os().nth(1).filter(|arg| {
        arg == "aggregate"
            || arg == "export-workloads"
            || arg == "sweep-plan"
            || arg == "export-swebench"
    }) {
        let args = env::args_os()
            .skip(2)
            .map(|arg| {
                arg.into_string()
                    .map_err(|_| "benchmark tool arguments must be UTF-8")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output = if command == "aggregate" {
            Some(aggregate::command("te-bench aggregate", &args)?)
        } else if command == "export-workloads" {
            let root = env::current_dir()?;
            Some(workloads::export_command(
                "te-bench export-workloads",
                &args,
                &root.join("workloads"),
                &root,
            )?)
        } else if command == "sweep-plan" {
            let root = env::current_dir()?;
            Some(
                sweep::command("te-bench sweep-plan", &args, &root, &root)
                    .map_err(|e| e.message)?,
            )
        } else {
            datasets::parse_swebench("te-bench export-swebench", &args).err()
        };
        if let Some(output) = output {
            print!("{}", output.stdout);
            eprint!("{}", output.stderr);
            return Ok(output.exit.or(output.returncode).unwrap_or(0));
        }
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
