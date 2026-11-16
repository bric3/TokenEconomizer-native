//JAVA 25

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.math.BigDecimal;
import java.math.RoundingMode;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.Comparator;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Checks the new test transport and text reader without translating upstream tests. */
class BridgeProbe {
  public static void main(String[] args) throws Exception {
    var root = Path.of(args.length == 0 ? "." : args[0]).toAbsolutePath().normalize();
    var process = new ProcessBuilder(root.resolve("target/debug/te-bridge").toString())
        .redirectError(ProcessBuilder.Redirect.INHERIT).start();
    var directory = Files.createTempDirectory(root.resolve("validation/results"), "bridge-");
    var path = directory.resolve("lines.jsonl");

    try (var input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
         var output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
      input.println("{broken");
      input.flush();
      var error = output.readLine();
      if (error == null || !error.startsWith("{\"error\":")) {
        throw new AssertionError("Malformed request was not rejected: " + error);
      }

      expect(input, output, """
          {"op":"pricing.approx_tokens","args":{"text":"é😀é😀é😀é😀"}}
          """, "{\"result\":2}");

      expect(input, output, """
          {"op":"accounting.effective","args":{"usage":{"input_tokens":"garbage"}}}
          """, "{\"error\":\"pricing: input_tokens must be numeric\"}");
      expect(input, output, """
          {"op":"accounting.effective","args":{"usage":{"input_tokens":10,"model":42}}}
          """, "{\"error\":\"pricing: billed model must be a string\"}");

      expect(input, output, """
          {"op":"trace.cursor._response_text","args":{"response":{"content":{"a":"é","b":[true,null]}}}}
          """, "{\"result\":\"{'a': 'é', 'b': [True, None]}\"}");

      expect(input, output, """
          {"op":"state.basename","args":{"p":null}}
          """, "{\"result\":\"\"}");
      var fingerprint = HexFormat.of().formatHex(MessageDigest.getInstance("MD5")
          .digest("AssertionError:  pointer".getBytes(StandardCharsets.UTF_8))).substring(0, 10);
      expect(input, output, """
          {"op":"state._failure_sig","args":{"text":"context\\nAssertionError: /tmp/x42.py pointer 0xabc 123\\n"}}
          """, "{\"result\":\"" + fingerprint + "\"}");
      expect(input, output, """
          {"op":"state.completed_calls","args":{"pend":{"null":{"name":"Read","args":{}},"\\\"null\\\"":{"name":"Edit","args":{}}},"line":"{\\\"type\\\":\\\"user\\\",\\\"message\\\":{\\\"content\\\":[{\\\"type\\\":\\\"tool_result\\\",\\\"tool_use_id\\\":\\\"null\\\",\\\"content\\\":\\\"ok\\\"}]}}"}}
          """, "{\"result\":{\"calls\":[[\"edit\",{},\"ok\"]],\"pend\":{\"null\":{\"name\":\"Read\",\"args\":{}}}}}");
      expect(input, output, """
          {"op":"shell_edits.shell_edit_targets","args":{"cmd":"echo 'broken\\nsed -i s/a/b/ pkg/a.py"}}
          """, "{\"result\":[\"a.py\"]}");

      var foldArgs = """
          "self":{"st":{"turns":18,"edit_counts":{"one.py":4},"error_outputs":6,"fail_sigs":{"s":3},"repeated_fail_sigs":2},"cum_turns":18},
          "envelope":100,"judge":"sampled","name":"Bash","args":{"command":"pytest -q"},"result":"FAILED"
          """.replace("\n", "");
      var sampled = request(input, output, "{\"op\":\"turn.fold\",\"args\":{" + foldArgs + "}}");
      if (!sampled.contains("\"callback\":\"judge\"") || !sampled.contains("The last verification run failed.")
          || sampled.contains("Tests are still NOT green.")) {
        throw new AssertionError("Judge callback did not receive the live verification state: " + sampled);
      }
      var productive = request(input, output, "{\"op\":\"turn.fold\",\"args\":{" + foldArgs
          + ",\"replies\":[{\"value\":{\"stuck\":false,\"confidence\":0.8,\"_tokens\":7}}]}}");
      if (!productive.contains("\"judge_gap\":10") || !productive.contains("\"judge_calls\":1")
          || !productive.contains("\"judge_tokens\":7.0") || !productive.endsWith("\"value\":null}}")) {
        throw new AssertionError("Sampled judge accounting/backoff: " + productive);
      }
      var stuck = request(input, output, "{\"op\":\"turn.fold\",\"args\":{" + foldArgs
          + ",\"replies\":[{\"value\":{\"stuck\":true,\"confidence\":0.9,\"reason\":\"loop\"}}]}}");
      if (!stuck.endsWith("\"value\":[\"nudge\",\"judge-stuck@0.90:loop\"]}}")) {
        throw new AssertionError("Sampled judge decision: " + stuck);
      }
      var written = request(input, output, """
          {"op":"turn.nudge_body","args":{"self":{},"writer":true,"replies":[{"value":{"text":"  Try another file.  ","_tokens":3}}]}}
          """);
      if (!written.contains("\"writer_tokens\":3.0") || !written.endsWith("\"value\":\"Try another file.\"}}")) {
        throw new AssertionError("Nudge writer accounting: " + written);
      }
      var failedWriter = request(input, output, """
          {"op":"turn.nudge_body","args":{"self":{},"writer":true,"replies":[{"error":"offline"}]}}
          """);
      if (!failedWriter.contains("\"writer_tokens\":0.0") || !failedWriter.contains("Process check (automated):")) {
        throw new AssertionError("Nudge writer failure did not use the count template: " + failedWriter);
      }

      expect(input, output, """
          {"op":"judge.llm_stuck_judge","args":{"digest":"","runner":true}}
          """, "{\"result\":{\"stuck\":null,\"confidence\":0.0,\"reason\":\"no-runner\",\"_tokens\":0}}");
      expect(input, output, """
          {"op":"judge.llm_stuck_judge","args":{"digest":"turns=20","runner":true,"replies":[{"value":["prefix {\\\"stuck\\\":true,\\\"confidence\\\":2,\\\"reason\\\":\\\"repeat\\\"} suffix",0]}]}}
          """, "{\"result\":{\"stuck\":true,\"confidence\":1.0,\"reason\":\"repeat\",\"_tokens\":0.0}}");
      expect(input, output, """
          {"op":"judge.llm_stuck_judge","args":{"digest":"turns=20","runner":true,"replies":[{"value":["{\\\"stuck\\\":true,\\\"confidence\\\":\\\"bad\\\",\\\"reason\\\":\\\"repeat\\\"}",7]}]}}
          """, "{\"result\":{\"stuck\":true,\"confidence\":0.0,\"reason\":\"\",\"_tokens\":7.0}}");
      expect(input, output, """
          {"op":"judge.llm_stuck_judge","args":{"digest":"turns=20","runner":true,"replies":[{"error":"offline"}]}}
          """, "{\"result\":{\"stuck\":null,\"confidence\":0.0,\"reason\":\"runner-error:offline\",\"_tokens\":0}}");
      expect(input, output, """
          {"op":"pricing.runner_effective_or_approx","args":{"runner":true,"prompt":"😀😀😀😀😀😀😀😀","raw":"reply","replies":[{"error":"cannot estimate"}]}}
          """, "{\"result\":7.0}");
      expect(input, output, """
          {"op":"pricing.runner_effective_or_approx","args":{"runner":true,"prompt":"prompt","raw":"reply","replies":[{"value":42}]}}
          """, "{\"result\":42.0}");
      var longRedirect = "😀".repeat(1_205);
      expect(input, output, "{\"op\":\"judge.llm_nudge_writer\",\"args\":{\"digest\":\"actions\",\"runner\":true,\"replies\":[{\"value\":[\"  " + longRedirect + "  \",7]}]}}",
          "{\"result\":{\"text\":\"" + "😀".repeat(1_200) + "\",\"_tokens\":7.0}}");

      checkEnvironmentFacts(input, output);
      checkComposition(input, output);
      checkSavedLayers(directory, input, output);
      checkToolJudge(input, output);
      checkToolingJudge(input, output);
      checkCapabilityRecovery(directory, input, output);

      input.println("""
          {"op":"rules.nudge_text","args":{"state":{"edit_counts":{"one file":5,"other":1}}}}
          """.strip());
      input.flush();
      var nudge = output.readLine();
      if (nudge == null || !nudge.contains("most to `one file`")) {
        throw new AssertionError("The redirect chose the wrong file: " + nudge);
      }

      Files.writeString(path, " one \r two\r\nthree\n\n");
      var jsonPath = path.toString().replace("\\", "\\\\").replace("\"", "\\\"");
      expect(input, output, "{\"op\":\"trace.jsonl.lines\",\"args\":{\"path\":\"" + jsonPath + "\"}}",
          "{\"result\":[\"one\",\"two\",\"three\"]}");

      Files.writeString(path, "{}\r\n[]\r{\"ok\":true}\nno\n");
      expect(input, output, "{\"op\":\"trace.jsonl.records\",\"args\":{\"path\":\"" + jsonPath + "\"}}",
          "{\"result\":[{},{\"ok\":true}]}");

      var first = "{\"content\":\"é😀\"}\n";
      Files.writeString(path, first + "{\"content\":");
      var offset = first.getBytes(StandardCharsets.UTF_8).length;
      expect(input, output, "{\"op\":\"trace.antigravity._read_rows\",\"args\":{\"path\":\"" + jsonPath + "\",\"offset\":0}}",
          "{\"result\":[[" + offset + ",{\"content\":\"é😀\"}]]}");
      Files.writeString(path, "\"完\"}\n", StandardOpenOption.APPEND);
      expect(input, output, "{\"op\":\"trace.antigravity._read_rows\",\"args\":{\"path\":\"" + jsonPath + "\",\"offset\":" + offset + "}}",
          "{\"result\":[[" + Files.size(path) + ",{\"content\":\"完\"}]]}");

      for (double value : new double[] {0.1234575, 2.6750005, 0.0000005, -2.6750005}) {
        Files.writeString(path, "{\"type\":\"message\",\"id\":\"m\",\"parentId\":null,\"message\":{\"role\":\"assistant\",\"usage\":{\"cost\":{\"total\":" + value + "}}}}\n");
        input.println("{\"op\":\"trace.pi.pi_session_usage\",\"args\":{\"path\":\"" + jsonPath + "\"}}");
        input.flush();
        var line = output.readLine();
        var marker = "\"cost_usd_vendor\":";
        var start = line.indexOf(marker) + marker.length();
        var actual = Double.parseDouble(line.substring(start, line.indexOf(',', start)));
        var expected = new BigDecimal(value).setScale(6, RoundingMode.HALF_EVEN).doubleValue();
        if (Double.compare(actual, expected) != 0) {
          throw new AssertionError("Cost rounding: " + value + " expected " + expected + ", received " + actual);
        }
      }

      var metadata = "{\"sessionId\":\"right\",\"cwd\":\"/repo\",\"timestamp\":\"2026-01-01T00:00:00Z\",\"entrypoint\":\"cli\"}\n";
      var headRequest = "{\"op\":\"trace.claude.claude_log_head\",\"args\":{\"path\":\"" + jsonPath + "\"}}";
      var expectedHead = "{\"result\":{\"session_id\":\"right\",\"cwd\":\"/repo\",\"started\":1767225600.0,\"entrypoint\":\"cli\"}}";
      Files.writeString(path, "{\"sessionId\":\"wrong\",\"padding\":\"" + "x".repeat((1 << 20) + 50) + "\"}\n" + metadata);
      expect(input, output, headRequest, expectedHead);

      // The reference reader's readline limit counts decoded characters, including emoji once.
      Files.writeString(path, metadata.stripTrailing().replace("\"cwd\"", "\"padding\":\"" + "😀".repeat(300_000) + "\",\"cwd\"") + "\n");
      expect(input, output, headRequest, expectedHead);

      Files.writeString(path, "\n".repeat(200) + metadata);
      expect(input, output, headRequest,
          "{\"result\":{\"session_id\":null,\"cwd\":null,\"started\":null,\"entrypoint\":\"\"}}");

      var invalid = metadata.replace("/repo", "/r@po").getBytes(StandardCharsets.UTF_8);
      invalid[metadata.replace("/repo", "/r@po").indexOf('@')] = (byte) 0xff;
      Files.write(path, invalid);
      expect(input, output, headRequest, expectedHead.replace("/repo", "/r�po"));

      var missingDatabase = directory.resolve("missing.db");
      var databasePath = missingDatabase.toString().replace("\\", "\\\\").replace("\"", "\\\"");
      expect(input, output, "{\"op\":\"trace.hermes.hermes_export\",\"args\":{\"db_path\":\"" + databasePath + "\",\"session_id\":\"s\",\"path\":\"" + jsonPath + "\"}}",
          "{\"result\":false}");
      if (Files.exists(missingDatabase)) {
        throw new AssertionError("Reading a missing Hermes database created it");
      }

      checkStorage(root, directory, input, output);
    } finally {
      process.getOutputStream().close();
      if (!process.waitFor(10, TimeUnit.SECONDS)) {
        process.destroyForcibly();
        throw new AssertionError("Bridge did not exit after EOF");
      }
      try (var entries = Files.walk(directory)) {
        for (var entry : entries.sorted(Comparator.reverseOrder()).toList()) {
          Files.deleteIfExists(entry);
        }
      }
    }

    if (process.exitValue() != 0) {
      throw new AssertionError("Bridge exit code: " + process.exitValue());
    }
    System.out.println("Bridge, pricing, traces, loop state, judge callbacks, metadata and database checks passed");
  }

