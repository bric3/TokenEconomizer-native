//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Verifies published/runnable workloads, source constants and actual manifest bytes. */
class WorkloadProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var actual = nativeCode.call(request); var expected = source.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected);
    checks++; return actual;
  }
  static String method(String name, String params) { return "{\"method\":" + q(name) + ",\"params\":{" + params + "}}"; }
  static String main(Path base, Path out, String argv) {
    return "{\"method\":\"main\",\"base\":" + q(base) + ",\"out\":" + q(out) + ",\"argv\":" + argv + ",\"files\":true}";
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var dump = NativeCliProbe.process(checkout, Map.of("PATH", ""), List.of(checkout.resolve(".venv/bin/python").toString(), "validation/workload_host.py", "--dump"));
    require(dump.code() == 0 && dump.err().isEmpty() && dump.out().startsWith("{\n"), "workload data export failed: " + dump);
    var asset = checkout.resolve("src/experiments/assets/workloads.json");
    if (List.of(args).equals(List.of("--write"))) {
      Files.writeString(asset, dump.out());
      System.out.println("Exported original workload constants and manifest descriptions"); return;
    }
    require(Files.readString(asset).equals(dump.out()), "native workload data differs from the pinned source");
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "workloads-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "workload_host.py");
         var source = new AdapterProbe.Client(checkout, false, "workload_host.py")) {
      compare(nativeCode, source, method("constants", ""));
      for (var domain : List.of("swebench", "webbench", "creddata", "reposvul", "unknown", "é😀")) {
        for (var method : List.of("instances", "runnable_instances", "task_counts", "total_tasks", "build"))
          compare(nativeCode, source, method(method, "\"domain\":" + q(domain)));
        for (var group : List.of("django/django", "scikit-learn/scikit-learn", "sympy/sympy", "pydata/xarray", "calculator", "table", "react", "zustand", "expressjs", "creddata", "reposvul", "missing", "é😀")) {
          for (var method : List.of("instances", "runnable_instances"))
            compare(nativeCode, source, method(method, "\"domain\":" + q(domain) + ",\"group\":" + q(group)));
          var tasks = "[{\"instance_id\":\"unknown\"},{\"instance_id\":\"django__django-7530\",\"keep\":1},{\"instance_id\":\"scikit-learn__scikit-learn-14710\"},{\"instance_id\":\"scikit-learn__scikit-learn-3840\"},{\"instance_id\":\"task-1\"},{\"instance_id\":\"init\"},{\"instance_id\":\"task-1\"},{},{\"instance_id\":null}]";
          compare(nativeCode, source, method("filter_tasks", "\"domain\":" + q(domain) + ",\"group\":" + q(group) + ",\"tasks\":" + tasks));
        }
      }
      for (var instance : List.of("", "django__django-11532", "a__b__c", "é😀__repo-1"))
        compare(nativeCode, source, method("swebench_image_tag", "\"instance_id\":" + q(instance)));
      for (var tasks : List.of("[null]", "[1]", "[\"a\"]", "[{\"instance_id\":[]}]", "[{\"instance_id\":{}}]"))
        compare(nativeCode, source, method("filter_tasks", "\"domain\":\"swebench\",\"tasks\":" + tasks));
      for (var domain : List.of("swebench", "unknown", "../relative"))
        compare(nativeCode, source, "{\"method\":\"path_for\",\"out\":" + q(root.resolve("custom out")) + ",\"params\":{\"domain\":" + q(domain) + "}}");
      var out = root.resolve("workloads");
      compare(nativeCode, source, main(root, out, "[\"--check\"]"));
      compare(nativeCode, source, main(root, out, "[]"));
      compare(nativeCode, source, main(root, out, "[\"--check\"]"));
      var stale = out.resolve("swebench.json"); var bytes = Files.readAllBytes(stale);
      Files.writeString(stale, "stale\n");
      compare(nativeCode, source, main(root, out, "[\"--check\"]"));
      require(Files.readString(stale).equals("stale\n"), "check mode rewrote a stale manifest");
      Files.write(stale, bytes);
      var windowsLines = out.resolve("webbench.json");
      Files.writeString(windowsLines, Files.readString(windowsLines).replace("\n", "\r\n"));
      compare(nativeCode, source, main(root, out, "[\"--check\"]"));
      var alternate = root.resolve("elsewhere/custom output");
      compare(nativeCode, source, main(root, alternate, "[]"));
      for (var argv : List.of("[\"--help\"]", "[\"-h\"]", "[\"--unknown\"]", "[\"--check=1\"]", "[\"unexpected\"]"))
        compare(nativeCode, source, main(root, out, argv));
      var nativeRun = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"),
          List.of(checkout.resolve("target/debug/te-bench").toString(), "export-workloads", "--check"));
      require(nativeRun.code() == 0 && nativeRun.out().contains("up to date (4 domains)"), "native export required Python: " + nativeRun);
      var empty = Files.createDirectory(root.resolve("empty-cwd"));
      var missing = NativeCliProbe.process(empty, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"),
          List.of(checkout.resolve("target/debug/te-bench").toString(), "export-workloads", "--check"));
      require(missing.code() == 1 && missing.out().contains("STALE") && !Files.exists(empty.resolve("workloads/swebench.json")), "check mode lost its failure status or wrote a manifest: " + missing);
      var invalid = NativeCliProbe.process(empty, Map.of("PATH", ""),
          List.of(checkout.resolve("target/debug/te-bench").toString(), "export-workloads", "--unknown"));
      require(invalid.code() == 2 && invalid.err().contains("unrecognized arguments"), "invalid arguments lost the parser exit status: " + invalid);
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root); }
    System.out.println("Workloads passed " + checks + " source comparisons; all constants, published/runnable groups, ordering, identity, manifest bytes and check-mode preservation match; the CLI ran without Python");
  }
}
