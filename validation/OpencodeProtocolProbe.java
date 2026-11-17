//JAVA 25
//SOURCES CliProbe.java ContextProbe.java ParserProbe.java

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** The authenticated exit path can finish while an SDK callback is still awaiting its caller. */
class OpencodeProtocolProbe {
  static void require(boolean condition, String message) { CliProbe.require(condition, message); }
  static String q(Object value) { return ParserProbe.q(value); }
  static String read(Process process, BufferedReader output) throws Exception {
    long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
    while (!output.ready()) {
      require(process.isAlive(), "Backend stopped before replying");
      require(System.nanoTime() < deadline, "Backend did not reply"); Thread.sleep(5);
    }
    var line = output.readLine(); require(line != null, "Backend closed its output");
    return line.startsWith("{\"checkpoint\":") ? read(process, output) : line;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "opencode-protocol-");
    var token = "0".repeat(64);
    var builder = new ProcessBuilder(checkout.resolve("target/debug/te").toString(), "--opencode-bridge", "2")
        .directory(root.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().clear(); builder.environment().putAll(Map.of("HOME", root.toString(), "PATH", "", "TE_NATIVE_CONTROL_TOKEN", token));
    var process = builder.start();
    try (var input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
         var output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
      var ready = Pattern.compile("^\\{\"ready\":(\\d+)\\}$").matcher(read(process, output));
      require(ready.matches(), "No loopback control address"); int port = Integer.parseInt(ready.group(1));
      try (var socket = new Socket("127.0.0.1", port)) {
        socket.setSoTimeout(5_000);
        socket.getOutputStream().write("{\"token\":\"wrong\",\"context\":{}}\n".getBytes(StandardCharsets.UTF_8));
        require(socket.getInputStream().read() == -1, "Wrong token was accepted");
      }
      require(process.isAlive(), "Unauthenticated control request stopped the backend");
      input.println("{"); input.flush(); require(read(process, output).contains("\"error\""), "Malformed request was accepted");
      input.println("{\"id\":1,\"method\":\"context\",\"body\":{}}"); input.flush();
      require(read(process, output).contains("not initialised"), "SDK callback ran before initialisation");
      input.println("{\"id\":2,\"method\":\"init\",\"body\":{\"directory\":\"/repo\"}}"); input.flush();
      ContextProbe.expect(read(process, output), "{\"id\":2,\"value\":{\"actions\":[]}}");
      input.println("{\"id\":3,\"method\":\"init\",\"body\":{}}"); input.flush();
      require(read(process, output).contains("already initialised"), "Live configuration was replaced");
      input.println("{\"id\":4,\"method\":\"unknown\",\"body\":{}}"); input.flush();
      require(read(process, output).contains("unknown opencode callback"), "Unknown callback was accepted");
      input.println("{\"id\":5,\"method\":\"prompt\",\"body\":{\"sessionID\":\"pending\",\"prompt\":{\"text\":\"unprocessed\"}}}"); input.flush();
      require(read(process, output).contains("\"callback\":\"context\""), "SDK context was not requested");
      input.println("{\"id\":5,\"method\":\"prompt\",\"body\":{\"sessionID\":\"duplicate\"}}"); input.flush();
      require(read(process, output).contains("duplicate opencode request id"), "Duplicate request replaced its waiting caller");
      var request = root.resolve("exit.json");
      Files.writeString(request, "{\"port\":" + port + ",\"token\":" + q(token) + ",\"context\":{\"env\":{\"TE_DISABLE\":\"1\",\"TOKENECONOMIZER_HOME\":" + q(root.resolve("state"))
          + "},\"home\":" + q(root) + ",\"process_cwd\":" + q(root) + ",\"argv\":[],\"exec_path\":\"/no-agent\"},\"restore\":[[\"unused\"," + q("x".repeat(1_100_000)) + "]]}");
      var helper = new ProcessBuilder(checkout.resolve("target/debug/te").toString(), "--opencode-exit")
          .directory(root.toFile()).redirectInput(request.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
      try {
        require(helper.waitFor(10, TimeUnit.SECONDS) && helper.exitValue() == 0, "Synchronous helper did not finish");
        require(process.waitFor(5, TimeUnit.SECONDS) && process.exitValue() == 0, "Backend did not finish the authenticated exit");
      } finally { if (helper.isAlive()) helper.destroyForcibly().waitFor(); }
      var exported = Files.readString(root.resolve("state/opencode-sessions/pending.json"));
      require(exported.contains("\"messages\":[]") && !exported.contains("unprocessed"), exported);
      require(!Files.exists(root.resolve("state/opencode-sessions/duplicate.json")), "Rejected duplicate changed the open sessions");
      System.out.println("Native opencode rejects invalid configuration, duplicate IDs and unauthenticated control; synchronous exit completes an accepted session while its SDK callback is pending");
    } finally {
      if (process.isAlive()) process.destroyForcibly().waitFor();
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
