// Converted from reference/upstream/tests/test_live_core.py (shell edits and verification).
use serde_json::{Value, json};
use std::sync::LazyLock;
use tokeneconomizer::{
    loopguard::{
        shell_edits::{shell_edit_targets, shell_revert_targets},
        state::{SpiralState, failure_sig},
        turn::{self, FoldOptions, TurnGuard},
    },
    plugin::hosts,
    rules,
};

fn cases(name: &str) -> &'static [Value] {
    static CASES: LazyLock<Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!("fixtures/live_core_shell.json")).unwrap()
    });
    CASES[name].as_array().unwrap()
}

fn fold(guard: &mut TurnGuard, name: &str, args: Value, output: &str, code: Option<i64>) {
    guard
        .fold(name, &args, output, code, FoldOptions::default())
        .unwrap();
}

#[test]
fn edits_made_through_the_shell_are_recognised() {
    for case in cases("edits_made_through_the_shell_are_recognised") {
        let expected: Vec<String> = serde_json::from_value(case[1].clone()).unwrap();
        assert_eq!(
            shell_edit_targets(case[0].as_str().unwrap()).unwrap(),
            expected,
            "{case}"
        );
    }
}

#[test]
fn a_git_command_run_with_dash_c_still_counts_its_edit_and_its_revert() {
    let revert = "git -C /w/repo checkout -- pkg/mod.py";
    assert_eq!(shell_revert_targets(revert).unwrap(), ["mod.py"]);
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "Bash",
        json!({"command":"git -C /w/repo apply /tmp/fix.diff"}),
        "",
        Some(0),
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"sed -i 's/a/b/' pkg/mod.py"}),
        "",
        Some(0),
    );
    fold(&mut guard, "Bash", json!({"command":revert}), "", Some(0));
    assert_eq!(guard.edits, 3);
    assert_eq!(
        json!(guard.whole.edit_counts),
        json!({"(patch)":1,"mod.py":2})
    );
}

#[test]
fn an_edit_run_through_a_wrapper_with_options_still_counts() {
    let revert = "sudo -u bob git checkout -- pkg/mod.py";
    assert_eq!(shell_revert_targets(revert).unwrap(), ["mod.py"]);
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "Bash",
        json!({"command":"env -u CURSOR_AGENT sed -i 's/a/b/' pkg/mod.py"}),
        "",
        Some(0),
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"timeout 60 sed -i 's/b/c/' pkg/mod.py"}),
        "",
        Some(0),
    );
    fold(&mut guard, "Bash", json!({"command":revert}), "", Some(0));
    assert_eq!(guard.edits, 3);
    assert_eq!(json!(guard.whole.edit_counts), json!({"mod.py":3}));
}

#[test]
fn an_edit_after_a_here_string_is_counted() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "Bash",
        json!({"command":"grep -q ok <<< ok\nsed -i 's/a/b/' pkg/mod.py"}),
        "",
        Some(0),
    );
    assert_eq!(guard.edits, 1);
    assert_eq!(json!(guard.whole.edit_counts), json!({"mod.py":1}));
}

#[test]
fn shell_edits_reach_the_state_the_judge_reads_and_the_ended_green_proxy() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    let edit = "python - <<'E'\np='pkg/mod.py'; s=open(p).read()\nopen(p,'w').write(s[::-1])\nE";
    for _ in 0..2 {
        fold(&mut guard, "Bash", json!({"command":edit}), "", None);
    }
    assert_eq!(guard.st.edit_churn(), 2);
    assert_eq!(guard.whole.distinct_edit_files(), 1);
    assert_eq!(guard.edits, 2);
    let digest = guard.st.recent_digest(24, None);
    assert!(digest.contains("edit_churn=2") && digest.contains("[edits mod.py]"));
    fold(
        &mut guard,
        "Bash",
        json!({"command":"sed -i 's/b/c/' pkg/mod.py && pytest -q"}),
        "FAILED tests/test_m.py::t - AssertionError",
        None,
    );
    assert_eq!(guard.edits, 3);
    assert!(guard.last_verify > guard.last_edit && !guard.resolved_proxy(true));
}

#[test]
fn a_sed_script_given_inside_its_option_word_still_counts_the_edit() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "Bash",
        json!({"command":"sed -i --expression='s/a/b/' pkg/mod.py"}),
        "",
        Some(0),
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"perl -i -pe's/b/c/' pkg/mod.py"}),
        "",
        Some(0),
    );
    assert_eq!(guard.edits, 2);
    assert_eq!(json!(guard.whole.edit_counts), json!({"mod.py":2}));
    assert!(guard.st.recent_digest(24, None).contains("[edits mod.py]"));
}

