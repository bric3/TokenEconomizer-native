use super::super::applied;
use super::*;
use crate::{
    envelope::ENVELOPE_HISTORY,
    names::repo_filename,
    store::StoreLock,
    value::{json_pretty, json_text},
};

pub fn write_lines(st: &Store, name: &str, records: &[Value]) -> Result<(), String> {
    let target = st.base.join(name);
    fs::create_dir_all(target.parent().ok_or("missing log parent")?).map_err(|e| e.to_string())?;
    let temporary = PathBuf::from(format!("{}.{}.tmp", target.display(), std::process::id()));
    let mut f = fs::File::create(&temporary).map_err(|e| e.to_string())?;
    for rec in records {
        let mut rec = rec.clone();
        rec.sort_all_objects();
        writeln!(f, "{}", json_text(&rec)).map_err(|e| e.to_string())?;
    }
    drop(f);
    fs::rename(temporary, target).map_err(|e| e.to_string())
}
pub fn rename_label(directory: &Path, label: &str) -> Result<(), String> {
    if label.is_empty() {
        return Ok(());
    }
    let want = repo_filename(label);
    for sub in ["envfacts_state", "manual_state"] {
        let d = directory.join(sub);
        let names: Vec<_> = list(&d)
            .into_iter()
            .filter(|n| n.ends_with(".json"))
            .collect();
        let target = d.join(format!("{want}.json"));
        if names.len() == 1 && names[0] != format!("{want}.json") && !target.exists() {
            fs::rename(d.join(&names[0]), &target).map_err(|e| e.to_string())?;
            let _ = (|| -> Result<(), String> {
                let bytes = fs::read(&target).map_err(|e| e.to_string())?;
                let mut doc: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if doc.is_object() && doc.get("repo").is_some() {
                    doc["repo"] = json!(label);
                    fs::write(&target, json_pretty(&doc, false)).map_err(|e| e.to_string())?;
                }
                Ok(())
            })();
        }
    }
    let d = directory.join("interface_state");
    let names: Vec<_> = list(&d)
        .into_iter()
        .filter(|n| d.join(n).is_dir())
        .collect();
    if names.len() == 1 && names[0] != want && !d.join(&want).exists() {
        fs::rename(d.join(&names[0]), d.join(want)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn move_path(src: &Path, dst: &Path) -> Result<(), String> {
    let target = if dst.is_dir() {
        dst.join(src.file_name().ok_or("missing moved name")?)
    } else {
        dst.into()
    };
    if fs::rename(src, &target).is_ok() {
        return Ok(());
    }
    copy_move(src, &target)
}

pub fn copy_move(src: &Path, target: &Path) -> Result<(), String> {
    if src.is_dir() && paths::realpath(target)?.starts_with(paths::realpath(src)?) {
        return Err("cannot move a directory into itself".into());
    }
    copy_path(src, target)?;
    let metadata = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(src)
    } else {
        fs::remove_file(src)
    }
    .map_err(|e| e.to_string())
}

fn copy_path(src: &Path, target: &Path) -> Result<(), String> {
    if fs::symlink_metadata(src)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        let link = fs::read_link(src).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(link, target).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            if src.is_dir() {
                std::os::windows::fs::symlink_dir(link, target)
            } else {
                std::os::windows::fs::symlink_file(link, target)
            }
            .map_err(|e| e.to_string())?;
        }
    } else if src.is_dir() {
        fs::create_dir(target).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_path(&entry.path(), &target.join(entry.file_name()))?;
        }
        copy_metadata(src, target)?;
    } else {
        fs::copy(src, target).map_err(|e| e.to_string())?;
        copy_metadata(src, target)?;
    }
    Ok(())
}
fn copy_metadata(src: &Path, dst: &Path) -> Result<(), String> {
    let metadata = fs::metadata(src).map_err(|e| e.to_string())?;
    fs::set_permissions(dst, metadata.permissions()).map_err(|e| e.to_string())?;
    let mut times = fs::FileTimes::new();
    if let Ok(accessed) = metadata.accessed() {
        times = times.set_accessed(accessed);
    }
    if let Ok(modified) = metadata.modified() {
        times = times.set_modified(modified);
    }
    fs::File::open(dst)
        .and_then(|f| f.set_times(times))
        .map_err(|e| e.to_string())
}
pub fn take_learned(src: &Store, dst: &Store, label: &str) -> Result<Option<PathBuf>, String> {
    let mut aside = None;
    for name in LEARNED {
        let p = dst.base.join(name);
        if p.is_dir()
            && fs::read_dir(&p)
                .map_err(|e| e.to_string())?
                .next()
                .is_none()
        {
            fs::remove_dir(p).map_err(|e| e.to_string())?;
        } else if fs::symlink_metadata(&p).is_ok() {
            if aside.is_none() {
                aside = Some(
                    dst.ensure(&[format!("premerge-{}", stamp())])
                        .map_err(|e| e.to_string())?,
                );
            }
            fs::rename(p, aside.as_ref().unwrap().join(name)).map_err(|e| e.to_string())?;
        }
    }
    for name in LEARNED {
        if src.base.join(name).exists() {
            dst.ensure(&[] as &[&str]).map_err(|e| e.to_string())?;
            move_path(&src.base.join(name), &dst.base.join(name))?;
        }
    }
    rename_label(&dst.base.join("mechanisms"), label)?;
    Ok(aside)
}
pub fn merge_applied(src: &Store, dst: &Store) -> Result<(), String> {
    let d = read(dst, applied::APPLIED)?;
    let s = read(src, applied::APPLIED)?;
    let mut by = applied::applied_by_path(&d)?;
    let mut kept = applied::kept_by_path(&d)?;
    for (target, source) in [
        (&mut by, applied::applied_by_path(&s)?),
        (&mut kept, applied::kept_by_path(&s)?),
    ] {
        for (path, tools) in source {
            let values: BTreeSet<_> = items(target.get(&path).unwrap_or(&Value::Null))
                .into_iter()
                .chain(items(&tools))
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            target.insert(path, json!(values));
        }
    }
    if !by.is_empty() || !kept.is_empty() {
        applied::write_applied(dst, &by, Some(&kept), now())?;
    }
    Ok(())
}
pub fn merge_envelope(src: &Store, dst: &Store) -> Result<(), String> {
    let a = read(src, "envelope.json")?;
    let b = read(dst, "envelope.json")?;
    if !truthy(&a) {
        return Ok(());
    }
    let mut merged = if truthy(&b) { b.clone() } else { a.clone() };
    for key in ["resolved_turns", "resolved_snaps"] {
        let mut values = items(&a[key]);
        values.extend(items(&b[key]));
        if values.len() > ENVELOPE_HISTORY {
            values.drain(..values.len() - ENVELOPE_HISTORY);
        }
        merged[key] = json!(values);
    }
    dst.write("envelope.json", &merged)
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn locks(stores: &[Store], timeout: f64) -> Result<Option<Vec<StoreLock>>, String> {
    let mut held = Vec::new();
    for st in stores {
        for name in ["fold", "envelope"] {
            let lock = st.locked(name, timeout).map_err(|e| e.to_string())?;
            if !lock.acquired {
                return Ok(None);
            }
            held.push(lock);
        }
    }
    Ok(Some(held))
}
pub fn merge_host(
    src: &Store,
    dst: &Store,
    sids: &[String],
    whole: bool,
    label: &str,
    timeout: f64,
) -> Result<Option<Value>, String> {
    let Some(_locks) = locks(&[src.clone(), dst.clone()], timeout)? else {
        return Ok(None);
    };
    let s_folded = object(&read(src, "folded.json")?);
    let d_folded = object(&read(dst, "folded.json")?);
    let s_sess = sessions(src)?;
    let d_sess = sessions(dst)?;
    let incoming: BTreeSet<_> = sids
        .iter()
        .filter(|sid| s_sess.contains_key(*sid))
        .cloned()
        .collect();
    let take =
        whole && has_learned(src)? && (!has_learned(dst)? || s_folded.len() > d_folded.len());
    let unlearned: BTreeSet<_> = if take {
        take_learned(src, dst, label)?;
        names(&d_folded)
            .difference(&names(&s_folded))
            .cloned()
            .collect()
    } else {
        incoming
            .intersection(&names(&s_folded))
            .filter(|sid| !d_folded.contains_key(*sid))
            .cloned()
            .collect()
    };
    let mut refold = BTreeSet::new();
    for sid in unlearned {
        let (folded, sess) = if incoming.contains(&sid) {
            (&s_folded, &s_sess)
        } else {
            (&d_folded, &d_sess)
        };
        let entry = folded.get(&sid).unwrap_or(&Value::Null);
        if imported(entry) {
            continue;
        }
        let tp = transcript_of(sess.get(&sid).unwrap_or(&Value::Null), entry);
        if tp
            .as_str()
            .is_some_and(|p| !p.is_empty() && Path::new(p).exists())
        {
            refold.insert(sid);
        }
    }
    let s_ledger = src
        .read_lines("ledger.jsonl", None)
        .map_err(|e| e.to_string())?;
    let d_ledger = dst
        .read_lines("ledger.jsonl", None)
        .map_err(|e| e.to_string())?;
    let mut session_records = indexmap::IndexMap::<String, Value>::new();
    let mut other = indexmap::IndexMap::<String, Value>::new();
    for row in d_ledger.into_iter().chain(
        s_ledger
            .iter()
            .filter(|r| {
                r["session_id"]
                    .as_str()
                    .is_some_and(|id| incoming.contains(id))
            })
            .cloned(),
    ) {
        let sid = row["session_id"].as_str();
        let key = json_text(&row["session_id"]);
        if row["kind"] == "session" {
            let prior = session_records.get(&key);
            if !sid.is_some_and(|id| refold.contains(id))
                && (prior.is_none()
                    || number(&row["folded_at"])? > number(&prior.unwrap()["folded_at"])?)
            {
                session_records.insert(key, row);
            }
        } else {
            let key = json_text(&json!([row["kind"], row["session_id"], row["at"]]));
            other.entry(key).or_insert(row);
        }
    }
    let mut ledger: Vec<_> = session_records
        .into_values()
        .chain(other.into_values())
        .map(|row| {
            let time = number(if truthy(&row["folded_at"]) {
                &row["folded_at"]
            } else {
                &row["at"]
            })?;
            Ok((time, row))
        })
        .collect::<Result<_, String>>()?;
    ledger.sort_by(|a, b| a.0.total_cmp(&b.0));
    write_lines(
        dst,
        "ledger.jsonl",
        &ledger.into_iter().map(|(_, r)| r).collect::<Vec<_>>(),
    )?;
    let mut folded: Map<_, _> = d_folded
        .iter()
        .filter(|(sid, _)| !refold.contains(*sid))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for sid in &incoming {
        if !refold.contains(sid)
            && let Some(entry) = s_folded.get(sid)
        {
            folded.insert(sid.clone(), entry.clone());
        }
    }
    dst.write("folded.json", &json!(folded))
        .map_err(|e| e.to_string())?;
    for sid in &incoming {
        let mut rec = s_sess.get(sid).cloned().unwrap_or(json!({}));
        if let Some(mine) = d_sess.get(sid).filter(|v| truthy(v))
            && number(&mine["updated"])? > number(&rec["updated"])?
        {
            rec = mine.clone();
        }
        if truthy(&rec) {
            if refold.contains(sid) {
                rec["folded"] = json!(false);
            }
            dst.write(&format!("sessions/{sid}.json"), &rec)
                .map_err(|e| e.to_string())?;
        }
    }
    for sid in refold.difference(&incoming) {
        if let Some(rec) = d_sess.get(sid).filter(|v| truthy(v)) {
            let mut rec = rec.clone();
            rec["folded"] = json!(false);
            dst.write(&format!("sessions/{sid}.json"), &rec)
                .map_err(|e| e.to_string())?;
        }
    }
    for row in src
        .read_lines("events.jsonl", None)
        .map_err(|e| e.to_string())?
    {
        if row["session_id"]
            .as_str()
            .is_some_and(|id| incoming.contains(id))
        {
            dst.append("events.jsonl", &row)
                .map_err(|e| e.to_string())?;
        }
    }
    if whole {
        merge_envelope(src, dst)?;
        merge_applied(src, dst)?;
        let notices = read(src, "notices.json")?;
        if truthy(&notices) && !truthy(&read(dst, "notices.json")?) {
            dst.write("notices.json", &notices)
                .map_err(|e| e.to_string())?;
        }
        let mut hist = read(src, "history.json")?;
        if hist.is_object() && !truthy(&read(dst, "history.json")?) {
            if [json!("running"), json!("partial")].contains(&hist["state"]) {
                hist.as_object_mut().unwrap().remove("plan");
                hist["state"] = json!("stopped");
                hist["reason"] = json!("its state was merged");
            }
            dst.write("history.json", &hist)
                .map_err(|e| e.to_string())?;
        }
    } else {
        write_lines(
            src,
            "ledger.jsonl",
            &s_ledger
                .into_iter()
                .filter(|r| {
                    !r["session_id"]
                        .as_str()
                        .is_some_and(|id| incoming.contains(id))
                })
                .collect::<Vec<_>>(),
        )?;
        src.write(
            "folded.json",
            &json!(
                s_folded
                    .into_iter()
                    .filter(|(sid, _)| !incoming.contains(sid))
                    .collect::<Map<_, _>>()
            ),
        )
        .map_err(|e| e.to_string())?;
        for sid in &incoming {
            let _ = fs::remove_file(src.base.join("sessions").join(format!("{sid}.json")));
        }
    }
    Ok(Some(json!({"moved":incoming.len(),"refold":refold.len()})))
}
pub fn merge_whole(
    root: &Path,
    src: &str,
    dst: &str,
    label: &str,
    timeout: f64,
) -> Result<Option<Value>, String> {
    let source = repo_dir(root, src);
    let destination = repo_dir(root, dst);
    let mut label = label.to_string();
    if label.is_empty() {
        label = dir_label(&destination)?;
    }
    if label.is_empty() {
        label = dir_label(&source)?;
    }
    let mut out = Map::new();
    for host in host_dirs(&source) {
        let s = store(&source.join(&host))?;
        let d = store(&destination.join(&host))?;
        let Some(r) = merge_host(
            &s,
            &d,
            &sessions(&s)?.keys().cloned().collect::<Vec<_>>(),
            true,
            &label,
            timeout,
        )?
        else {
            return Ok(None);
        };
        out.insert(host, r);
    }
    if source.join("config.json").exists() && !destination.join("config.json").exists() {
        fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
        fs::copy(source.join("config.json"), destination.join("config.json"))
            .map_err(|e| e.to_string())?;
    }
    let aside = root.join("merged").join(format!("{src}.{}", stamp()));
    fs::create_dir_all(aside.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::rename(&source, aside).map_err(|e| e.to_string())?;
    let st = store(root)?;
    let mut idx = json!(object(&read(&st, INDEX)?));
    let mut redirects: Map<_, _> = object(&idx["redirects"])
        .into_iter()
        .filter(|(k, v)| k != dst && v.is_object())
        .collect();
    redirects.insert(src.into(), json!({"to":dst,"at":now()}));
    idx["redirects"] = json!(redirects);
    st.write(INDEX, &idx).map_err(|e| e.to_string())?;
    Ok(Some(json!(out)))
}
pub fn resolve(
    cwd: &str,
    env: &Map<String, Value>,
    home: &Path,
    cache: &mut Map<String, Value>,
) -> Result<Option<RepoId>, String> {
    if cwd.is_empty() || !Path::new(cwd).is_dir() {
        return Ok(None);
    }
    if !cache.contains_key(cwd) {
        cache.insert(
            cwd.into(),
            json!(paths::repo_identity(Path::new(cwd), env, home)?),
        );
    }
    Ok(repo_from(&cache[cwd]))
}
pub fn plan_dir(
    root: &Path,
    key: &str,
    env: &Map<String, Value>,
    home: &Path,
    cache: &mut Map<String, Value>,
) -> Result<(Map<String, Value>, bool), String> {
    let mut groups = Map::new();
    let mut here = false;
    for host in host_dirs(&repo_dir(root, key)) {
        let st = store(&repo_dir(root, key).join(&host))?;
        let folded = read(&st, "folded.json")?;
        for (sid, rec) in sessions(&st)? {
            let cwd = session_cwd(&rec, &folded[&sid]);
            if let Some(repo) = resolve(
                cwd.as_ref().and_then(Value::as_str).unwrap_or(""),
                env,
                home,
                cache,
            )? {
                let group = groups
                    .entry(repo.key.clone())
                    .or_insert_with(|| json!({"repo":repo,"hosts":{}}));
                group["hosts"]
                    .as_object_mut()
                    .unwrap()
                    .entry(host.clone())
                    .or_insert(json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(json!(sid));
            }
            if !here && let Some(cwd) = rec["cwd"].as_str().filter(|s| !s.is_empty()) {
                here = resolve(cwd, env, home, cache)?.is_some_and(|r| r.key == key);
            }
        }
    }
    Ok((groups, here))
}
pub fn live(
    root_key: &str,
    env: &Map<String, Value>,
    home: &Path,
    cache: &mut Map<String, Value>,
    idx: &Value,
) -> Result<bool, String> {
    for (path, value) in object(&idx["checkouts"]) {
        if value["key"] == root_key
            && resolve(&path, env, home, cache)?.is_some_and(|r| r.key == root_key)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn migrate_dir(
    root: &Path,
    key: &str,
    env: &Map<String, Value>,
    home: &Path,
    timeout: f64,
    cache: &mut Map<String, Value>,
    is_live: bool,
) -> Result<Option<Value>, String> {
    let mut out = json!({"key":key,"moves":[],"into":null});
    if !repo_dir(root, key).is_dir() {
        return Ok(Some(out));
    }
    let (groups, here) = plan_dir(root, key, env, home, cache)?;
    if let Some(repo) = groups.get(key).and_then(|g| repo_from(&g["repo"])) {
        note_repo(root, &repo)?;
    }
    let contains_here = groups.contains_key(key);
    let away: Map<_, _> = groups.into_iter().filter(|(k, _)| k != key).collect();
    if away.is_empty() {
        return Ok(Some(out));
    }
    let is_live = is_live
        || here
        || contains_here
        || live(key, env, home, cache, &read(&store(root)?, INDEX)?)?;
    let mut whole_into = None;
    let mut largest = 0;
    if !is_live {
        for (key, group) in &away {
            let count = object(&group["hosts"])
                .values()
                .map(|v| items(v).len())
                .sum::<usize>();
            if whole_into.is_none() || count > largest {
                whole_into = Some(key.clone());
                largest = count;
            }
        }
    }
    let mut keys: Vec<_> = away.keys().cloned().collect();
    keys.sort();
    for destination in keys {
        if whole_into.as_ref() == Some(&destination) {
            continue;
        }
        let group = &away[&destination];
        let repo = repo_from(&group["repo"]).ok_or("invalid planned repository")?;
        for (host, sids) in object(&group["hosts"]) {
            let sids: Vec<String> = serde_json::from_value(sids).map_err(|e| e.to_string())?;
            let Some(mut r) = merge_host(
                &store(&repo_dir(root, key).join(&host))?,
                &store(&repo_dir(root, &destination).join(&host))?,
                &sids,
                false,
                &repo.label,
                timeout,
            )?
            else {
                return Ok(None);
            };
            r["host"] = json!(host);
            r["into"] = json!(destination);
            r["cwd"] = json!(repo.root);
            out["moves"].as_array_mut().unwrap().push(r);
        }
        note_repo(root, &repo)?;
    }
    if let Some(destination) = whole_into {
        let repo = repo_from(&away[&destination]["repo"]).ok_or("invalid planned repository")?;
        let Some(result) = merge_whole(root, key, &destination, &repo.label, timeout)? else {
            return Ok(None);
        };
        out["into"] = json!(destination);
        note_repo(root, &repo)?;
        for (host, mut r) in object(&result) {
            r["host"] = json!(host);
            r["into"] = json!(destination);
            r["cwd"] = json!(repo.root);
            out["moves"].as_array_mut().unwrap().push(r);
        }
    }
    Ok(Some(out))
}