  private static void expect(PrintWriter input, BufferedReader output, String request, String expected)
      throws Exception {
    var actual = request(input, output, request);
    if (!expected.equals(actual)) {
      throw new AssertionError("Expected " + expected + ", received " + actual);
    }
  }

  private static void checkEnvironmentFacts(PrintWriter input, BufferedReader output) throws Exception {
    expect(input, output, """
        {"op":"lifecycle.from_dict","args":{"d":{"key":"old","high":false}}}
        """, "{\"result\":{\"key\":\"old\",\"high\":false,\"drift\":true,\"state\":\"CANDIDATE\",\"confirms\":[],\"contradicts\":[],\"fire_num\":0,\"fire_den\":0,\"promoted_at\":null,\"retired_at\":null}}");
    var unknown = request(input, output, """
        {"op":"envfacts.forget","args":{"self":{},"fid":"unknown"}}
        """);
    if (!unknown.endsWith("\"value\":{\"raised\":\"KeyError\",\"message\":\"unknown\"}}}")
        || !unknown.contains("\"facts\":{},\"forgotten\":{}")) {
      throw new AssertionError("An unknown fact changed the distiller: " + unknown);
    }
    var unrecorded = request(input, output, """
        {"op":"envfacts.unforget","args":{"self":{"forgotten":{"gone":{"claim":"remember"}}},"fid":"gone"}}
        """);
    if (!unrecorded.endsWith("\"value\":\"no record\"}}") || !unrecorded.contains("\"facts\":{},\"forgotten\":{}")) {
      throw new AssertionError("A forgotten id without a record was restored: " + unrecorded);
    }
    var activeFact = """
        {"id":"%s","family":"empty_probe","params":{},"life":{"key":"key","high":false,"drift":false,"state":"ACTIVE","confirms":[],"contradicts":[],"fire_num":4,"fire_den":4,"promoted_at":null,"retired_at":null}}
        """.strip();
    var selected = request(input, output, "{\"op\":\"envfacts._select\",\"args\":{\"self\":{\"max_facts\":1,\"max_tokens\":1000,\"facts\":{\"z\":"
        + activeFact.formatted("z") + ",\"a\":" + activeFact.formatted("a") + "}}}}");
    if (!selected.endsWith("\"value\":[\"z\"]}}")) {
      throw new AssertionError("Selection lost insertion order when facts tied: " + selected);
    }
  }