#[test]
fn an_edit_named_by_a_windows_path_counts_by_its_basename() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "Edit",
        json!({"file_path":r"C:\w\repo\pkg\mod.py","old_string":"a","new_string":"b"}),
        "ok",
        None,
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"git checkout -- pkg/mod.py"}),
        "",
        Some(0),
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"sed -i 's/a/b/' pkg/mod.py"}),
        "",
        Some(0),
    );
    assert_eq!(json!(guard.whole.edit_counts), json!({"mod.py":3}));
    assert_eq!(guard.whole.distinct_edit_files(), 1);
    assert!(!guard.st.recent_digest(24, None).contains("C:"));
}

#[test]
fn a_patch_that_changes_several_files_counts_each_of_them() {
    let patch = "*** Begin Patch\n*** Update File: pkg/a.py\n@@\n-x\n+y\n*** Update File: pkg/b.py\n@@\n-x\n+y\n*** Add File: tests/test_c.py\n+z\n*** End Patch";
    let (name, args, text, code) = hosts::tool_call(
        "codex",
        &json!({"tool_name":"apply_patch","tool_input":{"input":patch},"tool_response":"ok"}),
    )
    .unwrap();
    let mut tool = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut tool,
        name.as_str().unwrap(),
        args,
        &text,
        code.as_i64(),
    );
    let mut shell = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut shell,
        "Bash",
        json!({"command":format!("apply_patch <<'PATCH'\n{patch}\nPATCH")}),
        "ok",
        Some(0),
    );
    let expected = json!({"a.py":1,"b.py":1,"test_c.py":1});
    assert_eq!(json!(tool.whole.edit_counts), expected);
    assert_eq!(json!(shell.whole.edit_counts), expected);
    assert_eq!(tool.whole.distinct_edit_files(), 3);
    assert!(
        tool.st
            .recent_digest(24, None)
            .contains("edit a.py, b.py, test_c.py")
    );
}

#[test]
fn reading_a_file_full_of_asserts_is_not_a_failure() {
    let text = "def test_total():\n    assert total([1, 2]) == 3\n    raise ValueError('no')\n";
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    for _ in 0..3 {
        fold(
            &mut guard,
            "read",
            json!({"path":"tests/test_total.py"}),
            text,
            None,
        );
        fold(
            &mut guard,
            "grep",
            json!({"pattern":"assert"}),
            "tests/test_total.py:2:    assert total",
            None,
        );
    }
    fold(
        &mut guard,
        "bash",
        json!({"command":"cat tests/test_total.py"}),
        text,
        Some(0),
    );
    assert_eq!(
        (guard.whole.error_outputs, guard.whole.repeated_fail_sigs),
        (0, 0)
    );
    let mut thresholds = rules::scaled_thresholds("moderate");
    thresholds["gate_min_turns"] = json!(1);
    assert!(!rules::gate_for_judge(&guard.whole.signals(), &thresholds));
    fold(
        &mut guard,
        "bash",
        json!({"command":"pytest -q"}),
        "FAILED tests/test_total.py - AssertionError",
        Some(1),
    );
    fold(
        &mut guard,
        "bash",
        json!({"command":"pytest -q"}),
        "FAILED tests/test_total.py - AssertionError",
        None,
    );
    assert_eq!(
        (guard.whole.error_outputs, guard.whole.repeated_fail_sigs),
        (2, 1)
    );
    let mut state = SpiralState::default();
    state.update(
        "Read",
        &json!({"file_path":"x.py"}),
        "Error: File does not exist.",
        Some(true),
    );
    assert_eq!(state.error_outputs, 1);
}

#[test]
fn the_judge_is_told_how_the_last_verification_went_not_that_tests_are_red() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    let vendored =
        "turns=3\nrepo_context: ... = 1.2x that typical length. Tests are still NOT green. A";
    assert!(
        guard
            .live_digest(vendored)
            .contains("No test or verification run has been seen")
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"python bin/test sympy/core"}),
        "tests finished: 10 passed",
        None,
    );
    let told = guard.live_digest(vendored);
    assert!(told.contains("The last verification run passed.") && !told.contains("NOT green"));
    fold(
        &mut guard,
        "Edit",
        json!({"file_path":"/r/a.py","old_string":"a","new_string":"b"}),
        "ok",
        None,
    );
    assert_eq!(
        guard
            .live_digest(vendored)
            .matches("There have been edits since.")
            .count(),
        1
    );
}

