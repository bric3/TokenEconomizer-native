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
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Compare both SDK entries with the pinned plugin; the original runtime harness also stays intact. */
class OpencodeSdkProbe {
  static String q(Object value) { return ParserProbe.q(value).replace("\u2028", "\\u2028").replace("\u2029", "\\u2029"); }
  static void require(boolean condition, String message) { CliProbe.require(condition, message); }
  static int cases;
  static String reply(String text, String notice) {
    return "{\"hookSpecificOutput\":{\"additionalContext\":" + q(text) + "},\"systemMessage\":" + q(notice) + "}";
  }
  static class Client implements AutoCloseable {
    final Process process; final PrintWriter input; final BufferedReader output;
    final Path state, log;
    Client(Path checkout, Path root, Path sdk, int version, String name, String runtime, String... missing) throws Exception {
      state = root.resolve(name + "-state"); log = root.resolve(name + ".jsonl");
      var command = new java.util.ArrayList<String>(List.of(runtime));
      if (runtime.endsWith("node")) command.addAll(List.of("--experimental-strip-types", "--no-warnings"));
      command.addAll(List.of(checkout.resolve("validation/opencode_sdk_host.mjs").toString(), sdk.toString(), Integer.toString(version)));
      var builder = new ProcessBuilder(command).directory(root.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
      builder.environment().clear(); builder.environment().putAll(env(root, state, log));
      builder.environment().put("TE_OC_UNAVAILABLE", ParserProbe.array(List.of(missing)));
      process = builder.start(); input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
      output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
    }
    String call(String request) throws Exception {
      input.println(request); input.flush();
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(25);
      while (!output.ready()) {
        if (!process.isAlive()) throw new AssertionError("SDK exited: " + process.exitValue());
        require(System.nanoTime() < deadline, "SDK callback remained pending: " + request); Thread.sleep(5);
      }
      var line = output.readLine();
      require(line != null && !line.startsWith("{\"error\":"), String.valueOf(line)); return line;
    }
    public void close() throws Exception {
      input.close();
      if (!process.waitFor(15, TimeUnit.SECONDS)) { process.destroyForcibly().waitFor(); throw new AssertionError("SDK did not finish its synchronous exit"); }
      require(process.exitValue() == 0, "SDK failed"); output.close();
    }
  }
  static Map<String,String> env(Path root, Path state, Path log) {
    return new HashMap<>(Map.of("HOME", root.toString(), "PATH", root.resolve("bin").toString(),
        "TOKENECONOMIZER_HOME", state.toString(), "TE_OC_LOG", log.toString(), "TE_OC_WORK", root.resolve("repo").toString(),
        "TE_OC_SessionStart", reply("facts é😀", "started"), "TE_OC_UserPromptSubmit", reply("map", "prompted"),
        "TE_OC_PostToolUse", reply("redirect", "guard notice"), "TE_OC_EXPORT", "{\"info\":{\"id\":\"alpha\"},\"messages\":[]}",
        "TE_MICROSKILLS", "off"));
  }
  static String r(String method, String fields) { return "{\"method\":" + q(method) + (fields.isEmpty() ? "" : "," + fields) + "}"; }
  static String event(String type, String fields) { return "\"body\":{\"type\":" + q(type) + ",\"properties\":{" + fields + "}}"; }
  static String compare(Client nativeClient, Client source, String request, String... fragments) throws Exception {
    var expected = source.call(request).replace(source.state.toString(), nativeClient.state.toString());
    var actual = nativeClient.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected);
    for (var fragment : fragments) require(actual.contains(fragment), "Missing " + fragment + " in " + actual);
    cases++; return actual;
  }
  static String prompt(int version, String id, String text, boolean map) {
    var fields = version == 1 ? "\"input\":{\"sessionID\":" + q(id) + "},\"output\":{\"parts\":[{\"type\":\"text\",\"text\":" + q(text) + "},{\"type\":\"image\",\"data\":\"SECRET IMAGE\"}]}"
        : "\"body\":{\"sessionID\":" + q(id) + ",\"messageID\":" + q("user-" + text) + ",\"prompt\":{\"text\":" + q(text) + "}}";
    if (!map) fields += ",\"env\":{\"TE_OC_UserPromptSubmit\":\"{}\"}";
    return r(version == 1 ? "chat.message" : "prompt", fields);
  }
  static String context(int version, String id) {
    return r(version == 1 ? "experimental.chat.system.transform" : "context",
        version == 1 ? "\"input\":{" + (id == null ? "" : "\"sessionID\":" + q(id)) + "},\"output\":{\"system\":[\"keep\"]}"
            : "\"body\":{\"sessionID\":" + q(id) + ",\"system\":[{\"type\":\"text\",\"text\":\"keep\"}]}" );
  }
  static void scenario(Path checkout, Path root, Path nativeSdk, Path sourceSdk, int version, String runtime) throws Exception {
    var tag = (runtime.endsWith("bun") ? "bun" : "node") + version;
    try (var nativeClient = new Client(checkout, root, nativeSdk, version, "native-" + tag, runtime);
         var source = new Client(checkout, root, sourceSdk, version, "source-" + tag, runtime)) {
      if (version == 1) {
        compare(nativeClient, source, r("event", event("session.created", "\"info\":{\"id\":\"child\",\"parentID\":\"parent\"}")), "\"calls\":[]");
        compare(nativeClient, source, r("event", event("session.created", "\"info\":{\"id\":\"alpha\"}")), "SessionStart");
        compare(nativeClient, source, r("event", event("session.created", "\"sessionID\":\"alpha\"")), "\"calls\":[]");
      }
      compare(nativeClient, source, prompt(version, "alpha", "first", true), "alpha:1");
      compare(nativeClient, source, context(version, "alpha"), "facts é😀\\n\\nmap");
      compare(nativeClient, source, prompt(version, "alpha", "second", false), "alpha:2");
      compare(nativeClient, source, context(version, version == 1 ? null : "alpha"), "facts é😀\\n\\nmap");
      if (version == 1) {
        compare(nativeClient, source, r("tool.execute.after", "\"observe\":true,\"input\":{\"sessionID\":\"alpha\",\"tool\":\"bash\",\"callID\":\"call\",\"args\":null},\"output\":{\"output\":\"failed é😀\",\"metadata\":{\"exit\":3}}"), "failed é😀\\n\\nredirect", "\"exit_code\":3", "\"method\":\"toast\"", "\"observed\"");
        compare(nativeClient, source, r("tool.execute.after", "\"rejectToast\":true,\"input\":{\"sessionID\":\"alpha\",\"tool\":\"bash\"},\"output\":{\"output\":17,\"metadata\":{\"exit\":\"7\"}}"), "17\\n\\nredirect", "\"method\":\"stderr\"", "\"isError\":false");
        compare(nativeClient, source, r("tool.execute.after", "\"env\":{\"TE_SIDECALL\":\"\ufeff YES \"},\"input\":{\"sessionID\":\"alpha\"},\"output\":{\"output\":\"unchanged\"}"), "\"calls\":[]", "\"output\":\"unchanged\"");
        compare(nativeClient, source, r("tool.execute.after", "\"input\":{},\"output\":{\"output\":\"missing id\"}"), "\"calls\":[]");
        compare(nativeClient, source, r("event", event("session.idle", "\"sessionID\":\"alpha\"")), "\"hook_event_name\":\"Stop\"", "alpha:2", "alpha.json");
        compare(nativeClient, source, r("event", event("session.deleted", "\"info\":{\"id\":\"alpha\"}")), "\"reason\":\"deleted\"");
        compare(nativeClient, source, r("event", event("session.deleted", "\"sessionID\":\"alpha\"")), "\"calls\":[]");
      } else {
        compare(nativeClient, source, r("execute.after", "\"observe\":true,\"body\":{\"sessionID\":\"alpha\",\"tool\":\"bash\",\"id\":\"call\",\"status\":\"completed\",\"input\":{},\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"},{\"type\":\"image\",\"data\":\"KEEP IMAGE\"}],\"metadata\":{\"exit\":0}}}"), "KEEP IMAGE", "redirect", "\"method\":\"stderr\"", "\"observed\"");
        for (var result : List.of("{\"content\":\"text\",\"metadata\":{\"exit\":\"7\"}}", "{\"content\":[],\"output\":\"fallback\"}", "{\"content\":false}", "{\"output\":null}", "null")) {
          compare(nativeClient, source, r("execute.after", "\"body\":{\"sessionID\":\"alpha\",\"status\":\"completed\",\"result\":" + result + "}"));
        }
        compare(nativeClient, source, r("execute.after", "\"body\":{\"sessionID\":\"alpha\",\"tool\":\"bash\",\"id\":\"bad\",\"status\":\"failed\",\"error\":{\"message\":\"broken\"}}"), "\"isError\":true", "\"content\":\"broken\"");
        compare(nativeClient, source, r("execute.after", "\"env\":{\"TE_DISABLE\":\" on \"},\"body\":{\"sessionID\":\"alpha\",\"status\":\"completed\",\"result\":{\"content\":\"unchanged\"}}"), "\"calls\":[]", "\"content\":\"unchanged\"");
        for (var data : List.of(
            event("session.step.started", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"model\":{\"id\":\"p/model\"}"),
            event("session.tool.input.started", "\"sessionID\":\"alpha\",\"id\":\"c1\",\"name\":\"bash\""),
            event("session.tool.called", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"id\":\"c1\",\"input\":{\"command\":\"pwd\"}"),
            event("session.tool.success", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"id\":\"c1\",\"content\":[\"/repo\",{\"text\":\"é😀\"}],\"metadata\":{\"exit\":0}"),
            event("session.tool.called", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"id\":\"c2\""),
            event("session.tool.failed", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"id\":\"c2\",\"error\":{\"message\":\"failed tool\"}"),
            event("session.tool.failed", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"missing\",\"id\":\"not recorded\""),
            event("session.step.ended", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"cost\":0.25,\"tokens\":{\"input\":8,\"output\":3,\"cache\":{\"read\":5}}"))) {
          compare(nativeClient, source, r("stream", data));
        }
        compare(nativeClient, source, r("stream", event("session.execution.succeeded", "\"sessionID\":\"alpha\"")), "\"hook_event_name\":\"Stop\"", "alpha:2", "p/model", "\\n");
        compare(nativeClient, source, r("stream", event("session.idle", "\"sessionID\":\"alpha\"")), "\"calls\":[]");
        compare(nativeClient, source, r("stream", event("session.step.ended", "\"sessionID\":\"alpha\",\"assistantMessageID\":\"a1\",\"cost\":null")));
        compare(nativeClient, source, r("stream", event("session.deleted", "\"sessionID\":\"alpha\"")), "\"reason\":\"deleted\"");
        compare(nativeClient, source, r("stream", event("session.deleted", "\"sessionID\":\"alpha\"")), "\"calls\":[]");
      }
      compare(nativeClient, source, prompt(version, "beta", "headless", true).replace("\"method\":", "\"env\":{\"TE_OPENCODE_MODE\":\"run\"},\"method\":"), "beta:1");
      compare(nativeClient, source, r(version == 1 ? "event" : "stream", "\"env\":{\"TE_OPENCODE_MODE\":\"run\"}," + event("session.idle", "\"sessionID\":\"beta\"")), "\"reason\":\"run\"");
      compare(nativeClient, source, prompt(version, "exit-session", "exit", false), "exit-session:1");
    }
    var nativeState = root.resolve("native-" + tag + "-state");
    var sourceState = root.resolve("source-" + tag + "-state");
    var nativeExit = Files.readString(nativeState.resolve("opencode-sessions/exit-session.json"));
    var sourceExit = Files.readString(sourceState.resolve("opencode-sessions/exit-session.json"));
    require(nativeExit.equals(sourceExit), "Synchronous exit export differs\nNative: " + nativeExit + "\nSource: " + sourceExit);
    require(Files.readString(root.resolve("native-" + tag + ".jsonl")).contains("\"reason\":\"exit\""), "Native backend did not finish its exit hook");
  }
  static void unavailable(Path checkout, Path root, Path sdk, Path original) throws Exception {
    for (var missing : List.of("prompt", "context", "execute.after")) {
      try (var nativeClient = new Client(checkout, root, sdk, 2, "missing-native-" + missing, "/opt/homebrew/bin/node", missing);
           var source = new Client(checkout, root, original, 2, "missing-source-" + missing, "/opt/homebrew/bin/node", missing)) {
        compare(nativeClient, source, prompt(2, "partial", "partial SDK", true));
        compare(nativeClient, source, context(2, "partial"));
        compare(nativeClient, source, r("execute.after", "\"body\":{\"sessionID\":\"partial\",\"status\":\"completed\",\"result\":{\"content\":\"ok\"}}"));
        compare(nativeClient, source, r("stream", event("session.idle", "\"sessionID\":\"partial\"")));
        compare(nativeClient, source, r("cleanup", ""));
        compare(nativeClient, source, r("stream", event("session.idle", "\"sessionID\":\"partial\"")), "closed");
      }
    }
  }
  static void fixture(String[] args) throws Exception {
    var log = Path.of(System.getenv("TE_OC_LOG"));
    if (args[0].equals("--export")) {
      Files.writeString(log, "{\"export\":" + ParserProbe.array(List.of(args).subList(1, args.length)) + "}\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      System.out.print(System.getenv().getOrDefault("TE_OC_EXPORT", "{}")); return;
    }
    var payload = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
    var event = Pattern.compile("\"hook_event_name\"\\s*:\\s*\"([^\"]+)\"").matcher(payload);
    require(event.find(), payload);
    Files.writeString(log, "{\"payload\":" + payload + ",\"argv\":" + ParserProbe.array(List.of(args).subList(1, args.length))
        + ",\"mode\":" + q(System.getenv("TE_OPENCODE_MODE")) + ",\"control_token\":" + System.getenv().containsKey("TE_NATIVE_CONTROL_TOKEN") + "}\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
    System.out.print(System.getenv().getOrDefault("TE_OC_" + event.group(1), "{}"));
  }
  static void launcher(Path path, String mode) throws Exception {
    Files.writeString(path, "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''")
        + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' OpencodeSdkProbe " + mode + " \"$@\"\n");
    Files.setPosixFilePermissions(path, PosixFilePermissions.fromString("rwx------"));
  }
  static void originalHarness(Path checkout, Path root, Path sdk) throws Exception {
    var test = Files.readString(checkout.resolve("reference/upstream/tests/test_opencode_plugin.py"));
    var harness = Pattern.compile("(?s)HARNESS = '''(.*?)'''").matcher(test); require(harness.find(), "Original SDK harness missing");
    var file = root.resolve("original-harness.ts");
    Files.writeString(file, harness.group(1).replace("%s", q(sdk.toUri())));
    var log = root.resolve("original-harness.jsonl");
    for (var runtime : List.of("/opt/homebrew/bin/bun", "/opt/homebrew/bin/opencode")) {
      Files.deleteIfExists(log);
      var environment = env(root, root.resolve("original-harness-state"), log);
      environment.put("BUN_BE_BUN", "1");
      var out = NativeCliProbe.process(root, environment, List.of(runtime, "run", file.toString()));
      require(out.code() == 0 && out.out().contains("harness done"), out.toString());
      var calls = Files.readAllLines(log); require(calls.size() == 4, calls.toString());
      for (var event : List.of("SessionStart", "UserPromptSubmit", "PostToolUse", "SessionEnd")) require(calls.stream().anyMatch(line -> line.contains(event)), calls.toString());
    }
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0) { fixture(args); return; }
    var checkout = Path.of("").toAbsolutePath(); var root = Files.createTempDirectory(checkout.resolve("validation/results"), "opencode-sdk-");
    boolean passed = false;
    try {
      Files.createDirectories(root.resolve("repo")); Files.createDirectories(root.resolve("bin"));
      launcher(root.resolve("bin/opencode"), "--export");
      var built = NativeCliProbe.process(root, Map.of("HOME", root.toString(), "PATH", ""), List.of(checkout.resolve("target/release/te-package").toString(), "--out", root.resolve("market").toString()));
      require(built.code() == 0, built.toString());
      var plugin = root.resolve("market/tokeneconomizer"); launcher(plugin.resolve("scripts/te-hook"), "--hook");
      var source = Files.createDirectories(root.resolve("source/opencode")).resolve("tokeneconomizer.ts");
      Files.copy(checkout.resolve("reference/upstream/plugin/opencode/tokeneconomizer.ts"), source);
      launcher(Files.createDirectories(root.resolve("source/scripts")).resolve("te-hook"), "--hook");
      for (var runtime : List.of("/opt/homebrew/bin/node", "/opt/homebrew/bin/bun")) for (int version : List.of(1, 2)) scenario(checkout, root, plugin.resolve("opencode/tokeneconomizer.ts"), source, version, runtime);
      originalHarness(checkout, root, plugin.resolve("opencode/tokeneconomizer.ts"));
      unavailable(checkout, root, plugin.resolve("opencode/tokeneconomizer.ts"), source);
      System.out.println("Packaged opencode SDK matches upstream in " + cases + " callback cases across v1/v2 and Node/Bun; the original runtime harness and synchronous exits passed without an agent on PATH");
      passed = true;
    } finally {
      if (passed) { try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); } }
      else System.err.println("Probe artifacts: " + root);
    }
  }
}
