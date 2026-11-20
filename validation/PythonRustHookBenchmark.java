//JAVA 25
import java.io.BufferedWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Compares the pinned Python hook and the release Rust hook on paired offline inputs. */
class PythonRustHookBenchmark {
  record Scenario(String name, int prompts, int callsPerPrompt, int pairs) {}
  record Target(String name, List<String> command) {}
  record Sample(double wallMs, double cpuMs, long peakBytes) {}
  static final Path CHECKOUT = Path.of("").toAbsolutePath();
  static final Path RESULTS = CHECKOUT.resolve("validation/results");
  static final Path REFERENCE = CHECKOUT.resolve("reference/upstream");
  static final Path PYTHON = CHECKOUT.resolve(".venv/bin/python");
  static final Path RUST_HOOK = CHECKOUT.resolve("target/release/te-hook");
  static final List<Target> TARGETS = List.of(
      new Target("Python", List.of(PYTHON.toString(), "-m", "tokeneconomizer.plugin.hook")),
      new Target("Rust", List.of(RUST_HOOK.toString())));
  static final Pattern CPU = Pattern.compile("([0-9.]+) user\\s+([0-9.]+) sys");
  static final Pattern RSS = Pattern.compile("(?m)^\\s*(\\d+)\\s+maximum resident set size$");
  static boolean direct = false;

  static String quote(String text) {
    return "\"" + text.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n") + "\"";
  }

  static void require(boolean condition, String detail) {
    if (!condition) throw new AssertionError(detail);
  }

  static Sample hook(Target target, Path lane, Path repo, Path trace, String event, String fields,
                     boolean timed) throws Exception {
    var input = lane.resolve("in");
    var output = lane.resolve("out");
    var diagnostics = lane.resolve("stderr");
    Files.writeString(input, "{\"hook_event_name\":" + quote(event)
        + ",\"session_id\":\"bench\",\"prompt_id\":\"p1\",\"cwd\":" + quote(repo.toString())
        + ",\"transcript_path\":" + quote(trace == null ? "" : trace.toString()) + fields + "}");
    var argv = new ArrayList<String>();
    if (timed && !direct) argv.addAll(List.of("/usr/bin/time", "-l"));
    argv.addAll(target.command());
    argv.addAll(List.of("--host", "claude"));
    var command = new ProcessBuilder(argv).directory(repo.toFile())
        .redirectInput(input.toFile()).redirectOutput(output.toFile()).redirectError(diagnostics.toFile());
    var env = command.environment();
    env.clear();
    env.put("HOME", lane.toString());
    env.put("PATH", "/usr/bin:/bin");
    env.put("LC_ALL", "en_US.UTF-8");
    env.put("PYTHONPATH", REFERENCE.toString());
    env.put("PYTHONPYCACHEPREFIX", lane.getParent().resolve("pycache").toString());
    env.put("GIT_CONFIG_NOSYSTEM", "1");
    env.put("GIT_CONFIG_GLOBAL", "/dev/null");
    env.put("TOKENECONOMIZER_HOME", lane.resolve("state").toString());
    env.put("CLAUDE_CONFIG_DIR", lane.resolve("claude").toString());
    env.put("TE_MODE", "headless");
    env.put("TE_FOLD_SYNC", "1");
    env.put("TE_MICROSKILLS", "envfacts");
    env.put("TE_TOOLPRUNE", "off");
    env.put("TE_LOCAL_MODEL", "off");
    long start = System.nanoTime();
    var process = command.start();
    if (!process.waitFor(60, TimeUnit.SECONDS)) {
      process.descendants().forEach(ProcessHandle::destroyForcibly);
      process.destroyForcibly();
      throw new AssertionError("Timed out: " + target.name() + " " + event + " " + lane);
    }
    double wallMs = (System.nanoTime() - start) / 1e6;
    String error = Files.readString(diagnostics);
    require(process.exitValue() == 0 && Files.readString(output).trim().equals("{}")
        && !Files.exists(lane.resolve("state/errors.log")), target.name() + " " + event + " " + lane + " " + error);
    if (!timed) return null;
    if (direct) return new Sample(wallMs, Double.NaN, 0);
    var cpu = CPU.matcher(error);
    var rss = RSS.matcher(error);
    require(cpu.find() && rss.find(), "Missing counters: " + error);
    return new Sample(wallMs, 1000 * (Double.parseDouble(cpu.group(1)) + Double.parseDouble(cpu.group(2))),
        Long.parseLong(rss.group(1)));
  }

