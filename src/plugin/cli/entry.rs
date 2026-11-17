use super::{Error, Result, Runtime, expand_argline, hosts, parser, paths, recent_host};
use crate::{plugin::hosts::session::Environment, value::truthy};
use serde_json::{Value, json};
use std::path::Path;

pub fn main(
    argv: &[String],
    environment: &Environment,
    columns: usize,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let argv = match expand_argline(argv) {
        Ok(argv) => argv,
        Err(error) => {
            runtime.say(&format!("te: could not read the arguments: {error}"))?;
            return Ok(json!(2));
        }
    };
    let mut parsed = parser::parse(&argv, &environment.cwd, columns);
    if !parsed.stdout.is_empty() {
        runtime.say(parsed.stdout.trim_end_matches('\n'))?;
    }
    if !parsed.stderr.is_empty() {
        runtime.global("stderr", json!([parsed.stderr]))?;
    }
    if let Some(code) = parsed.exit {
        return Err(Error::new("SystemExit", code.to_string()));
    }
    if parsed.args["host"].is_null() {
        let cwd = Path::new(
            parsed.args["cwd"]
                .as_str()
                .ok_or("expected working directory")?,
        );
        let recent = recent_host(cwd, environment)?;
        parsed.args["host"] = json!(hosts::default_host(
            &json!(environment.env),
            recent.as_str(),
            &hosts::HOSTS.map(String::from)
        ));
    }
    let result = runtime.global("execute", json!([parsed.args]));
    let mut cleanup_env = environment.env.clone();
    if let Some(root) = environment
        .env
        .get(paths::HOME_ENV)
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
    {
        cleanup_env.insert(
            paths::HOME_ENV.into(),
            json!(environment.path(Path::new(root))),
        );
    }
    paths::secure_root(&cleanup_env, &environment.home)?;
    result.map(|value| if truthy(&value) { value } else { json!(0) })
}
