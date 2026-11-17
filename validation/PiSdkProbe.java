//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/** Run original and packaged Pi callbacks against identical hook I/O, without an installed agent. */
class PiSdkProbe {
  static String q(Object value) { return ParserProbe.q(value).replace("\u2028", "\\u2028").replace("\u2029", "\\u2029"); }
  static void require(boolean condition, String message) { CliProbe.require(condition, message); }
  static int cases;
  static class Client implements AutoCloseable {
    final Process process;
    final PrintWriter input;
    final BufferedReader output;
    Client(Path checkout, Path root, Path sdk, String name) throws Exception {
      var builder = new ProcessBuilder("/opt/homebrew/bin/node", "--experimental-strip-types", "--no-warnings",
          checkout.resolve("validation/pi_sdk_host.mjs").toString(), sdk.toString())
          .directory(root.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
      builder.environment().clear();
      builder.environment().putAll(Map.of("HOME", root.toString(), "PATH", "", "TOKENECONOMIZER_HOME", root.resolve("state").toString(),
          "TE_PI_LOG", root.resolve(name + ".jsonl").toString(), "TE_MICROSKILLS", "off", "TE_LOOPGUARD", "off"));
      process = builder.start();
      input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
      output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
    }
    String call(String request) throws Exception {
      input.println(request); input.flush();
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(25);
      while (!output.ready()) {
        if (!process.isAlive()) throw new AssertionError("Pi SDK fixture exited: " + process.exitValue());
        require(System.nanoTime() < deadline, "Pi SDK callback remained pending");
        Thread.sleep(5);
      }
      var line = output.readLine(); require(line != null && !line.contains("\"error\":"), String.valueOf(line));
      return line;
    }
    public void close() throws Exception {
      input.close();
      if (!process.waitFor(5, TimeUnit.SECONDS)) {
        process.destroyForcibly().waitFor(); throw new AssertionError("Idle Pi backend kept Node alive");
      }
      require(process.exitValue() == 0, "Pi SDK fixture failed"); output.close();
    }
  }
  static String response(String context, String notice) {
    return "{\"hookSpecificOutput\":{\"additionalContext\":" + context + "},\"systemMessage\":" + notice + "}";
  }
  static String event(String event, String fields, String reply) {
    return "{\"event\":" + q(event) + ",\"env\":{\"TE_PI_REPLY\":" + q(reply) + "}" + (fields.isEmpty() ? "" : "," + fields) + "}";
  }
  static String crash(String event, String fields) {
    return event(event, fields, "{}").replace("\"env\":{", "\"env\":{\"TE_PI_CRASH\":\"1\",");
  }
  static String compare(Client nativeClient, Client source, String request, String... fragments) throws Exception {
    var expected = source.call(request); var actual = nativeClient.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected);
    for (var fragment : fragments) require(actual.contains(fragment), "Missing " + fragment + " in " + actual);
    cases++; return actual;
  }
  static String prompt(String text) {
    return "\"body\":{\"prompt\":" + q(text) + ",\"systemPromptOptions\":{\"sections\":{\"other\":\"keep\"}}}";
  }
  static void scenario(Path checkout, Path root, Path nativeSdk, Path sourceSdk) throws Exception {
    try (var nativeClient = new Client(checkout, root, nativeSdk, "native"); var source = new Client(checkout, root, sourceSdk, "source")) {
      compare(nativeClient, source, event("session_start", "\"body\":{\"reason\":\"new\"}", response(q("facts é😀"), q("ready"))), "\"source\":\"clear\"", "\"text\":\"ready\"");
      compare(nativeClient, source, event("before_agent_start", prompt("first"), response(q("map"), q("prompt"))), "facts é😀\\n\\nmap", "\"other\":\"keep\"", "session:1");
      compare(nativeClient, source, event("before_agent_start", prompt("second"), "{}"), "facts é😀\\n\\nmap", "session:2");
      var tool = "\"body\":{\"toolName\":\"bash\",\"toolCallId\":\"t1\",\"input\":null,\"isError\":true,\"structuredContent\":{\"exit_code\":7,\"secret\":\"omit\"},\"content\":[{\"type\":\"text\",\"text\":\"é😀\"},{\"type\":\"image\",\"data\":\"SECRET IMAGE\"},{\"type\":\"text\"},{\"type\":\"text\",\"text\":null},{\"type\":\"text\",\"text\":false}]}";
      compare(nativeClient, source, event("tool_result", tool, response(q("redirect"), q("notice"))), "\"deliverAs\":\"steer\"", "\"content\":\"é😀\\n\\n\\nfalse\"", "\"exit_code\":7", "session:2");
      compare(nativeClient, source, event("agent_end", "", response(q("ignore"), q("ignore"))), "\"actions\":[]", "\"hook_event_name\":\"Stop\"", "session:2");
      compare(nativeClient, source, event("session_shutdown", "\"body\":{\"reason\":\"closed\"}", response(q("ignore"), q("ignore"))), "\"actions\":[]", "\"reason\":\"closed\"");
      for (var reason : List.of("startup", "resume", "fork", "reload", "unknown")) {
        compare(nativeClient, source, event("session_start", "\"body\":{\"reason\":" + q(reason) + "}", "{}"));
        compare(nativeClient, source, event("before_agent_start", prompt(reason), "{}"), "session:1");
      }
      String wrapped = "<tokeneconomizer>\ncarried é😀\n</tokeneconomizer>";
      for (var entries : List.of(
          "[{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"SECRET PROMPT\"}}]",
          "[{\"type\":\"message\",\"message\":{\"role\":\"system\",\"sections\":{\"tokeneconomizer\":" + q(wrapped) + "}}}]",
          "[{\"type\":\"message\",\"message\":{\"role\":\"system\",\"sections\":{\"tokeneconomizer\":\"old\"}}},{\"type\":\"compaction\",\"systemMessage\":{\"sections\":{\"other\":\"reset\"}}}]",
          "[{\"type\":\"compaction\",\"systemMessage\":{\"sections\":{\"tokeneconomizer\":" + q(wrapped) + "}}},{\"type\":\"message\",\"message\":{\"role\":\"system\",\"sections\":{\"other\":\"keep\"}}}]",
          "[{\"type\":\"message\",\"message\":{\"role\":\"system\",\"sections\":{\"tokeneconomizer\":null}}}]")) {
        compare(nativeClient, source, event("session_start", "\"body\":{\"reason\":\"new\"},\"entries\":" + entries, response(q("NEW FACTS MUST NOT REPLACE CARRIED"), "null")), "\"source\":\"resume\"");
        var actual = compare(nativeClient, source, event("before_agent_start", prompt("resume"), "{}"));
        require(!actual.contains("NEW FACTS"), actual);
      }
      for (var reply : List.of("log line\n" + response(q("whole é😀"), q("log parsed")), "invalid JSON", "null", "[]", "false", "7", "\ufeff{}\u2029")) {
        compare(nativeClient, source, event("session_start", "", reply));
        compare(nativeClient, source, event("before_agent_start", prompt("output"), "{}"));
      }
      compare(nativeClient, source, event("session_start", "", response(q("keep on hook failure"), "null")));
      for (var flag : List.of("1", " TRUE ", "\ufeffyes\u2029", "ON")) {
        var request = "{\"event\":\"before_agent_start\"," + prompt("disabled") + ",\"env\":{\"TE_DISABLE\":" + q(flag) + ",\"TE_PI_REPLY\":" + q(response(q("omit"), q("omit"))) + "}}";
        compare(nativeClient, source, request, "keep on hook failure", "\"calls\":[]");
      }
      compare(nativeClient, source, "{\"event\":\"before_agent_start\"," + prompt("sidecall") + ",\"env\":{\"TE_SIDECALL\":\"yes\"}}", "keep on hook failure", "\"calls\":[]");
      compare(nativeClient, source, event("before_agent_start", "\"hasUI\":false," + prompt("headless"), response("null", q("headless notice"))), "\"method\":\"stderr\"");
      compare(nativeClient, source, event("tool_result", "\"body\":{}", response("{}", "42")), "\"content\":{}", "\"tool_input\":{}");
      compare(nativeClient, source, event("tool_result", "\"body\":{\"content\":[{\"type\":\"text\",\"text\":1e20},{\"type\":\"text\",\"text\":1e21},{\"type\":\"text\",\"text\":1e-6},{\"type\":\"text\",\"text\":1e-7},{\"type\":\"text\",\"text\":-0.0},{\"type\":\"text\",\"text\":9007199254740993},{\"type\":\"text\",\"text\":[1,null,{}]}]}", "{}"));
      compare(nativeClient, source, event("session_start", "", response("[]", "null")));
      compare(nativeClient, source, event("before_agent_start", prompt("coerce"), response("{}", "null")), "\\n\\n[object Object]");
      compare(nativeClient, source, event("command", "\"args\":\" status --json \"", "{}"), "\"method\":\"notify\"");
      compare(nativeClient, source, event("command", "\"args\":\"\ufeff \"", "{}"), "\"method\":\"notify\"");
      compare(nativeClient, source, event("session_start", "\"sid\":\"new-session\",\"file\":\"/tmp/transcript.jsonl\",\"mode\":\"json\"", "{}"), "new-session", "transcript.jsonl", "\"mode\":\"json\"");
      compare(nativeClient, source, event("agent_end", "\"sid\":\"new-session\"", "{}"), "session:1");
      var slow = event("before_agent_start", prompt("slow"), response(q("slow map"), q("slow notice")))
          .replace("\"env\":{", "\"env\":{\"TE_PI_WAIT_FOR\":\"fast-prompt\",");
      var fast = event("before_agent_start", "\"release\":\"fast-prompt\"," + prompt("fast"), response(q("fast map"), q("fast notice")));
      compare(nativeClient, source, "{\"batch\":[" + slow + "," + fast + "]}", "fast map\\n\\nslow map", "session:1", "session:2");
      compare(nativeClient, source, event("before_agent_start", prompt("after concurrent"), "{}"), "fast map\\n\\nslow map", "session:3");
      compare(nativeClient, source, event("before_agent_start", prompt("broken hook"), "invalid JSON"), "fast map\\n\\nslow map", "session:4");
      var nonzero = event("before_agent_start", prompt("nonzero exit"), response("null", q("stdout survives")))
          .replace("\"env\":{", "\"env\":{\"TE_PI_EXIT\":\"7\",");
      compare(nativeClient, source, nonzero, "stdout survives", "fast map\\n\\nslow map");
      var nativeHook = nativeSdk.getParent().getParent().resolve("scripts/te-hook");
      var sourceHook = sourceSdk.getParent().getParent().resolve("scripts/te-hook");
      Files.move(nativeHook, nativeHook.resolveSibling("te-hook.saved"));
      Files.move(sourceHook, sourceHook.resolveSibling("te-hook.saved"));
      try {
        compare(nativeClient, source, event("before_agent_start", prompt("missing hook"), "{}"), "fast map\\n\\nslow map", "\"calls\":[]");
      } finally {
        Files.move(nativeHook.resolveSibling("te-hook.saved"), nativeHook);
        Files.move(sourceHook.resolveSibling("te-hook.saved"), sourceHook);
      }
      var gone = Files.createDirectory(root.resolve("removed-pi-cwd"));
      compare(nativeClient, source, "{\"chdir\":" + q(gone) + "}");
      Files.delete(gone);
      compare(nativeClient, source, event("before_agent_start", prompt("removed cwd"), "{}"), "fast map\\n\\nslow map");
    }
  }
  static void recovery(Path checkout, Path root, Path nativeSdk, Path sourceSdk) throws Exception {
    try (var nativeClient = new Client(checkout, root, nativeSdk, "recover-native");
         var source = new Client(checkout, root, sourceSdk, "recover-source")) {
      compare(nativeClient, source, event("session_start", "", response(q("recovery facts é😀"), "null")));
      compare(nativeClient, source, event("before_agent_start", prompt("first"), response(q("map"), "null")), "session:1");
      compare(nativeClient, source, crash("before_agent_start", prompt("crash while prompting")), "session:2", "recovery facts é😀\\n\\nmap");
      compare(nativeClient, source, event("before_agent_start", prompt("after crash"), "{}"), "session:3", "recovery facts é😀\\n\\nmap");
      compare(nativeClient, source, crash("tool_result", "\"body\":{\"toolName\":\"read\",\"toolCallId\":\"crash\"}"), "session:3");
      compare(nativeClient, source, event("before_agent_start", prompt("after tool crash"), "{}"), "session:4", "recovery facts é😀\\n\\nmap");
      var slow = crash("before_agent_start", "\"release\":\"crash-finished\"," + prompt("crash overlapping prompts"))
          .replace("\"env\":{", "\"env\":{\"TE_PI_WAIT_FOR_LOGS\":\"2\",");
      var other = event("before_agent_start", prompt("also waiting"), "{}")
          .replace("\"env\":{", "\"env\":{\"TE_PI_WAIT_FOR\":\"crash-finished\",");
      compare(nativeClient, source, "{\"batch\":[" + slow + "," + other + "]}", "session:5", "session:6", "recovery facts é😀\\n\\nmap");
      compare(nativeClient, source, event("before_agent_start", prompt("after overlapping crash"), "{}"), "session:7", "recovery facts é😀\\n\\nmap");
      var entries = "\"entries\":[{\"type\":\"message\",\"message\":{\"role\":\"system\",\"sections\":{\"tokeneconomizer\":\"<tokeneconomizer>\\ncarried after crash\\n</tokeneconomizer>\"}}}]";
      compare(nativeClient, source, crash("session_start", entries), "\"source\":\"resume\"");
      compare(nativeClient, source, event("before_agent_start", prompt("after resumed crash"), "{}"), "session:1", "carried after crash");
      compare(nativeClient, source, crash("agent_end", ""), "session:1");
      compare(nativeClient, source, event("before_agent_start", prompt("after stop crash"), "{}"), "session:2", "carried after crash");
      compare(nativeClient, source, crash("session_start", "\"body\":{\"reason\":\"new\"}"), "\"source\":\"clear\"");
      var cleared = compare(nativeClient, source, event("before_agent_start", prompt("after cleared crash"), "{}"), "session:1");
      require(!cleared.contains("carried after crash"), cleared);
      require(Files.readAllLines(root.resolve("recover-native.jsonl.crashes")).size() == 6, "Recovery fixture did not kill six native backends");
    }
  }
  static void hookChild(String[] args) throws Exception {
    var input = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
    var record = "{\"argv\":" + ParserProbe.array(List.of(args).subList(1, args.length)) + ",\"payload\":" + input
        + ",\"mode\":" + q(System.getenv("TE_PI_MODE")) + ",\"utf8\":" + q(System.getenv("PYTHONUTF8")) + "}\n";
    Files.writeString(Path.of(System.getenv("TE_PI_LOG")), record, StandardOpenOption.CREATE, StandardOpenOption.APPEND);
    var parent = ProcessHandle.current().parent().orElseThrow();
    var waitFor = System.getenv("TE_PI_WAIT_FOR");
    var log = Path.of(System.getenv("TE_PI_LOG"));
    int expected = Integer.parseInt(System.getenv().getOrDefault("TE_PI_WAIT_FOR_LOGS", "0"));
    long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
    while ((waitFor != null && !Files.exists(Path.of(log + ".barrier-" + waitFor))) || Files.readAllLines(log).size() < expected) {
      if (!parent.isAlive()) return;
      require(System.nanoTime() < deadline, "Hook fixture barrier did not open");
      Thread.sleep(10);
    }
    if ("1".equals(System.getenv("TE_PI_CRASH"))) {
      if (Path.of(parent.info().command().orElseThrow()).getFileName().toString().equals("te")) {
        require(parent.destroyForcibly(), "Fixture could not kill its native parent");
        Files.writeString(Path.of(System.getenv("TE_PI_LOG") + ".crashes"), "killed\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      }
      return;
    }
    var bytes = System.getenv().getOrDefault("TE_PI_REPLY", "{}").getBytes(StandardCharsets.UTF_8);
    for (var b : bytes) { System.out.write(b); System.out.flush(); }
    System.exit(Integer.parseInt(System.getenv().getOrDefault("TE_PI_EXIT", "0")));
  }
  static void hookFixture(Path scripts) throws Exception {
    var wrapper = "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''")
        + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' PiSdkProbe --hook-child \"$@\"\n";
    Files.writeString(scripts.resolve("te-hook"), wrapper);
    Files.setPosixFilePermissions(scripts.resolve("te-hook"), PosixFilePermissions.fromString("rwx------"));
  }
  static List<String> exchange(Path checkout, Path root, String requests) throws Exception {
    var process = new ProcessBuilder(checkout.resolve("target/debug/te").toString(), "--pi-bridge")
        .directory(root.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
    try {
      try (var input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8))) {
        input.println(requests);
      }
      require(process.waitFor(5, TimeUnit.SECONDS), "Pi protocol did not finish after EOF");
      var lines = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8).lines().toList();
      require(process.exitValue() == 0, lines.toString());
      return lines;
    } finally { if (process.isAlive()) process.destroyForcibly().waitFor(); }
  }
  static void protocol(Path checkout, Path root) throws Exception {
    var lines = exchange(checkout, root, String.join("\n", "{", "{\"id\":\"bad\"}", "{\"id\":1,\"event\":\"unknown\"}",
        "{\"id\":2,\"event\":\"session_start\",\"context\":{\"env\":{\"TE_DISABLE\":\"1\"}},\"body\":{\"entries\":[]}}",
        "{\"id\":3,\"restore\":{}}"));
    require(lines.size() == 6, lines.toString());
    require(lines.get(0).contains("\"error\"") && lines.get(1).contains("invalid request id")
        && lines.get(2).contains("unknown Pi event") && lines.stream().anyMatch(line -> line.contains("only valid on the first request")), lines.toString());
    require(lines.get(3).contains("\"replies\":{\"2\":{\"actions\":[]}}"), lines.toString());
    require(lines.stream().anyMatch(line -> line.startsWith("{\"id\":2,\"value\":{\"actions\":[]},\"checkpoint\":")), lines.toString());
    var base = ",\"event\":\"before_agent_start\",\"context\":{\"session_id\":\"restored\",\"env\":{\"TE_DISABLE\":\"1\",\"SECRET\":\"private environment\"}},\"body\":{\"prompt\":\"PRIVATE PROMPT\"}}";
    for (var counter : List.of("-1", "0.5", "9007199254740994", "null", "\"bad\"")) {
      var result = exchange(checkout, root, "{\"id\":1,\"restore\":{\"manual\":\"facts\",\"prompts\":" + counter + ",\"turn\":\"old:9\"}" + base);
      require(result.size() == 1 && result.getFirst().contains("\"error\""), result.toString());
    }
    var restored = exchange(checkout, root, "{\"id\":1,\"restore\":{\"manual\":\"facts é😀\",\"prompts\":9,\"turn\":\"old:9\"}" + base);
    require(restored.size() == 2 && restored.getLast().contains("\"turn\":\"restored:10\"") && restored.getLast().contains("\"tokeneconomizer\":\"facts é😀\""), restored.toString());
    require(restored.getLast().contains("\"replies\":{}"), "Completed requests remained in the recovery snapshot");
    require(restored.stream().noneMatch(line -> line.contains("PRIVATE PROMPT") || line.contains("private environment")), "Checkpoint copied prompt or environment data");
    var maximum = exchange(checkout, root, "{\"id\":1,\"restore\":{\"manual\":\"facts\",\"prompts\":9007199254740992,\"turn\":\"old\"}" + base);
    require(maximum.size() == 2 && maximum.getLast().contains("\"turn\":\"restored:9007199254740992\""), maximum.toString());
    var failedCommand = exchange(checkout, root, "{\"id\":1,\"event\":\"command\",\"context\":{\"cwd\":\"/tmp\"},\"body\":{\"args\":\"status\"}}");
    require(failedCommand.size() == 2 && failedCommand.stream().allMatch(line -> line.contains("te: no output (check the native CLI)")), failedCommand.toString());
  }
  static void brokenBackend(Path checkout, Path root, Path sdk) throws Exception {
    var executable = sdk.getParent().getParent().resolve("scripts/te");
    try (var client = new Client(checkout, root, sdk, "broken")) {
      client.call(event("session_start", "", response(q("facts survive malformed backends"), "null")));
      client.call(event("before_agent_start", prompt("before malformed backend"), "{}"));
      client.call(crash("tool_result", "\"body\":{}"));
      Files.move(executable, executable.resolveSibling("te.saved"));
      Files.writeString(executable, "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''")
          + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' PiSdkProbe --bad-backend\n");
      Files.setPosixFilePermissions(executable, PosixFilePermissions.fromString("rwx------"));
      for (int i = 0; i < 2; i++) {
        var actual = client.call(event("agent_end", "", "{}"));
        require(actual.contains("\"actions\":[]") && actual.contains("\"calls\":[]"), actual);
      }
      Files.delete(executable); Files.move(executable.resolveSibling("te.saved"), executable);
      var resumed = client.call(event("before_agent_start", prompt("after malformed backend"), "{}"));
      require(resumed.contains("facts survive malformed backends") && resumed.contains("session:2"), resumed);
    }
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--hook-child")) { hookChild(args); return; }
    if (args.length > 0 && args[0].equals("--bad-backend")) { System.out.println("invalid backend JSON"); System.out.flush(); Thread.sleep(30_000); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "pi-sdk-");
    try {
      var built = NativeCliProbe.process(root, Map.of("HOME", root.toString(), "PATH", ""),
          List.of(checkout.resolve("target/release/te-package").toString(), "--out", root.resolve("market").toString()));
      require(built.code() == 0, built.toString());
      var plugin = root.resolve("market/tokeneconomizer");
      var source = Files.createDirectories(root.resolve("source/pi")).resolve("tokeneconomizer.ts");
      Files.copy(checkout.resolve("reference/upstream/plugin/pi/tokeneconomizer.ts"), source);
      var scripts = Files.createDirectories(root.resolve("source/scripts"));
      Files.copy(checkout.resolve("target/release/te"), scripts.resolve("te"));
      hookFixture(scripts); hookFixture(plugin.resolve("scripts"));
      scenario(checkout, root, plugin.resolve("pi/tokeneconomizer.ts"), source);
      recovery(checkout, root, plugin.resolve("pi/tokeneconomizer.ts"), source);
      protocol(checkout, root);
      brokenBackend(checkout, root, plugin.resolve("pi/tokeneconomizer.ts"));
      System.out.println("Packaged Pi SDK matches upstream in " + cases + " callback cases; seven backend kills, overlapping pending calls, malformed backend replacement, snapshot privacy and restore validation passed without an agent on PATH");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
