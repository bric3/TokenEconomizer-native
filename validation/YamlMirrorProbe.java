//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Compares real YAML values, file bytes, lazy selection and partial-output failures. */
class YamlMirrorProbe {
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
  static final String TASK = """
      - id: task-1
        date: 2026-11-19
        level: 2
        description: |
          é😀 first
          second
        ignored: drop this
      - id: task-2
        date: 2026-11-19T09:30:15Z
        description: '  \"quotes\" and slash \\  '
      """;
  static String projects(String... names) {
    return "\"projects\":{" + java.util.Arrays.stream(names).map(name -> q(name) + ":" + q(TASK)).collect(java.util.stream.Collectors.joining(",")) + "}";
  }
  static void standalone(Path checkout, Path root, Path executable) throws Exception {
    try (var source = new AdapterProbe.Client(checkout, false, "yaml_mirror_host.py")) {
      var response = source.call(request("main", root, projects("calculator", "table", "unswept")));
      require(!response.contains("\"raised\""), "YAML reference failed: " + response);
    }
    var expected = Files.readAllBytes(root.resolve("projects/calculator/tasks.jsonl"));
    var guard = Files.createDirectory(root.resolve("sdk-guard"));
    Files.writeString(guard.resolve("sitecustomize.py"), """
        import builtins, os
        original_import = builtins.__import__
        def guarded(name, *args, **kwargs):
            if name.split('.')[0] in ('tokeneconomizer', 'te_bench', 'te_harness', 'native_plugin'):
                raise AssertionError('YAML SDK imported reference/test code: ' + name)
            if name == 'yaml' and os.environ.get('TE_YAML_BLOCK_IMPORT') == '1':
                raise ImportError('fixture missing PyYAML')
            return original_import(name, *args, **kwargs)
        builtins.__import__ = guarded
        """);
    Files.delete(root.resolve("projects/calculator/tasks.jsonl"));
    Files.delete(root.resolve("projects/table/tasks.jsonl"));
    var env = Map.of("PATH", "", "PYTHONPATH", guard.toString(), "TE_BENCH_PYTHON", checkout.resolve(".venv/bin/python").toString(),
        "TOKENECONOMIZER_WEBBENCH_PROJECTS", root.resolve("projects").toString());
    var wrote = NativeCliProbe.process(root, env, List.of(executable.toString(), "webbench-from-yaml", "--all"));
    require(wrote.code() == 0 && wrote.out().contains("wrote   calculator  (2 tasks)") && wrote.out().contains("wrote   table  (2 tasks)"), "standalone YAML mirror failed: " + wrote);
    require(java.util.Arrays.equals(expected, Files.readAllBytes(root.resolve("projects/calculator/tasks.jsonl"))), "standalone YAML bytes differ");
    require(!Files.exists(root.resolve("projects/unswept/tasks.jsonl")), "--all wrote an unswept project");
    var before = Files.getLastModifiedTime(root.resolve("projects/table/tasks.jsonl"));
    var checked = NativeCliProbe.process(root, env, List.of(executable.toString(), "webbench-from-yaml", "--check", "--all"));
    require(checked.code() == 0 && checked.out().contains("ok      table") && Files.getLastModifiedTime(root.resolve("projects/table/tasks.jsonl")).equals(before), "matching YAML mirror was changed: " + checked);
    Files.writeString(root.resolve("projects/table/tasks.jsonl"), "stale\n");
    var stale = NativeCliProbe.process(root, env, List.of(executable.toString(), "webbench-from-yaml", "--check", "--all"));
    require(stale.code() == 1 && stale.err().contains("1 project(s) stale: table") && Files.readString(root.resolve("projects/table/tasks.jsonl")).equals("stale\n"), "stale YAML check wrote output: " + stale);
    var help = NativeCliProbe.process(root, Map.of("PATH", "", "TE_BENCH_PYTHON", "/missing-python"), List.of(executable.toString(), "webbench-from-yaml", "--help"));
    require(help.code() == 0 && help.out().contains("--projects-root"), "YAML help required Python: " + help);
    var missingEnv = new java.util.HashMap<>(env); missingEnv.put("TE_YAML_BLOCK_IMPORT", "1");
    var missing = NativeCliProbe.process(root, missingEnv, List.of(executable.toString(), "webbench-from-yaml", "--all"));
    require(missing.code() == 1 && missing.err().contains("PyYAML is required: pip install pyyaml") && Files.readString(root.resolve("projects/table/tasks.jsonl")).equals("stale\n"), "missing PyYAML was not diagnosed before writing: " + missing);
    checks += 7;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "yaml-mirror-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "yaml_mirror_host.py");
         var source = new AdapterProbe.Client(checkout, false, "yaml_mirror_host.py")) {
      var base = root.resolve("case");
      for (var text : List.of("", "# empty\n", "null", "false", "0", "''", "[]", "{}", "true", "3", "abc", "{id: task-1}", "- null", "- []", "- 3", "- text", "- {}", TASK,
          "- id: null\n  date: null\n  level: null\n  description: null\n", "- id: [1, false]\n  level: {a: 1.0e-6}\n  date: [yes, null]\n  description: '  é😀  '\n",
          "- id: !!set {a: null}\n  description: 4\n", "- id: !!set {a: null}\n", "- id: !!binary YWJj\n", "- description: !!binary YWJj\n", "- date: [2026-11-19, 2]\n", "- date: !!set {a: null}\n",
          "- level: .inf\n", "- level: .nan\n", "- id: 123456789012345678901234567890\n", "- &base {id: task-1, description: ' x '}\n- <<: *base\n  id: task-2\n", "- id: &cycle [*cycle]\n", "- id: 1\n  description: false\n", "[", "---\n[]\n---\n[]", "- id: !unknown value\n", "- date: 2026-99-99\n"))
        compare(nativeCode, source, request("render", base, "\"projects\":{\"calculator\":" + q(text) + "}"));
      compare(nativeCode, source, request("render", base, projects("calculator") + ",\"path_object\":true"));
      for (var path : List.of("missing", "projects/calculator", "")) compare(nativeCode, source, request("render", base, "\"path\":" + q(path)));
      for (var failure : List.of("RuntimeError", "KeyboardInterrupt")) compare(nativeCode, source, request("render", base, "\"fail\":" + q(failure)));
      for (var explicit : List.of("null", "\"\"", "false", "0", "[]", "\"explicit\"", "[1]", "true"))
        for (var env : List.of("", ",\"env\":\"@ROOT@\"", ",\"env\":\"\"")) compare(nativeCode, source, request("root", base, "\"explicit\":" + explicit + env));
      compare(nativeCode, source, request("root", base, "\"explicit\":\"explicit\",\"path_object\":true"));
      for (var names : List.of(new String[]{}, new String[]{"unswept"}, new String[]{"calculator", "zustand", "table", "unswept"}))
        compare(nativeCode, source, request("projects", base, projects(names)));
      compare(nativeCode, source, request("lazy", base, projects("calculator", "expressjs", "table")));
      for (var state : List.of("missing", "good", "crlf", "stale", "directory", "invalid-utf8"))
        for (var check : List.of("", ",\"--check\"")) compare(nativeCode, source, request("main", base, projects("calculator", "table", "unswept") + ",\"state\":{\"calculator\":" + q(state) + "},\"argv\":[\"--projects-root\",\"projects\",\"--all\"" + check + "]"));
      for (var argv : List.of("[]", "[\"--help\"]", "[\"--help=x\"]", "[\"--all=x\"]", "[\"--projects-root\"]", "[\"--unknown\"]", "[\"--projects-root\",\"projects\"]", "[\"--all\"]", "[\"--projects-root=projects\",\"--all\",\"unknown\"]", "[\"--projects-root=projects\",\"calculator\",\"missing\"]", "[\"calculator\",\"--check\"]", "[\"calculator\",\"--check\",\"table\"]", "[\"--projects-root=projects\",\"calculator\",\"calculator\"]", "[\"--projects-root=\",\"--all\"]"))
        compare(nativeCode, source, request("main", base, projects("calculator", "table") + ",\"argv\":" + argv + ",\"env\":\"@ROOT@\""));
      compare(nativeCode, source, request("main", base, "\"projects\":{},\"argv\":[\"--all\"],\"env\":\"@ROOT@\""));
      compare(nativeCode, source, request("main", base, "\"projects\":{\"calculator\":" + q(TASK) + ",\"table\":\"[\"}"));
      compare(nativeCode, source, request("main", base, "\"fail\":\"KeyboardInterrupt\""));
      for (var env : List.of("", ",\"env\":\"\""))
        compare(nativeCode, source, request("main", base, "\"argv\":[]" + env));
      compare(nativeCode, source, request("render", base, "\"projects\":{\"calculator\":" + q("- description: \"\\u001cé😀\\u001f\"\n") + "}"));
      standalone(checkout, root.resolve("standalone"), checkout.resolve("target/debug/te-bench"));
      System.out.println("Verified " + (checks - 7) + " YAML mirror source comparisons and seven standalone checks");
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Preserved YAML fixtures: " + root); }
  }
}