#[test]
fn a_runner_named_by_path_counts_as_a_verification() {
    for command in cases("a_runner_named_by_path_counts_as_a_verification") {
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "FAILED (failures=1)",
            Some(1),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn a_runner_started_by_a_launcher_counts_as_a_verification() {
    for command in cases("a_runner_started_by_a_launcher_counts_as_a_verification") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            Some("own"),
            "{command}"
        );
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "FAILED (failures=1)",
            Some(1),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn a_runner_behind_env_or_nice_options_counts_as_a_verification() {
    for command in cases("a_runner_behind_env_or_nice_options_counts_as_a_verification") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            Some("own"),
            "{command}"
        );
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "FAILED (failures=1)",
            Some(1),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn env_and_nice_leave_what_they_run_to_decide() {
    for case in cases("env_and_nice_leave_what_they_run_to_decide") {
        assert_eq!(
            turn::status_of_tests(case[0].as_str().unwrap()).unwrap(),
            case[1].as_str(),
            "{case}"
        );
    }
}

#[test]
fn a_runner_behind_pythons_own_options_counts_as_a_verification() {
    for command in cases("a_runner_behind_pythons_own_options_counts_as_a_verification") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            Some("own"),
            "{command}"
        );
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "FAILED (failures=1)",
            Some(1),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn pythons_own_options_leave_what_it_runs_to_decide() {
    for command in cases("pythons_own_options_leave_what_it_runs_to_decide") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            None,
            "{command}"
        );
    }
}

#[test]
fn a_ruby_test_run_counts_as_a_verification() {
    for command in cases("a_ruby_test_run_counts_as_a_verification") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            Some("own"),
            "{command}"
        );
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/lib/a.rb","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "8 runs, 8 assertions, 1 failures, 0 errors, 0 skips",
            Some(1),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn ruby_that_runs_no_test_file_is_not_a_verification() {
    for command in cases("ruby_that_runs_no_test_file_is_not_a_verification") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            None,
            "{command}"
        );
    }
}

#[test]
fn a_piped_minitest_run_is_judged_by_its_summary() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "Edit",
        json!({"file_path":"/r/lib/a.rb","old_string":"x","new_string":"y"}),
        "ok",
        None,
    );
    fold(
        &mut guard,
        "Bash",
        json!({"command":"ruby test/a_test.rb 2>&1 | tail -5"}),
        "8 runs, 8 assertions, 1 failures, 0 errors, 0 skips",
        Some(0),
    );
    assert_eq!(guard.verifications, 1);
    assert!(!guard.resolved_proxy(true));
    fold(
        &mut guard,
        "Bash",
        json!({"command":"ruby test/a_test.rb 2>&1 | tail -5"}),
        "8 runs, 8 assertions, 0 failures, 0 errors, 0 skips",
        Some(0),
    );
    assert_eq!(guard.verifications, 2);
    assert!(guard.resolved_proxy(true));
}

#[test]
fn a_launcher_leaves_what_it_runs_to_decide() {
    for case in cases("a_launcher_leaves_what_it_runs_to_decide") {
        assert_eq!(
            turn::status_of_tests(case[0].as_str().unwrap()).unwrap(),
            case[1].as_str(),
            "{case}"
        );
    }
}

#[test]
fn a_file_named_like_a_runner_is_not_a_verification() {
    for command in cases("a_file_named_like_a_runner_is_not_a_verification") {
        assert!(
            !turn::VERIFY_RE.is_match(command.as_str().unwrap()).unwrap(),
            "{command}"
        );
    }
}

