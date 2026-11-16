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
    System.out.println("Bridge, pricing, Unicode, newline, incremental offset and cost rounding checks passed");
  }

  private static void expect(PrintWriter input, BufferedReader output, String request, String expected)
      throws Exception {
    input.println(request.strip());
    input.flush();
    var actual = output.readLine();
    if (!expected.equals(actual)) {
      throw new AssertionError("Expected " + expected + ", received " + actual);
    }
  }
}