  private static void checkComposition(PrintWriter input, BufferedReader output) throws Exception {
    var life = """
        {"key":"fact","high":false,"drift":false,"state":"ACTIVE","confirms":[],"contradicts":[],"fire_num":4,"fire_den":4,"promoted_at":null,"retired_at":null}
        """.strip();
    var layers = "{\"env\":{\"repo\":\"acme/widget\",\"now\":9,\"opt_tokens\":88,\"facts\":{\"env\":{\"id\":\"env\",\"family\":\"empty_probe\",\"params\":{},\"life\":" + life
        + "}}},\"dist\":{\"repo\":\"acme/widget\",\"now\":3,\"opt_tokens\":7,\"facts\":{\"manual\":{\"spec\":{\"id\":\"manual\",\"injection\":\"manual instruction\"},\"life\":" + life + "}}}}";
    var cost = request(input, output, "{\"op\":\"layers.opt_tokens\",\"args\":{\"self\":" + layers + "}}");
    var now = request(input, output, "{\"op\":\"layers.now\",\"args\":{\"self\":" + layers + "}}");
    if (!cost.endsWith("\"value\":7.0}}") || !now.endsWith("\"value\":3}}")) {
      throw new AssertionError("Composition did not use the paid layer's accounting: " + cost + " / " + now);
    }
    var rendered = request(input, output, "{\"op\":\"layers.render\",\"args\":{\"self\":" + layers + "}}");
    if (!rendered.contains("\"last_injected\":[\"env\"]")
        || !rendered.contains("its history ends at the checked-out commit.\\n\\n# repo operating manual (widget)")
        || !rendered.endsWith("- manual instruction\\n\"}}")) {
      throw new AssertionError("Composition changed the environment selection or rendering order: " + rendered);
    }
  }

