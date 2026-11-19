// Converted from reference/upstream/tests/test_repo_key_env.py.
mod support;

use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::plugin::paths::{self, BY_DIR, BY_PATH, BY_REMOTE};

fn git_repo(root: &Path, remote: Option<&str>) -> PathBuf {
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let remote = remote.map_or_else(String::new, |url| {
        format!("[remote \"origin\"]\n\turl = {url}\n")
    });
    fs::write(
        root.join(".git/config"),
        format!("[core]\n\trepositoryformatversion = 0\n{remote}"),
    )
    .unwrap();
    root.into()
}

fn environment(root: &TempDir, named: Option<&str>) -> Map<String, Value> {
    let home = root.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let mut env = json!({"TOKENECONOMIZER_HOME":root.path().join("te"), "HOME":home})
        .as_object()
        .unwrap()
        .clone();
    if let Some(named) = named {
        env.insert("TOKENECONOMIZER_REPO".into(), json!(named));
    }
    env
}

#[test]
fn a_named_remote_keys_an_export_without_git_metadata_as_its_clones() {
    let root = TempDir::new("named-repo");
    let env = environment(&root, None);
    let named = environment(&root, Some("git@github.com:Microsoft/TokenEconomizer.git"));
    let clone = git_repo(
        &root.path().join("TokenEconomizer"),
        Some("https://github.com/microsoft/TokenEconomizer"),
    );
    let export = root.path().join("runs/job-1/work");
    fs::create_dir_all(&export).unwrap();
    let repo = paths::repo_identity(&export, &named, root.path()).unwrap();
    assert_eq!(
        repo.key,
        paths::repo_identity(&clone, &env, root.path()).unwrap().key
    );
    assert_eq!(repo.how, BY_REMOTE);
    assert_eq!(repo.root, export);
    assert_eq!(repo.label, "microsoft/tokeneconomizer");
    assert_eq!(repo.remote, "github.com/microsoft/tokeneconomizer");
    let other = root.path().join("runs/job-2/work");
    fs::create_dir_all(&other).unwrap();
    assert_eq!(
        paths::repo_identity(&other, &named, root.path())
            .unwrap()
            .key,
        repo.key
    );
    let plain = paths::repo_identity(&other, &env, root.path()).unwrap();
    assert_eq!(plain.how, BY_DIR);
    assert_ne!(plain.key, repo.key);
}

#[test]
fn an_empty_value_changes_nothing_and_a_checkouts_own_remote_wins() {
    let root = TempDir::new("own-remote");
    let env = environment(&root, None);
    let export = root.path().join("work");
    fs::create_dir(&export).unwrap();
    assert_eq!(
        paths::repo_identity(&export, &environment(&root, Some("")), root.path())
            .unwrap()
            .how,
        BY_DIR
    );
    let named = environment(&root, Some("https://github.com/microsoft/TokenEconomizer"));
    let elsewhere = git_repo(
        &root.path().join("elsewhere"),
        Some("https://example.org/team/elsewhere.git"),
    );
    assert_eq!(
        paths::repo_identity(&elsewhere, &named, root.path()).unwrap(),
        paths::repo_identity(&elsewhere, &env, root.path()).unwrap()
    );
    let fresh = git_repo(&root.path().join("fresh"), None);
    assert_eq!(
        paths::repo_identity(&fresh, &env, root.path()).unwrap().how,
        BY_PATH
    );
    assert_eq!(
        paths::repo_identity(&fresh, &named, root.path())
            .unwrap()
            .key,
        paths::repo_identity(&export, &named, root.path())
            .unwrap()
            .key
    );
}
