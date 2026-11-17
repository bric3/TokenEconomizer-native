//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

/** Native private spools, Unicode limits and concurrent delivery checks. */
class HostStateProbe {
  static String quote(Object value) { return ContextProbe.quote(value.toString()); }
  static void expect(String actual, String expected) { ContextProbe.expect(actual, expected); }
  static String value(String actual) { return ContextProbe.value(actual); }
  static String race(ContextProbe.Client client, String args, CountDownLatch ready, CountDownLatch go) throws Exception {
    ready.countDown();
    if (!go.await(3, TimeUnit.SECONDS)) throw new AssertionError("Parallel delivery was not released");
    return client.call("host_runtime.render", args);
  }

  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "host-state-");
    var state = root.resolve("state");
    var env = "{\"TOKENECONOMIZER_HOME\":" + quote(state) + "}";
    var common = "\"process_env\":" + env + ",\"process_cwd\":" + quote(root) + ",\"home\":" + quote(root) + ",\"env\":" + env;
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"), "host_runtime");
         var other = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"), "host_runtime");
         var pool = Executors.newFixedThreadPool(2)) {
      var input = "é😀".repeat(10002);
      var payload = "{\"hook_event_name\":\"postToolUse\",\"conversation_id\":\"c\",\"cwd\":" + quote(root)
          + ",\"tool_name\":\"Write\",\"tool_input\":{\"file_path\":\"a\",\"content\":" + quote(input)
          + "},\"tool_output\":\"{\\\"stdout\\\":\\\"ok\\\"}\",\"user_email\":\"private-marker@example.test\"}";
      var prepared = value(client.call("host_runtime.prepare", common + ",\"host\":\"cursor\",\"payload\":" + payload));
      expect(client.call("host_runtime.record_event", common + ",\"payload\":" + prepared), "{\"result\":{\"value\":null}}");
      var spool = state.resolve("cursor-sessions/c.jsonl");
      var recorded = Files.readString(spool);
      if (recorded.contains("private-marker") || recorded.contains("tool_output") || !recorded.contains("\"content\": " + quote("é😀".repeat(10000)))) throw new AssertionError("Cursor retained private fields or exceeded its Unicode cap");
      if (!Files.getPosixFilePermissions(spool).equals(PosixFilePermissions.fromString("rw-------"))) throw new AssertionError("Cursor spool permissions changed");

      var start = common + ",\"host\":\"grok\",\"ctx_env\":" + env + ",\"payload\":{\"session_id\":\"g\",\"hook_event_name\":\"SessionStart\"}";
      expect(client.call("host_runtime.render", start + ",\"out\":{}"), "{\"result\":{\"value\":{\"out\":{}}}}");
      var unicode = client.call("host_runtime._spool", common + ",\"payload\":{\"session_id\":\"éⅧ\"}");
      expect(unicode, "{\"result\":{\"value\":" + quote(state.resolve("grok-start/e_Ⅷ.txt")) + "}}");
      var tool = common + ",\"host\":\"grok\",\"ctx_env\":" + env + ",\"payload\":{\"session_id\":\"g\",\"hook_event_name\":\"PostToolUse\"},\"out\":{\"hookSpecificOutput\":{\"additionalContext\":\"own\"}}";
      for (int i = 0; i < 8; i++) {
        expect(client.call("host_runtime.render", start + ",\"out\":{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"learned é😀\"}}"), "{\"result\":{\"value\":{\"out\":{}}}}");
        var ready = new CountDownLatch(2);
        var go = new CountDownLatch(1);
        var one = pool.submit(() -> race(client, tool, ready, go));
        var two = pool.submit(() -> race(other, tool, ready, go));
        if (!ready.await(3, TimeUnit.SECONDS)) throw new AssertionError("Parallel hooks did not start");
        go.countDown();
        var first = one.get(5, TimeUnit.SECONDS);
        var second = two.get(5, TimeUnit.SECONDS);
        int deliveries = (first.contains("learned é😀") ? 1 : 0) + (second.contains("learned é😀") ? 1 : 0);
        if (deliveries != 1 || Files.exists(state.resolve("grok-start/g.txt"))) throw new AssertionError("Parallel hooks delivered the saved context " + deliveries + " times");
      }
      var own = "é".repeat(9990);
      client.call("host_runtime.render", start + ",\"out\":{\"hookSpecificOutput\":{\"additionalContext\":" + quote("😀".repeat(20)) + "}}" );
      var capped = client.call("host_runtime.render", tool.replace("\"own\"", quote(own)));
      if (!capped.contains("😀".repeat(8) + "\\n\\n" + own) || capped.contains("😀".repeat(9))) throw new AssertionError("Grok's Unicode cap displaced the tool's own context");

      var st = "{\"offset\":0,\"last_step_index\":-1,\"turn_seq\":0}";
      expect(client.call("host_runtime._save", common + ",\"cid\":\"s\",\"st\":" + st), "{\"result\":{\"value\":null}}");
      var saved = state.resolve("antigravity-sessions/s.json");
      if (!Files.getPosixFilePermissions(saved).equals(PosixFilePermissions.fromString("rw-------"))) throw new AssertionError("Replay state permissions changed");
      var invalid = Files.readString(saved).replace("\"turn_seq\": 0", "\"turn_seq\": \"invalid\"");
      Files.writeString(saved, invalid);
      var rejected = client.call("host_runtime.expand", common + ",\"payload\":{\"session_id\":\"s\",\"hook_event_name\":\"PreInvocation\",\"transcript_path\":\"missing.jsonl\"}");
      if (!rejected.contains("\"raised\":\"ValueError\"") || !Files.readString(saved).equals(invalid)) throw new AssertionError("Invalid replay state was committed: " + rejected);
      client.call("host_runtime._save", common + ",\"cid\":\"s\",\"st\":" + st.replace("\"turn_seq\":0", "\"turn_seq\":\"9007199254740993\""));
      var advanced = client.call("host_runtime.expand", common + ",\"payload\":{\"session_id\":\"s\",\"hook_event_name\":\"PreInvocation\",\"transcript_path\":\"missing.jsonl\"}");
      if (!advanced.contains("s:9007199254740994") || !Files.readString(saved).contains("\"turn_seq\": 9007199254740994")) throw new AssertionError("A saved integer string lost precision: " + advanced);
      var old = saved.resolveSibling("old.json");
      Files.writeString(old, "{}");
      Files.setLastModifiedTime(old, FileTime.fromMillis(1));
      client.call("host_runtime._prune", common);
      if (Files.exists(old) || !Files.exists(saved)) throw new AssertionError("Replay pruning removed fresh state or retained expired state");
      System.out.println("Native Cursor privacy and caps, parallel Grok delivery, replay validation and private-state checks passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
