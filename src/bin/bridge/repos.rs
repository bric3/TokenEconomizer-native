use std::path::Path;

use crate::{callback::CallbackTape, storage_dispatch};
use serde_json::{Value, json};
use tokeneconomizer::{
    plugin::{applied, paths::RepoId, repos},
    store::Store,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn repo(args: &Value, key: &str) -> Result<RepoId, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}
fn store(args: &Value, key: &str) -> Result<Store, String> {
    Store::new(Path::new(text(&args[key], "base"))).map_err(|e| e.to_string())
}
fn values(args: &Value, key: &str) -> Vec<Value> {
    args[key].as_array().cloned().unwrap_or_default()
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}

pub(super) fn dispatch(op: &str, args: &Value) -> Result<Value, String> {
    let env = args["env"].as_object().cloned().unwrap_or_default();
    let root = Path::new(text(args, "root"));
    let home = Path::new(text(args, "home"));
    let timeout = args["timeout"].as_f64().unwrap_or(repos::LOCK_WAIT_S);
    let mut cache = args["cache"].as_object().cloned().unwrap_or_default();
    let value = match op {
        "repos.constants" => {
            json!({"HOSTS":repos::HOSTS,"LEARNED":repos::LEARNED,"LAYOUT":repos::LAYOUT})
        }
        "repos.repos_dir" => json!(repos::repos_dir(root)),
        "repos._repo_dir" => json!(repos::repo_dir(root, text(args, "key"))),
        "repos._record_error" => {
            repos::record_error(root, text(args, "error"));
            Value::Null
        }
        "repos._pin_store" => json!({"base":repos::pin_store(root,text(args,"host"))?.base}),
        "repos._pin_name" => json!(repos::pin_name(&args["session_id"])),
        "repos._repo_from" => json!(repos::repo_from(&args["d"])),
        "repos.pinned" => json!(repos::pinned(
            root,
            text(args, "host"),
            &args["session_id"]
        )?),
        "repos.pin" => {
            repos::pin(
                root,
                text(args, "host"),
                &args["session_id"],
                &repo(args, "repo")?,
                Path::new(text(args, "cwd")),
            )?;
            Value::Null
        }
        "repos.prune_pins" => json!(repos::prune_pins(
            root,
            text(args, "host"),
            args["now"].as_f64(),
            args["max_age_s"].as_f64().unwrap_or(repos::PIN_MAX_AGE_S)
        )?),
        "repos._legacy_root" => json!(repos::legacy_root(Path::new(text(args, "start")), &env)?),
        "repos.legacy_key" => json!(repos::legacy_key(Path::new(text(args, "cwd")), &env)?),
        "repos._host_dirs" => json!(repos::host_dirs(Path::new(text(args, "repo_dir")))),
        "repos._sessions" => json!(repos::sessions(&store(args, "st")?)?),
        "repos._transcript_cwd" => json!(repos::transcript_cwd(Path::new(text(args, "path")))),
        "repos.session_cwd" => json!(repos::session_cwd(&args["rec"], &args["folded"])),
        "repos._imported" => json!(repos::imported(&args["folded_entry"])),
        "repos._transcript_of" => repos::transcript_of(&args["rec"], &args["folded_entry"]),
        "repos._has_learned" => json!(repos::has_learned(&store(args, "st")?)?),
        "repos.dir_label" => json!(repos::dir_label(Path::new(text(args, "repo_dir")))?),
        "repos.note_repo" => {
            repos::note_repo(root, &repo(args, "repo")?)?;
            Value::Null
        }
        "repos._write_lines" => {
            repos::write_lines(
                &store(args, "st")?,
                text(args, "name"),
                &values(args, "records"),
            )?;
            Value::Null
        }
        "repos._rename_label" => {
            repos::rename_label(Path::new(text(args, "mech_dir")), text(args, "label"))?;
            Value::Null
        }
        "repos._take_learned" => json!(repos::take_learned(
            &store(args, "src")?,
            &store(args, "dst")?,
            text(args, "label")
        )?),
        "repos.copy_move" => {
            repos::copy_move(
                Path::new(text(args, "source")),
                Path::new(text(args, "destination")),
            )?;
            Value::Null
        }
        "repos._merge_applied" => {
            repos::merge_applied(&store(args, "src")?, &store(args, "dst")?)?;
            Value::Null
        }
        "repos._merge_envelope" => {
            repos::merge_envelope(&store(args, "src")?, &store(args, "dst")?)?;
            Value::Null
        }
        "repos._merge_host" => json!(repos::merge_host(
            &store(args, "src")?,
            &store(args, "dst")?,
            &strings(args, "sids")?,
            args["whole"] == true,
            text(args, "label"),
            timeout
        )?),
        "repos.merge_whole" => json!(repos::merge_whole(
            root,
            text(args, "src_key"),
            text(args, "dst_key"),
            text(args, "label"),
            timeout
        )?),
        "repos._resolve" => json!(repos::resolve(text(args, "cwd"), &env, home, &mut cache)?),
        "repos.plan_dir" => json!(repos::plan_dir(
            root,
            text(args, "key"),
            &env,
            home,
            &mut cache
        )?),
        "repos._live" => json!(repos::live(
            text(args, "key"),
            &env,
            home,
            &mut cache,
            &args["idx"]
        )?),
        "repos.migrate_dir" => json!(repos::migrate_dir(
            root,
            text(args, "key"),
            &env,
            home,
            timeout,
            &mut cache,
            args["live"] == true
        )?),
        "repos._record_moves" => {
            let mut idx = args["idx"].clone();
            repos::record_moves(&mut idx, &values(args, "done"))?;
            idx
        }
        "repos._candidates" => json!(repos::candidates(
            root,
            &repo(args, "repo")?,
            &env,
            &args["idx"]
        )?),
        "repos._prune_index" => {
            let mut idx = args["idx"].clone();
            repos::prune_index(&mut idx, args["now"].as_f64().unwrap_or_else(repos::now))?;
            idx
        }
        "repos.adopt" => json!(repos::adopt(
            root,
            &repo(args, "repo")?,
            &env,
            home,
            timeout
        )?),
        "repos.migrate_all" => json!(repos::migrate_all(root, &env, home, timeout)?),
        "repos.merge" => json!(repos::merge(
            root,
            text(args, "src_key"),
            &args["dst_repo"],
            timeout
        )?),
        "repos.needs_migration" => json!(repos::needs_migration(root)?),
        "repos.start_migration" => json!(repos::start_migration(root, &env, home)?),
        "repos.run_migration" => json!(repos::run_migration(root, &env, home)?),
        "repos.migration_notice" => json!(repos::migration_notice(
            root,
            args["command"].as_str().unwrap_or("te repos")
        )?),
        "repos.refold_moved" => json!(repos::refold_targets(&values(args, "done"))),
        "repos.finish_refolds" => {
            repos::finish_refolds(&values(args, "replies"), &env, home)?;
            Value::Null
        }
        "repos.settle" => json!(repos::settle(
            root,
            text(args, "host"),
            &repo(args, "repo")?,
            Path::new(text(args, "cwd")),
            &env,
            home,
            args.get("session_id")
        )?),
        "repos.how_words" => json!(repos::how_words(text(args, "how"), text(args, "remote"))),
        "repos.describe" => json!(repos::describe(&repo(args, "repo")?)),
        "repos.ignored_git_entries" => {
            json!(repos::ignored_git_entries(Path::new(text(args, "cwd")))?)
        }
        "repos.listing" => json!(repos::listing(
            root,
            &env,
            home,
            args.get("current")
                .filter(|v| !v.is_null())
                .map(|_| repo(args, "current"))
                .transpose()?
                .as_ref()
        )?),
        "repos.home_git_note" => json!(repos::home_git_note(&env, home)?),
        "applied._canon" => json!(applied::canonical(text(args, "path"))?),
        "applied.applied_by_path" => json!(applied::applied_by_path(&args["doc"])?),
        "applied.kept_by_path" => json!(applied::kept_by_path(&args["doc"])?),
        "applied.write_applied" => {
            let by = args["by_path"].as_object().cloned().unwrap_or_default();
            let kept = args["kept"].as_object();
            let st = store(args, "store")?;
            if args["write_transport"] == true {
                let doc = applied::document(&st, &by, kept, repos::now())?;
                let mut tape = CallbackTape::new(args);
                let result = tape
                    .io_call(json!({"callback":"store_write","name":applied::APPLIED,"obj":doc}))
                    .map(|_| Value::Null);
                tape.finish(storage_dispatch::response(result))
            } else {
                applied::write_applied(&st, &by, kept, repos::now())?;
                json!({"value":null})
            }
        }
        _ => return Err(format!("unknown repository operation: {op}")),
    };
    if [
        "repos._resolve",
        "repos.plan_dir",
        "repos._live",
        "repos.migrate_dir",
    ]
    .contains(&op)
    {
        Ok(json!({"value":value,"cache":cache}))
    } else {
        Ok(value)
    }
}
