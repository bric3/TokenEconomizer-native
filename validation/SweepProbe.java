//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Verifies the complete planner offline, including manifest precedence and CLI bytes. */
class SweepProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var actual = nativeCode.call(request); var expected = source.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected);
    checks++; return actual;
  }
  static String method(String method, String params) { return "{\"method\":" + q(method) + ",\"params\":{" + params + "}}"; }
  static String main(Path root, String argv) { return "{\"method\":\"main\",\"root\":" + q(root) + ",\"argv\":" + argv + "}"; }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "sweep-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "sweep_host.py");
         var source = new AdapterProbe.Client(checkout, false, "sweep_host.py")) {
      compare(nativeCode, source, method("constants", ""));
      for (var domain : List.of("swebench", "webbench", "creddata", "reposvul", "unknown"))
        compare(nativeCode, source, method("group_sizes", "\"domain\":" + q(domain)));
      for (var arms : List.of("[]", "[\"a\"]", "[\"a\",\"b\",\"a\"]"))
        for (var repos : List.of("[]", "[\"r1\",\"r2\",\"r1\"]"))
          for (var seeds : List.of("[]", "[0,1]", "[\"A\",\"B\"]"))
            compare(nativeCode, source, method("cells", "\"domain\":\"unused\",\"arms\":" + arms + ",\"repos\":" + repos + ",\"seeds\":" + seeds));
      for (var domain : List.of("swebench", "webbench"))
        for (var repos : List.of("[]", "[\"django/django\",\"sympy/sympy\"]", "[\"calculator\",\"table\"]", "[\"unknown\",\"unknown\"]", "[\"sympy/sympy\",\"sympy/sympy\",\"unknown\"]"))
          for (var limit : List.of(-3, 0, 1, 40, 1000))
            for (var usd : List.of("0", "1", "1.0", "0.34", "-0.5"))
              compare(nativeCode, source, method("estimate", "\"domain\":" + q(domain) + ",\"arms\":[\"baseline\",\"trimech\"],\"repos\":" + repos + ",\"seeds\":[0,1,2],\"sec_per_task\":120.0,\"usd_per_task\":" + usd + ",\"limit\":" + limit));
      for (var tasks : List.of(0, 999, 1000, 27700, 1234567))
        for (var amount : List.of("0.0", "-0.0", "2.5", "-2.5", "1234567.5"))
          for (boolean show : List.of(false, true))
            compare(nativeCode, source, method("_fmt_estimate", "\"e\":{\"domain\":\"é😀\",\"n_arms\":12,\"n_repos\":6,\"n_seeds\":3,\"n_cells\":216,\"n_task_runs\":" + tasks + ",\"hours_serial\":" + amount + ",\"usd\":" + amount + ",\"limit\":1},\"show_cost\":" + show));
      var manifest = root.resolve("custom manifest.json");
      for (var text : List.of("{}", "{\"groups\":null}", "{\"groups\":{}}", "{\"groups\":[]}",
          "{\"groups\":{\"z\":[\"a\",\"b\"],\"a\":[],\"é😀\":\"é😀\"}}", "{\"groups\":{\"r\":{\"a\":1,\"b\":2}}}",
          "[]", "null", "{\"groups\":true}", "{\"groups\":{\"r\":null}}", "{\"groups\":{\"r\":3}}", "{\"groups\": }", "\ufeff{}")) {
        Files.writeString(manifest, text);
        compare(nativeCode, source, method("group_sizes", "\"domain\":\"swebench\",\"manifest\":" + q(manifest)));
      }
      Files.write(manifest, new byte[]{'{', '"', 'x', '"', ':', (byte)0xff, '}'});
      compare(nativeCode, source, method("group_sizes", "\"domain\":\"swebench\",\"manifest\":" + q(manifest)));
      for (var path : List.of(root, root.resolve("missing.json")))
        compare(nativeCode, source, method("group_sizes", "\"domain\":\"swebench\",\"manifest\":" + q(path)));
      Files.writeString(manifest, "{\r\n\"groups\":{\"custom\":[\"one\",\"two\"]}\r\n}");
      compare(nativeCode, source, "{\"method\":\"group_sizes\",\"cwd\":" + q(root) + ",\"params\":{\"domain\":\"custom\",\"manifest\":\"custom manifest.json\"}}");
      for (var domain : List.of("swebench", "webbench"))
        for (var format : List.of("arms", "cells", "counts", "estimate"))
          for (var extra : List.of("", ",\"--limit\",\"1\",\"--show-cost\"", ",\"--seeds\"", ",\"--arms\"", ",\"--repos\""))
            compare(nativeCode, source, main(root, "[\"--domain\"," + q(domain) + ",\"--format\"," + q(format) + extra + "]"));
      for (var format : List.of("cells", "estimate", "arms"))
        compare(nativeCode, source, main(root, "[\"--domain\",\"webbench\",\"--format\"," + q(format) + ",\"--arms\",\"serena\",\"baseline\",\"repomap\",\"inject-only\",\"trimech-web\",\"--repos\",\"calculator\",\"--seeds\",\"0\",\"1\"]"));
      var defaults = Files.createDirectories(root.resolve("workloads")).resolve("swebench.json");
      Files.writeString(defaults, "{\"groups\":{\"custom\":[1,2,3]}}");
      for (var format : List.of("cells", "counts", "estimate"))
        compare(nativeCode, source, main(root, "[\"--format\"," + q(format) + ",\"--arms\",\"baseline\",\"--seeds\",\"0\"]"));
      compare(nativeCode, source, main(root, "[\"--format\",\"estimate\",\"--repos\",\"unknown\",\"--limit=-1\",\"--show-cost\"]"));
      for (var argv : List.of("[]", "[\"--help\"]", "[\"-h\"]", "[\"--domain\",\"bad\"]", "[\"--format\",\"cell\"]", "[\"--arms\",\"missing\"]", "[\"--limit\",\"bad\"]", "[\"--limit\"]", "[\"--seeds\",\"bad\"]", "[\"--sec-per-task\",\"bad\"]", "[\"--usd-per-task\",\"bad\"]", "[\"--unknown\"]", "[\"--s\"]", "[\"--format=counts\"]", "[\"--seeds\",\"١\",\"-2\",\"3_0\",\"--format\",\"cells\"]"))
        compare(nativeCode, source, main(root, argv));
      var output = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"),
          List.of(checkout.resolve("target/debug/te-bench").toString(), "sweep-plan", "--format", "counts"));
      require(output.code() == 0 && output.out().equals("custom\t3\n"), "native planner ignored its manifest or required Python: " + output);
      compare(nativeCode, source, main(root, "[\"--format\",\"cells\",\"--seeds\",\"+01\",\"-00\",\"00\",\"--limit\",\"+0001\"]"));
      var refused = NativeCliProbe.process(root, Map.of("PATH", ""), List.of(checkout.resolve("target/debug/te-bench").toString(), "sweep-plan", "--arms", "missing"));
      require(refused.code() == 1 && refused.err().contains("unknown arm(s)"), "native planner lost unknown-arm failure: " + refused);
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root); }
    System.out.println("Sweep planning passed " + checks + " source comparisons; manifests, seed-major cells, limits, duplicate repos, cost formatting, scaffold exclusions, CLI output and errors match; the CLI ran without Python");
  }
}
