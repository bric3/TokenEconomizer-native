//! Native child fixture for the original tool-init probe's pipe and deadline tests.
use std::{
    env, fs,
    io::{self, Write},
    thread,
    time::Duration,
};

fn main() -> io::Result<()> {
    if let Some(path) = env::args_os().nth(1) {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&fs::read(path)?)?;
        stdout.flush()?;
    }
    thread::sleep(Duration::from_secs(5));
    Ok(())
}