  private static void checkSavedLayers(Path directory, PrintWriter input, BufferedReader output) throws Exception {
    var envPath = directory.resolve("envfacts_state/acme__widget.json");
    var manualPath = directory.resolve("manual_state/acme__widget.json");
    Files.createDirectories(envPath.getParent());
    Files.createDirectories(manualPath.getParent());
    var envState = """
        {"facts":{"shared":{"id":"shared","family":"empty_probe","params":{},"life":{"key":"shared","high":false}}},"forgotten":{"missing-record":{"claim":"old claim"}}}
        """;
    var manualState = """
        {"facts":{"shared":{"spec":{"id":"shared","claim":"paid shared","detector":{}},"life":{"key":"shared","high":false}},"paid":{"spec":{"id":"paid","claim":"paid claim","detector":{}},"life":{"key":"paid","high":false}}}}
        """;
    Files.writeString(envPath, envState);
    Files.writeString(manualPath, manualState);
    var envClaim = "This checkout has no git remote and no branches, tags or unreachable objects beyond HEAD; its history ends at the checked-out commit.";
    var base = "\"repo\":\"acme/widget\",\"root\":\"" + directory.toString().replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    expect(input, output, "{\"op\":\"stage.forget_facts\",\"args\":{" + base + ",\"ids\":[\"shared\",\"unknown\",\"unknown\"]}}",
        "{\"result\":{\"raised\":\"KeyError\",\"message\":[\"unknown\",\"unknown\"]}}");
    if (!envState.equals(Files.readString(envPath)) || !manualState.equals(Files.readString(manualPath))) {
      throw new AssertionError("Unknown-id validation changed saved facts");
    }
    expect(input, output, "{\"op\":\"stage.forget_facts\",\"args\":{" + base + ",\"ids\":[\"paid\",\"shared\",\"paid\"]}}",
        "{\"result\":[[\"paid\",true,\"paid claim\"],[\"shared\",true,\"" + envClaim + "\"]]}");
    expect(input, output, "{\"op\":\"stage.forgotten_facts\",\"args\":{" + base + "}}",
        "{\"result\":{\"missing-record\":\"old claim\",\"shared\":\"" + envClaim + "\",\"paid\":\"paid claim\"}}");
    var manualAfter = Files.readString(manualPath);
    expect(input, output, "{\"op\":\"stage.forget_facts\",\"args\":{" + base + ",\"ids\":[\"shared\"],\"undo\":true}}",
        "{\"result\":[[\"shared\",true,\"" + envClaim + "\"]]}");
    if (!manualAfter.equals(Files.readString(manualPath))) {
      throw new AssertionError("Restoring an environment fact rewrote the paid layer");
    }
    expect(input, output, "{\"op\":\"stage.forget_facts\",\"args\":{" + base + ",\"ids\":[\"missing-record\"],\"undo\":true}}",
        "{\"result\":[[\"missing-record\",\"no record\",\"\"]]}");
  }

