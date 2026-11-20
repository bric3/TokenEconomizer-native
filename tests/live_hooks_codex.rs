// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{HookFixture, fixture, session, transcript};
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};
use support::TempDir;
use tokeneconomizer::plugin::denylist::{self, toml};

#[derive(Default)]
struct ConfigCheck {
    rejection: Option<String>,
    refuse_writes: bool,
}

impl denylist::Runtime for ConfigCheck {
    fn codex_config_error(
        &mut self,
        _: &Path,
        cli: &str,
        _: f64,
    ) -> Result<Option<String>, denylist::Error> {
        assert_eq!(cli, "codex");
        Ok(self.rejection.take())
    }

    fn write_text(&mut self, path: &Path, text: &str) -> Result<(), denylist::Error> {
        if self.refuse_writes {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied).into());
        }
        denylist::Runtime::write_text(&mut denylist::NativeRuntime, path, text)
    }
}

fn codex(root: &Path) -> HookFixture {
    let mut f = fixture(root, "codex");
    f.fold_settings = Box::new(ConfigCheck::default());
    f
}

fn rollout(f: &HookFixture, index: usize, count: usize, exec_inputs: &[&str]) -> PathBuf {
    let mut rows = vec![json!({
        "type":"session_meta","payload":{"id":format!("thread-{index}"),"cwd":f.repo,"cli_version":"0.153.1"}
    })];
    for k in 1..=count {
        let turn = format!("t{k}");
        rows.push(json!({"type":"turn_context","payload":{"turn_id":turn,"model":"gpt-5.6-sol"}}));
        rows.push(json!({"type":"event_msg","payload":{"type":"task_started","turn_id":turn}}));
        if k != 1 {
            continue;
        }
        rows.push(json!({"type":"event_msg","payload":{"type":"item_completed","turn_id":turn,
            "item":{"type":"CommandExecution","id":format!("c{index}"),"command":["bash","-lc","ls"],
                "aggregated_output":"a.py","exit_code":0,"status":"completed"}
        }}));
        for (n, source) in exec_inputs.iter().enumerate() {
            rows.push(
                json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec",
                    "call_id":format!("x{index}-{n}"),"status":"completed","input":source
                }}),
            );
        }
    }
    rows.push(json!({"type":"event_msg","payload":{"type":"token_count","info":{
        "total_token_usage":{"input_tokens":3000,"cached_input_tokens":2000,"cache_write_input_tokens":0,
            "output_tokens":90,"reasoning_output_tokens":10}
    }}}));
    let path = transcript(&f.environment.home, &format!("rollout-{index}"));
    fs::write(
        &path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    path
}

fn fold(f: &mut HookFixture, index: usize, prompts: usize, commands: &[&str]) {
    let path = rollout(f, index, prompts, commands);
    session(f, &format!("cx{index}"), &path, "startup");
}

fn untool(f: &HookFixture) -> Value {
    f.store.read("untool.json", Value::Null).unwrap()
}

fn propose(f: &mut HookFixture) {
    f.environment
        .env
        .insert("TE_TOOLPRUNE".into(), json!("propose"));
}

fn config(f: &HookFixture) -> PathBuf {
    denylist::settings_path(&f.repo, "codex")
}

