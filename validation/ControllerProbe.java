//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

/** Drives the native controller without the reference hook or an agent CLI. */
class ControllerProbe {
  static String quote(Object value) { return ContextProbe.quote(value.toString()); }
  static String context(Path root) {
    return """
        {"root":%s,"base":%s,"repo":{"root":%s,"key":"probe","label":"probe","remote":"","how":"path"},
         "host":"claude","env":{},"home":%s,"config":{"enabled":true,"loopguard":"on",
         "loopguard_aggr":"medium","loopguard_thresholds":{},"loopguard_nudges":1,
         "microskills":"off","toolprune":"off","repomap":"off","shadow":false}}
        """.formatted(quote(root), quote(root.resolve("repo-state")), quote(root), quote(root)).strip();
  }
  static String arguments(Path root, String ctx, String payload, boolean replaying) {
    return "\"self\":{\"ctx\":" + ctx + ",\"p\":" + payload + ",\"sid\":\"s\",\"replaying\":" + replaying
        + "},\"home\":" + quote(root) + ",\"process_cwd\":" + quote(root)
        + ",\"process_env\":{},\"queue_limits\":{\"wait\":5,\"budget\":2}";
  }
  static String payload(String event, String fields) {
    return "{\"session_id\":\"s\",\"prompt_id\":\"k\",\"hook_event_name\":" + quote(event) + fields + "}";
  }
  static String finish(ContextProbe.Client client, String result) throws Exception {
    while (result.contains("\"callback\":\"controller\"")) {
      String method;
      String reply;
      if (result.contains("\"method\":\"launcher\"")) { method = "launcher"; reply = "null"; }
      else if (result.contains("\"method\":\"paused_bool\"")) { method = "paused_bool"; reply = "true"; }
      else if (result.contains("\"method\":\"local_routes\"")) { method = "local_routes"; reply = "false"; }
      else throw new AssertionError("Unexpected production callback: " + result);
      result = client.resume(result, method, "{\"value\":" + reply + "}");
    }
    if (!result.startsWith("{\"result\":{\"value\":")) throw new AssertionError(result);
    return result;
  }
  static String call(ContextProbe.Client client, Path root, String ctx, String event, String fields) throws Exception {
    return finish(client, client.call("controller.handle", arguments(root, ctx, payload(event, fields), false)));
  }
  static void empty(String result) {
    if (!result.startsWith("{\"result\":{\"value\":{},")) throw new AssertionError(result);
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "controller-");
    var executable = checkout.resolve("target/debug/te-bridge");
    var ctx = context(root).replace("\n", "");
    var state = root.resolve("repo-state");
    var session = state.resolve("sessions/s.json");
    var read = ",\"tool_name\":\"Read\",\"tool_input\":{\"file_path\":\"a\"},\"tool_response\":\"ok\"";
    try (var client = new ContextProbe.Client(executable, "controller")) {
      empty(call(client, root, ctx, "UserPromptSubmit", ""));
      var clients = new ArrayList<ContextProbe.Client>();
      var start = new CountDownLatch(1);
      try (var pool = Executors.newVirtualThreadPerTaskExecutor()) {
        var jobs = new ArrayList<Future<?>>();
        for (int i = 0; i < 9; i++) {
          var other = new ContextProbe.Client(executable, "controller");
          clients.add(other);
          jobs.add(pool.submit(() -> {
            start.await();
            empty(call(other, root, ctx, "PostToolUse", read));
            return null;
          }));
        }
        start.countDown();
        for (var job : jobs) job.get(15, TimeUnit.SECONDS);
      } finally { for (var other : clients) other.close(); }
      if (!Files.readString(session).contains("\"cum_turns\": 9")) throw new AssertionError("Concurrent native hooks lost calls");
      empty(call(client, root, ctx, "Stop", ""));
      if (!Files.readString(state.resolve("envelope.json")).contains("9.0")) throw new AssertionError("A green turn did not feed the envelope");
      empty(call(client, root, ctx, "UserPromptSubmit", ""));
      Files.writeString(state.resolve("envelope.json"), "{\"resolved_turns\":[8,8,8,8,8]}");
      String queued = null;
      for (int i = 0; i < 40; i++) {
        var edit = i % 2 == 0;
        var fields = edit
            ? ",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"pkg/a.py\",\"old_string\":\"x\",\"new_string\":\"y\"},\"tool_response\":\"ok\""
            : ",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"pytest -q\"},\"error\":\"Exit code 1\\nFAILED tests/test_a.py::test_x - AssertionError: assert 1 == 2\"";
        queued = payload(edit ? "PostToolUse" : "PostToolUseFailure", fields);
        empty(finish(client, client.call("controller.post_tool", arguments(root, ctx, queued, true))));
        if (Files.readString(session).contains("\"nudge\"")) break;
      }
      var held = Files.readString(session);
      if (!held.contains("\"nudge\"") || !held.contains("\"nudges\": 0")) throw new AssertionError("A replay spent or lost its redirect");
      var queue = Files.createDirectories(root.resolve("locks/claude/s.queue"));
      Files.writeString(queue.resolve("000.json"), queued);
      var delivered = call(client, root, ctx, "PostToolUseFailure", ",\"tool_name\":\"Read\",\"tool_input\":{},\"error\":\"Exit code 1\"");
      if (!delivered.contains("\"hookEventName\":\"PostToolUseFailure\"") || !delivered.contains("TokenEconomizer nudged")) throw new AssertionError(delivered);
      if (!Files.readString(session).contains("\"nudges\": 1")) throw new AssertionError("The held redirect was not counted once");
      empty(call(client, root, ctx, "PostToolUse", read));
      var events = Files.readAllLines(state.resolve("events.jsonl"));
      if (events.stream().filter(line -> line.contains("\"kind\": \"nudge\"")).count() != 1) throw new AssertionError("A redirect was delivered twice");
      try (var files = Files.list(queue)) { if (files.findAny().isPresent()) throw new AssertionError("The native replay left its queued event"); }
      System.out.println("Native controller concurrency, envelope learning, queued redirect delivery and failure-event labels passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