  private static void checkToolJudge(PrintWriter input, BufferedReader output) throws Exception {
    var base = "\"problem_statement\":\"repair\",\"catalog\":{\"Bash\":\"shell\",\"WebFetch\":\"http\"},\"runner\":true";
    expect(input, output, "{\"op\":\"toolprune.tools.judge_tools_for_task\",\"args\":{" + base
        + ",\"replies\":[{\"value\":[\"{\\\"Bash\\\":{\\\"load\\\":false},\\\"WebFetch\\\":false}\",7]}]}}",
        "{\"result\":[{\"Bash\":true,\"WebFetch\":false},7.0]}");
    expect(input, output, "{\"op\":\"toolprune.tools.judge_tools_for_task\",\"args\":{" + base
        + ",\"replies\":[{\"value\":[\"invalid {json}\",7]}]}}", "{\"result\":[{},7.0]}");
    expect(input, output, "{\"op\":\"toolprune.tools.judge_tools_for_task\",\"args\":{" + base
        + ",\"replies\":[{\"error\":\"offline\"}]}}", "{\"result\":[{},0.0]}");
    var unicode = "\"problem_statement\":\"é😀ab\",\"catalog\":{\"Bash\":\"shell\"},\"runner\":true,\"domain\":\"brief\",\"max_issue_chars\":2";
    var callback = request(input, output, "{\"op\":\"toolprune.tools.judge_tools_for_task\",\"args\":{" + unicode + "}}");
    if (!callback.contains("\\nISSUE:\\né😀\\n\\nTOOLS:") || callback.contains("é😀a")) {
      throw new AssertionError("Tool judge did not truncate the issue by Unicode characters: " + callback);
    }
    var prompt = "brief\nISSUE:\né😀\n\nTOOLS:\n- Bash: shell\n\nReturn the JSON classification:";
    var approximate = Math.max(1, prompt.codePointCount(0, prompt.length()) / 4) + 5;
    expect(input, output, "{\"op\":\"toolprune.tools.judge_tools_for_task\",\"args\":{" + unicode
        + ",\"replies\":[{\"value\":[\"{}\",null]}]}}", "{\"result\":[{}," + approximate + ".0]}");
  }