  static String state(Path lane, String filename) throws Exception {
    try (var paths = Files.walk(lane.resolve("state/repos"))) {
      var files = paths.filter(path -> path.getFileName().toString().equals(filename)).toList();
      require(files.size() == 1, "Expected one " + filename + " in " + lane);
      return Files.readString(files.getFirst());
    }
  }

  static double number(String json, String key) {
    var number = Pattern.compile("\"" + key + "\"\\s*:\\s*([0-9.]+)").matcher(json);
    require(number.find(), "Missing " + key + " in " + json);
    return Double.parseDouble(number.group(1));
  }

  static Sample run(Target target, Scenario scenario, Path lane, Path repo, Path trace) throws Exception {
    Files.createDirectories(lane.resolve("claude/projects"));
    if (scenario.name().equals("session-start")) {
      var result = hook(target, lane, repo, trace, "SessionStart", ",\"source\":\"startup\"", true);
      require(state(lane, "bench.json").contains("\"started\""), "Missing started session");
      return result;
    }
    hook(target, lane, repo, trace, "SessionStart", ",\"source\":\"startup\"", false);
    if (scenario.name().equals("post-tool-use")) {
      hook(target, lane, repo, trace, "UserPromptSubmit", ",\"prompt\":\"Read the file\"", false);
      var result = hook(target, lane, repo, trace, "PostToolUse",
          ",\"tool_name\":\"Read\",\"tool_input\":{\"file_path\":\"src/module.py\"},\"tool_response\":\"ok\"", true);
      hook(target, lane, repo, trace, "Stop", ",\"stop_hook_active\":false", false);
      require(number(state(lane, "events.jsonl"), "tool_calls") == 1, "Expected one recorded tool call");
      return result;
    }
    var result = hook(target, lane, repo, trace, "SessionEnd", ",\"reason\":\"other\"", true);
    String row = state(lane, "ledger.jsonl");
    int calls = scenario.prompts() * scenario.callsPerPrompt();
    require(number(row, "prompts") == scenario.prompts() && number(row, "requests") == calls
        && number(row, "effective_tokens") == calls * 435.0 && row.strip().lines().count() == 1,
        target.name() + " " + scenario.name() + " ledger mismatch: " + row);
    return result;
  }

  static void trace(Path path, Path repo, int prompts, int perPrompt) throws Exception {
    String read = "file content é ".repeat(32);
    try (BufferedWriter out = Files.newBufferedWriter(path)) {
      for (int turn = 0; turn < prompts; turn++) {
        out.write("{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Fix and test this module\"}}\n");
        for (int slot = 0; slot < perPrompt; slot++) {
          int id = turn * perPrompt + slot;
          String tool = slot == 0 ? "Edit" : slot < 3 ? "Bash" : "Read";
          String file = quote(repo.resolve("pkg/module" + id + ".py").toString());
          String args = switch (slot) {
            case 0 -> "{\"file_path\":" + file + ",\"old_string\":\"a\",\"new_string\":\"b\"}";
            case 1 -> "{\"command\":\"pytest -q tests/test_module.py\"}";
            case 2 -> "{\"command\":\"python -m pytest -q tests/test_module.py\"}";
            default -> "{\"file_path\":" + file + "}";
          };
          String result = switch (slot) {
            case 0 -> "ok";
            case 1 -> "bash: pytest: command not found";
            case 2 -> "1 passed in 0.1s";
            default -> read;
          };
          out.write("{\"type\":\"assistant\",\"message\":{\"id\":\"msg" + id
              + "\",\"model\":\"claude-opus-5-5\",\"role\":\"assistant\","
              + "\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":1000,\"cache_creation_input_tokens\":100,\"output_tokens\":50},"
              + "\"content\":[{\"type\":\"tool_use\",\"id\":\"tu" + id + "\",\"name\":" + quote(tool)
              + ",\"input\":" + args + "}]}}\n");
          out.write("{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\","
              + "\"tool_use_id\":\"tu" + id + "\",\"is_error\":" + (slot == 1)
              + ",\"content\":" + quote(result) + "}]}}\n");
        }
      }
    }
  }

  static void delete(Path directory) throws Exception {
    try (var paths = Files.walk(directory)) {
      for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
    }
  }

  static double median(double[] values) {
    Arrays.sort(values);
    return values[values.length / 2];
  }