#[test]
fn a_non_test_command_mentioning_a_runner_is_not_a_verification() {
    for command in cases("a_non_test_command_mentioning_a_runner_is_not_a_verification") {
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":"pytest -q"}),
            "FAILED (failures=1)",
            Some(1),
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "ok",
            Some(0),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn a_heredoc_writing_a_script_runs_no_tests() {
    assert_eq!(
        turn::status_of_tests("cat <<EOF > ci.sh\nnpm test\nEOF").unwrap(),
        None
    );
}

#[test]
fn a_heredoc_line_naming_a_runner_is_not_a_verification() {
    for command in cases("a_heredoc_line_naming_a_runner_is_not_a_verification") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            None,
            "{command}"
        );
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "Edit",
            json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":"pytest -q"}),
            "FAILED (failures=1)",
            Some(1),
        );
        fold(
            &mut guard,
            "Bash",
            json!({"command":command}),
            "ok",
            Some(0),
        );
        assert_eq!(guard.verifications, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
}

#[test]
fn a_runner_beside_a_heredoc_is_still_judged_by_its_status() {
    for command in cases("a_runner_beside_a_heredoc_is_still_judged_by_its_status") {
        assert_eq!(
            turn::status_of_tests(command.as_str().unwrap()).unwrap(),
            Some("own"),
            "{command}"
        );
    }
}

#[test]
fn a_heredoc_that_ends_a_codex_script_is_read_inside_the_script() {
    for case in cases("a_heredoc_that_ends_a_codex_script_is_read_inside_the_script") {
        assert_eq!(
            turn::status_of_tests(case[0].as_str().unwrap()).unwrap(),
            case[1].as_str(),
            "{case}"
        );
    }
}

const TAP_FAILING: &str = "# tests 9\n# suites 0\n# pass 8\n# fail 1\n# cancelled 0";

#[test]
fn a_test_run_whose_status_was_replaced_is_judged_by_its_output() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    for _ in 0..3 {
        fold(
            &mut guard,
            "bash",
            json!({"command":"npm test 2>&1 | tail -n 8"}),
            TAP_FAILING,
            Some(0),
        );
    }
    assert_eq!(
        (guard.whole.error_outputs, guard.whole.repeated_fail_sigs),
        (3, 2)
    );
    assert!(!guard.resolved_proxy(true));
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "bash",
        json!({"command":"/bin/bash -lc 'cd /w && timeout 60 go test ./... 2>&1 | tail -3'"}),
        "--- FAIL: TestAdd (0.00s)\nFAIL\tacme/x\t0.01s",
        Some(0),
    );
    assert_eq!(guard.whole.error_outputs, 1);
    assert!(!guard.resolved_proxy(true));
    for command in [
        "python3 -m unittest tests.test_x; echo \"exit=$?\"",
        "npm test || true",
        "python tests/runtests.py app 2>&1 | tail -5",
    ] {
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "bash",
            json!({"command":command}),
            "FAILED (failures=1)\nexit=1",
            Some(0),
        );
        assert_eq!(guard.whole.error_outputs, 1, "{command}");
        assert!(!guard.resolved_proxy(true), "{command}");
    }
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "bash",
        json!({"command":"set -o pipefail; npm test | tail -n 8"}),
        TAP_FAILING,
        Some(0),
    );
    fold(
        &mut guard,
        "bash",
        json!({"command":"pytest -q | grep FAILED"}),
        "",
        Some(1),
    );
    assert_eq!(guard.whole.error_outputs, 0);
    assert!(guard.resolved_proxy(true));
}

#[test]
fn a_test_run_followed_by_other_commands_is_judged_by_its_runners_summary() {
    let diff = "diff --git a/m.py b/m.py\n+    raise ValueError('no')\n";
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    fold(
        &mut guard,
        "bash",
        json!({"command":"pytest -q; git diff"}),
        &format!("F.\n1 failed, 1 passed in 0.12s\n{diff}"),
        Some(0),
    );
    assert_eq!(guard.whole.error_outputs, 1);
    assert!(!guard.resolved_proxy(true));
    for code in [Some(1), Some(0), None] {
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "bash",
            json!({"command":"pytest -q\ngit diff --exit-code"}),
            &format!("2 passed in 0.10s\n{diff}"),
            code,
        );
        assert_eq!(guard.whole.error_outputs, 0, "{code:?}");
        assert!(guard.resolved_proxy(true), "{code:?}");
    }
}

#[test]
fn a_runners_summary_gives_its_verdict() {
    for case in cases("a_runners_summary_gives_its_verdict") {
        assert_eq!(
            turn::runner_verdict(case[0].as_str().unwrap()),
            case[1].as_bool(),
            "{case}"
        );
    }
}

#[test]
fn any_other_command_keeps_its_exit_status() {
    for command in cases("any_other_command_keeps_its_exit_status") {
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "bash",
            json!({"command":command}),
            "def f():\n    assert x\n    raise ValueError('no')\n",
            Some(0),
        );
        assert_eq!(guard.whole.error_outputs, 0, "{command}");
    }
}

#[test]
fn other_ecosystems_failing_test_summaries_count_as_failures() {
    for case in cases("other_ecosystems_failing_test_summaries_count_as_failures") {
        let mut guard = TurnGuard::new("t", &json!({})).unwrap();
        fold(
            &mut guard,
            "bash",
            json!({"command":"make test"}),
            case[0].as_str().unwrap(),
            None,
        );
        assert_eq!(
            guard.whole.error_outputs == 1,
            case[1].as_bool().unwrap(),
            "{case}"
        );
    }
}

#[test]
fn a_unittest_failure_is_known_by_its_traceback() {
    assert_eq!(
        failure_sig("FAIL: test_x (validators.tests.T)\nTraceback (most recent call last):\n  ..."),
        failure_sig("Traceback (most recent call last):")
    );
}
