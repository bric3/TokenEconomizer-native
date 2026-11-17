//JAVA 25
//SOURCES ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.StringJoiner;

/** Drives native hook input, output and failure policy without Python or an agent. */
class HookProbe {
  static String quote(Object value) { return ContextProbe.quote(value.toString()).replace("\n", "\\n").replace("\r", "\\r"); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String bytes(String text) {
    var values = new StringJoiner(",", "[", "]");
    for (byte value : text.getBytes(StandardCharsets.UTF_8)) values.add(Integer.toString(Byte.toUnsignedInt(value)));
    return values.toString();
  }
  static String common(Path root, String flags) {
    return "\"home\":" + quote(root) + ",\"process_cwd\":" + quote(root) + ",\"pid\":1234,\"env\":{\"TOKENECONOMIZER_HOME\":"
        + quote(root.resolve("state")) + flags + "}";
  }
  static String main(ContextProbe.Client client, Path root, String flags, String input, String argv) throws Exception {
    return client.call("hook.main", common(root,flags) + ",\"argv\":" + argv + ",\"input\":" + input + ",\"patched_run\":true");
  }
  static String reply(ContextProbe.Client client, String pending, String value) throws Exception {
    return client.resume(pending,"run","{\"value\":" + value + "}");
  }
  static void output(String actual, String text) { ContextProbe.expect(actual, "{\"result\":{\"value\":" + quote(text) + "}}"); }

  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"hook-");
    var state = root.resolve("state");
    var errors = state.resolve("errors.log");
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"),"hook")) {
      for (var flag : new String[]{"1","TRUE"," yes ","On", "\u001cTRUE\u001f"}) {
        var result = client.call("hook._truthy","\"v\":" + quote(flag).replace("\u001c", "\\u001c").replace("\u001f", "\\u001f"));
        ContextProbe.expect(result,"{\"result\":true}");
      }
      for (var flag : new String[]{"0","false","enabled","", "[]"}) ContextProbe.expect(client.call("hook._truthy","\"v\":" + quote(flag)),"{\"result\":false}");
      for (var input : new String[]{"null","[]","42","false","\"text\""}) output(main(client,root,"",bytes(input),"[]"),"{}\n");
      output(main(client,root,",\"TE_DISABLE\":\"yes\"",bytes("not json"),"[]"),"{}\n");
      output(main(client,root,",\"TE_SIDECALL\":true",bytes("not json"),"[]"),"{}\n");
      require(!Files.exists(state),"Ignored inputs created state");

      output(main(client,root,",\"TE_DISABLE\":1","[255]","[]"),"{}\n");
      require(Files.readString(errors).contains("pid=1234"),"UTF-8 errors were suppressed by the disable flag");
      output(client.call("hook.main",common(root,"") + ",\"argv\":[],\"input_error\":\"OSError: unreadable\\n\""),"{}\n");
      require(Files.readString(errors).contains("OSError: unreadable\n"),"Input error detail was lost");
      output(main(client,root,"",bytes("{"),"[]"),"{}\n");

      var pending = main(client,root,"",bytes("{\"path\":\"é😀\"}"),"[\"--event\",\"SessionStart\",\"--host\",\"grok\"]");
      require(pending.contains("\"hook_event_name\":\"SessionStart\"") && pending.contains("é😀") && pending.contains("\"grok\"]"),pending);
      Files.setPosixFilePermissions(state,PosixFilePermissions.fromString("rwxrwxrwx"));
      output(reply(client,pending,"{\"_progress\":[\"é😀\",\"next\"],\"additionalContext\":\"ctx\"}"),
          "{\"type\": \"progress\", \"message\": \"\\u00e9\\ud83d\\ude00\"}\n{\"type\": \"progress\", \"message\": \"next\"}\n{\"additionalContext\": \"ctx\"}\n");
      require(Files.getPosixFilePermissions(state).equals(PosixFilePermissions.fromString("rwx------")),"Successful hook did not secure its state root");
      pending = main(client,root,"",bytes("{\"hook_event_name\":null}"),"[\"--event\",\"Stop\",\"--host\"]");
      require(pending.contains("\"hook_event_name\":null") && pending.contains(",null]"),pending);
      output(reply(client,pending,"null"),"{}\n");
      pending = main(client,root,"",bytes("{}"),"[\"--event\"]");
      require(!pending.contains("hook_event_name"),pending);
      output(client.resume(pending,"run","{\"error\":\"RuntimeError: bug\\n\"}"),"{}\n");
      require(Files.readString(errors).contains("RuntimeError: bug\n"),"Hook failure was not recorded");
      long size = Files.size(errors);
      pending = main(client,root,"",bytes("{}"),"[]");
      var invalid = reply(client,pending,"{\"_progress\":1}");
      require(invalid.contains("\"raised\":\"RuntimeError\"") && Files.size(errors) == size,"Progress errors crossed the hook error boundary: " + invalid);

      Files.delete(errors);
      Files.delete(state);
      Files.writeString(state,"occupied");
      output(main(client,root,"",bytes("{"),"[]"),"{}\n");
      ContextProbe.expect(Files.readString(state),"occupied");
      System.out.println("Native hook UTF-8, flags, event precedence, progress, error boundaries and state permissions passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
