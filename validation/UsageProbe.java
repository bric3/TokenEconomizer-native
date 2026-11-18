//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** Checks each harness's cost basis and interrupted-stream recovery against the source. */
class UsageProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  static int cases;
  static Path artifacts;
  static String request(String domain, String method, String arguments) { return "{\"domain\":" + q(domain) + ",\"method\":" + q(method) + ",\"arguments\":{" + arguments + "}}"; }
  static String op(String method, String arguments) { return "{\"method\":" + q(method) + ",\"arguments\":{" + arguments + "}}"; }
  static String observe(String id, String usage) { return op("observe", "\"ev\":{\"type\":\"assistant\",\"message\":{\"id\":" + id + ",\"usage\":" + usage + "}}"); }
  static String session(String id, String output) { return "{\"type\":\"assistant\",\"message\":{\"id\":" + id + ",\"usage\":{\"output_tokens\":" + output + "}}}"; }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var expected = source.call(request); var actual = nativeCode.call(request);
    if (!expected.equals(actual)) {
      Files.writeString(artifacts.resolve("request.json"), request); Files.writeString(artifacts.resolve("expected.json"), expected); Files.writeString(artifacts.resolve("actual.json"), actual);
      int at = 0; while (at < expected.length() && at < actual.length() && expected.charAt(at) == actual.charAt(at)) at++;
      throw new AssertionError("Case " + cases + " differs at " + at + "\nNative: " + actual.substring(Math.max(0,at-100), Math.min(actual.length(),at+240)) + "\nSource: " + expected.substring(Math.max(0,at-100),Math.min(expected.length(),at+240)));
    }
    cases++;
  }
  public static void main(String[] args) throws Exception {
    var root = Path.of("").toAbsolutePath(); artifacts = Files.createTempDirectory("te-usage-probe-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(root,true,"usage_host.py"); var source = new AdapterProbe.Client(root,false,"usage_host.py")) {
      var empty = List.of("", "garbage", "{}", "[]", "null", "false", "true", "12", "[1]", "{\"type\":\"other\"}");
      for (var text : empty) for (var separator : List.of("\n", "\r\n", "\r", "\u001c", "\u0085", "\u2028")) {
        var stream = text + separator + " " + text;
        compare(nativeCode,source,request("codex","row_usage","\"stream_text\":" + q(stream)));
        compare(nativeCode,source,request("claude","session_output_tokens","\"text\":" + q(stream)));
        compare(nativeCode,source,request("copilot","turns_from_stream","\"stream_text\":" + q(stream)));
        compare(nativeCode,source,request("copilot","stream_output_tokens","\"stream_text\":" + q(stream)));
      }
      var counters = List.of("0", "1", "true", "false", "null", "-4", "1.75", "\"15\"", "\"١٢\"", "9007199254740993", "[]", "\"invalid\"");
      for (var n : counters) {
        var usage = "{\"input_tokens\":" + n + ",\"cached_input_tokens\":3,\"cache_write_input_tokens\":2,\"output_tokens\":5,\"reasoning_output_tokens\":2}";
        var stream = "{\"type\":\"item.completed\",\"item\":{\"type\":\"command_execution\"}}\n{\"type\":\"turn.completed\",\"usage\":" + usage + "}";
        compare(nativeCode,source,request("codex","row_usage","\"stream_text\":" + q(stream)));
        compare(nativeCode,source,request("claude","_row_from_result","\"result\":{\"usage\":{\"input_tokens\":" + n + "}}"));
        compare(nativeCode,source,request("copilot","stream_output_tokens","\"stream_text\":" + q("{\"data\":{\"outputTokens\":" + n + ",\"output_tokens\":2}}")));
        compare(nativeCode,source,request("claude","session_output_tokens","\"text\":" + q(session("\"m1\"",n))));
      }
      for (var domain : List.of("claude","codex","copilot")) for (var a : List.of("{}", "null", "{\"input_tokens\":null,\"num_turns\":4,\"tokens_source\":\"debug_log\"}", "{\"input_tokens\":4,\"num_turns\":null,\"tokens_source\":\"stream_usage\"}"))
        for (var b : List.of("{}", "null", "{\"input_tokens\":8,\"output_tokens\":3,\"tokens_source\":\"stream_usage\"}", "{\"input_tokens\":null,\"num_turns\":2,\"tokens_source\":\"debug_log\",\"thread_id\":\"second\"}"))
          compare(nativeCode,source,request(domain,domain.equals("claude") ? "_merge_usage" : "merge_usage", "\"a\":" + a + ",\"b\":" + b));
      for (var details : List.of("{}", "null", "{\"cached_tokens\":50,\"cache_creation_tokens\":20}", "{\"cached_tokens\":200}", "{\"cached_tokens\":true}", "{\"cache_creation_tokens\":3.5}")) {
        var log = "request \"usage\": {\"prompt_tokens\":100,\"completion_tokens\":10,\"prompt_tokens_details\":" + details + "}\n";
        for (var text : List.of("",log,log+log,log+"\"usage\": {", "\"usage\": {}\n"+log, log.replace(": {", ":{"))) {
          compare(nativeCode,source,request("copilot","sum_openai_usage","\"log_text\":" + q(text)));
          compare(nativeCode,source,request("copilot","row_usage","\"stream_text\":" + q("{\"type\":\"assistant.turn_start\",\"data\":{\"outputTokens\":9}}") + ",\"log_text\":" + q(text)));
        }
      }
      for (var id : List.of("\"m1\"", "1", "true", "1.0", "0", "null", "[]", "[1]")) for (var log : List.of("", session(id,"500"), session(id,"500")+"\n"+session(id,"700"), session("\"different\"","300"), "[1]")) {
        var operations = List.of(observe(id,"{\"input_tokens\":10,\"cache_creation_input_tokens\":30,\"cache_read_input_tokens\":50,\"output_tokens\":1}"), observe(id,"{\"input_tokens\":20,\"cache_creation_input_tokens\":40,\"cache_read_input_tokens\":80,\"output_tokens\":9}"), op("synthesize","\"reason\":\"spiral_guard_stop\",\"session_text\":" + q(log)));
        compare(nativeCode,source,"{\"domain\":\"recovery\",\"operations\":[" + String.join(",",operations) + "]}");
      }
      var aliases = List.of(observe("true","{\"input_tokens\":10}"), observe("1.0","{\"input_tokens\":20}"), observe("1","{\"input_tokens\":30}"), op("synthesize","\"reason\":\"timeout\",\"session_text\":" + q(session("1","900"))));
      compare(nativeCode,source,"{\"domain\":\"recovery\",\"operations\":[" + String.join(",",aliases) + "]}");
      compare(nativeCode,source,"{\"domain\":\"recovery\",\"operations\":[" + observe("1e100","{\"input_tokens\":10}") + "," + observe("2e100","{\"input_tokens\":20}") + "," + op("synthesize","\"reason\":\"large numeric ids\"") + "]}");
      for (var event : List.of("null", "false", "true", "[]", "[1]", "{\"type\":\"assistant\",\"message\":1}"))
        compare(nativeCode,source,"{\"domain\":\"recovery\",\"operations\":[" + op("observe", "\"ev\":" + event) + "," + op("synthesize","\"reason\":\"malformed event\"") + "]}");
      for (var usage : List.of("{\"input_tokens\":true,\"cache_read_input_tokens\":2.9}", "{\"input_tokens\":9007199254740993}", "{\"input_tokens\":-1.75}", "\"bad\"", "[1]"))
        compare(nativeCode,source,"{\"domain\":\"recovery\",\"operations\":[" + observe("\"m\"",usage) + "," + op("synthesize","\"reason\":\"partial\"") + "," + op("synthesize","\"reason\":\"repeat\"") + "]}");
      for (var sourceValue : List.of("\"stream_usage\"", "true", "[1]", "{}"))
        compare(nativeCode,source,request("codex","merge_usage","\"a\":{\"tokens_source\":" + sourceValue + "},\"b\":{\"num_turns\":2}"));
      for (var stream : List.of("{\"session_id\":\"first\"}\n{\"conversation_id\":\"ignored\"}\n{\"type\":\"thread.started\",\"thread_id\":\"later\"}", "{\"type\":\"error\",\"message\":\"é😀 error\"}\n{\"type\":\"turn.completed\"}", "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":20}}\n{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":100}}"))
        compare(nativeCode,source,request("codex","row_usage","\"stream_text\":" + q(stream)));
      for (var a : List.of("{\"usage\":{\"input_tokens\":10,\"read\":null,\"bool\":true,\"extra\":\"skip\"},\"num_turns\":3,\"total_cost_usd\":0.3,\"usage_incomplete_reason\":\"timeout\",\"output_tokens_exact_turns\":true}", "{\"usage\":{\"input_tokens\":2.5},\"output_tokens_missing\":true}"))
        compare(nativeCode,source,request("claude","_merge_usage","\"a\":" + a + ",\"b\":{\"usage\":{\"input_tokens\":20,\"extra\":2},\"num_turns\":4,\"total_cost_usd\":0.2,\"usage_incomplete_reason\":\"guard\",\"output_tokens_exact_turns\":2}"));
      var logs = Files.createDirectory(artifacts.resolve("logs"));
      Files.writeString(logs.resolve("b.log"),"b\r\nc\rd"); Files.writeString(logs.resolve("a.log"),"a\n"); Files.writeString(logs.resolve(".hidden.log"),"hidden"); Files.write(logs.resolve("c.log"),new byte[]{(byte)0xff,(byte)0xc3, (byte)0xa9});
      compare(nativeCode,source,request("copilot","read_log_dir","\"log_dir\":" + q(logs)));
      compare(nativeCode,source,"{\"domain\":\"copilot\",\"method\":\"read_log_dir\",\"arguments\":{\"log_dir\":\"logs\"},\"cwd\":" + q(artifacts) + "}");
      Files.createDirectory(logs.resolve("ab.log"));
      compare(nativeCode,source,request("copilot","read_log_dir","\"log_dir\":" + q(logs)));
      compare(nativeCode,source,request("copilot","read_log_dir","\"log_dir\":" + q(logs.resolve("missing"))));
      System.out.println("Harness usage accounting matches the pinned implementation in " + cases + " cases; three schemas, unknown costs, segment merges, message deduplication and interrupted recovery passed without model calls");
      passed = true;
    } finally {
      if (passed) { try (var paths = Files.walk(artifacts)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); } }
      else System.err.println("Probe artifacts: " + artifacts);
    }
  }
}
