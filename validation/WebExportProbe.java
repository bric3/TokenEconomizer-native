//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Original/native comparisons with populated project files and standalone CLI checks. */
class WebExportProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String request(String method, Path base, String extra) {
    return "{\"method\":" + q(method) + ",\"base\":" + q(base) + (extra.isEmpty() ? "" : "," + extra) + "}";
  }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var actual = nativeCode.call(request); var expected = source.call(request);
    require(actual.equals(expected), request + "\nNative: " + actual + "\nSource: " + expected); checks++;
  }
  static NativeCliProbe.Output command(Path root, Path executable, String... arguments) throws Exception {
    var command = new java.util.ArrayList<>(List.of(executable.toString(), "export-webbench")); command.addAll(List.of(arguments));
    return NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python",
        "TOKENECONOMIZER_WEBBENCH_PROJECTS", root.resolve("projects").toString(),
        "TOKENECONOMIZER_WEBBENCH_TASKS", root.resolve("native.json").toString()), command);
  }
  static void standalone(Path checkout, Path root, Path executable) throws Exception {
    try (var source = new AdapterProbe.Client(checkout, false, "web_export_host.py")) {
      var reference = source.call(request("main", root, ""));
      require(!reference.contains("\"raised\""), "reference export failed: " + reference);
    }
    var wrote = command(root, executable);
    require(wrote.code() == 0 && wrote.out().contains("wrote ") && wrote.out().contains("102 tasks, 5 projects"), "standalone export failed: " + wrote);
    require(Files.mismatch(root.resolve("out.json"), root.resolve("native.json")) == -1, "standalone export bytes differ");
    var before = Files.getLastModifiedTime(root.resolve("native.json"));
    var checked = command(root, executable, "--check");
    require(checked.code() == 0 && checked.out().startsWith("ok  ") && Files.getLastModifiedTime(root.resolve("native.json")).equals(before), "matching export was rewritten: " + checked);
    Files.writeString(root.resolve("native.json"), "old export\n");
    var stale = command(root, executable, "--check");
    require(stale.code() == 1 && stale.err().contains("differs from the registry") && Files.readString(root.resolve("native.json")).equals("old export\n"), "stale check changed output: " + stale);
    var help = command(root, executable, "--help");
    require(help.code() == 0 && help.out().contains("--projects-root"), "native help required Python: " + help);
    checks += 5;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "web-export-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "web_export_host.py");
         var source = new AdapterProbe.Client(checkout, false, "web_export_host.py")) {
      var base = root.resolve("case");
      compare(nativeCode, source, request("build", base, ""));
      for (var description : List.of("null", "false", "42", "1e-6", "1e20", "{\"é😀\":[true,null,1e-7]}", "\"\u007f\u0085\u2028\"")) {
        compare(nativeCode, source, request("build", base, "\"description\":" + description));
        compare(nativeCode, source, request("main", base, "\"description\":" + description));
      }
      compare(nativeCode, source, request("main", base, "\"drop_description\":true"));
      for (var state : List.of("missing", "good", "crlf", "stale", "directory", "invalid-utf8")) {
        compare(nativeCode, source, request("main", base, "\"state\":" + q(state)));
        compare(nativeCode, source, request("main", base, "\"state\":" + q(state) + ",\"argv\":[\"--projects-root\",\"projects\",\"--out\",\"out.json\",\"--check\"]"));
      }
      for (var project : List.of("calculator", "expressjs", "table")) {
        for (var mutation : List.of("missing", "directory"))
          compare(nativeCode, source, request("main", base, "\"project\":" + q(project) + ",\"mutation\":" + q(mutation)));
      }
      for (var content : List.of("", "{}", "[]", "null", "3", "\"text\"", "{\"id\":[]}", "{\"id\":{}}", "{\"id\":false}", "{\"id\":null}", "{", "{\"id\":\"task-init\"} extra", "\ufeff{}", "\n\r \t\n{}"))
        compare(nativeCode, source, request("build", base, "\"content\":" + q(content)));
      for (var bytes : List.of("[255]", "[195]", "[195,32]")) compare(nativeCode, source, request("build", base, "\"bytes\":" + bytes));
      for (var argv : List.of("[]", "[\"--help\"]", "[\"--h\"]", "[\"--help=x\"]", "[\"--check=x\"]", "[\"--unknown\"]", "[\"--out\"]", "[\"--projects-root\"]", "[\"--\"]", "[\"--out\",\"x\"]", "[\"--projects-root\",\"projects\"]", "[\"--o=x\",\"--p=projects\"]", "[\"--projects-root\",\"projects\",\"--out\",\"absent/out\"]", "[\"--projects-root\",\"absent\",\"--out\",\"out.json\"]"))
        compare(nativeCode, source, request("main", base, "\"argv\":" + argv));
      for (var argv : List.of("[]", "[\"--out=\",\"--projects-root=\"]", "[\"--out=override.json\",\"--projects-root=projects\"]", "[\"--check\"]"))
        compare(nativeCode, source, request("main", base, "\"argv\":" + argv + ",\"env\":{\"TOKENECONOMIZER_WEBBENCH_PROJECTS\":\"@PROJECTS@\",\"TOKENECONOMIZER_WEBBENCH_TASKS\":\"@OUT@\"}"));
      for (var check : List.of("", ",\"--check\""))
        compare(nativeCode, source, request("main", base, "\"argv\":[\"--projects-root\",\"projects\",\"--out\"," + q("bad\0name") + check + "]"));
      standalone(checkout, root.resolve("standalone"), checkout.resolve("target/debug/te-bench"));
      var focused = NativeCliProbe.process(checkout, Map.of("PYTHONPATH", checkout.resolve("validation").toString(),
          "TOKENECONOMIZER_WEBBENCH_PROJECTS", root.resolve("standalone/projects").toString(),
          "TOKENECONOMIZER_WEBBENCH_TASKS", root.resolve("standalone/out.json").toString()),
          List.of(checkout.resolve(".venv/bin/python").toString(), "-m", "pytest", "-o", "addopts=", "-p", "native_plugin", "reference/upstream/experiments/tests/test_workloads.py"), 120);
      require(focused.code() == 0 && focused.out().contains("32 passed"), "configured workload tests failed: " + focused); checks++;
      System.out.println("Verified " + (checks - 6) + " WebBench source comparisons, five standalone checks and 32 configured original tests");
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Preserved WebBench fixtures: " + root); }
  }
}
