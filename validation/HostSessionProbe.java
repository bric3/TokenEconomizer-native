//JAVA 25
//SOURCES ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;

/** Exercises native host I/O with local command fixtures and no agent or model calls. */
class HostSessionProbe {
  static String quote(Object value) { return ContextProbe.quote(value.toString()); }
  static void expect(String actual, String expected) { ContextProbe.expect(actual, expected); }
  static String value(String value) { return "{\"value\":" + value + "}"; }
  static String shell(String value) { return "'" + value.replace("'", "'\\''") + "'"; }
  static final String EXPORT = "{\"info\":{},\"messages\":[{\"info\":{\"role\":\"user\"},\"parts\":[{\"type\":\"text\",\"text\":\"hello é😀\"}]}]}";

  static void exportFixture(String[] args) throws Exception {
    if (!"1".equals(System.getenv("TE_SIDECALL")) || !"é😀".equals(System.getenv("TEST_MARK"))
        || !Files.isSameFile(Path.of("."), Path.of(System.getenv("EXPECTED_CWD")))) throw new AssertionError("Export environment or cwd changed");
    if (args.length != 4 || !Arrays.asList(args).subList(1, 3).equals(Arrays.asList("session", "export"))) throw new AssertionError(Arrays.toString(args));
    System.out.write((args[3].equals("bad") ? "{invalid" : EXPORT).getBytes(StandardCharsets.UTF_8));
  }

  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--export-fixture")) { exportFixture(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "host-session's-");
    var cwd = Files.createDirectory(root.resolve("working directory"));
    var program = root.resolve("export-fixture");
    Files.writeString(program, "#!/bin/sh\nexec " + shell(Path.of(System.getProperty("java.home"), "bin/java").toString())
        + " -cp " + shell(System.getProperty("java.class.path")) + " HostSessionProbe --export-fixture \"$@\"\n");
    Files.setPosixFilePermissions(program, PosixFilePermissions.fromString("rwx------"));
    var environment = "\"process_env\":{\"TOKENECONOMIZER_HOME\":" + quote(root.resolve("state"))
        + ",\"EXPECTED_CWD\":" + quote(cwd) + ",\"TEST_MARK\":\"é😀\"},\"process_cwd\":" + quote(cwd) + ",\"home\":" + quote(root);
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"), "host_runtime")) {
      Files.writeString(cwd.resolve("rollout.jsonl"), "{\"payload\":{\"source\":\"exec\"}}\n{broken\n");
      expect(client.call("host_runtime.headless", environment + ",\"host\":\"codex\",\"env\":{},\"transcript\":\"rollout.jsonl\""), "{\"result\":{\"value\":true}}");

      var ancestor = client.call("host_runtime.headless", environment + ",\"host\":\"copilot\",\"env\":{},\"parent_pid\":123,\"transport\":{\"run\":true}");
      ancestor = client.resume(ancestor, "run", value("{\"code\":0,\"stdout\":\"7 /tmp/te-hook --host copilot\\n\",\"stderr\":\"\"}"));
      ancestor = client.resume(ancestor, "run", value("{\"code\":0,\"stdout\":\"0 copilot --prompt=fix\\n\",\"stderr\":\"\"}"));
      expect(ancestor, "{\"result\":{\"value\":true}}");
      expect(client.call("host_runtime._ancestor_args", environment + ",\"parent_pid\":123,\"windows\":true"), "{\"result\":{\"value\":[]}}");

      var copy = cwd.resolve("copy.json");
      Files.writeString(copy, "{\"messages\":[]}");
      var complete = environment + ",\"host\":\"opencode\",\"transcript_path\":\"copy.json\",\"session_id\":\"s\",\"cwd\":" + quote(cwd) + ",\"limits\":{\"export_tries\":1}";
      var pending = client.call("host_runtime.complete", complete);
      expect(client.resume(pending, "program", value("[" + quote(program) + "]")), "{\"result\":{\"value\":\"copy.json\"}}");
      expect(Files.readString(copy), EXPORT);
      Files.createDirectory(cwd.resolve("traces"));
      expect(client.call("host_runtime.mechanism_trace", environment + ",\"host\":\"opencode\",\"transcript_path\":\"copy.json\",\"workdir\":\"traces\""), "{\"result\":{\"value\":\"traces/opencode_trace.jsonl\"}}");
      if (!Files.isRegularFile(cwd.resolve("traces/opencode_trace.jsonl"))) throw new AssertionError("Relative trace output escaped the caller's cwd");

      Files.writeString(copy, "{\"messages\":[]}");
      pending = client.call("host_runtime.complete", complete.replace("\"session_id\":\"s\"", "\"session_id\":\"bad\""));
      expect(client.resume(pending, "program", value("[" + quote(program) + "]")), "{\"result\":{\"value\":\"copy.json\"}}");
      expect(Files.readString(copy), "{\"messages\":[]}");
      try (var files = Files.list(cwd)) { if (files.anyMatch(path -> path.toString().endsWith(".tmp"))) throw new AssertionError("Failed export left a temporary file"); }

      var log = cwd.resolve("copilot.jsonl");
      Files.writeString(log, "{\"type\":\"hook.end\"}\n");
      pending = client.once("host_runtime.settle", environment + ",\"host\":\"copilot\",\"transcript_path\":\"copilot.jsonl\",\"wait_s\":2,\"fresh_s\":60");
      if (!pending.contains("\"method\":\"pending\"")) throw new AssertionError("Fresh Copilot log was not awaited: " + pending);
      expect(client.call("host_helpers.session_id", "\"host\":\"claude\",\"p\":{\"session_id\":\"free\"}"), "{\"result\":\"free\"}");
      Files.writeString(log, "{\"type\":\"session.shutdown\"}\n", StandardOpenOption.APPEND);
      expect(client.call("host_runtime.resume", "\"job\":" + ContextProbe.job(pending)), "{\"result\":{\"value\":null}}");

      var spool = Files.createDirectory(cwd.resolve("cursor-sessions"));
      Files.writeString(spool.resolve("private.jsonl"), "private");
      expect(client.call("host_runtime.settle", environment + ",\"host\":\"cursor\",\"transcript_path\":\"cursor-sessions/private.jsonl\""), "{\"result\":{\"value\":null}}");
      if (Files.exists(spool.resolve("private.jsonl"))) throw new AssertionError("Private Cursor spool was retained");
      Files.createSymbolicLink(spool.resolve("external.jsonl"), log);
      client.call("host_runtime.settle", environment + ",\"host\":\"cursor\",\"transcript_path\":\"cursor-sessions/external.jsonl\"");
      if (!Files.isSymbolicLink(spool.resolve("external.jsonl")) || !Files.isRegularFile(log)) throw new AssertionError("Cursor cleanup touched an external transcript");
      System.out.println("Native host export, relative paths, command ancestry, final-event waiting and guarded spool cleanup passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
