//JAVA 25
//SOURCES ExtractProposerPrompts.java AdapterProbe.java ParserProbe.java CliProbe.java ContextProbe.java SpiralProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.ArrayList;

/** Source/native checks for the generic streamed-rollout controller. */
class NudgeProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  static int cases;
  static String op(String method, String arguments) { return "{\"method\":" + q(method) + ",\"arguments\":{" + arguments + "}}"; }
  static String monitor(String line) { return op("monitor", "\"line\":" + q(line)); }
  static String request(String config, List<String> operations) { return "{" + (config.isEmpty() ? "" : config + ",") + "\"operations\":[" + String.join(",", operations) + "]}"; }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var expected = source.call(request); var actual = nativeCode.call(request);
    if (!expected.equals(actual)) throw new AssertionError(request + "\nNative: " + actual + "\nSource: " + expected);
    cases++;
  }
  static String batch(boolean start, int count) {
    var content = new ArrayList<String>();
    for (int i = 0; i < count; i++) content.add(start
        ? "{\"type\":\"tool_use\",\"id\":" + q("t" + i) + ",\"name\":\"Bash\",\"input\":{\"command\":\"pytest tests/test_a.py\"}}"
        : "{\"type\":\"tool_result\",\"tool_use_id\":" + q("t" + i) + ",\"content\":\"FAILED test_a.py::test_a\"}");
    return "{\"type\":" + q(start ? "assistant" : "user") + ",\"message\":{\"content\":[" + String.join(",", content) + "]}}";
  }
  static void header(boolean write) throws Exception {
    var source = Files.readString(Path.of("reference/upstream/experiments/te_bench/nudge_guard.py"));
    var block = source.substring(source.indexOf("RESUME_HEADER = ("), source.indexOf("\n\n\nclass NudgeGuard"));
    block = block.substring(0, block.lastIndexOf(')')) + "\n)";
    var text = ExtractProposerPrompts.constant(block, "RESUME_HEADER", "");
    var path = Path.of("src/experiments/nudge_guard/resume-header.txt");
    if (write) { Files.createDirectories(path.getParent()); Files.writeString(path, text); }
    if (!Files.readString(path).equals(text)) throw new AssertionError("Native resume header differs");
  }
  public static void main(String[] args) throws Exception {
    header(args.length == 1 && args[0].equals("--write"));
    if (args.length > 0) { System.out.println("Verified native generic resume header"); return; }
    var checkout = Path.of("").toAbsolutePath();
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "nudge_host.py");
         var source = new AdapterProbe.Client(checkout, false, "nudge_host.py")) {
      var body = op("_nudge_body", "");
      var resume = op("continuation", "\"base_prompt\":\"Fix the task.\"");
      var stop = op("_stop", "\"action\":\"nudge\",\"reason\":\"fixture\"");
      for (var header : List.of("null", "\"\"", q("Custom resume.\n")))
        for (var writer : List.of("", ",\"writer\":[null]", ",\"writer\":[{\"error\":\"offline\"}]", ",\"writer\":[{}]", ",\"writer\":[{\"text\":\"   \\t\",\"_tokens\":3.5}]", ",\"writer\":[{\"text\":\" \\u001cTry é😀 now.\\u001f \",\"_tokens\":4}]"))
          compare(nativeCode, source, request("\"arguments\":{\"verbose\":true,\"resume_header\":" + header + "}" + writer, List.of(resume, stop, resume, resume, body)));
      for (var reply : List.of("null", "{}", "false", "[]", "{\"stuck\":false,\"confidence\":0.9,\"reason\":\"productive\",\"_tokens\":7}", "{\"stuck\":true,\"confidence\":0.95,\"reason\":\"loop\",\"_tokens\":2.5}", "{\"stuck\":0}", "{\"error\":\"unavailable\"}", "{\"stuck\":null,\"reason\":\"ambiguous\"}")) {
        var operations = new ArrayList<String>();
        for (int turn : List.of(0, 1, 4, 5, 9, 10, 15, 20, 25, 35, 55, 75)) operations.add("{\"method\":\"_sampled_judge\",\"state\":{\"turns\":" + turn + "},\"arguments\":{\"digest\":\"safe digest\"}}");
        operations.add(stop); operations.add(resume); operations.add(op("_sampled_judge", "\"digest\":\"after continuation\""));
        compare(nativeCode, source, request("\"arguments\":{\"verbose\":true},\"judge\":[" + String.join(",", java.util.Collections.nCopies(20, reply)) + "]", operations));
      }
      var failing = SpiralProbe.cmd("pytest tests/test_a.py", "1", "FAILED test_a.py::test_a");
      var edit = SpiralProbe.edit(1);
      for (var envelope : List.of("null", "5", "20")) for (int budget : List.of(-1, 0, 1, 2)) {
        var operations = new ArrayList<String>();
        for (int segment = 0; segment < 3; segment++) {
          for (int i = 0; i < 4; i++) operations.add(monitor(edit));
          for (int i = 0; i < 30; i++) operations.add(monitor(failing));
          operations.add(resume);
        }
        compare(nativeCode, source, request("\"arguments\":{\"envelope\":" + envelope + ",\"nudge_budget\":" + budget + ",\"verbose\":true}", operations));
      }
      for (var judge : List.of("", ",\"judge\":[{\"stuck\":true,\"confidence\":0.95,\"_tokens\":5}]", ",\"judge\":[{\"stuck\":false,\"confidence\":0.95,\"_tokens\":5}]", ",\"judge\":[null]", ",\"judge\":[{\"error\":\"no model\"}]")) {
        for (boolean aggressive : List.of(false, true)) for (var backstop : List.of("null", "6", "20.5")) {
          var operations = new ArrayList<String>();
          for (int segment = 0; segment < 2; segment++) {
            operations.add(monitor(edit)); operations.add(monitor(edit));
            operations.add(monitor(batch(true, 30))); operations.add(monitor(batch(false, 30)));
            operations.add(resume);
          }
          compare(nativeCode, source, request("\"arguments\":{\"envelope\":3,\"verbose\":true},\"thresholds\":{\"min_turns\":3,\"edit_churn\":2,\"error_outputs\":2,\"gate_min_turns\":3,\"gate_error_outputs\":1,\"nudge_on_ambiguous\":" + aggressive + ",\"backstop_turns\":" + backstop + "}" + judge, operations));
        }
      }
      for (var line : List.of("invalid", "null", "[]", "{\"type\":\"other\"}", "{\"type\":\"tool.execution_start\",\"data\":{\"toolCallId\":\"x\",\"toolName\":\"Edit\",\"arguments\":{\"path\":\"src/x.rs\"}}}", "{\"type\":\"tool.execution_complete\",\"data\":{\"toolCallId\":\"x\",\"result\":{\"content\":\"ok\"}}}"))
        compare(nativeCode, source, request("", List.of(monitor(line), stop, resume)));
      compare(nativeCode, source, request("\"arguments\":{\"verbose\":true},\"judge\":[{\"stuck\":false,\"reason\":null,\"_tokens\":false}],\"writer\":[{\"text\":\"é😀\"}]", List.of(op("_sampled_judge", "\"digest\":\"d\""), stop, resume)));
      for (var value : List.of("true", "12", "\"invalid\"", "[1]")) {
        compare(nativeCode, source, request("\"judge\":[" + value + "]", List.of(op("_sampled_judge", "\"digest\":\"d\""), stop, resume)));
        compare(nativeCode, source, request("\"writer\":[" + value + "]", List.of(stop, resume, resume)));
      }
      for (var value : List.of("{\"text\":23,\"_tokens\":1}", "{\"text\":false,\"_tokens\":2}", "{\"text\":[],\"_tokens\":true}"))
        compare(nativeCode, source, request("\"writer\":[" + value + "]", List.of(stop, resume, body)));
      compare(nativeCode, source, request("\"arguments\":{\"thr\":{}}", List.of(op("_sampled_judge", "\"digest\":\"unused\""), body)));
      System.out.println("Generic nudge controller matches the pinned implementation in " + cases + " cases; header, sampling, costs, continuation, batched streams and hard backstops passed without model calls");
    }
  }
}
