use std::path::Path;
use tokeneconomizer::plugin::{
    cli::{Error, Result},
    runtime::NativeRuntime,
    winspawn,
};

fn run() -> Result<i32> {
    let args = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| Error::from("arguments must be UTF-8"))
        })
        .collect::<Result<Vec<_>>>()?;
    if args.first().is_some_and(|a| a == "--run-spawn-spec") && args.len() == 2 {
        return Ok(winspawn::run(
            Path::new(&args[1]),
            &mut winspawn::NativeRuntime,
        )?);
    }
    let value = NativeRuntime::capture()?.run(&args)?;
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| "invalid command exit status".into())
}
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) if error.is("SystemExit") => error.message.parse().unwrap_or(1),
        Err(error) => {
            eprintln!("te: {error}");
            1
        }
    };
    std::process::exit(code);
}
