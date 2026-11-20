//! Native stand-in for the model-spelling tests' Copilot CLI.
use std::{
    env,
    fs::OpenOptions,
    io::{self, Write},
};

fn main() -> io::Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    let model = args
        .windows(2)
        .find(|pair| pair[0] == "--model")
        .map(|pair| pair[1].as_str())
        .expect("missing --model");
    assert_eq!(
        args,
        [
            "-p",
            "ok",
            "--no-color",
            "--log-level",
            "none",
            "--model",
            model,
            "--effort",
            "high"
        ]
    );
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(env::var_os("STUB_LOG").expect("missing STUB_LOG"))?,
        "{model}"
    )?;
    match model {
        "claude-opus-4.8" | "claude-sonnet-5" | "claude-opus-5" => println!("ok"),
        "claude-opus-4-8" => {
            println!("Model 'claude-opus-4-8' is not available. Available models: ...")
        }
        _ => println!(
            "Model '{model}' not found on provider at https://api.anthropic.com (HTTP 404)."
        ),
    }
    Ok(())
}