fn write_config(f: &HookFixture, text: &str) {
    let path = config(f);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn apply(f: &mut HookFixture, caps: &[&str]) -> Result<Vec<String>, denylist::Error> {
    let caps: Vec<_> = caps.iter().map(|cap| (*cap).into()).collect();
    denylist::apply(&f.repo, &caps, &f.store, "codex", f.fold_settings.as_mut())
}

fn restore(f: &mut HookFixture, caps: &[&str]) -> Vec<String> {
    let caps: Vec<_> = caps.iter().map(|cap| (*cap).into()).collect();
    denylist::restore(&f.repo, &f.store, &caps, "codex", f.fold_settings.as_mut()).unwrap()
}

fn applied(f: &HookFixture) -> Vec<String> {
    denylist::applied_for(&f.store, &f.repo, "codex").unwrap()
}

fn git(f: &HookFixture, args: &[&str]) {
    let result = Command::new("git")
        .current_dir(&f.repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn track_config(f: &HookFixture) {
    git(f, &["init", "-q"]);
    // The check reads the index, which includes the config once Git adds it.
    git(f, &["add", "-f", denylist::CODEX_CONFIG_REL]);
    assert!(denylist::git_tracked(&f.repo, denylist::CODEX_CONFIG_REL));
}

fn notice(f: &mut HookFixture) -> String {
    let path = rollout(f, 99, 1, &[]);
    let result = f.main(
        json!({"hook_event_name":"SessionStart","session_id":"next","source":"startup",
        "transcript_path":path,"cwd":f.repo}),
    );
    result["systemMessage"].as_str().unwrap().into()
}

const COLLAB_GOALS: [&str; 2] = ["collaboration", "goals"];
const ALL_CAPABILITIES: [&str; 3] = ["collaboration", "goals", "image_media"];
const USER_MODEL: &str = "model = \"gpt-5.6-sol\"\n";

#[test]
fn codex_proposes_capabilities_it_never_saw_used() {
    let tmp = TempDir::new("codex-hook-capabilities");
    let mut f = codex(tmp.path());
    propose(&mut f);
    for i in 0..5 {
        let commands: &[_] = if i == 2 {
            &["await tools.view_image({path: 'a.png'})"]
        } else {
            &[]
        };
        fold(&mut f, i, 2, commands);
    }
    let data = untool(&f);
    assert_eq!(data["proposal"], json!(COLLAB_GOALS));
    assert_eq!(data["capabilities"]["image_media"]["used"], true);
    assert_eq!(
        data["capabilities"]["goals"],
        json!({"loaded_sessions":5,"loaded_prompts":10,"unused_sessions":5,"used":false})
    );
    assert_eq!(data["sessions"], 5);
    assert_eq!(data["prompts"], 10);
    assert!(!config(&f).exists());
}

#[test]
fn codex_never_proposes_a_protected_capability() {
    let tmp = TempDir::new("codex-hook-protected");
    let mut f = codex(tmp.path());
    propose(&mut f);
    for i in 0..5 {
        fold(&mut f, i, 6, &[]);
    }
    assert_eq!(untool(&f)["proposal"], json!(ALL_CAPABILITIES));
    f.environment
        .env
        .insert("TE_TOOLPRUNE_PROTECT".into(), json!("collab*,image_media"));
    fold(&mut f, 5, 1, &[]);
    assert_eq!(untool(&f)["proposal"], json!(["goals"]));
}

#[test]
fn a_codex_package_applied_before_it_was_protected_comes_back() {
    let tmp = TempDir::new("codex-hook-return-package");
    let mut f = codex(tmp.path());
    assert_eq!(apply(&mut f, &COLLAB_GOALS).unwrap(), COLLAB_GOALS);
    propose(&mut f);
    f.environment
        .env
        .insert("TE_TOOLPRUNE_PROTECT".into(), json!("collab*"));
    fold(&mut f, 0, 1, &[]);
    assert_eq!(applied(&f), ["goals"]);
    let text = fs::read_to_string(config(&f)).unwrap();
    assert!(
        !text.contains("[agents]") && !text.contains("include_collaboration_mode_instructions")
    );
    assert!(text.contains("goals = false  # tokeneconomizer:goals"));
    let data = untool(&f);
    assert_eq!(data["capabilities"]["collaboration"]["loaded_sessions"], 0);
    assert_eq!(data["capabilities"]["image_media"]["loaded_sessions"], 1);
}

#[test]
fn codex_needs_the_full_evidence_before_proposing() {
    let tmp = TempDir::new("codex-hook-evidence");
    let mut f = codex(tmp.path());
    propose(&mut f);
    for i in 0..3 {
        fold(&mut f, i, 3, &[]);
    }
    assert_eq!(untool(&f)["proposal"], json!([]));
    fold(&mut f, 3, 1, &[]);
    assert_eq!(untool(&f)["proposal"], json!(COLLAB_GOALS));
}

#[test]
fn codex_apply_writes_marked_lines_and_restore_leaves_the_users_config() {
    let tmp = TempDir::new("codex-hook-markers");
    let mut f = codex(tmp.path());
    let user = "model_reasoning_effort = \"high\"\n\n[features]\nhooks = true\n\n[[skills.config]]\npath = \"/x/SKILL.md\"\nenabled = false\n";
    write_config(&f, user);
    assert_eq!(apply(&mut f, &ALL_CAPABILITIES).unwrap(), ALL_CAPABILITIES);
    let text = fs::read_to_string(config(&f)).unwrap();
    let lines: Vec<_> = text.lines().collect();
    let index = |line| lines.iter().position(|entry| *entry == line).unwrap();
    let first_feature = index("[features]");
    assert!(
        index("include_collaboration_mode_instructions = false  # tokeneconomizer:collaboration")
            < first_feature
    );
    let feats = &lines[first_feature + 1..index("hooks = true")];
    assert!(feats.contains(&"goals = false  # tokeneconomizer:goals"));
    assert!(feats.contains(&"view_image = false  # tokeneconomizer:image_media"));
    assert_eq!(
        &lines[lines.len() - 2..],
        [
            "[agents]  # tokeneconomizer:collaboration",
            "enabled = false  # tokeneconomizer:collaboration"
        ]
    );
    assert_eq!(restore(&mut f, &["goals"]), ["goals"]);
    assert!(
        !fs::read_to_string(config(&f))
            .unwrap()
            .contains("goals = false")
    );
    assert_eq!(restore(&mut f, &[]), ["collaboration", "image_media"]);
    assert_eq!(fs::read_to_string(config(&f)).unwrap(), user);
}

#[test]
fn a_config_codex_rejects_is_put_back() {
    let tmp = TempDir::new("codex-hook-config-rejected");
    let mut f = codex(tmp.path());
    write_config(&f, USER_MODEL);
    f.fold_settings = Box::new(ConfigCheck {
        rejection: Some("Error loading config.toml: duplicate key".into()),
        ..Default::default()
    });
    assert!(
        apply(&mut f, &["goals"])
            .unwrap_err()
            .to_string()
            .contains("put back")
    );
    let path = config(&f);
    assert_eq!(fs::read_to_string(&path).unwrap(), USER_MODEL);
    assert!(applied(&f).is_empty());
    fs::remove_file(&path).unwrap();
    f.fold_settings = Box::new(ConfigCheck {
        rejection: Some("bad".into()),
        ..Default::default()
    });
    assert!(apply(&mut f, &["goals"]).is_err());
    assert!(!path.exists());
}

#[test]
fn a_committed_codex_config_is_never_edited() {
    let tmp = TempDir::new("codex-hook-config-tracked");
    let mut f = codex(tmp.path());
    write_config(&f, USER_MODEL);
    track_config(&f);
    assert!(
        apply(&mut f, &["goals"])
            .unwrap_err()
            .to_string()
            .contains("committed")
    );
    assert_eq!(fs::read_to_string(config(&f)).unwrap(), USER_MODEL);
}

#[test]
fn by_default_codex_writes_the_unused_capabilities_off_and_says_so() {
    let tmp = TempDir::new("codex-hook-auto-capability");
    let mut f = codex(tmp.path());
    for i in 0..5 {
        fold(&mut f, i, 2, &[]);
    }
    let text = fs::read_to_string(config(&f)).unwrap();
    assert!(
        text.contains("include_collaboration_mode_instructions = false")
            && text.contains("goals = false")
    );
    let message = notice(&mut f);
    assert!(message.contains("removed 2 capability(s)") && message.contains("toolprune restore"));
}

#[test]
fn a_committed_codex_config_keeps_the_default_a_proposal_and_says_why() {
    let tmp = TempDir::new("codex-hook-tracked-proposal");
    let mut f = codex(tmp.path());
    write_config(&f, USER_MODEL);
    track_config(&f);
    for i in 0..5 {
        fold(&mut f, i, 2, &[]);
    }
    assert_eq!(fs::read_to_string(config(&f)).unwrap(), USER_MODEL);
    let message = notice(&mut f);
    assert!(
        message.contains("2 capability(s) went unused") && message.contains("could not be applied")
    );
    assert!(message.contains("is committed in this repository"));
}

#[test]
fn codex_never_overrides_a_key_the_user_set() {
    let user = "[features]\ngoals = true\n";
    let (text, changed) = toml::insert(user, Some("features"), "goals", "false", "# t");
    assert!(!changed);
    assert_eq!(text, user);
    let (_, changed) = toml::insert(
        "features.goals = true\n",
        Some("features"),
        "goals",
        "false",
        "# t",
    );
    assert!(!changed);
    let (text, changed) = toml::insert(
        "features.hooks = true\n",
        Some("features"),
        "view_image",
        "false",
        "# t",
    );
    assert!(changed);
    assert!(!text.contains("[features]") && text.contains("features.view_image = false  # t"));
    let inline = "features = { hooks = true }\n";
    let (text, changed) = toml::insert(inline, Some("features"), "goals", "false", "# t");
    assert!(!changed);
    assert_eq!(text, inline);
    assert_eq!(
        toml::remove(
            "[agents]  # t\nenabled = false  # t\nmax_threads = 4\n",
            &["# t".into()]
        ),
        "[agents]  # t\nmax_threads = 4\n"
    );
}

#[test]
fn codex_apply_does_not_record_a_capability_the_user_already_set() {
    let tmp = TempDir::new("codex-hook-user-capability");
    let mut f = codex(tmp.path());
    let user = "[features]\ngoals = true\n";
    write_config(&f, user);
    assert!(apply(&mut f, &["goals"]).unwrap().is_empty());
    assert!(applied(&f).is_empty());
    assert_eq!(fs::read_to_string(config(&f)).unwrap(), user);
    assert_eq!(
        apply(&mut f, &["goals", "image_media"]).unwrap(),
        ["image_media"]
    );
    assert_eq!(applied(&f), ["image_media"]);
}

#[test]
fn a_protected_codex_package_in_a_committed_or_unreadable_config_stays() {
    let tmp = TempDir::new("codex-hook-protected-config");
    let mut f = codex(tmp.path());
    apply(&mut f, &COLLAB_GOALS).unwrap();
    let before = fs::read_to_string(config(&f)).unwrap();
    track_config(&f);
    propose(&mut f);
    f.environment
        .env
        .insert("TE_TOOLPRUNE_PROTECT".into(), json!("collaboration"));
    fold(&mut f, 0, 1, &[]);
    assert_eq!(fs::read_to_string(config(&f)).unwrap(), before);
    git(&f, &["rm", "--cached", "--", denylist::CODEX_CONFIG_REL]);
    f.fold_settings = Box::new(ConfigCheck {
        refuse_writes: true,
        ..Default::default()
    });
    fold(&mut f, 1, 1, &[]);
    assert_eq!(untool(&f)["sessions"], 2);
    assert_eq!(fs::read_to_string(config(&f)).unwrap(), before);
}