  private static void checkToolingJudge(PrintWriter input, BufferedReader output) throws Exception {
    var constructor = "\"cfg\":{\"evidence\":2,\"evidence_reaudit\":3,\"judge\":true},\"agent\":\"claude\",\"optimizer_model\":\"agent\"";
    var created = request(input, output, "{\"op\":\"tooling.new\",\"args\":{" + constructor + "}}");
    var stage = created.substring("{\"result\":{\"self\":".length(), created.indexOf(",\"messages\":"))
        .replace("\"catalog_tools\":[]", "\"catalog_tools\":[\"Bash\",\"WebFetch\",\"Workflow\",\"Used\"]")
        .replace("\"used_tools\":[]", "\"used_tools\":[\"Used\"]")
        .replace("\"tool_unused_streaks\":{}", "\"tool_unused_streaks\":{\"Workflow\":2,\"Used\":9}")
        .replace("\"catalog_desc\":{}", "\"catalog_desc\":{\"WebFetch\":\"http\"}")
        .replace("\"problem_by_iid\":{}", "\"problem_by_iid\":{\"w-1\":\"repair\"}");
    var build = request(input, output, "{\"op\":\"tooling.build_tools\",\"args\":{\"self\":" + stage
        + ",\"task\":{\"instance_id\":\"w-1\"},\"i\":1,\"adapter\":true,\"runner\":true,\"replies\":[{\"value\":[\"Bash\"]},"
        + "{\"value\":[\"{\\\"WebFetch\\\":false,\\\"Used\\\":false,\\\"unknown\\\":false}\",11]}]}}");
    if (!build.contains("\"value\":[\"WebFetch\",\"Workflow\"]") || !build.contains("\"_task_judge_tokens\":11.0")
        || !build.contains("\"_task_judge_dropped\":[\"WebFetch\"]")) throw new AssertionError("Evidence/judge exclusions were not filtered: " + build);
    var completed = request(input, output, "{\"op\":\"tooling.after_task\",\"args\":{\"self\":" + profileSelf(build)
        + ",\"row\":{},\"srow\":{},\"run_dir_path\":\"unused\",\"adapter\":true,\"replies\":[{\"value\":{}}]}}");
    if (!completed.contains("\"untool_judge_tokens\":11.0") || !completed.contains("\"untool_judge_dropped\":[\"WebFetch\"]")
        || !completed.contains("\"value\":0,")) throw new AssertionError("Judge costs were lost after the task: " + completed);
    var reaudit = request(input, output, "{\"op\":\"tooling.build_tools\",\"args\":{\"self\":" + profileSelf(completed)
        + ",\"task\":{\"instance_id\":\"w-1\"},\"i\":3,\"adapter\":true,\"runner\":true}}");
    if (!reaudit.contains("\"value\":[]") || !reaudit.contains("\"_ev_reaudit\":true") || !reaudit.contains("\"_task_judge_tokens\":0.0")) {
      throw new AssertionError("Reaudit did not restore the full tool catalog: " + reaudit);
    }
    var noProblems = request(input, output, "{\"op\":\"tooling.new\",\"args\":{" + constructor + ",\"adapter\":true,\"replies\":[{\"error\":\"offline\"}]}}");
    if (!noProblems.contains("could not load problem statements (offline); judge disabled") || !noProblems.contains("\"problem_by_iid\":{}")) throw new AssertionError(noProblems);
  }

  private static String profileSelf(String response) {
    var start = "{\"result\":{\"self\":";
    int end = response.indexOf(",\"value\":");
    if (!response.startsWith(start) || end < 0) throw new AssertionError("Invalid profile response: " + response);
    return response.substring(start.length(), end);
  }

