use std::{env, error::Error};
use tokeneconomizer::dead_code;

fn run() -> Result<i32, Box<dyn Error>> {
    let args = env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "dead-code arguments must be UTF-8")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let python = env::var_os("TE_PYTHON")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "python3".into());
    let output = dead_code::run("te-dead-code", &args, &env::current_dir()?, &python)?;
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);
    Ok(output.code)
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("te-dead-code: {error}");
            std::process::exit(1);
        }
    }
}
