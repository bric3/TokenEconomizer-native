//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Original/native report bytes, paired statistics and result-file traversal. */
class CompareProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String request(String method, Path base, String extra) {
    return "{\"method\":" + q(method) + ",\"base\":" + q(base) + (extra.isEmpty() ? "" : "," + extra) + "}";
  }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var actual = nativeCode.call(request); var expected = source.call(request);
    if (!actual.equals(expected)) {
      int at = 0; while (at < actual.length() && at < expected.length() && actual.charAt(at) == expected.charAt(at)) at++;
      throw new AssertionError(request + "\nNative: " + actual.substring(Math.max(0, at - 80), Math.min(actual.length(), at + 400)) + "\nSource: " + expected.substring(Math.max(0, at - 80), Math.min(expected.length(), at + 400)));
    }
    checks++;
  }
  static String row(String id, String tokens, String extra) {
    return "{\"instance\":" + q(id) + ",\"tokens_total\":" + tokens + (extra.isEmpty() ? "" : "," + extra) + "}";
  }
  static void standalone(Path checkout, Path root, Path executable) throws Exception {
    String expected;
    try (var source = new AdapterProbe.Client(checkout, false, "compare_host.py")) {
      var response = source.call(request("main", root, "\"facts\":true,\"stream\":\"bad\\n{}\\n{\\\"manual_facts\\\":{\\\"x\\\":{\\\"inject\\\":true}}}\\n\""));
      require(!response.contains("\"raised\""), "reference report failed: " + response);
      expected = Files.readString(root.resolve("comparison.json"));
    }
    var result = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"), List.of(executable.toString(), "compare-arms", "--baseline", "baseline", "--arm", "arm", "--json", "native.json"));
    require(result.code() == 0 && result.out().contains("paired comparison over 3 tasks") && result.out().contains("current block:\n") && result.out().contains("(stream): 2"), "standalone comparison failed: " + result);
    require(Files.readString(root.resolve("native.json")).equals(expected), "standalone report bytes differ");
    var help = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"), List.of(executable.toString(), "compare-arms", "--help"));
    require(help.code() == 0 && help.out().contains("--baseline"), "comparison help required Python: " + help);
    checks += 3;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "compare-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "compare_host.py");
         var source = new AdapterProbe.Client(checkout, false, "compare_host.py")) {
      var base = root.resolve("case");
      for (var cell : List.of("base*", "?rm", "[ab]*", "missing")) compare(nativeCode, source, request("load_cell", base, "\"cell\":" + q(cell)));
      for (var rows : List.of("[{\"instance\":9007199254740993,\"tokens_total\":10},{\"instance\":9007199254740992,\"tokens_total\":20}]", "[{\"instance\":true,\"tokens_total\":10},{\"instance\":1,\"tokens_total\":20}]"))
        compare(nativeCode, source, request("load_cell", base, "\"baseline\":" + rows));
      compare(nativeCode, source, request("main", base, ""));
      compare(nativeCode, source, request("cost", base, "\"row\":{\"tokens_total\":100,\"optimizer_tokens\":1e16,\"untool_classify_tokens\":1.0,\"untool_judge_tokens\":-1e16}"));
      compare(nativeCode, source, request("cost", base, "\"row\":{\"tokens_total\":0,\"optimizer_tokens\":1e16,\"untool_classify_tokens\":1.0,\"untool_judge_tokens\":1.0}"));
      for (var extra : List.of(
          "\"optimizer_tokens\":9007199254740993,\"untool_classify_tokens\":-9007199254740992,\"untool_judge_tokens\":0.0",
          "\"optimizer_tokens\":18446744073709551615,\"untool_classify_tokens\":-9223372036854775808,\"untool_judge_tokens\":-9223372036854775808,\"codex_untool_adapt_tokens\":0.0,\"spiral_judge_tokens\":2")) {
        compare(nativeCode, source, request("cost", base, "\"row\":{\"tokens_total\":0.0,"+extra+"}"));
        compare(nativeCode, source, request("main", base, "\"baseline\":["+row("mixed", "0.0", "\"optimizer_tokens\":1.0")+"],\"arm\":["+row("mixed", "0.0", extra)+"]"));
      }
      for (var values : List.of(
          List.of("9007199254740993", "-9007199254740992", "0.0", "3"),
          List.of("18446744073709551615", "-9223372036854775808", "-9223372036854775808", "0.0", "3"))) {
        var rows = new java.util.ArrayList<String>();
        for (int i=0;i<values.size();i++) rows.add(row("mixed-"+i, values.get(i), ""));
        var data = "["+String.join(",",rows)+"]";
        compare(nativeCode, source, request("main", base, "\"baseline\":"+data+",\"arm\":"+data));
      }
      for (var row : List.of("{}", "{\"tokens_total\":10}", "{\"tokens_total\":10.0}", "{\"tokens_total\":true,\"optimizer_tokens\":4}", "{\"tokens_total\":100,\"optimizer_tokens\":3.5}", "{\"input_tokens\":10,\"output_tokens\":4,\"model\":\"claude-sonnet-4-5\"}", "{\"input_tokens\":\"bad\"}", "{\"model\":\"unknown-model\",\"input_tokens\":1}"))
        compare(nativeCode, source, request("cost", base, "\"row\":" + row));
      for (var rows : List.of("[]", "[{}]", "[null]", "[1]", "[{\"_raw\":\"{\"}]", "[" + row("x", "1", "\"_config\":{\"version\":1}") + "," + row("x", "2", "\"_config\":null") + "]", "[" + row("x", "1", "\"_config\":{\"version\":1}") + "," + row("x", "2", "\"_config\":\"{\"") + "]"))
        compare(nativeCode, source, request("load_cell", base, "\"baseline\":" + rows));
      for (var rows : List.of("[]", "[{}]", "[" + row("x", "1", "\"usage_incomplete\":true") + "]", "[" + row("x", "0", "") + "]", "[" + row("x", "0.0", "") + "]"))
        compare(nativeCode, source, request("main", base, "\"baseline\":" + rows + ",\"arm\":" + rows + ",\"existing_json\":true"));
      compare(nativeCode, source, request("main", base, "\"arm\":[" + row("task-1", "5", "\"_config\":{\"injection\":{\"present\":true,\"chars\":17}}") + "," + row("task-2", "6", "") + "]"));
      compare(nativeCode, source, request("main", base, "\"facts\":true"));
      for (var state : List.of("null", "{\"facts\":null}", "{\"facts\":{\"x\":{}}}")) compare(nativeCode, source, request("main", base, "\"state\":" + state));
      for (var chars : List.of("\"bad\"", "null", "false", "9007199254740993"))
        compare(nativeCode, source, request("main", base, "\"arm\":[" + row("task-1", "5", "\"_config\":{\"injection\":{\"present\":true,\"chars\":" + chars + "}}") + "]"));
      for (var config : List.of("[]", "null", "{\"routing\":[1]}")) compare(nativeCode, source, request("describe", base, "\"configs\":[[\"x\"," + config + "]]"));
      compare(nativeCode, source, request("describe", base, "\"rows\":[[\"x\",3]]"));
      for (var stream : List.of("bad\n{}\n{\"manual_facts\":{\"x\":{\"inject\":true}}}\n", "{}\n", "[]\n", "{\"manual_facts\":[1]}\n", "{\"manual_facts\":{\"x\":{\"inject\":true}}}\n{\"manual_facts\":[1]}\n"))
        compare(nativeCode, source, request("main", base, "\"stream\":" + q(stream)));
      for (var tokens : List.of(List.of("100", "200", "300"), List.of("100.25", "200.5", "300.75"), List.of("1e20", "1", "1e-20"), List.of("9007199254740991", "9007199254740992", "9007199254740993"), List.of("9007199254740993"))) {
        String rows = java.util.stream.IntStream.range(0, tokens.size()).mapToObj(i -> row("task-" + i, tokens.get(i), "")).collect(java.util.stream.Collectors.joining(",", "[", "]"));
        compare(nativeCode, source, request("main", base, "\"baseline\":" + rows + ",\"arm\":" + rows));
      }
      var random = new java.util.Random(47291);
      for (int trial = 0; trial < 30; trial++) {
        var b = new java.util.ArrayList<String>(); var a = new java.util.ArrayList<String>();
        for (int i = 0; i < 7; i++) { b.add(row("task-" + i, Double.toString(random.nextDouble(1, 1e6)), "")); a.add(row("task-" + i, Double.toString(random.nextDouble(1, 1e6)), "")); }
        compare(nativeCode, source, request("main", base, "\"baseline\":[" + String.join(",", b) + "],\"arm\":[" + String.join(",", a) + "]"));
      }
      for (var argv : List.of("[]", "[\"--help\"]", "[\"--baseline\"]", "[\"--arm\",\"arm\"]", "[\"--baseline\",\"baseline\",\"--arm\",\"arm\",\"--extra\"]", "[\"--b=baseline\",\"--a=arm\"]", "[\"--baseline\",\"baseline\",\"--arm\",\"arm\",\"--json\",\"missing/file.json\"]"))
        compare(nativeCode, source, request("main", base, "\"argv\":" + argv));
      standalone(checkout, root.resolve("standalone"), checkout.resolve("target/debug/te-bench"));
      System.out.println("Verified " + (checks - 3) + " paired-report source comparisons and three standalone checks");
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Preserved comparison fixtures: " + root); }
  }
}
