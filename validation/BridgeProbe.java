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
