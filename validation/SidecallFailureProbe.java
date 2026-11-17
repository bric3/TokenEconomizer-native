//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;

/** Native failure-note bounds and formatting without a model or host CLI. */
class SidecallFailureProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static void fail(ProcessInputProbe.Client client, String common, String failure, Path log, String expected) throws Exception {
    var pending = client.call("sidecall.noted_call", common);
    var result = client.resume(pending, "noted", "{\"failure\":" + failure + "}");
    require(result.contains("\"failed\":true"), result);
    var lines = Files.readAllLines(log);
    var last = lines.getLast();
    require(last.startsWith("--- ") && last.endsWith("sidecall grok: " + expected) && !last.contains("SECRET"), last);
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "failure-note-");
    var log = root.resolve("errors.log");
    var common = "\"host\":\"grok\",\"root\":" + quote(root) + ",\"prompt\":\"SECRET prompt\",\"model\":\"agent\"";
    try (var client = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      var pending = client.call("sidecall.noted_call", common);
      require(client.resume(pending, "noted", "{\"value\":null}").contains("\"failed\":false") && !Files.exists(log), "A successful call wrote an error note");
      double[] timeouts = {180, 180.123456, 0.000012345678, 0.00012345678, 999999.5, -0.0};
      String[] text = {"180", "180.123", "1.23457e-05", "0.000123457", "1e+06", "-0"};
      for (int i = 0; i < timeouts.length; i++) {
        fail(client, common, "{\"type\":\"timeout\",\"timeout\":" + timeouts[i] + "}", log, "no reply (timed out after " + text[i] + "s)");
      }
      fail(client, common, "{\"type\":\"exit\",\"returncode\":-9}", log, "no reply (exit -9)");
      fail(client, common, "{\"type\":\"exception\",\"name\":\"RuntimeError\",\"message\":\"head\\u001cmore\\u2028" + "😀".repeat(220) + "\"}", log,
          "failed (RuntimeError: head more " + "😀".repeat(190) + ")");
      require(Files.readAllLines(log).size() == 8, "A failure wrote more than one line");
      System.out.println("Native side-call failure propagation, prompt privacy, general timeout formatting and Unicode message bounds passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
