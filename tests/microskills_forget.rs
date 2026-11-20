// Converted from reference/upstream/tests/test_microskills_forget.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{CLAIM, FID, HookFixture, claude_log, cli, fixture, session, transcript};
use serde_json::{Value, json};
use std::{fs, io, path::PathBuf};
use support::TempDir;
use tokeneconomizer::{
    microskills::{
        distiller::{Distiller, Fact},
        envfacts::EnvFactsDistiller,
        observe::Vocabulary,
        stage::{SavedFactError, forget_facts},
    },
    plugin::cli::expand_argline,
    store::{write_json_atomic, write_json_atomic_with},
};

const REPO: &str = "acme/widget";

fn learn(f: &mut HookFixture, suffix: &str) -> String {
    let mut last_start = Value::Null;
    for i in 0..6 {
        let original = format!("s{i}");
        let path = transcript(&f.environment.home, &original);
        if !path.exists() {
            claude_log(&f.environment.home, f, &original, i, false, 1);
        }
        last_start = session(f, &format!("{original}{suffix}"), &path, "startup");
    }
    last_start["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn te(f: &HookFixture, args: &[&str]) -> (i32, String) {
    let output = cli(f, args);
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

fn env_state_path(f: &HookFixture) -> PathBuf {
    EnvFactsDistiller::path(&f.store.base.join("mechanisms"), REPO).unwrap()
}

fn injection(f: &HookFixture) -> Value {
    f.store.read("injection.json", Value::Null).unwrap()
}

#[test]
fn forget_takes_a_fact_out_of_the_block_and_it_is_not_learned_again() {
    let tmp = TempDir::new("learn-forget");
    let mut f = fixture(tmp.path(), "claude");
    assert!(learn(&mut f, "").contains(CLAIM));
    let (code, out) = te(&f, &["microskills"]);
    assert_eq!(code, 0);
    assert!(out.contains(FID));

    let (code, out) = te(&f, &["microskills", "forget", FID]);
    assert_eq!(code, 0);
    assert!(out.contains(&format!("forgotten: {FID}")));
    let saved = injection(&f);
    assert!(!saved["rendered"].as_str().unwrap().contains(CLAIM));
    assert!(
        !saved["rendered_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(FID))
    );
    assert!(saved["facts"].get(FID).is_none());

    assert!(!learn(&mut f, "-again").contains(CLAIM));
    assert!(!injection(&f)["rendered"].as_str().unwrap().contains(CLAIM));
    let state = live_fixture::env_state(&f);
    assert!(state["facts"].get(FID).is_none());
    assert!(state["forgotten"].get(FID).is_some());
    let (code, out) = te(&f, &["microskills"]);
    assert_eq!(code, 0);
    assert!(out.split_once("forgotten (").unwrap().1.contains(FID));
}

#[test]
fn forget_is_idempotent_and_unforget_brings_the_fact_back() {
    let tmp = TempDir::new("unforget-life");
    let mut f = fixture(tmp.path(), "claude");
    learn(&mut f, "");
    let life = live_fixture::env_state(&f)["facts"][FID]["life"].clone();
    assert_eq!(te(&f, &["microskills", "forget", FID]).0, 0);
    let (code, out) = te(&f, &["microskills", "forget", FID]);
    assert_eq!(code, 0);
    assert!(out.contains(&format!("already forgotten: {FID}")));

    let (code, out) = te(&f, &["microskills", "unforget", FID]);
    assert_eq!(code, 0);
    assert!(out.contains(&format!("brought back: {FID}")));
    assert!(injection(&f)["rendered"].as_str().unwrap().contains(CLAIM));
    let saved = live_fixture::env_state(&f);
    assert_eq!(saved["facts"][FID]["life"], life);
    assert_eq!(saved["forgotten"], json!({}));
    let (code, out) = te(&f, &["microskills", "unforget", FID]);
    assert_eq!(code, 0);
    assert!(out.contains("not forgotten"));
}

#[test]
fn unforget_without_a_record_says_nothing_came_back() {
    let tmp = TempDir::new("unforget-no-record");
    let mut f = fixture(tmp.path(), "claude");
    learn(&mut f, "");
    assert_eq!(te(&f, &["microskills", "forget", FID]).0, 0);
    let mut state = live_fixture::env_state(&f);
    state["forgotten"][FID]
        .as_object_mut()
        .unwrap()
        .shift_remove("record");
    write_json_atomic(&env_state_path(&f), &state, false).unwrap();

    let (code, out) = te(&f, &["microskills", "unforget", FID]);
    assert_eq!(code, 0);
    assert!(out.contains("nothing to bring back"));
    assert!(!out.contains("brought back"));
    let saved = live_fixture::env_state(&f);
    assert!(saved["forgotten"].get(FID).is_none());
    assert!(saved["facts"].get(FID).is_none());
}

#[test]
fn an_unknown_id_is_an_error_and_changes_nothing() {
    let tmp = TempDir::new("forget-unknown");
    let mut f = fixture(tmp.path(), "claude");
    learn(&mut f, "");
    let path = env_state_path(&f);
    let before = fs::read(&path).unwrap();
    let inj = injection(&f);
    let (code, out) = te(&f, &["microskills", "forget", FID, "no-such-fact"]);
    assert_eq!(code, 1);
    assert!(out.contains("no-such-fact") && out.contains("not changed"));
    assert_eq!(fs::read(path).unwrap(), before);
    assert_eq!(injection(&f), inj);
    assert_eq!(te(&f, &["microskills", "unforget", "no-such-fact"]).0, 1);
    assert_eq!(te(&f, &["microskills", "forget"]).0, 2);
}

#[test]
fn unknown_id_with_nothing_learned() {
    let tmp = TempDir::new("forget-empty");
    let f = fixture(tmp.path(), "claude");
    let (code, out) = te(&f, &["microskills", "forget", "anything"]);
    assert_eq!(code, 1);
    assert!(out.contains("anything"));
}

#[test]
fn state_from_before_forgetting_still_loads() {
    let tmp = TempDir::new("forget-old-envfacts");
    let mut f = fixture(tmp.path(), "claude");
    learn(&mut f, "");
    let mut state = live_fixture::env_state(&f);
    state.as_object_mut().unwrap().shift_remove("forgotten");
    write_json_atomic(&env_state_path(&f), &state, false).unwrap();
    let loaded = EnvFactsDistiller::load(
        REPO,
        &f.store.base.join("mechanisms"),
        Vocabulary::default(),
        None,
        None,
    )
    .unwrap();
    assert!(loaded.forgotten.is_empty());
    assert!(loaded.facts.contains_key(FID));
    assert_eq!(te(&f, &["microskills", "forget", FID]).0, 0);
    assert!(live_fixture::env_state(&f)["forgotten"].get(FID).is_some());
}

fn fact(id: &str) -> Fact {
    Fact::new(&json!({"id":id,"claim":format!("claim of {id}"),"injection":format!("say {id}"),"detector":{}})).unwrap()
}

#[test]
fn a_forgotten_proposed_fact_is_not_admitted_again() {
    let tmp = TempDir::new("forget-proposed");
    let mut distiller = Distiller::new(REPO, "agent", false);
    let mut proposed = fact("entrypoint");
    proposed.life.state = "ACTIVE".into();
    distiller.facts.insert("entrypoint".into(), proposed);
    distiller.save(tmp.path()).unwrap();
    let ids = ["entrypoint".into()];
    let vocab = Vocabulary::default();
    assert_eq!(
        forget_facts(REPO, tmp.path(), &ids, &vocab, false).unwrap(),
        [(
            "entrypoint".into(),
            json!(true),
            json!("claim of entrypoint")
        )]
    );
    let mut loaded = Distiller::load(REPO, tmp.path(), "agent", false).unwrap();
    assert!(!loaded.facts.contains_key("entrypoint"));
    assert!(loaded.forgotten.contains_key("entrypoint"));
    assert_eq!(
        loaded
            .admit(&mut json!({"id":"entrypoint","claim":"again","cite":[]}))
            .unwrap(),
        (false, "forgotten".into())
    );
    assert!(!loaded.render().contains("say entrypoint"));
    assert!(matches!(
        forget_facts(REPO, tmp.path(), &["entrypoint".into(),"unknown".into()], &vocab, false),
        Err(SavedFactError::UnknownIds(ids)) if ids == ["unknown"]
    ));
    assert_eq!(
        forget_facts(REPO, tmp.path(), &ids, &vocab, false).unwrap()[0].1,
        false
    );
}

#[test]
fn proposer_state_without_the_forgotten_key_loads() {
    let tmp = TempDir::new("forget-old-proposer");
    let mut distiller = Distiller::new(REPO, "agent", false);
    distiller.facts.insert("x".into(), fact("x"));
    distiller.save(tmp.path()).unwrap();
    let path = Distiller::path(tmp.path(), REPO).unwrap();
    let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    saved.as_object_mut().unwrap().shift_remove("forgotten");
    write_json_atomic(&path, &saved, false).unwrap();
    let loaded = Distiller::load(REPO, tmp.path(), "agent", false).unwrap();
    assert!(loaded.forgotten.is_empty());
    assert_eq!(loaded.facts.keys().collect::<Vec<_>>(), ["x"]);
}

#[test]
fn a_failed_write_leaves_the_state_as_it_was() {
    let tmp = TempDir::new("forget-atomic-write");
    let path = tmp.path().join("state.json");
    write_json_atomic(&path, &json!({"a":1}), false).unwrap();
    // The Rust writer takes a JSON value. Fail at replacement to exercise cleanup.
    let failed = write_json_atomic_with(&path, &json!({"a":2}), false, cfg!(windows), |_, _| {
        Err(io::Error::other("cannot replace state.json"))
    });
    assert_eq!(failed.unwrap_err().to_string(), "cannot replace state.json");
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved, json!({"a":1}));
    assert_eq!(
        fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>(),
        ["state.json"]
    );
}

fn expand(args: &[&str]) -> Vec<String> {
    expand_argline(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>()).unwrap()
}

#[test]
fn argline_keeps_a_quoted_id_with_spaces_as_one_argument() {
    let line = r#"microskills forget "form_repair:run_tests:python -m pytest" other-id"#;
    assert_eq!(
        expand(&["--host", "pi", "--argline", line]),
        [
            "--host",
            "pi",
            "microskills",
            "forget",
            "form_repair:run_tests:python -m pytest",
            "other-id"
        ]
    );
    assert_eq!(expand(&["--argline=status"]), ["status"]);
    assert_eq!(expand(&["status"]), ["status"]);
}

#[test]
fn argline_keeps_backslashes_as_typed() {
    let line = r"config set repomap_python=C:\Python312\python.exe";
    assert_eq!(
        expand(&["--argline", line]),
        ["config", "set", r"repomap_python=C:\Python312\python.exe"]
    );
    let line = r#"microskills forget "C:\a b" x"#;
    assert_eq!(
        expand(&["--argline", line]),
        ["microskills", "forget", r"C:\a b", "x"]
    );
}

#[test]
fn an_unclosed_quote_in_argline_changes_nothing() {
    let tmp = TempDir::new("forget-unclosed");
    let f = fixture(tmp.path(), "claude");
    let before = injection(&f);
    assert_eq!(te(&f, &["--argline", "microskills forget \"unclosed"]).0, 2);
    assert_eq!(injection(&f), before);
}
