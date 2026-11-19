use std::{
    env,
    io::{self, Read},
    path::PathBuf,
};
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
    if args.first().is_some_and(|arg| arg == "--catalog-recorder") {
        if args.len() != 3 {
            eprintln!("usage: te-net-guard --catalog-recorder <path> <upstream>");
            std::process::exit(2);
        }
        let result = tokeneconomizer::experiments::swebench::catalog::recorder::serve(
            io::stdin().lock(),
            io::stdout().lock(),
            &PathBuf::from(&args[1]),
            &args[2].to_string_lossy(),
            &env::var("REAL_ANTHROPIC_KEY").unwrap_or_default(),
        );
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().is_some_and(|arg| arg == "--install-entry") {
        if args.len() != 3 {
            eprintln!("usage: te-net-guard --install-entry <event> <path> < entry.json");
            std::process::exit(2);
        }
        let mut entry = Vec::new();
        let result = io::stdin()
            .read_to_end(&mut entry)
            .map_err(|error| format!("OSError: {error}"))
            .and_then(|_| {
                net_guard::install_entry(
                    &PathBuf::from(&args[2]),
                    &args[1].to_string_lossy(),
                    &entry,
                    int_max_digits,
                )
            });
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
    if args.first().is_some_and(|arg| arg == "--merge-json") {
        if args.len() != 3 {
            eprintln!("usage: te-net-guard --merge-json <path> <patch-file>");
            std::process::exit(2);
        }
        let result = std::fs::read(&args[2])
            .map_err(|error| format!("OSError: {error}"))
            .and_then(|patch| {
                tokeneconomizer::experiments::scaffolds::merge_json_file(
                    &PathBuf::from(&args[1]),
                    &patch,
                    int_max_digits,
                )
            });
        match result {
            Ok(()) => {
                println!("merged");
                return;
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
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
