//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** Checks facade ownership, runner resolution and ordered stage calls against the source. */
class FacadeProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  static int cases;
  static String op(String method, String arguments) { return "{\"method\":" + q(method) + ",\"arguments\":{" + arguments + "}}"; }
  static String task = "{\"instance_id\":\"task\",\"problem_statement\":\"Fix this task\"}";
  static String prefix = op("injection_prefix", "\"task\":" + task);
  static String tools(int i) { return op("build_tools", "\"task\":" + task + ",\"i\":" + i); }
  static String kwargs = op("run_kwargs", "\"task\":" + task);
  static String after(boolean resolved, String extra) {
    return op("after_task", "\"task\":" + task + ",\"run_dir_path\":\"$ROOT/run\",\"row\":{\"resolved\":" + resolved + ",\"num_turns\":30,\"tokens_total\":5000" + extra + "},\"srow\":{\"resolved\":" + resolved + ",\"num_turns\":30}");
  }
  static String request(String config, List<String> operations) { return "{\"root\":\"$ROOT\"," + (config.isEmpty() ? "" : config + ",") + "\"operations\":[" + String.join(",", operations) + "]}"; }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, Path root, String request) throws Exception {
    var a = Files.createDirectories(root.resolve("source-" + cases)).toString();
    var b = Files.createDirectories(root.resolve("native-" + cases)).toString();
    var expected = source.call(request.replace("$ROOT", a)).replace(a, "$ROOT");
    var actual = nativeCode.call(request.replace("$ROOT", b)).replace(b, "$ROOT");
    if (!expected.equals(actual)) {
      Files.writeString(root.resolve("request.json"), request);
      Files.writeString(root.resolve("expected.json"), expected);
      Files.writeString(root.resolve("actual.json"), actual);
      int at = 0;
      while (at < expected.length() && at < actual.length() && expected.charAt(at) == actual.charAt(at)) at++;
      throw new AssertionError("Case " + cases + " differs at character " + at + "\nNative: " + actual.substring(Math.max(0, at - 100), Math.min(actual.length(), at + 220)) + "\nSource: " + expected.substring(Math.max(0, at - 100), Math.min(expected.length(), at + 220)));
    }
    cases++;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory("te-facade-probe-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "facade_host.py"); var source = new AdapterProbe.Client(checkout, false, "facade_host.py")) {
      for (var runner : List.of("none", "callable", "protocol", "both", "invalid", "false")) {
        compare(nativeCode, source, root, request("\"kind\":\"optimizer\",\"runner\":" + q(runner), List.of()));
        for (boolean factory : List.of(false, true)) for (var profile : List.of("distiller", "envfacts", "envfacts+distiller")) {
          var cfg = "\"config\":{\"injection\":{\"enabled\":true,\"profile\":" + q(profile) + "},\"tooling\":{\"judge\":true},\"spiral\":{\"guard\":true,\"judge\":true,\"nudge\":true,\"nudge_llm\":true}}";
          compare(nativeCode, source, root, request(cfg + ",\"runner\":" + q(runner) + ",\"factory\":" + factory, List.of(prefix, tools(0), kwargs)));
        }
      }
      var sequence = new ArrayList<String>();
      for (int i = 0; i < 6; i++) { sequence.add(prefix); sequence.add(tools(i)); sequence.add(kwargs); sequence.add(after(i % 2 == 0, "")); }
      for (var agent : List.of("claude", "codex", "copilot")) for (var profile : List.of("distiller", "envfacts", "envfacts+distiller")) {
        var cfg = "\"config\":{\"injection\":{\"enabled\":true,\"profile\":" + q(profile) + ",\"propose_every\":2},\"tooling\":{\"evidence\":2,\"judge\":true,\"codex_untool\":true},\"spiral\":{\"guard\":true,\"judge\":true,\"nudge\":true,\"nudge_llm\":true,\"codex_spiral\":true}}";
        compare(nativeCode, source, root, request(cfg + ",\"agent\":" + q(agent) + ",\"factory\":true,\"reload\":true,\"factory_kinds\":[\"callable\",\"protocol\",\"both\"]", sequence));
      }
      for (var extra : List.of("", ",\"problem_error\":true", ",\"usage_error\":true", ",\"catalog_error\":true", ",\"snapshot_error\":true", ",\"factory_error\":true", ",\"false_factory\":true", ",\"model_error\":true", ",\"tokens\":null", ",\"vocabulary\":\"node\"")) {
        var cfg = "\"config\":{\"injection\":{\"enabled\":true,\"propose_every\":1},\"tooling\":{\"evidence\":1,\"judge\":true},\"spiral\":{\"guard\":true,\"nudge\":true,\"judge\":true,\"nudge_llm\":true}}";
        compare(nativeCode, source, root, request(cfg + ",\"factory\":true" + extra, sequence));
      }
      for (var arguments : List.of("{\"agent\":\"claude\"}", "{\"agent\":\"codex\",\"codex_untool\":true,\"codex_spiral\":true}", "{\"agent\":\"claude\",\"inject\":true,\"spiral_guard\":true,\"spiral_judge\":true,\"max_turns\":10,\"optimizer_model\":\"cheap\"}"))
        compare(nativeCode, source, root, request("\"kind\":\"from_args\",\"arguments\":" + arguments + ",\"factory\":true", sequence));
      compare(nativeCode, source, root, request("\"adapter\":false", List.of(prefix, tools(0), kwargs, after(true, ""))));
      compare(nativeCode, source, root, request("\"config\":{\"injection\":{\"profile\":\"envfacts\"}}", List.of(prefix, "{\"method\":\"injection_prefix\",\"config\":{\"injection\":{\"enabled\":true}}}", after(true, ""))));
      compare(nativeCode, source, root, request("\"runner\":\"invalid\",\"factory\":true,\"factory_error\":true", List.of(prefix, tools(0), kwargs)));
      for (var kinds : List.of("[\"none\",\"protocol\",\"none\"]", "[\"callable\",\"invalid\"]", "[\"none\",\"false\"]"))
        compare(nativeCode, source, root, request("\"config\":{\"injection\":{\"enabled\":true},\"tooling\":{\"judge\":true},\"spiral\":{\"guard\":true,\"judge\":true}},\"factory\":true,\"factory_kinds\":" + kinds, sequence));
      for (boolean guard : List.of(false, true)) for (boolean judge : List.of(false, true))
        for (boolean nudge : List.of(false, true)) for (boolean writer : List.of(false, true)) {
          var cfg = "\"config\":{\"spiral\":{\"guard\":" + guard + ",\"judge\":" + judge + ",\"nudge\":" + nudge + ",\"nudge_llm\":" + writer + "}}";
          var operations = new ArrayList<String>(List.of(kwargs));
          if (guard && judge) operations.add("{\"target\":\"spiral\",\"method\":\"spiral_judge\",\"arguments\":{\"digest\":\"safe trace\"}}");
          if (nudge && writer) operations.add("{\"target\":\"spiral\",\"method\":\"spiral_nudge_writer\",\"arguments\":{\"digest\":\"safe trace\"}}");
          compare(nativeCode, source, root, request(cfg + ",\"factory\":true,\"factory_kinds\":[\"protocol\"]", operations));
        }
      compare(nativeCode, source, root, request("\"config\":{\"tooling\":{\"evidence\":1},\"spiral\":{\"guard\":true}},\"runner\":\"callable\"", List.of(tools(0), after(true, ""),
          "{\"method\":\"run_kwargs\",\"arguments\":{\"task\":" + task + "},\"config\":{\"tooling\":{\"evidence\":0},\"spiral\":{\"guard\":false}}}", tools(1), after(false, ""))));
      System.out.println("Experiment facade matches the pinned implementation in " + cases + " cases; runner resolution, all stages, persisted state, summaries and failure order passed without model calls");
      passed = true;
    } finally {
      if (passed) { try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); } }
      else System.err.println("Probe artifacts: " + root);
    }
  }
}
