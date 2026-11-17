//JAVA 25
//SOURCES ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.concurrent.TimeUnit;

/** Native stdin, simultaneous output, early close and timeout checks using a local fixture. */
class ProcessInputProbe {
  static String quote(Object value) { return ContextProbe.quote(value.toString()).replace("\r", "\\r").replace("\n", "\\n"); }
  static String digest(byte[] bytes) throws Exception { return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes)); }
  static class Client extends ContextProbe.Client {
    Client(Path executable) throws Exception { super(executable, "sidecall"); }
    @Override String once(String op, String args) throws Exception {
      return super.once(op.equals("sidecall.resume") ? "sidecall.resume_job" : op, args);
    }
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--child")) {
      if (args[1].equals("early")) return;
      if (args[1].equals("sleep")) {
        Files.writeString(Path.of(args[2]), Long.toString(ProcessHandle.current().pid()));
        Thread.sleep(10_000);
        return;
      }
      System.out.write("x".repeat(200_000).getBytes(StandardCharsets.UTF_8));
      System.err.write("é😀".repeat(40_000).getBytes(StandardCharsets.UTF_8));
      var input = System.in.readAllBytes();
      System.out.write((":" + digest(input)).getBytes(StandardCharsets.UTF_8));
      return;
    }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "process-input-");
    var java = Path.of(System.getProperty("java.home"), "bin/java");
    var command = quote(java) + ",\"-cp\"," + quote(System.getProperty("java.class.path")) + ",\"ProcessInputProbe\",\"--child\",";
    var text = "é😀<prompt>\r\n&%PATH%".repeat(70_000);
    var common = "\"env\":{},\"process_cwd\":" + quote(root) + ",\"input\":" + quote(text);
    try (var client = new Client(checkout.resolve("target/debug/te-bridge"))) {
      var result = client.call("sidecall.process_input", common + ",\"argv\":[" + command + "\"flood\"]");
      if (!result.contains("\"code\":0") || !result.contains("x".repeat(200_000) + ":" + digest(text.getBytes(StandardCharsets.UTF_8)))
          || !result.contains("é😀".repeat(40_000))) throw new AssertionError("Native stdin or simultaneous output was truncated");
      ContextProbe.expect(client.call("sidecall.process_input", common + ",\"argv\":[" + command + "\"early\"]"), "{\"result\":{\"value\":{\"code\":0,\"stdout\":\"\",\"stderr\":\"\"}}}");
      var pidFile = root.resolve("pid");
      var timedOut = client.call("sidecall.process_input", common + ",\"timeout\":0.5,\"argv\":[" + command + "\"sleep\"," + quote(pidFile) + "]");
      if (!timedOut.contains("\"raised\":\"TimeoutExpired\"")) throw new AssertionError(timedOut);
      long pid = Long.parseLong(Files.readString(pidFile));
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(3);
      while (ProcessHandle.of(pid).isPresent()) {
        if (System.nanoTime() >= deadline) throw new AssertionError("Timed-out native child survived");
        Thread.sleep(10);
      }
      System.out.println("Native UTF-8 stdin, simultaneous pipe output, early close and timeout cleanup passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
