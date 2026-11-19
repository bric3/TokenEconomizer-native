#[cfg(unix)]
use libc::umask;
use std::{
    env,
    io::{self, Read, Write},
};
use tokeneconomizer::plugin::runtime::NativeRuntime;

fn run() -> Result<String, String> {
    let argv = env::args_os()
        .skip(1)
        .map(|a| {
            a.into_string()
                .map_err(|_| "hook arguments must be UTF-8".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut runtime = NativeRuntime::capture().map_err(|e| e.to_string())?;
    runtime.executable = runtime
        .executable
        .with_file_name(format!("te{}", env::consts::EXE_SUFFIX));
    let mut input = Vec::new();
    let read = io::stdin()
        .lock()
        .read_to_end(&mut input)
        .map(|_| input)
        .map_err(|e| e.to_string());
    runtime.run_hook(&argv, read)
}
fn main() {
    #[cfg(unix)]
    // Set private creation permissions before state writes or worker threads start.
    unsafe {
        umask(0o077);
    }
    let output = run().unwrap_or_else(|error| {
        eprintln!("te-hook: {error}");
        "{}\n".into()
    });
    let _ = io::stdout().lock().write_all(output.as_bytes());
}
