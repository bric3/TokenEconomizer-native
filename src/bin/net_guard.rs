use std::{env, io, path::PathBuf};
use tokeneconomizer::experiments::net_guard;

fn main() {
    let allow = env::var("TE_ALLOW_NETWORK_TOOLS").unwrap_or_default();
    let path = env::var_os(net_guard::LOG_ENV).map(PathBuf::from);
    let int_max_digits = env::var("PYTHONINTMAXSTRDIGITS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n == 0 || *n >= 640)
        .unwrap_or(4300);
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "--install-hook") {
        if args.len() != 4 {
            eprintln!("usage: te-net-guard --install-hook <claude|copilot|codex> <path> <command>");
            std::process::exit(2);
        }
        let result = net_guard::install_hook(
            &args[1].to_string_lossy(),
            &PathBuf::from(&args[2]),
            &args[3].to_string_lossy(),
            int_max_digits,
        );
        match result {
            Ok(()) => {
                println!("armed");
                return;
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
    let code = net_guard::run(
        &mut io::stdin().lock(),
        &mut io::stderr().lock(),
        &allow,
        path.as_deref(),
        int_max_digits,
    );
    std::process::exit(code);
}
