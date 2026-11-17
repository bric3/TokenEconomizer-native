use std::io::{self, Write};
use tokeneconomizer::plugin::repomap::generator::Options;

fn run() -> Result<i32, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    let options = Options::parse(&args, &cwd)?;
    let env = std::env::vars()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect();
    let out = options.run(&env, &cwd).map_err(|e| e.to_string())?;
    io::stdout()
        .write_all(out.stdout.as_bytes())
        .map_err(|e| e.to_string())?;
    io::stderr()
        .write_all(out.stderr.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(out.code)
}
fn main() {
    match run() {
        Ok(code) => {
            #[cfg(unix)]
            if code < 0 {
                // Preserve the SDK process's signal status across the native entry point.
                unsafe {
                    libc::signal(-code, libc::SIG_DFL);
                    libc::raise(-code);
                }
            }
            std::process::exit(code);
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
