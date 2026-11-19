// Converted from reference/upstream/experiments/tests/test_adapters.py.
mod support;

use serde_json::{Map, json};
use std::{fs, path::Path};
use support::TempDir;
use tokeneconomizer::experiments::{adapters::Adapter, facade::DomainAdapter};

#[test]
fn swebench_is_domain_adapter() {
    let adapter = Adapter::swebench(None, &Map::new());
    let _: &dyn DomainAdapter = &adapter;
}

#[test]
fn all_adapters_importable_and_named() {
    assert_eq!(Adapter::swebench(None, &Map::new()).name(), "swebench");
    assert_eq!(
        Adapter::webbench("dom".into(), None, &Map::new()).name(),
        "webbench"
    );
    assert_eq!(
        Adapter::creddata(None, None, &Map::new()).name(),
        "creddata"
    );
    assert_eq!(
        Adapter::reposvul(None, None, &Map::new()).name(),
        "reposvul"
    );
}

#[test]
fn swebench_reading_methods_delegate() {
    let adapter = Adapter::swebench(None, &Map::new());
    let _ = adapter.protected_tools();
    let _ = adapter.inapplicable_tools();
    assert!(
        adapter
            .parse_catalog(Path::new("/nonexistent"), "codex")
            .contains_key("shell")
    );
}

#[test]
fn no_hardcoded_dataset_roots() {
    assert_eq!(
        Adapter::creddata(None, None, &Map::new())
            .repo_ids()
            .unwrap_err()
            .kind(),
        "RuntimeError"
    );
    assert_eq!(
        Adapter::reposvul(None, None, &Map::new())
            .instance_ids()
            .unwrap_err()
            .kind(),
        "RuntimeError"
    );
}

#[test]
fn swebench_problem_statements_requires_config() {
    assert_eq!(
        Adapter::swebench(None, &Map::new())
            .problem_statements()
            .unwrap_err()
            .kind(),
        "RuntimeError"
    );
}

#[test]
fn swebench_problem_statements_from_file() {
    let root = TempDir::new("adapter-statements");
    let path = root.path().join("tasks.json");
    fs::write(&path,json!([{"instance_id":"django-1","problem_statement":"see https://github.com/django/django/pull/42 for the fix"}]).to_string()).unwrap();
    let statements = Adapter::swebench(Some(path), &Map::new())
        .problem_statements()
        .unwrap();
    assert!(statements.contains_key("django-1"));
    assert!(
        statements["django-1"]
            .as_str()
            .unwrap()
            .contains("[link redacted]")
    );
}

#[test]
fn creddata_classify_domain_differs() {
    assert!(
        Adapter::swebench(None, &Map::new())
            .classify_domain()
            .is_none()
    );
    assert!(
        Adapter::creddata(None, None, &Map::new())
            .classify_domain()
            .is_some()
    );
}
