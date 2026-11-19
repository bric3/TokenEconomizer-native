//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Compares full SWE-bench exports, iterator boundaries, failure identity and file bytes. */
class DatasetExportProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var actual = nativeCode.call(request); var expected = source.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected);
    checks++; return actual;
  }
  static String request(String method, Path base, String extra) {
    return "{\"method\":" + q(method) + ",\"base\":" + q(base) + (extra.isEmpty() ? "" : "," + extra) + "}";
  }
  static void fakeSdk(Path root) throws Exception {
    var sdk = Files.createDirectories(root.resolve("fake-sdk"));
    Files.writeString(sdk.resolve("datasets.py"), """
        import builtins, json, os
        from pathlib import Path
        original_import = builtins.__import__
        def guarded(name, *args, **kwargs):
            if name.split('.')[0] in ('tokeneconomizer','te_bench','te_harness','native_plugin'):
                raise AssertionError('dataset runtime imported reference/test code: ' + name)
            return original_import(name, *args, **kwargs)
        builtins.__import__ = guarded
        def load_dataset(path, *, split):
            assert split == 'test'
            index = 0 if path == 'princeton-nlp/SWE-bench_Verified' else 1
            assert path in ('princeton-nlp/SWE-bench_Verified','princeton-nlp/SWE-bench')
            return json.loads((Path(os.environ['TE_DATASET_FIXTURE']) / f'split-{index}.json').read_text(encoding='utf-8'))
        """);
  }
  static void fixtures(AdapterProbe.Client source, Path root) throws Exception {
    var response = source.call(request("fixtures", root.resolve("fixtures"), ""));
    require(!response.contains("\"raised\""), "dataset fixture creation failed: " + response);
    fakeSdk(root);
  }
  static NativeCliProbe.Output command(Path checkout, Path root, Path executable, String... arguments) throws Exception {
    var command = new java.util.ArrayList<>(List.of(executable.toString(), "export-swebench")); command.addAll(List.of(arguments));
    return NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", checkout.resolve(".venv/bin/python").toString(),
        "PYTHONPATH", root.resolve("fake-sdk").toString(), "TE_DATASET_FIXTURE", root.resolve("fixtures").toString()), command);
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "dataset-export-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "dataset_export_host.py");
         var source = new AdapterProbe.Client(checkout, false, "dataset_export_host.py")) {
      compare(nativeCode, source, request("constants", root, ""));
      compare(nativeCode, source, request("build", root, ""));
      for (var wanted : List.of("[]", "[\"django__django-7530\"]", "[\"unknown\"]", "[\"scikit-learn__scikit-learn-14710\"]"))
        compare(nativeCode, source, request("build", root, "\"wanted\":" + wanted));
      for (var rows : List.of("[]", "[[]]", "[[{}]]", "[[null]]", "[[1]]", "[[[]]]", "[[\"row\"]]", "[[{\"instance_id\":[]}]]", "[[{\"instance_id\":{}}]]",
          "[[{\"instance_id\":\"unknown\"}]]", "[[{\"instance_id\":\"django__django-7530\"}]]",
          "[[{\"instance_id\":\"django__django-7530\",\"repo\":\"x\"}]]"))
        compare(nativeCode, source, request("build", root, "\"rows\":" + rows));
      for (var field : List.of("repo", "problem_statement", "base_commit", "FAIL_TO_PASS"))
        compare(nativeCode, source, request("build", root, "\"drop_field\":" + q(field)));
      for (var failure : List.of("outer.iter", "outer.next:0", "outer.next:1", "rows.iter:0", "rows.next:0:1", "rows.next:1:1"))
        for (var error : List.of("RuntimeError", "KeyboardInterrupt"))
          compare(nativeCode, source, request("build", root, "\"fail\":" + q(failure) + ",\"error\":" + q(error)));
      compare(nativeCode, source, request("build", root, "\"rows\":[[{},{}]],\"fail\":\"rows.next:0:1\""));
      compare(nativeCode, source, request("main", root.resolve("main-complete"), ""));
      for (boolean existing : List.of(false, true))
        for (var extra : List.of("\"missing\":[\"django__django-7530\"]", "\"drop_field\":\"base_commit\"", "\"fail\":\"load_dataset:1\"", "\"fail\":\"rows.next:0:1\",\"error\":\"KeyboardInterrupt\""))
          compare(nativeCode, source, request("main", root.resolve("main-failed"), "\"existing\":" + existing + "," + extra));
      for (var argv : List.of("[]", "[\"--help\"]", "[\"-h\"]", "[\"--out-dir\"]", "[\"--unknown\"]", "[\"--out-dir\",\"out\",\"--unknown\"]", "[\"--out-dir=out\"]", "[\"--out-dir\",\"\"]", "[\"--out\",\"out\"]"))
        compare(nativeCode, source, request("main", root.resolve("main-args"), "\"argv\":" + argv));
      for (var name : List.of("swebench_tasks.json", "swebench_grading.json", "image_tags.txt"))
        compare(nativeCode, source, request("main", root.resolve("main-io"), "\"existing\":true,\"blocked_file\":" + q(name)));
      compare(nativeCode, source, request("main", root.resolve("main-import"), "\"sdk_missing\":true"));
      fixtures(source, root);
      var output = command(checkout, root, checkout.resolve("target/debug/te-bench"), "--out-dir", "standalone");
      require(output.code() == 0 && output.out().contains("wrote 277 tasks"), "standalone export did not use the SDK fixture: " + output);
      for (var name : List.of("swebench_tasks.json", "swebench_grading.json", "image_tags.txt"))
        require(Files.mismatch(root.resolve("standalone").resolve(name), root.resolve("main-complete/out").resolve(name)) == -1, "standalone export bytes changed: " + name);
      var help = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"), List.of(checkout.resolve("target/debug/te-bench").toString(), "export-swebench", "--help"));
      require(help.code() == 0 && help.out().contains("--out-dir OUT_DIR"), "help required the dataset SDK or Python: " + help);
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root); }
    System.out.println("SWE-bench exports passed " + checks + " source comparisons; full file bytes, Verified precedence, missing-task preservation, iterator stop points, callback exception identity and standalone SDK loading passed offline");
  }
}
