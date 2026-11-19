//JAVA 25
//SOURCES MainSweepProbe.java NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java LaunchProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/** Original/native Codex launch policy and actual offline cell execution. */
class CodexSweepProbe {
  static Path checkout, root, script, executable, python, cgroup;
  static int checks;
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); checks++; }
  static void python(String[] args) throws Exception {
    var arguments = List.of(args).subList(1, args.length);
    int first = arguments.indexOf("--arm");
    if (first >= 0) {
      var data = new ArrayList<>(arguments.subList(first, arguments.size()));
      for (var key : List.of("TE_SWEBENCH_MAX_CPUS", "TE_FLEET_CPUS", "AGENT", "MODEL", "RUNNER")) data.add(key + "=" + System.getenv(key));
      Files.writeString(Path.of(System.getenv("PROBE_CALLS")), ParserProbe.array(data) + "\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
    }
    var command = new ArrayList<>(List.of(System.getenv("ACTUAL_PYTHON"))); command.addAll(arguments);
    System.exit(new ProcessBuilder(command).inheritIO().start().waitFor());
  }
  static String clean(String text) {
    return MainSweepProbe.clean(text).replace("    " + python + " scripts/compare_arms.py", "    te-bench compare-arms");
  }
  static Map<String,String> environment() {
    var env = MainSweepProbe.environment();
    env.putAll(Map.of("PYTHON", python.toString(), "TE_BENCH_PYTHON", python.toString(), "ACTUAL_PYTHON", checkout.resolve(".venv/bin/python").toString(),
        "PROBE_CALLS", root.resolve("calls.jsonl").toString(), "TASKS_FILE", root.resolve("tasks.json").toString(), "TE_CGROUP_ROOT", root.resolve("cgroups").toString()));
    return env;
  }
  static NativeCliProbe.Output run(boolean nativeCode, Map<String,String> env) throws Exception {
    var actual = new HashMap<>(env);
    if (nativeCode) { actual.remove("PYTHONPATH"); actual.put("PATH", ""); }
    return NativeCliProbe.process(root, actual, nativeCode ? List.of(executable.toString(), "codex-ab") : List.of(LaunchProbe.tool("bash").toString(), script.toString()), 120);
  }
  static void compare(Map<String,String> env) throws Exception {
    var expected = run(false, env); var actual = run(true, env);
    require(expected.code() == actual.code() && clean(expected.out()).equals(clean(actual.out())) && clean(expected.err()).equals(clean(actual.err())), "Codex sweep differs for " + env + "\nNative: " + actual + "\nSource: " + expected);
    require(!Files.exists(root.resolve("results")) && Files.readString(cgroup.resolve("cpu.max")).equals("max 100000\n"), "Dry run changed results or CPU budget");
  }
  static void executed(String domain, String failure, boolean cpuOverride) throws Exception {
    boolean fail = !failure.isEmpty(); MainSweepProbe.inputs(fail);
    var env = environment(); env.putAll(Map.of("DRYRUN", "0", "DOMAIN", domain, "ARMS", "baseline codex-inject-envfacts", "SEEDS", fail ? "0 1" : "0", "LIMIT", "1", "AGENT", "claude"));
    if (fail) env.putAll(Map.of("ON_FAIL", failure, "REPOS", "org/missing org/a-b"));
    if (cpuOverride) env.put("TE_SWEBENCH_MAX_CPUS", "2");
    var results = root.resolve("results"); var tree = results.resolve("codex_" + domain + "_gpt56sol"); var calls = root.resolve("calls.jsonl");
    List<NativeCliProbe.Output> expected = null; Map<String,String> expectedArtifacts = null; String expectedCalls = null;
    for (boolean nativeCode : List.of(false, true)) {
      if (Files.exists(results)) ContainerProbe.clear(results);
      MainSweepProbe.extraArm(tree); Files.deleteIfExists(calls); Files.writeString(cgroup.resolve("cpu.max"), "max 100000\n");
      var first = run(nativeCode, env); var outputs = new ArrayList<NativeCliProbe.Output>(); outputs.add(first);
      boolean abort = failure.equals("abort");
      require(first.code() == (fail ? 1 : 0) && MainSweepProbe.rows(tree) == (abort ? 1 : 5), "Wrong Codex execution result: " + first);
      require(Files.readString(cgroup.resolve("cpu.max")).equals("1600000 100000\n"), "Codex did not set the shared fleet budget");
      if (!fail) {
        var resumed = run(nativeCode, env); outputs.add(resumed);
        require(resumed.code() == 0 && resumed.out().contains("ran=0 skipped=4 failed=0") && MainSweepProbe.rows(tree) == 5, "Completed Codex cells ran again: " + resumed);
        try (var files = Files.walk(tree.resolve("baseline/a_b_s0"))) { Files.delete(files.filter(path -> path.getFileName().toString().equals("row.json")).findFirst().orElseThrow()); }
        var partial = run(nativeCode, env); outputs.add(partial);
        require(partial.code() == 0 && partial.out().contains("ran=1 skipped=3 failed=0") && MainSweepProbe.rows(tree) == 5, "Partial Codex resume changed: " + partial);
        require(Files.readString(tree.resolve("index/all_rows.csv")).contains("stored-arm,"), "Codex aggregation omitted a stored arm");
      } else require(first.out().contains(abort ? "ran=0 skipped=0 failed=1" : "ran=4 skipped=0 failed=4"), "Codex failed-cell policy changed: " + first);
      var recorded = Files.readString(calls);
      require(recorded.contains("TE_SWEBENCH_MAX_CPUS=" + (cpuOverride ? "2" : "4")) && recorded.contains("AGENT=codex"), "Codex child environment was not exported: " + recorded);
      require(recorded.contains("\"--optimizer-model\",\"agent\",\"--max-turns\",\"80\",\"--fresh\""), "Codex child flags changed: " + recorded);
      var actualArtifacts = MainSweepProbe.artifacts(tree);
      if (expected == null) { expected = outputs; expectedArtifacts = actualArtifacts; expectedCalls = recorded; }
      else {
        for (int i = 0; i < outputs.size(); i++) {
          var actual = outputs.get(i); var source = expected.get(i);
          require(clean(actual.out()).equals(clean(source.out())) && clean(actual.err()).equals(clean(source.err())), "Executed Codex sweep differs\nNative: " + actual + "\nSource: " + source);
        }
        require(recorded.equals(expectedCalls), "Codex driver arguments/exports differ\nNative: " + recorded + "\nSource: " + expectedCalls);
        require(actualArtifacts.equals(expectedArtifacts), "Codex CSV or transcript artifacts differ\nNative: " + actualArtifacts + "\nSource: " + expectedArtifacts);
      }
    }
    ContainerProbe.clear(results); Files.writeString(cgroup.resolve("cpu.max"), "max 100000\n");
  }
  static void detached() throws Exception {
    MainSweepProbe.inputs(false);
    var env = environment(); env.putAll(Map.of("DRYRUN", "0", "DETACH", "1", "ARMS", "baseline", "REPOS", "org/b", "SEEDS", "0", "TE_LOG", root.resolve("detached.log").toString()));
    var output = run(true, env);
    require(output.code() == 0 && output.out().contains("codex_ab__gpt56sol_swebench_arms started in the background") && !output.out().contains("DONE in"), "Codex did not detach: " + output);
    var deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30); String log = "";
    while (System.nanoTime() < deadline) { log = Files.readString(root.resolve("detached.log")); if (log.contains("DONE in")) break; Thread.sleep(25); }
    require(log.contains("ran=1 skipped=0 failed=0") && log.contains("fleet 16") && MainSweepProbe.rows(root.resolve("results")) == 1, "Detached Codex sweep did not finish: " + log);
    require(Files.readString(root.resolve("detached.pid")).matches("[0-9]+\n"), "Detached Codex PID file missing");
    ContainerProbe.clear(root.resolve("results"));
  }
  static void standalone(Path project, Path work, Path bench) throws Exception {
    Files.createDirectories(work.resolve("workloads"));
    Files.writeString(work.resolve("workloads/swebench.json"), "{\"groups\":{\"org/pkg\":[\"pkg__0\"]}}");
    Files.writeString(work.resolve("tasks.json"), "[{\"instance_id\":\"pkg__0\",\"repo\":\"org/pkg\",\"problem_statement\":\"Offline fixture\"}]");
    var env = new HashMap<>(Map.of("PATH", "", "HOME", work.toString(), "TE_BENCH_PYTHON", "/missing-python", "DRYRUN", "1", "SEEDS", "0", "TASKS_FILE", work.resolve("tasks.json").toString(), "TOKENECONOMIZER_ENV_FILE", work.resolve("absent.env").toString()));
    var planned = NativeCliProbe.process(work, env, List.of(bench.toString(), "codex-ab"));
    require(planned.code() == 0 && planned.out().contains("  baseline\torg/pkg\t0\n  codex-untool\torg/pkg\t0\n") && !Files.exists(work.resolve("results")), "Standalone Codex dry run needed Python or wrote results: " + planned);
    env.putAll(Map.of("DRYRUN", "0", "DETACH", "0", "ARMS", "baseline", "TE_FLEET_CPUS", "0", "TE_CGROUP_ROOT", work.resolve("absent-cgroup").toString(), "TE_BENCH_PYTHON", project.resolve(".venv/bin/python").toString()));
    var first = NativeCliProbe.process(work, env, List.of(bench.toString(), "codex-ab"));
    require(first.code() == 0 && first.out().contains("ran=1 skipped=0 failed=0") && MainSweepProbe.rows(work.resolve("results")) == 1, "Standalone Codex execution failed: " + first);
    var resumed = NativeCliProbe.process(work, env, List.of(bench.toString(), "codex-ab"));
    require(resumed.code() == 0 && resumed.out().contains("ran=0 skipped=1 failed=0") && MainSweepProbe.rows(work.resolve("results")) == 1, "Standalone Codex resume failed: " + resumed);
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--python")) { python(args); return; }
    checkout = Path.of("").toAbsolutePath(); root = Files.createTempDirectory(checkout.resolve("validation/results"), "codex-sweep-");
    MainSweepProbe.checkout = checkout; MainSweepProbe.root = root; executable = checkout.resolve("target/debug/te-bench");
    script = root.resolve("scripts/launch/codex_ab.sh"); python = root.resolve("python-fixture"); boolean passed = false;
    try {
      Files.createDirectories(script.getParent()); Files.createDirectory(root.resolve("workloads"));
      for (var name : List.of("launch/codex_ab.sh", "launch/_env.sh", "launch/fleet_budget.sh", "sweep_plan.py", "run_experiment.py", "aggregate.py", "_env.py")) Files.copy(checkout.resolve("reference/upstream/experiments/scripts/" + name), root.resolve("scripts/" + name));
      LaunchProbe.executable(python, "#!/bin/sh\nexec " + LaunchProbe.sh(Path.of(System.getProperty("java.home"), "bin/java")) + " -cp " + LaunchProbe.sh(System.getProperty("java.class.path")) + " CodexSweepProbe --python \"$@\"\n");
      var uid = Files.getAttribute(root, "unix:uid").toString();
      cgroup = Files.createDirectories(root.resolve("cgroups/user.slice/user-" + uid + ".slice/user@" + uid + ".service/user.slice"));
      Files.writeString(cgroup.resolve("cpu.max"), "max 100000\n"); Files.writeString(cgroup.resolve("cpu.stat"), "nr_throttled 2\nthrottled_usec 120\n");
      MainSweepProbe.inputs(false); compare(environment());
      for (var arm : List.of("baseline", "baseline-medium", "baseline-low", "repomap", "rtk", "serena", "codex-untool", "codex-spiral", "codex-spiral-postgreen", "codex-spiral-defensive", "codex-combo", "codex-trimech", "codex-trimech-rtk", "codex-trimech-repomap", "codex-inject", "codex-inject-untool", "codex-inject-envfacts", "codex-inject-distiller")) {
        var env = environment(); env.putAll(Map.of("ARM", arm, "BASELINE", "0")); compare(env);
      }
      for (var baseline : List.of("", "0", "1", "only", "other")) { var env = environment(); env.put("BASELINE", baseline); compare(env); }
      var compound = environment(); compound.put("ARM", "baseline baseline-medium"); compare(compound);
      for (var fields : List.of(Map.of("ARMS", "codex-spiral baseline codex-inject-envfacts", "BASELINE", "only"), Map.of("ARMS", "baseline baseline"), Map.of("ARMS", " \t "), Map.of("ARMS", "baseline\ninvalid"), Map.of("ARM", "invalid"), Map.of("ARM", "invalid", "BASELINE", "only"), Map.of("ARMS", "invalid"), Map.of("DOMAIN", "unknown"), Map.of("TASKS_FILE", ""), Map.of("TASKS_FILE", "", "TOKENECONOMIZER_SWEBENCH_TASKS", "fallback.json"), Map.of("DOMAIN", "webbench", "TASKS_FILE", "", "TOKENECONOMIZER_WEBBENCH_TASKS", "web.json"), Map.of("WORKLOAD", "missing.json"), Map.of("WORKLOAD", "none"), Map.of("WORKLOAD", "none", "REPOS", "other/pkg"), Map.of("MODEL", "auto"), Map.of("MODEL", "agent"), Map.of("CODEX_MODEL", "gpt-5.6-terra"), Map.of("MODEL", "claude-opus-4.8", "CODEX_MODEL", "ignored"), Map.of("AGENT", "claude", "DOMAIN", "webbench"), Map.of("RUNNER", "fixture:runner"), Map.of("RUNNER", "fixture:runner", "OPENAI_API_KEY", "fake-1234"), Map.of("RUNNER", "fixture:swebench", "OPENAI_API_KEY", "fake-1234"), Map.of("OPENAI_BASE_URL", "https://fixture.invalid"), Map.of("SEEDS", "2 0 2", "REPOS", "org/b\norg/a-b"), Map.of("SEEDS", "bad -1"))) {
        var env = environment(); env.putAll(fields); compare(env);
      }
      var config = root.resolve("fixture.env"); Files.writeString(config, "MODEL=gpt-5.6-terra\nARM=codex-spiral\nBASELINE=only\nSEEDS=4\nTE_SWEBENCH_MAX_CPUS=2\n");
      var env = environment(); env.put("TOKENECONOMIZER_ENV_FILE", config.toString()); compare(env); env.put("MODEL", "gpt-5.6-sol"); compare(env);
      executed("swebench", "", false); executed("webbench", "", true); executed("swebench", "continue", false); executed("swebench", "abort", false); detached();
      standalone(checkout, root.resolve("standalone"), executable);
      passed = true; System.out.println("Verified " + checks + " Codex-sweep source comparisons and offline execution, child arguments/exports, fleet, resume, failure, detachment and standalone checks");
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Codex-sweep fixtures: " + root); }
  }
}
