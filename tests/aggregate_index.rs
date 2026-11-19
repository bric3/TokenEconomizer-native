// Converted from reference/upstream/experiments/tests/test_aggregate_index.py.
mod support;

use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::experiments::aggregate;

fn row(root: &Path, arm: &str, stamp: &str, instance: &str, updates: Value) {
    let directory = root
        .join(arm)
        .join("sympy_s0")
        .join(format!("{stamp}__{instance}__{arm}__standalone"));
    fs::create_dir_all(&directory).unwrap();
    let mut row = json!({"instance":instance,"repo":"sympy/sympy","seed":0,"arm":arm,
        "resolved":true,"num_turns":10,"tokens_total":1000,"input_tokens":10,"output_tokens":100,
        "cache_read_input_tokens":800,"cache_creation_input_tokens":90,"model":"gpt-5.6-terra",
        "effort":"high","agent":"codex","run_dir":directory});
    row.as_object_mut()
        .unwrap()
        .extend(updates.as_object().unwrap().clone());
    fs::write(directory.join("row.json"), row.to_string()).unwrap();
}

fn tree(root: &TempDir) -> PathBuf {
    let root = root.path().join("codex_swebench_gpt56terra");
    row(
        &root,
        "baseline",
        "20260916T000000Z",
        "sympy-11618",
        json!({}),
    );
    row(
        &root,
        "baseline",
        "20260916T000100Z",
        "sympy-12096",
        json!({}),
    );
    row(
        &root,
        "repomap",
        "20260916T010000Z",
        "sympy-11618",
        json!({}),
    );
    root
}

fn run(root: &Path, extra: &[&str]) -> i32 {
    let mut args = vec![root.to_str().unwrap().into()];
    args.extend(extra.iter().map(|arg| (*arg).to_owned()));
    aggregate::command("te-bench aggregate", &args)
        .unwrap()
        .exit
        .unwrap_or(0)
}

fn csv(path: &Path) -> Vec<BTreeMap<String, String>> {
    let text = fs::read_to_string(path).unwrap();
    // These synthetic rows contain no quoted CSV fields.
    assert!(!text.contains('"'));
    let mut lines = text.lines();
    let headers: Vec<_> = lines.next().unwrap().split(',').collect();
    lines
        .map(|line| {
            let values: Vec<_> = line.split(',').collect();
            assert_eq!(headers.len(), values.len());
            headers
                .iter()
                .zip(values)
                .map(|(key, value)| ((*key).into(), value.into()))
                .collect()
        })
        .collect()
}

fn cells(index: &Path) -> Vec<(String, String, String)> {
    csv(&index.join("summary_by_cell.csv"))
        .into_iter()
        .map(|row| {
            (
                row["arm"].clone(),
                row["repo"].clone(),
                row["n_tasks"].clone(),
            )
        })
        .collect()
}

fn expected() -> Vec<(String, String, String)> {
    vec![
        ("baseline".into(), "sympy".into(), "2".into()),
        ("repomap".into(), "sympy".into(), "1".into()),
    ]
}

#[test]
fn whole_tree_is_the_default() {
    let temporary = TempDir::new("aggregate-whole");
    let root = tree(&temporary);
    assert_eq!(run(&root, &[]), 0);
    assert_eq!(cells(&root.join("index")), expected());
}

#[test]
fn subset_rewrite_of_the_canonical_index_is_refused() {
    let temporary = TempDir::new("aggregate-refused");
    let root = tree(&temporary);
    assert_eq!(run(&root, &[]), 0);
    let before = cells(&root.join("index"));
    assert_ne!(run(&root, &["--arms", "repomap"]), 0);
    assert_eq!(cells(&root.join("index")), before);
}

#[test]
fn subset_is_allowed_somewhere_of_its_own() {
    let temporary = TempDir::new("aggregate-apart");
    let root = tree(&temporary);
    assert_eq!(run(&root, &[]), 0);
    let before = cells(&root.join("index"));
    let out = temporary.path().join("repomap_only");
    assert_eq!(
        run(
            &root,
            &["--arms", "repomap", "--out", out.to_str().unwrap()]
        ),
        0
    );
    assert_eq!(
        cells(&out),
        [("repomap".into(), "sympy".into(), "1".into())]
    );
    assert_eq!(cells(&root.join("index")), before);
}

#[test]
fn naming_every_arm_is_not_a_subset() {
    let temporary = TempDir::new("aggregate-all-arms");
    let root = tree(&temporary);
    assert_eq!(run(&root, &["--arms", "baseline", "repomap"]), 0);
    assert_eq!(cells(&root.join("index")), expected());
}

#[test]
fn superseded_run_dirs_are_not_double_counted() {
    let temporary = TempDir::new("aggregate-superseded");
    let root = tree(&temporary);
    row(
        &root,
        "repomap",
        "20260916T020000Z",
        "sympy-11618",
        json!({"num_turns":99}),
    );
    assert_eq!(run(&root, &[]), 0);
    assert_eq!(cells(&root.join("index")), expected());
    let turns: Vec<_> = csv(&root.join("index/all_rows.csv"))
        .into_iter()
        .filter(|row| row["arm"] == "repomap")
        .map(|row| row["num_turns"].clone())
        .collect();
    assert_eq!(turns, ["99"]);
}
