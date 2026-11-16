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
import java.util.concurrent.TimeUnit;

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
    } finally {
      process.getOutputStream().close();
      if (!process.waitFor(10, TimeUnit.SECONDS)) {
        process.destroyForcibly();
        throw new AssertionError("Bridge did not exit after EOF");
      }
      Files.deleteIfExists(path);
      Files.delete(directory);
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

  private static String request(PrintWriter input, BufferedReader output, String request) throws Exception {
    input.println(request.strip());
    input.flush();
    return output.readLine();
  }
}