  private static void checkCapabilityRecovery(Path directory, PrintWriter input, BufferedReader output) throws Exception {
    var base = "\"repo\":\"probe/recovery\",\"results_root\":\"" + directory.toString().replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    var fresh = request(input, output, "{\"op\":\"toolprune.codex.new\",\"args\":{" + base + ",\"fresh\":true}}");
    var profile = fresh.substring("{\"result\":".length(), fresh.length() - 1);
    var noPending = request(input, output, "{\"op\":\"toolprune.codex.observe\",\"args\":{\"self\":" + profile + ",\"usage\":{}}}");
    if (!noPending.contains("observe() called without begin_task()")) throw new AssertionError(noPending);
    profile = profile.replace("\"tasks_completed\":0", "\"tasks_completed\":24")
        .replace("\"current_pruned\":[]", "\"current_pruned\":[\"image_media\",\"skills\",\"web\"]");
    var selected = request(input, output, "{\"op\":\"toolprune.codex.begin_task\",\"args\":{\"self\":" + profile + ",\"task_id\":\"probe\"}}");
    if (!selected.contains("\"probe_capability\":\"image_media\"") || !selected.contains("\"logical_capabilities\":[\"skills\",\"web\"]")) {
      throw new AssertionError("Sparse probe restored the wrong package: " + selected);
    }
    profile = profileSelf(selected);
    var protectedUse = request(input, output, "{\"op\":\"toolprune.codex.observe\",\"args\":{\"self\":" + profile
        + ",\"usage\":{\"attempted\":{\"image_media\":1},\"completed\":{\"image_media\":1}}}}");
    if (!protectedUse.contains("\"probe_gap\":64") || !protectedUse.contains("\"next_probe_at\":56")
        || !protectedUse.contains("\"probe_cursor\":1") || !protectedUse.contains("\"ever_completed\":[\"image_media\"]")
        || !protectedUse.contains("\"kind\":\"probe-protect\"")) throw new AssertionError("Successful probe was not protected: " + protectedUse);
    var loaded = request(input, output, "{\"op\":\"toolprune.codex.new\",\"args\":{" + base + "}}");
    if (!loaded.contains("\"tasks_completed\":25") || !loaded.contains("\"ever_completed\":[\"image_media\"]")) throw new AssertionError("Saved controller was not restored: " + loaded);
    profile = loaded.substring("{\"result\":".length(), loaded.length() - 1);
    var hinted = request(input, output, "{\"op\":\"toolprune.codex.begin_task\",\"args\":{\"self\":" + profile + ",\"task_id\":\"hint\",\"problem_statement\":\"update a Codex plugin\"}}");
    if (!hinted.contains("\"logical_capabilities\":[\"collaboration\",\"goals\",\"web\"]") || !hinted.contains("\"task_hints\":[\"skills\"]")) {
      throw new AssertionError("Task hint restored unrelated packages: " + hinted);
    }
    var saved = directory.resolve("interface_state/probe__recovery/codex_untool.json");
    var compatible = Files.readString(saved);
    for (var field : new String[]{"version", "contract_fingerprint", "repo"}) {
      Files.writeString(saved, compatible.replace("\"" + field + "\": \"", "\"" + field + "\": \"old-"));
      var reset = request(input, output, "{\"op\":\"toolprune.codex.new\",\"args\":{" + base + "}}");
      if (!reset.contains("\"tasks_completed\":0") || !reset.contains("\"current_pruned\":[]")) throw new AssertionError("Incompatible " + field + " was reused: " + reset);
    }
    var fingerprint = Pattern.compile("\\\"contract_fingerprint\\\": \\\"([a-f0-9]+)\\\"").matcher(compatible);
    if (!fingerprint.find()) throw new AssertionError("Missing contract fingerprint");
    Files.writeString(saved, "{\"version\":\"codex-recent-experience-untool-indirect-use-core\",\"contract_fingerprint\":\"" + fingerprint.group(1)
        + "\",\"repo\":\"probe/recovery\",\"attempted_total\":{\"web\":\"2\",\"unknown\":99},\"current_pruned\":[\"web\",\"deferred_wait\",\"unknown\"],"
        + "\"ever_completed\":[\"goals\",\"unknown\"],\"recent_loaded_outcomes\":{\"web\":[\"completed\",\"junk\",\"unused\",\"incomplete_attempt\",\"unused\"]},\"core_capabilities\":[]}");
    var sanitized = request(input, output, "{\"op\":\"toolprune.codex.new\",\"args\":{" + base + "}}");
    if (sanitized.contains("unknown") || sanitized.contains("junk") || !sanitized.contains("\"current_pruned\":[\"web\"]")
        || !sanitized.contains("\"ever_completed\":[\"goals\"]") || !sanitized.contains("\"web\":[\"unused\",\"incomplete_attempt\",\"unused\"]")
        || !sanitized.contains("\"attempted_total\":{\"collaboration\":0,\"web\":2,") || !sanitized.contains("\"core_capabilities\":[\"deferred_wait\"]")) {
      throw new AssertionError("Saved evidence was not normalized: " + sanitized);
    }
    var trace = directory.resolve("tooling.jsonl");
    Files.writeString(trace, "\n{\"type\":\"item.started\",\"item\":{\"type\":\"web_search\"}}\n\n{\"type\":\"item.completed\",\"item\":{\"type\":\"web_search\",\"id\":\"2\"}}\n");
    var usage = request(input, output, "{\"op\":\"toolprune.codex.inspect_usage\",\"args\":{\"transcript_path\":\"" + trace.toString().replace("\\", "\\\\").replace("\"", "\\\"") + "\"}}");
    if (usage.contains("\"web\":2") || !usage.contains("\"tools\":{\"web_search\":1}")) throw new AssertionError("Blank lines changed fallback item IDs: " + usage);
  }

