//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.util.List;
import java.util.Map;

/** Compares aggregation policy, actual CSV bytes and canonical-index preservation. */
class AggregateProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var actual = nativeCode.call(request); var expected = source.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected);
    checks++; return actual;
  }
  static String method(String name, String params) { return "{\"method\":" + q(name) + ",\"params\":{" + params + "}}"; }
  static String main(Path root, Path out, String extra) {
    return "{\"method\":\"main\",\"argv\":[" + q(root) + extra + "],\"out\":" + q(out) + "}";
  }
  static Path row(Path root, String arm, String cell, String name, String fields) throws Exception {
    var dir = Files.createDirectories(root.resolve(arm).resolve(cell).resolve(name));
    var path = dir.resolve("row.json");
    Files.writeString(path, "{\"instance\":\"pkg-1\",\"model\":\"gpt-5.6-terra\",\"run_dir\":" + q(dir) + fields + "}");
    return path;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "aggregate-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "aggregate_host.py");
         var source = new AdapterProbe.Client(checkout, false, "aggregate_host.py")) {
      for (var row : List.of("{}", "{\"resolved\":true}", "{\"resolved\":1}", "{\"patch_bytes\":8}",
          "{\"usage_incomplete\":true,\"usage_incomplete_reason\":\"truncated\"}",
          "{\"usage_incomplete\":true,\"usage_incomplete_reason\":12}",
          "{\"resolved\":true,\"input_tokens\":1}", "{\"tokens_total\":7}", "{\"agent\":\"copilot-fixture\",\"resolved\":true}",
          "{\"agent\":\"codex\",\"patch_bytes\":1}", "{\"agent\":\"claude\",\"cost_usd_vendor\":0.0,\"resolved\":true}",
          "{\"cost_usd_vendor\":true,\"cost_usd\":1.5}", "{\"cost_usd_vendor\":1,\"cost_usd\":2}",
          "{\"eval_report\":{\"eval_exec_failed\":true}}", "{\"eval_report\":{\"eval_exec_failed\":true,\"eval_exec_rc\":-1}}",
          "{\"eval_report\":{\"eval_exec_failed\":true,\"eval_exec_error\":\"gone\",\"grading_error\":\"ignored\"}}",
          "{\"eval_report\":{\"grading_error\":\"lost\\ncontainer\"}}", "{\"eval_report\":[]}"))
        for (var method : List.of("usage_incomplete", "not_graded", "vendor_usd", "vendor_cost_missing"))
          compare(nativeCode, source, method(method, "\"d\":" + row));
      for (var repo : List.of("", "scikit-learn", "pydata__xarray", "org__package", "sympy"))
        for (var instance : List.of("", "org__pkg-42", "alone"))
          compare(nativeCode, source, method("normalize_repo", "\"rs\":" + q(repo) + ",\"inst\":" + q(instance)));
      for (var cell : List.of("sympy_s0", "foo_sbar_s1", "no-seed", "scikit-learn_s-1", "é😀_s１２"))
        compare(nativeCode, source, method("cell_of", "\"path\":" + q(root.resolve("arm/" + cell + "/run/row.json")) + ",\"root\":" + q(root) + ",\"arm\":\"arm\""));
      for (var verdict : List.of("true", "false", "null", "\"\"", "1"))
        for (var extra : List.of("", ",\"not_graded\":true", ",\"resolved_p1\":null,\"resolved_p2\":false",
            ",\"webbench_attempts\":0", ",\"webbench_attempts\":1", ",\"webbench_attempts\":2", ",\"webbench_attempts\":\"2\"",
            ",\"resolved_p1\":false,\"resolved_p2\":true", ",\"usage_incomplete\":true,\"vendor_cost_missing\":true")) {
          var row = "{\"arm\":\"a\",\"repo\":\"r\",\"seed\":0,\"resolved\":" + verdict + ",\"is_error\":false,\"cost_effective_tokens\":\"1.5\",\"num_turns\":\"2\"" + extra + "}";
          compare(nativeCode, source, method("summarize", "\"rows\":[" + row + "," + row + "]"));
        }
      var old = row(root, "baseline", "sympy_s0", "20260101T000000Z__manual4", ",\"resolved\":true,\"num_turns\":1,\"tokens_total\":100,\"input_tokens\":10,\"output_tokens\":20,\"cost_usd_vendor\":0.123456");
      var newest = row(root, "baseline", "sympy_s0", "20260102T000000Z__manual4", ",\"resolved\":true,\"num_turns\":99,\"tokens_total\":100,\"input_tokens\":10,\"output_tokens\":20,\"cost_usd_vendor\":0.0,\"optimizer_tokens\":7");
      var legacy = row(root, "baseline", "scikit-learn_s1", "legacy-standalone", ",\"resolved\":false,\"num_turns\":\"3\",\"tokens_total\":1000,\"input_tokens\":100,\"output_tokens\":20,\"eval_report\":{\"grading_error\":\"lost, \\\"container\\\"\\r\\né😀\"}");
      Files.setLastModifiedTime(legacy, FileTime.fromMillis(1234567890000L));
      row(root, "repomap", "pydata__xarray_s0", "20260103T000000Z__standalone", ",\"resolved\":true,\"num_turns\":2,\"webbench_attempts\":2,\"usage_incomplete\":true,\"usage_incomplete_reason\":\"truncated\",\"cost_usd\":0.000001");
      row(root, "repomap", "pydata__xarray_s1", "20260103T000000Z__unreached", ",\"resolved\":false,\"webbench_attempts\":0,\"resolved_p1\":null,\"resolved_p2\":null");
      row(root, "repomap", "pydata__xarray_s2", "20260103T000000Z__spent", ",\"resolved\":false,\"input_tokens\":3");
      row(root, "baseline", "large_s0", "20260103T000000Z__legacy", ",\"tokens_total\":10000000000000000001,\"optimizer_tokens\":9007199254740993");
      row(root, "baseline", "mixed-prefix_s0", "20260103T000000Z__legacy", ",\"tokens_total\":0.0,\"optimizer_tokens\":9007199254740993,\"untool_classify_tokens\":-9007199254740992,\"untool_judge_tokens\":0.0");
      row(root, "baseline", "mixed-overflow_s0", "20260103T000000Z__legacy", ",\"tokens_total\":0.0,\"optimizer_tokens\":18446744073709551615,\"untool_classify_tokens\":-9223372036854775808,\"untool_judge_tokens\":-9223372036854775808,\"codex_untool_adapt_tokens\":0.0,\"spiral_judge_tokens\":2");
      row(root, "baseline", ".hidden_s0", "20260101T000000Z__ignored", ",\"resolved\":true");
      row(root, ".explicit-hidden-arm", "sympy_s0", "20260101T000000Z__visible", ",\"resolved\":true");
      Files.createDirectories(root.resolve("empty"));
      for (var path : List.of(old, newest, legacy, root.resolve("missing/row.json")))
        compare(nativeCode, source, method("_run_order", "\"rj\":" + q(path)));
      compare(nativeCode, source, method("discover_arms", "\"root\":" + q(root)));
      compare(nativeCode, source, method("collect", "\"root\":" + q(root) + ",\"arms\":[\"repomap\",\"baseline\",\".explicit-hidden-arm\"]"));
      var index = root.resolve("index");
      var completed = compare(nativeCode, source, main(root, index, ""));
      require(completed.contains("99") && completed.contains("vendor_cost_missing:") && completed.contains("not_graded:") && completed.contains("usage_incomplete:"), "fixture did not cover diagnostic and deduplication paths");
      var before = Files.readAllBytes(index.resolve("all_rows.csv"));
      var refused = compare(nativeCode, source, main(root, index, ",\"--arms\",\"repomap\""));
      require(refused.contains("SUBSET") && java.util.Arrays.equals(before, Files.readAllBytes(index.resolve("all_rows.csv"))), "subset changed the canonical index");
      var out = root.resolve("subset output");
      compare(nativeCode, source, main(root, out, ",\"--arms\",\"repomap\",\"--out\"," + q(out)));
      compare(nativeCode, source, main(root, index, ",\"--arms\""));
      compare(nativeCode, source, main(root, out, ",\"--arms\",\"unknown\"," + q("--out=" + out)));
      var empty = Files.createDirectory(root.resolve("empty-tree"));
      compare(nativeCode, source, main(empty, empty.resolve("index"), ""));
      for (var argv : List.of("[]", "[\"--help\"]", "[\"-h\"]", "[\"--out\"]", "[" + q(root) + ",\"--unknown\"]"))
        compare(nativeCode, source, "{\"method\":\"main\",\"argv\":" + argv + "}");
      Files.writeString(Files.createDirectories(root.resolve("baseline/broken_s0/invalid")).resolve("row.json"), "{\"bad\": }");
      compare(nativeCode, source, method("collect", "\"root\":" + q(root) + ",\"arms\":[\"baseline\"]"));
      var nativeRun = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"),
          List.of(checkout.resolve("target/debug/te-bench").toString(), "aggregate", root.toString()));
      require(nativeRun.code() == 0 && nativeRun.out().contains("summary_by_cell.csv:"), "native aggregation required Python: " + nativeRun);
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root); }
    System.out.println("Aggregation passed " + checks + " source comparisons, including exact CSV bytes, duplicate runs, grading and cost flags, and subset-index preservation; the CLI ran with Python unavailable");
  }
}
