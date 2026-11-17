//JAVA 25
//SOURCES ExtractProposerPrompts.java AdapterProbe.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.ArrayList;

/** Compares native experiment spiral policy with the pinned implementation. */
class SpiralProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  static int cases;
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var expected = source.call(request); var actual = nativeCode.call(request);
    if (!expected.equals(actual)) throw new AssertionError(request + "\nNative: " + actual + "\nSource: " + expected);
    cases++;
  }
  static String call(String method, String arguments) { return "{\"method\":" + q(method) + ",\"arguments\":{" + arguments + "}}"; }
  static String thread(String id) { return "{\"type\":\"thread.started\",\"thread_id\":" + q(id) + "}"; }
  static String cmd(String command, String code, String output) { return "{\"type\":\"item.completed\",\"item\":{\"type\":\"command_execution\",\"command\":" + q(command) + ",\"aggregated_output\":" + q(output) + ",\"exit_code\":" + code + "}}"; }
  static String edit(int n) {
    var paths = new ArrayList<String>();
    for (int i = 0; i < n; i++) paths.add("{\"path\":" + q("src/file" + i + ".py") + "}");
    return "{\"type\":\"item.completed\",\"item\":{\"type\":\"file_change\",\"changes\":[" + String.join(",", paths) + "]}}";
  }
  static String replay(List<String> lines, String policy, String extra) { return "{\"kind\":\"replay\",\"lines\":" + ParserProbe.array(lines) + ",\"policy\":{" + policy + "}" + (extra.isEmpty() ? "" : "," + extra) + "}"; }
  static void prompts(boolean write) throws Exception {
    var source = Files.readString(Path.of("reference/upstream/experiments/te_bench/codex_spiral.py"));
    var root = Path.of("src/experiments/codex_spiral/prompts");
    for (var reason : List.of("validated-patch-should-finalize", "green-reverification-loop", "beyond-twice-the-resolved-envelope",
        "medium-effort-envelope-rescue", "very-wide-patch-still-failing", "wide-edit-scope-needs-verification", "validated-patch-reopened-research", "generic")) {
      String body;
      if (reason.equals("generic")) {
        int start = source.indexOf("\"Automated process check: \" + focus +") + "\"Automated process check: \" + focus +".length();
        body = source.substring(start, source.indexOf("\n    )", start));
      } else {
        int start = source.indexOf("return (", source.indexOf("if reason == \"" + reason + "\"")) + "return (".length();
        body = source.substring(start, source.indexOf("\n        )", start));
      }
      var text = ExtractProposerPrompts.constant("PROMPT = (" + body + "\n)", "PROMPT", "");
      var file = root.resolve(reason + ".txt");
      if (write) { Files.createDirectories(root); Files.writeString(file, text); }
      if (!Files.readString(file).equals(text)) throw new AssertionError("Native nudge differs: " + reason);
    }
  }
  public static void main(String[] args) throws Exception {
    prompts(args.length == 1 && args[0].equals("--write"));
    if (args.length > 0) { System.out.println("Verified eight native Codex nudge resources"); return; }
    var checkout = Path.of("").toAbsolutePath();
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "spiral_host.py");
         var source = new AdapterProbe.Client(checkout, false, "spiral_host.py")) {
      var commands = List.of("", "pytest", "K=1 pytest -q", "rtk pytest -q", "rtk proxy python3.14 -m pytest tests/test_a.py::test_foo", "rtk K=2 pytest -q", "cd x && rtk pytest -q",
          "python3 -m unittest", "python -m django test", "python ./bin/test", "./tests/runtests.py", "tox", "nox -s test", "pytest tests/test_a.py tests/test_b.py -q",
          "pytest -k foo", "pytest --deselect=x", "pytest --lf", "pytest --failed-first", "pytest -m 'slow'", "python - <<'PY'\nassert x\nPY", "python3 - <<'PY'\nnp.testing.assert_equal(a,b)\nPY",
          "pip install x", "git status", "git diff --check", "git log -1", "sed -n '1,2p' file.py", "curl http://example.invalid", "ruff check .", "black --check .", "python -m compileall .",
          "/bin/bash -lc 'pytest tests/test_a.py'", "/bin/sh -c 'rg abc123 /tmp/x9'", "sh -c 'unterminated");
      for (var command : commands) for (var method : List.of("green_is_trustworthy", "is_test_command", "is_verification_command", "is_research_command", "verification_scopes", "_command_fingerprint")) compare(nativeCode, source, call(method, "\"command\":" + q(command)));
      for (var command : List.of("pytest -k\u0301word", "pytest -k\u0345word", "pytest --last-faİled", "pytest\u001c-k word", "py.test\u001f-m\u001f'unit'", "python -m unİttest", "rtk\u001cpytest", "İ=1 pytest", "python - <<'PY'\n\u0301assert\u0301\nPY", "\u0301rg\u0301 needle", "gİt log -1", "pip\u001fcache list")) for (var method : List.of("green_is_trustworthy", "is_test_command", "is_verification_command", "is_research_command", "verification_scopes")) compare(nativeCode, source, call(method, "\"command\":" + q(command)));
      for (int limit : List.of(-500, -1, 0, 1, 180, 320)) compare(nativeCode, source, call("_normalize", "\"text\":" + q("  FAIL É😀 0xFF /tmp/a2.py 123 ٢٣\u001c\u001f end ") + ",\"limit\":" + limit));
      for (var output : List.of("FAILED tests/test_a.py::test_12 - AssertionError 31", "ERROR /tmp/build123/test_foo.py\n___ test_42 ___", "Traceback 0x123\nAssertionError value ٢", "normal\nnoise", "ERROR: first\u2028FAILED second", "___  ___")) compare(nativeCode, source, call("failure_signatures", "\"output\":" + q(output)));
      for (var reason : List.of("validated-patch-should-finalize", "green-reverification-loop", "beyond-twice-the-resolved-envelope", "medium-effort-envelope-rescue", "very-wide-patch-still-failing", "wide-edit-scope-needs-verification", "validated-patch-reopened-research", "repeated-failing-verification", "scattered-edits-still-failing", "edit-churn-still-failing", "repeated-command-without-progress", "blocked-retrieval-without-progress", "unknown")) compare(nativeCode, source, call("nudge_text", "\"reason\":" + q(reason)));
      for (var profile : List.of("post-green", "defensive", "", "unknown")) for (var threshold : List.of(-10, 0, 1.5, 2.5, 23.5, 24.5)) compare(nativeCode, source, call("profile_policy", "\"profile\":" + q(profile) + ",\"threshold\":" + threshold));
      for (var values : List.of("[]", "[1]", "[1,9]", "[3,1,2,9,4]")) for (var pct : List.of(-125, -25, 0, 50, 90, 100, 125, 300)) compare(nativeCode, source, call("_percentile", "\"values\":" + values + ",\"pct\":" + pct));
      var patch = "apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: src/a.py\n*** Add File: src/b.py\n*** Update File: src/a.py\n*** End Patch\nPATCH\npytest -q";
      for (var exit : List.of("null", "0", "1", "\"0\"", "\"bad\"", "false", "1.2")) {
        compare(nativeCode, source, call("parse_events", "\"line\":" + q(cmd(patch, exit, ""))));
        compare(nativeCode, source, call("parse_events", "\"line\":" + q(cmd(patch, exit, "Success. Applied"))));
      }
      for (var value : List.of("{", "[]", "null", "{\"type\":\"other\"}", thread("thread"), edit(2), "{\"type\":\"item.completed\",\"item\":true}")) compare(nativeCode, source, call("parse_events", "\"line\":" + q(value)));
      for (var changes : List.of("true", "3", "1.5")) compare(nativeCode, source, call("parse_events", "\"line\":" + q("{\"type\":\"item.completed\",\"item\":{\"type\":\"file_change\",\"changes\":" + changes + "}}")));
      for (var first : List.of(true, false)) for (int take : List.of(0, 1, 2, 10)) compare(nativeCode, source, "{\"kind\":\"iterator\",\"lines\":" + ParserProbe.array(List.of(thread("one"), cmd(patch, "0", ""), thread("two"), "[]")) + ",\"first_thread_only\":" + first + ",\"take\":" + take + "}");
      var fail = cmd("pytest tests/test_a.py", "1", "FAILED test_a.py::test_12");
      var green = cmd("pytest tests/test_a.py", "0", "1 passed");
      var research = cmd("rg needle src", "0", "found");
      var casesByRule = List.of(
          new String[]{"\"wide_failure_enabled\":true,\"scatter_files\":2", edit(2), fail, fail},
          new String[]{"\"repeated_failure_enabled\":true", edit(1), fail, fail},
          new String[]{"\"churn_failure_enabled\":true", edit(1), edit(1), edit(1), fail, fail},
          new String[]{"\"scope_checkpoint_enabled\":true,\"scope_checkpoint_files\":2", edit(2)},
          new String[]{"\"post_green_enabled\":true,\"post_green_research_turns\":2", edit(1), green, research, research},
          new String[]{"\"repeated_command_enabled\":true,\"no_progress_turns\":2", edit(1), fail, research, research, research},
          new String[]{"\"blocked_retrieval_enabled\":true,\"no_progress_turns\":2", edit(1), fail, cmd("curl x", "1", "connection refused"), cmd("curl y", "1", "network unreachable")},
          new String[]{"\"effort_rescue_enabled\":true,\"resume_effort\":\"high\"", research, research, research},
          new String[]{"\"effort_rescue_enabled\":true,\"effort_rescue_progress_grace_turns\":3", edit(1), research, research, research},
          new String[]{"\"post_green_finalize_enabled\":true,\"post_green_finalize_require_cleared_failures\":true", edit(1), fail, cmd("pytest tests/test_b.py", "0", "ok"), research, research, research, green, research},
          new String[]{"\"post_green_finalize_enabled\":true,\"post_green_finalize_require_cleared_failures\":true", edit(1), cmd("pytest", "1", "FAILED test_c.py"), green, research, research, cmd("pytest", "0", "ok"), research},
          new String[]{"\"reverify_loop_enabled\":true", edit(1), green, green, green, green},
          new String[]{"\"runaway_enabled\":true,\"runaway_floor\":3", research, research, research, research}
      );
      for (var scenario : casesByRule) {
        var lines = new ArrayList<String>(List.of(scenario).subList(1, scenario.length)); lines.add(0, thread("same-thread"));
        for (boolean verbose : List.of(false, true)) compare(nativeCode, source, replay(lines, "\"min_tool_turns\":1,\"nudge_budget\":2," + scenario[0], "\"verbose\":" + verbose));
        compare(nativeCode, source, replay(lines, "\"min_tool_turns\":1,\"nudge_budget\":2," + scenario[0], "\"continue\":false"));
        compare(nativeCode, source, replay(lines.subList(1, lines.size()), "\"min_tool_turns\":1," + scenario[0], ""));
      }
      for (var min : List.of(0, 1, 3, 5)) for (var pct : List.of(0, 50, 90, 100)) compare(nativeCode, source, "{\"kind\":\"tracker\",\"arguments\":{\"min_samples\":" + min + ",\"percentile\":" + pct + "},\"rows\":[[0,true],[3,false],[-1,true],[10,true],[30,true],[20,true],[45,true]]}");
      compare(nativeCode, source, "{\"kind\":\"tracker\",\"rows\":[[\"unused\",false],[20.9,true],[\"٢٣\",true],[false,true],[\"1_2\",true]]}");
      for (int window : List.of(-1, 0, 1)) compare(nativeCode, source, replay(List.of(thread("one"), edit(1), fail, research, research), "\"recent_window\":" + window + ",\"min_tool_turns\":1", ""));
      var rows = "[{\"resolved\":true,\"num_turns\":20,\"codex_spiral_tool_turns\":10},{\"resolved\":false,\"num_turns\":30,\"codex_spiral_tool_turns\":20},{\"resolved\":true,\"num_turns\":40,\"spiral_judge_tokens\":2.5,\"spiral_nudges\":null,\"spiral_nudge_writer_tokens\":3,\"codex_spiral_paid_judge_tokens\":7,\"codex_spiral_events\":[{\"type\":\"resume\"}]},{\"resolved\":true,\"num_turns\":50},{\"resolved\":true,\"num_turns\":60},{\"resolved\":true,\"num_turns\":70}]";
      for (var config : List.of("{}", "{\"guard\":true,\"judge\":true,\"nudge\":true,\"nudge_llm\":true}", "{\"codex_spiral\":true,\"codex_spiral_profile\":\"defensive\"}", "{\"guard\":true,\"nudge\":true,\"codex_spiral\":true}", "{\"guard\":true,\"frozen_envelope\":30}")) {
        for (var agent : List.of("claude", "codex", "copilot")) for (var adapter : List.of(false, true)) compare(nativeCode, source, "{\"kind\":\"stage\",\"agent\":" + q(agent) + ",\"config\":" + config + ",\"adapter\":" + adapter + ",\"rows\":" + rows + ",\"snapshot\":{\"edit_churn\":4,\"distinct_edit_files\":3,\"error_outputs\":2,\"stagnation\":0.5}}");
      }
      for (var reply : List.of("{\"stuck\":false,\"confidence\":0.7,\"reason\":\"productive\"}", "a targeted redirect", "")) for (var runnerError : List.of(false, true)) compare(nativeCode, source, "{\"kind\":\"stage\",\"config\":{\"guard\":true,\"judge\":true,\"nudge\":true,\"nudge_llm\":true},\"runner\":true,\"runner_error\":" + runnerError + ",\"model\":\"lane-model\",\"digest\":\"recent process é😀\",\"reply\":" + q(reply) + "}");
      compare(nativeCode, source, "{\"kind\":\"stage\",\"config\":{\"guard\":true},\"adapter\":true,\"snapshot_error\":true,\"rows\":" + rows + "}");
      for (var cap : List.of("null", "0", "20")) compare(nativeCode, source, "{\"kind\":\"stage\",\"config\":{\"guard\":true,\"frozen_envelope\":30},\"max_turns\":" + cap + ",\"rows\":" + rows + "}");
      compare(nativeCode, source, "{\"kind\":\"stage\",\"config\":{\"guard\":true,\"judge\":true,\"nudge\":true,\"nudge_llm\":true},\"runner\":true,\"tokens\":null,\"digest\":\"recent process\",\"reply\":\"redirect\"}");
      compare(nativeCode, source, "{\"kind\":\"stage\",\"config\":{\"guard\":true},\"rows\":[{\"resolved\":false,\"num_turns\":\"unused\"}]}");
      System.out.println("Native Codex spiral and the shared stage match upstream in " + cases + " comparisons, including all decision rules, lazy iteration, nudge texts, envelope learning, callback costs and telemetry");
    }
  }
}
