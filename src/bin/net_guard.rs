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
    let code = net_guard::run(
        &mut io::stdin().lock(),
        &mut io::stderr().lock(),
        &allow,
        path.as_deref(),
        int_max_digits,
    );
    std::process::exit(code);
}
