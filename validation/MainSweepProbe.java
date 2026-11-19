//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java LaunchProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.TimeUnit;

/** Source/native main-sweep comparisons, real offline driver cells, resume and detachment. */
class MainSweepProbe {
  static Path checkout, root, script, executable;
  static int checks;
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String clean(String text) {
    return text.replaceAll("[0-9]{8}T[0-9]{6}Z", "TIMESTAMP")
        .replace(root.resolve("scripts/sweep_plan.py").toString(), "te-bench sweep-plan");
  }
  static Map<String,String> environment() {
    return new HashMap<>(Map.of("HOME", root.toString(), "PATH", "/usr/bin:/bin", "PYTHON", checkout.resolve(".venv/bin/python").toString(),
        "TE_BENCH_PYTHON", checkout.resolve(".venv/bin/python").toString(), "PYTHONPATH", checkout.resolve("reference/upstream") + ":" + checkout.resolve("reference/upstream/experiments"),
        "TOKENECONOMIZER_ENV_FILE", root.resolve("absent.env").toString(), "TOKENECONOMIZER_RESULTS", root.resolve("results").toString(), "DETACH", "0", "DRYRUN", "1"));
  }
  static NativeCliProbe.Output run(boolean nativeCode, Map<String,String> env) throws Exception {
    var actualEnv = new HashMap<>(env);
    if (nativeCode) { actualEnv.remove("PYTHONPATH"); actualEnv.put("PATH", ""); }
    var command = nativeCode ? List.of(executable.toString(), "main-sweep") : List.of(LaunchProbe.tool("bash").toString(), script.toString());
    return NativeCliProbe.process(root, actualEnv, command, 120);
  }
  static NativeCliProbe.Output compared(Map<String,String> env) throws Exception {
    var expected = run(false, env); var actual = run(true, env);
    require(actual.code() == expected.code() && clean(actual.out()).equals(clean(expected.out())) && clean(actual.err()).equals(clean(expected.err())),
        "Main sweep differs for " + env + "\nNative: " + actual + "\nSource: " + expected);
    checks++; return actual;
  }
  static void inputs(boolean missing) throws Exception {
    Files.writeString(root.resolve("tasks.json"), "[{\"instance_id\":\"a_b__0\",\"repo\":\"org/a-b\",\"problem_statement\":\"Offline fixture\"},{\"instance_id\":\"a_b__1\",\"repo\":\"org/a-b\",\"problem_statement\":\"Second fixture\"},{\"instance_id\":\"b__0\",\"repo\":\"org/b\",\"problem_statement\":\"Other fixture\"}]");
    var workload = missing ? "{\"groups\":{\"org/a-b\":[\"a_b__0\"],\"org/missing\":[\"absent__0\"]}}" : "{\"groups\":{\"org/a-b\":[\"a_b__0\",\"a_b__1\"],\"org/b\":[\"b__0\"]}}";
    for (var domain : List.of("swebench", "webbench")) Files.writeString(root.resolve("workloads/" + domain + ".json"), workload);
  }
  static long rows(Path results) throws Exception {
    if (!Files.exists(results)) return 0;
    try (var paths = Files.walk(results)) { return paths.filter(path -> path.getFileName().toString().equals("row.json")).count(); }
  }
  static Map<String,String> artifacts(Path results) throws Exception {
    var artifacts = new TreeMap<String,String>();
    try (var files = Files.walk(results)) {
      for (var path : files.filter(Files::isRegularFile).filter(path -> path.toString().endsWith(".csv") || path.getFileName().toString().equals("transcript.jsonl")).toList())
        artifacts.put(clean(results.relativize(path).toString()), clean(Files.readString(path)));
    }
    return artifacts;
  }
  static void extraArm(Path results) throws Exception {
    Files.createDirectories(results.resolve("stored-arm/pkg_s7/run"));
    Files.writeString(results.resolve("stored-arm/pkg_s7/run/row.json"), "{\"instance_id\":\"stored__0\",\"repo\":\"org/stored\",\"agent\":\"claude\",\"model\":\"claude-opus-4-8\",\"tokens_total\":100,\"input_tokens\":100,\"resolved\":false}");
  }
  static void executed(boolean fail, String onFail, String agent, String domain) throws Exception {
    inputs(fail);
    var env = environment(); env.putAll(Map.of("DRYRUN", "0", "ARMS", "baseline baseline-medium", "SEEDS", "0", "LIMIT", "1", "TASKS_FILE", root.resolve("tasks.json").toString(), "ON_FAIL", onFail, "AGENT", agent, "DOMAIN", domain));
    if (fail) env.put("REPOS", "org/missing org/a-b");
    var results = root.resolve("results"); var tree = results.resolve(agent + "_" + domain + "_sonnet5");
    List<NativeCliProbe.Output> expected = null; Map<String,String> expectedArtifacts = null;
    for (boolean nativeCode : List.of(false, true)) {
      if (Files.exists(results)) ContainerProbe.clear(results);
      extraArm(tree);
      var first = run(nativeCode, env); var outputs = new ArrayList<NativeCliProbe.Output>(); outputs.add(first);
      long expectedRows = fail ? onFail.equals("abort") ? 1 : 3 : 5;
      require(rows(tree) == expectedRows && first.code() == (fail ? 1 : 0), "Wrong executed result: " + first + ", rows=" + rows(tree)); checks++;
      if (!fail) {
        var resumed = run(nativeCode, env); outputs.add(resumed);
        require(resumed.code() == 0 && resumed.out().contains("ran=0 skipped=4 failed=0") && rows(tree) == 5, "Resume reran a finished cell: " + resumed); checks++;
        require(Files.readString(tree.resolve("index/all_rows.csv")).contains("stored-arm,"), "Aggregation omitted an arm already in the tree"); checks++;
        try (var files = Files.walk(tree.resolve("baseline/a_b_s0"))) { Files.delete(files.filter(path -> path.getFileName().toString().equals("row.json")).findFirst().orElseThrow()); }
        var partial = run(nativeCode, env); outputs.add(partial);
        require(partial.code() == 0 && partial.out().contains("ran=1 skipped=3 failed=0") && rows(tree) == 5, "Partial resume did not complete only the unfinished cell: " + partial); checks++;
      } else require(first.out().contains(onFail.equals("abort") ? "ran=0 skipped=0 failed=1" : "ran=2 skipped=0 failed=2"), "Failure policy changed: " + first);
      var actualArtifacts = artifacts(tree);
      if (expected == null) { expected = outputs; expectedArtifacts = actualArtifacts; }
      else {
        for (int i = 0; i < outputs.size(); i++) {
          var actual = outputs.get(i); var source = expected.get(i);
          require(clean(actual.out()).equals(clean(source.out())) && clean(actual.err()).equals(clean(source.err())), "Executed sweep differs\nNative: " + actual + "\nSource: " + source); checks++;
        }
        require(actualArtifacts.equals(expectedArtifacts), "CSV or transcript artifacts differ\nNative: " + actualArtifacts + "\nSource: " + expectedArtifacts); checks++;
      }
    }
    ContainerProbe.clear(results);
  }
  static void detached() throws Exception {
    inputs(false);
    var env = environment(); env.putAll(Map.of("DRYRUN", "0", "DETACH", "1", "ARMS", "baseline", "REPOS", "org/b", "SEEDS", "0", "TASKS_FILE", root.resolve("tasks.json").toString(), "TE_LOG", root.resolve("sweep.log").toString()));
    var output = run(true, env);
    require(output.code() == 0 && output.out().contains("started in the background") && !output.out().contains("DONE in"), "Native sweep did not detach: " + output); checks++;
    var deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30); String log = "";
    while (System.nanoTime() < deadline) {
      log = Files.readString(root.resolve("sweep.log")); if (log.contains("DONE in")) break; Thread.sleep(25);
    }
    require(log.contains("ran=1 skipped=0 failed=0") && rows(root.resolve("results")) == 1, "Detached native sweep did not finish: " + log); checks++;
    require(Files.readString(root.resolve("sweep.pid")).matches("[0-9]+\n"), "Missing detached sweep PID file"); checks++;
    ContainerProbe.clear(root.resolve("results"));
  }
  static void literalLimits() throws Exception {
    inputs(false);
    for (var fields : List.of(Map.of("LIMIT", "00"), Map.of("LIMIT", "0", "SKIP_DONE", "0"), Map.of("LIMIT", "020"))) {
      var env = environment(); env.putAll(Map.of("DRYRUN", "0", "ARMS", "baseline", "REPOS", "org/a-b", "SEEDS", "0", "TASKS_FILE", root.resolve("tasks.json").toString())); env.putAll(fields);
      var results = root.resolve("results"); var tree = results.resolve("claude_swebench_sonnet5");
      boolean octal = fields.get("LIMIT").equals("020"); int stored = octal ? 16 : 2;
      var ids = new ArrayList<String>(); for (int i = 0; i < (octal ? 20 : 2); i++) ids.add("a_b__" + i);
      Files.writeString(root.resolve("workloads/swebench.json"), "{\"groups\":{\"org/a-b\":" + ParserProbe.array(ids) + "}}");
      NativeCliProbe.Output expected = null;
      for (boolean nativeCode : List.of(false, true)) {
        if (Files.exists(results)) ContainerProbe.clear(results);
        for (int i = 0; i < stored; i++) {
          var run = Files.createDirectories(tree.resolve("baseline/a_b_s0/old-" + i));
          Files.writeString(run.resolve("row.json"), "{\"instance\":\"a_b__" + i + "\",\"repo\":\"org/a-b\",\"agent\":\"claude\",\"tokens_total\":100,\"input_tokens\":100}");
        }
        var output = run(nativeCode, env);
        require(output.code() == 0 && output.out().contains(octal ? "ran=0 skipped=1 failed=0" : "ran=1 skipped=0 failed=0"), "Literal limit changed resume: " + fields + ": " + output); checks++;
        if (expected == null) expected = output;
        else { require(clean(output.out()).equals(clean(expected.out())) && clean(output.err()).equals(clean(expected.err())), "Literal-limit output differs\nNative: " + output + "\nSource: " + expected); checks++; }
      }
      ContainerProbe.clear(results);
    }
  }
  static void standalone(Path project, Path work, Path bench) throws Exception {
    Files.createDirectories(work.resolve("workloads"));
    Files.writeString(work.resolve("workloads/swebench.json"), "{\"groups\":{\"org/pkg\":[\"pkg__0\"]}}");
    var env = new HashMap<>(Map.of("PATH", "", "HOME", work.toString(), "TE_BENCH_PYTHON", "/missing-python", "DRYRUN", "1", "ARMS", "baseline", "SEEDS", "0", "TOKENECONOMIZER_ENV_FILE", work.resolve("absent.env").toString()));
    var output = NativeCliProbe.process(work, env, List.of(bench.toString(), "main-sweep"));
    require(output.code() == 0 && output.out().contains("  baseline\torg/pkg\t0\n") && !Files.exists(work.resolve("results")), "Standalone native dry run needed Python or wrote results: " + output); checks++;
    Files.writeString(work.resolve("tasks.json"), "[{\"instance_id\":\"pkg__0\",\"problem_statement\":\"Offline fixture\",\"repo\":\"org/pkg\"}]");
    env.putAll(Map.of("DRYRUN", "0", "DETACH", "0", "TASKS_FILE", work.resolve("tasks.json").toString(), "TE_BENCH_PYTHON", "", "PYTHON", project.resolve(".venv/bin/python").toString()));
    var first = NativeCliProbe.process(work, env, List.of(bench.toString(), "main-sweep"));
    require(first.code() == 0 && first.out().contains("ran=1 skipped=0 failed=0") && rows(work.resolve("results")) == 1, "Standalone native sweep did not run its offline SDK cell: " + first); checks++;
    var overrideResults = work.resolve("override-results");
    env.putAll(Map.of("TE_BENCH_PYTHON", project.resolve(".venv/bin/python").toString(), "PYTHON", "/missing-python", "TOKENECONOMIZER_RESULTS", overrideResults.toString()));
    var overridden = NativeCliProbe.process(work, env, List.of(bench.toString(), "main-sweep"));
    require(overridden.code() == 0 && overridden.out().contains("ran=1 skipped=0 failed=0") && rows(overrideResults) == 1, "Standalone native sweep ignored TE_BENCH_PYTHON: " + overridden); checks++;
    var resumed = NativeCliProbe.process(work, env, List.of(bench.toString(), "main-sweep"));
    require(resumed.code() == 0 && resumed.out().contains("ran=0 skipped=1 failed=0") && rows(overrideResults) == 1, "Standalone native sweep did not resume: " + resumed); checks++;
  }
  public static void main(String[] args) throws Exception {
    checkout = Path.of("").toAbsolutePath(); root = Files.createTempDirectory(checkout.resolve("validation/results"), "main-sweep-");
    executable = checkout.resolve("target/debug/te-bench"); script = root.resolve("scripts/launch/main_sweep.sh"); boolean passed = false;
    try {
      Files.createDirectories(script.getParent()); Files.createDirectory(root.resolve("workloads"));
      for (var name : List.of("launch/main_sweep.sh", "launch/_env.sh", "sweep_plan.py", "run_experiment.py", "aggregate.py", "_env.py")) Files.copy(checkout.resolve("reference/upstream/experiments/scripts/" + name), root.resolve("scripts/" + name));
      inputs(false); compared(environment());
      for (var domain : List.of("swebench", "webbench")) for (var agent : List.of("claude", "copilot", "codex")) {
        var env = environment(); env.putAll(Map.of("DOMAIN", domain, "AGENT", agent, "MODEL", "claude-opus-4-8", "ARMS", "serena baseline repomap", "SEEDS", "1 0", "REPOS", "org/b org/a-b", "LIMIT", "1")); compared(env);
      }
      for (var fields : List.of(Map.of("DOMAIN", "unknown"), Map.of("DRYRUN", "0"), Map.of("MODEL", "auto"), Map.of("MODEL", "agent"), Map.of("AGENT", "unknown"), Map.of("WORKLOAD", "absent.json"), Map.of("ARMS", "invalid"), Map.of("DOMAIN", "webbench", "ARMS", "serena repomap"), Map.of("RUNNER", "fixture:runner"), Map.of("AGENT", "codex", "OPENAI_BASE_URL", "https://fixture.invalid"), Map.of("AGENT", "codex", "RUNNER", "fixture:runner", "OPENAI_API_KEY", "fake-1234"), Map.of("WORKLOAD", "none", "REPOS", "other/pkg"))) {
        var env = environment(); env.putAll(fields); compared(env);
      }
      var config = root.resolve("fixture.env"); Files.writeString(config, "AGENT=codex\nMODEL=gpt-5.6-sol\nARMS=baseline\nSEEDS=0\nRUNNER=stub\n");
      var env = environment(); env.put("TOKENECONOMIZER_ENV_FILE", config.toString()); compared(env);
      env.put("MODEL", "claude-opus-4-8"); compared(env);
      executed(false, "continue", "claude", "swebench");
      executed(false, "continue", "codex", "swebench");
      executed(false, "continue", "copilot", "webbench");
      executed(true, "continue", "claude", "swebench"); executed(true, "abort", "claude", "swebench"); literalLimits(); detached();
      standalone(checkout, root.resolve("standalone"), executable);
      passed = true;
      System.out.println("Verified " + checks + " main-sweep source comparisons and offline execution, resume, whole-tree aggregation, failure, detachment and standalone checks");
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Main-sweep fixtures: " + root); }
  }
}