  public static void main(String[] args) throws Exception {
    if (args.length > 1 || (args.length == 1 && !List.of("--pilot", "--direct").contains(args[0]))) {
      throw new IllegalArgumentException("Usage: jbang validation/PythonRustHookBenchmark.java [--direct | --pilot]");
    }
    require(System.getProperty("os.name").equals("Mac OS X"), "The benchmark uses macOS /usr/bin/time -l and /private/tmp");
    require(Files.isRegularFile(REFERENCE.resolve("tokeneconomizer/plugin/hook.py")), "Run from the checkout root");
    require(Files.isExecutable(PYTHON), "Create .venv with python3.15 -m venv --without-pip .venv");
    require(Files.isExecutable(RUST_HOOK) && Files.isExecutable(RUST_HOOK.resolveSibling("te")),
        "Build te and te-hook with cargo build --release --locked --target-dir target --bin te --bin te-hook");
    Files.createDirectories(RESULTS);
    boolean pilot = args.length != 0 && args[0].equals("--pilot");
    direct = args.length != 0 && args[0].equals("--direct");
    var root = Files.createTempDirectory(Path.of("/private/tmp"), "te-python-rust-");
    var repo = Files.createDirectory(root.resolve("repo"));
    var git = Files.createDirectories(repo.resolve(".git/info")).getParent();
    Files.writeString(git.resolve("HEAD"), "ref: refs/heads/main\n");
    Files.writeString(git.resolve("config"), "[remote \"origin\"]\n  url = https://example.invalid/acme/benchmark.git\n");
    Files.writeString(repo.resolve("pyproject.toml"), "[project]\nname = 'bench'\n");
    var scenarios = List.of(new Scenario("session-start", 0, 0, 21),
        new Scenario("post-tool-use", 0, 0, 21), new Scenario("fold-96", 8, 12, 15),
        new Scenario("fold-3072", 128, 24, 11));
    var raw = RESULTS.resolve(pilot ? "python-vs-rust-pilot.tsv" :
        direct ? "python-vs-rust-direct-samples.tsv" : "python-vs-rust-samples.tsv");
    try (var writer = Files.newBufferedWriter(raw)) {
      writer.write("scenario\ttrace_bytes\tpair\ttarget\twall_ms\tcpu_ms\tpeak_rss_bytes\n");
      for (var scenario : scenarios) {
        Path transcript = scenario.prompts() == 0 ? null : root.resolve(scenario.name() + ".jsonl");
        if (transcript != null) trace(transcript, repo, scenario.prompts(), scenario.callsPerPrompt());
        long traceBytes = transcript == null ? 0 : Files.size(transcript);
        int pairs = pilot ? 1 : scenario.pairs();
        int warmups = pilot ? 0 : 2;
        var observations = new Sample[2][pairs];
        for (int i = -warmups; i < pairs; i++) {
          for (int order = 0; order < 2; order++) {
            int t = (i & 1) == 0 ? order : (1 - order);
            var target = TARGETS.get(t);
            var lane = root.resolve(scenario.name() + "-" + i + "-impl" + t);
            var sample = run(target, scenario, lane, repo, transcript);
            if (i >= 0) {
              observations[t][i] = sample;
              writer.write(String.format(Locale.ROOT, "%s\t%d\t%d\t%s\t%.4f",
                  scenario.name(), traceBytes, i + 1, target.name(), sample.wallMs()));
              writer.write(direct ? "\t\t\n" : String.format(Locale.ROOT, "\t%.0f\t%d%n",
                  sample.cpuMs(), sample.peakBytes()));
              writer.flush();
            }
            delete(lane);
          }
        }
        System.out.printf(Locale.ROOT, "%s, %d bytes, %d pairs%n", scenario.name(), traceBytes, pairs);
        for (int t = 0; t < 2; t++) {
          var samples = observations[t];
          double wall = median(Arrays.stream(samples).mapToDouble(Sample::wallMs).toArray());
          if (direct) {
            System.out.printf(Locale.ROOT, "  %s: wall %.2f ms%n", TARGETS.get(t).name(), wall);
          } else {
            double cpu = median(Arrays.stream(samples).mapToDouble(Sample::cpuMs).toArray());
            double rss = median(Arrays.stream(samples).mapToDouble(Sample::peakBytes).toArray()) / (1024.0 * 1024.0);
            System.out.printf(Locale.ROOT, "  %s: wall %.2f ms, CPU %.0f ms, peak RSS %.2f MiB%n",
                TARGETS.get(t).name(), wall, cpu, rss);
          }
        }
      }
    } catch (Throwable failure) {
      System.err.println("Benchmark fixtures retained at " + root);
      throw failure;
    }
    delete(root);
  }
}