  private static void checkStorage(Path root, Path directory, PrintWriter input, BufferedReader output) throws Exception {
    var base = directory.toString().replace("\\", "\\\\").replace("\"", "\\\"");
    var store = "{\"base\":\"" + base + "\"}";
    request(input, output, "{\"op\":\"store.write\",\"args\":{\"self\":" + store
        + ",\"name\":\"state.json\",\"obj\":{\"z\":\"é😀\",\"a\":{\"q\":1}}}}");
    var expected = """
        {
          "a": {
            "q": 1
          },
          "z": "\\u00e9\\ud83d\\ude00"
        }""";
    if (!Files.readString(directory.resolve("state.json")).equals(expected)) {
      throw new AssertionError("Atomic state output lost sorted keys, indentation or ASCII escaping");
    }
    Files.writeString(directory.resolve("records.jsonl"), "{\"n\":1}\r\n[]\r{\"n\":2}\n");
    expect(input, output, "{\"op\":\"store.read_lines\",\"args\":{\"self\":" + store + ",\"name\":\"records.jsonl\",\"limit\":-1}}",
        "{\"result\":{\"value\":[{\"n\":2}]}}");
    request(input, output, "{\"op\":\"store.ensure\",\"args\":{\"self\":" + store + ",\"parts\":[\"directory.json\"]}}");
    var blocked = request(input, output, "{\"op\":\"store.write\",\"args\":{\"self\":" + store + ",\"name\":\"directory.json\",\"obj\":{}}}");
    if (!blocked.contains("\"raised\":")) {
      throw new AssertionError("Replacing a directory did not fail: " + blocked);
    }
    try (var entries = Files.list(directory)) {
      if (entries.anyMatch(p -> p.toString().endsWith(".tmp"))) {
        throw new AssertionError("An atomic write left a temporary file after failure");
      }
    }

    var other = new ProcessBuilder(root.resolve("target/debug/te-bridge").toString())
        .redirectError(ProcessBuilder.Redirect.INHERIT).start();
    try (var otherInput = new PrintWriter(new OutputStreamWriter(other.getOutputStream(), StandardCharsets.UTF_8));
         var otherOutput = new BufferedReader(new InputStreamReader(other.getInputStream(), StandardCharsets.UTF_8))) {
      var acquire = "{\"op\":\"store.acquire\",\"args\":{\"self\":" + store + ",\"name\":\"probe\",\"timeout\":0.05}}";
      var first = request(input, output, acquire);
      var second = request(otherInput, otherOutput, acquire);
      if (!first.contains("\"acquired\":true") || !second.contains("\"acquired\":false")) {
        throw new AssertionError("Exclusive locks were not shared across processes: " + first + " / " + second);
      }
      var id = Pattern.compile("\"id\":(\\d+)").matcher(first);
      if (!id.find()) {
        throw new AssertionError("Lock handle missing: " + first);
      }
      expect(input, output, "{\"op\":\"store.release\",\"args\":{\"id\":" + id.group(1) + "}}", "{\"result\":{\"value\":null}}");
      var third = request(otherInput, otherOutput, acquire);
      if (!third.contains("\"acquired\":true")) {
        throw new AssertionError("Lock was not released: " + third);
      }

      Files.writeString(directory.resolve("envelope.json"), "{\"resolved_turns\":[" + "3,".repeat(499) + "3],\"resolved_snaps\":[]}");
      var record = "{\"op\":\"envelope.record_turn\",\"args\":{\"store\":" + store + ",\"guard\":{\"cum_turns\":%d}}}";
      try (var pool = Executors.newVirtualThreadPerTaskExecutor()) {
        var a = pool.submit(() -> {
          for (int i = 0; i < 32; i++) {
            var result = request(input, output, record.formatted(1));
            if (result.contains("\"raised\":") || result.contains("\"error\":")) {
              throw new AssertionError("Recording an envelope turn failed: " + result);
            }
          }
          return null;
        });
        var b = pool.submit(() -> {
          for (int i = 0; i < 32; i++) {
            var result = request(otherInput, otherOutput, record.formatted(2));
            if (result.contains("\"raised\":") || result.contains("\"error\":")) {
              throw new AssertionError("Recording a concurrent envelope turn failed: " + result);
            }
          }
          return null;
        });
        a.get();
        b.get();
      }
      var envelope = request(input, output, "{\"op\":\"envelope.load_envelope\",\"args\":{\"store\":" + store + "}}");
      var marker = "\"resolved_turns\":[";
      var start = envelope.indexOf(marker) + marker.length();
      var turns = envelope.substring(start, envelope.indexOf(']', start)).split(",");
      int ones = 0, twos = 0;
      for (var turn : turns) {
        if (Double.parseDouble(turn) == 1) ones++;
        if (Double.parseDouble(turn) == 2) twos++;
      }
      if (turns.length != 500 || ones != 32 || twos != 32) {
        throw new AssertionError("Concurrent envelope history lost turns or exceeded its cap: " + turns.length + ", " + ones + ", " + twos);
      }
    } finally {
      if (!other.waitFor(10, TimeUnit.SECONDS)) {
        other.destroyForcibly();
        throw new AssertionError("Second bridge did not exit after EOF");
      }
    }
    if (other.exitValue() != 0) {
      throw new AssertionError("Second bridge exit code: " + other.exitValue());
    }
  }

  private static String request(PrintWriter input, BufferedReader output, String request) throws Exception {
    input.println(request.strip());
    input.flush();
    return output.readLine();
  }
}
