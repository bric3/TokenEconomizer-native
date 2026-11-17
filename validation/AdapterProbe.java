//JAVA 25
//SOURCES ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.TimeUnit;

/** Verifies dataset adapters against the pinned implementation without model calls. */
class AdapterProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  static int cases;
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static class Client implements AutoCloseable {
    final Process process; final PrintWriter input; final BufferedReader output;
    Client(Path checkout, boolean nativeCode) throws Exception {
      var command = new java.util.ArrayList<>(List.of(checkout.resolve(".venv/bin/python").toString(), checkout.resolve("validation/adapter_host.py").toString()));
      if (nativeCode) command.add("--native");
      process = new ProcessBuilder(command).directory(checkout.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
      input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
      output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
    }
    String call(String request) throws Exception {
      input.println(request); input.flush();
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
      while (!output.ready()) { require(process.isAlive() && System.nanoTime() < deadline, "Adapter driver stopped or timed out"); Thread.sleep(5); }
      return output.readLine();
    }
    public void close() throws Exception {
      input.close();
      if (!process.waitFor(5, TimeUnit.SECONDS)) process.destroyForcibly().waitFor();
      require(process.exitValue() == 0, "Adapter driver failed"); output.close();
    }
  }
  static void compare(Client nativeCode, Client source, String request) throws Exception {
    var expected = source.call(request); var actual = nativeCode.call(request);
    require(expected.equals(actual), request + "\nNative: " + actual + "\nSource: " + expected); cases++;
  }
  static String request(String domain, String arguments, String method, String extra) {
    return "{\"domain\":" + q(domain) + ",\"arguments\":{" + arguments + "},\"method\":" + q(method) + (extra.isEmpty() ? "" : "," + extra) + "}";
  }
  static void prompts(boolean write) throws Exception {
    for (var domain : List.of("creddata", "reposvul")) {
      var source = Files.readString(Path.of("reference/upstream/experiments/te_bench/adapters", domain + ".py"));
      for (var kind : List.of("TASK_PROMPT", "CLASSIFY_DOMAIN")) {
        var expected = ExtractProposerPrompts.constant(source, domain.toUpperCase(java.util.Locale.ROOT) + "_" + kind, "");
        var path = Path.of("src/experiments/adapters/prompts", domain + "-" + (kind.equals("TASK_PROMPT") ? "task" : "classify") + ".txt");
        if (write) { Files.createDirectories(path.getParent()); Files.writeString(path, expected); }
        if (!Files.readString(path).equals(expected)) throw new AssertionError("Native adapter prompt differs: " + path);
      }
    }
  }
  public static void main(String[] args) throws Exception {
    prompts(args.length == 1 && args[0].equals("--write"));
    if (args.length > 0) { System.out.println("Verified four native dataset adapter prompts"); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "adapters-");
    boolean passed = false;
    try (var nativeCode = new Client(checkout, true); var source = new Client(checkout, false)) {
      var swe = root.resolve("swe.json");
      Files.writeString(swe, "[{\"instance_id\":\"first\",\"problem_statement\":\"  é😀 see https://github.com/a/b/pull/1  \"},{\"instance_id\":\"empty\",\"problem_statement\":null},{\"instance_id\":\"first\",\"problem_statement\":\"replacement #42\"}]");
      var tasks = Files.createDirectories(root.resolve("web/project")).resolve("tasks.jsonl");
      Files.writeString(tasks, "\r\n{\"id\":\"init\",\"description\":\"  init é😀  \"}\r{\"description\":\"skip\"}\n{\"id\":\"\",\"description\":\"skip\"}\n{\"id\":\"task-1\",\"description\":\"https://github.com/a/b/blob/main/a\"}\n{\"id\":\"task-1\",\"description\":\"last\"}\n\u001c\u001f\n{\"id\":\"empty\",\"description\":null}\n");
      for (var folder : List.of("meta", "instances")) {
        var dir = Files.createDirectories(root.resolve("dataset").resolve(folder));
        var suffix = folder.equals("meta") ? ".csv" : ".json";
        for (var name : List.of("z", "a", "é😀", ".hidden")) Files.writeString(dir.resolve(name + suffix), "");
        Files.createDirectory(dir.resolve("directory" + suffix));
        Files.writeString(dir.resolve("wrong.txt"), "");
      }
      var trace = root.resolve("transcript.jsonl");
      Files.writeString(trace, "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"c1\",\"name\":\"Bash\",\"input\":{\"command\":\"pytest\"}}]}}\n{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"c1\",\"content\":\"2 passed\"}]}}\n");
      for (var domain : List.of("swebench", "webbench", "creddata", "reposvul")) {
        var constructor = switch (domain) {
          case "swebench" -> "\"tasks_path\":" + q(swe);
          case "webbench" -> "\"project\":\"project\",\"projects_root\":" + q(root.resolve("web"));
          case "creddata" -> "\"creddata_root\":" + q(root.resolve("dataset"));
          default -> "\"reposvul_root\":" + q(root.resolve("dataset"));
        };
        for (var method : List.of("problem_statements", "classify_domain", "env_vocabulary", "protected_tools", "inapplicable_tools")) compare(nativeCode, source, request(domain, constructor, method, ""));
        compare(nativeCode, source, request(domain, constructor, "hardcoded_exclusions", "\"params\":{\"used_tools\":[\"task\",\"WebFetch\",\"Read\"]}"));
        compare(nativeCode, source, request(domain, constructor, "hardcoded_exclusions", "\"params\":{\"used_tools\":null}"));
        for (var method : List.of("load_trajectory", "tool_usage", "spiral_snapshot")) compare(nativeCode, source, request(domain, constructor, method, "\"params\":{\"path\":" + q(trace) + "}"));
        for (var method : List.of("parse_catalog", "parse_catalog_with_source")) for (var agent : List.of("codex", "claude", "unknown")) compare(nativeCode, source, request(domain, constructor, method, "\"params\":{\"run_dir_path\":" + q(root) + ",\"agent\":" + q(agent) + "}"));
        var empty = domain.equals("webbench") ? "\"project\":\"project\"" : "";
        compare(nativeCode, source, request(domain, empty, "problem_statements", "\"error_details\":true"));
        var env = switch (domain) { case "swebench" -> "TOKENECONOMIZER_SWEBENCH_TASKS"; case "webbench" -> "TOKENECONOMIZER_WEBBENCH_PROJECTS"; case "creddata" -> "TOKENECONOMIZER_CREDDATA_ROOT"; default -> "TOKENECONOMIZER_REPOSVUL_ROOT"; };
        var location = domain.equals("swebench") ? swe : domain.equals("webbench") ? root.resolve("web") : root.resolve("dataset");
        compare(nativeCode, source, request(domain, empty, "problem_statements", "\"env\":{" + q(env) + ":" + q(location) + "},\"after_env\":{" + q(env) + ":\"/not-read-after-construction\"}"));
        compare(nativeCode, source, request(domain, constructor, "problem_statements", "\"env\":{" + q(env) + ":\"/explicit-path-wins\"}"));
      }
      for (var domain : List.of("creddata", "reposvul")) {
        var ids = domain.equals("creddata") ? "repo_ids" : "instance_ids";
        var attr = domain.equals("creddata") ? "creddata_root" : "reposvul_root";
        for (var selected : List.of("[]", "[\"z\",\"a\",\"z\"]")) {
          compare(nativeCode, source, request(domain, q(ids) + ":" + selected, ids, "\"iterators\":[" + q(ids) + "]"));
          compare(nativeCode, source, request(domain, q(ids) + ":" + selected, "problem_statements", ""));
        }
        compare(nativeCode, source, request(domain, q(attr) + ":" + q(root.resolve("missing")), ids, ""));
        var alternate = Files.createDirectories(root.resolve("other-" + domain).resolve(domain.equals("creddata") ? "meta" : "instances"));
        Files.writeString(alternate.resolve("a" + (domain.equals("creddata") ? ".csv" : ".json")), "");
        compare(nativeCode, source, request(domain, q(attr) + ":" + q(root.resolve("*")), ids, ""));
      }
      for (var domain : List.of("swebench", "webbench", "creddata", "reposvul")) {
        var relative = switch (domain) { case "swebench" -> "\"tasks_path\":\"swe.json\""; case "webbench" -> "\"project\":\"project\",\"projects_root\":\"web\""; case "creddata" -> "\"creddata_root\":\"dataset\""; default -> "\"reposvul_root\":\"dataset\""; };
        compare(nativeCode, source, request(domain, relative, "problem_statements", "\"cwd\":" + q(root)));
        compare(nativeCode, source, request(domain, relative, "tool_usage", "\"cwd\":" + q(root) + ",\"params\":{\"path\":\"transcript.jsonl\"}"));
      }
      compare(nativeCode, source, request("webbench", "\"project\":\"project\",\"projects_root\":\"web\"", "tasks_path", "\"cwd\":" + q(root)));
      for (var method : List.of("redact_gold_pointers")) {
        for (var text : List.of("", "#42 and https://example.com/a", "https://github.com/a/b/issues/42, https://www.GITHUB.com/a/b/tree/main/code",
            "http://github.com/é²/Ⅷ/pull/1\u001cafter", "https://github.com/e\u0301/repo/commit/1", "https://gİthub.com/a/b/commıt/é😀", "https://gıthub.com/a/b/İSSUE/1", "https://github.com/a/b/blob/x\ufeffkeep")) {
          compare(nativeCode, source, "{\"method\":" + q(method) + ",\"params\":{\"text\":" + q(text) + "}}");
        }
        for (var route : List.of("pull", "pulls", "issues", "issue", "commit", "commits", "compare", "blob", "tree", "releases")) compare(nativeCode, source, "{\"method\":" + q(method) + ",\"params\":{\"text\":" + q("See HTTPS://WWW.GITHUB.COM/user.name/repo_name/" + route + "/42?x=1.") + "}}");
      }
      compare(nativeCode, source, "{\"method\":\"untool_exclusions\",\"params\":{\"used_tools\":[\"task\",\"WebSearch\"],\"extra_inapplicable\":[\"Read\",\"WebFetch\",\"WebFetch\",\"task\"]}}");
      for (var bad : List.of("not json", "[{}]", "[{\"instance_id\":\"bad\",\"problem_statement\":12}]", "[] trailing", "\ufeff[]")) {
        var path = root.resolve("bad.json"); Files.writeString(path, bad);
        compare(nativeCode, source, request("swebench", "\"tasks_path\":" + q(path), "problem_statements", ""));
      }
      for (var bad : List.of("not json\n", "[]\n", "{\"id\":\"x\",\"description\":12}\n")) {
        Files.writeString(tasks, bad);
        compare(nativeCode, source, request("webbench", "\"project\":\"project\",\"projects_root\":" + q(root.resolve("web")), "problem_statements", ""));
      }
      var invalid = root.resolve("invalid.json"); Files.write(invalid, new byte[]{'[', (byte)0xff, ']'});
      for (var path : List.of(invalid, root.resolve("absent.json"), root)) compare(nativeCode, source, request("swebench", "\"tasks_path\":" + q(path), "problem_statements", ""));
      System.out.println("All four dataset adapters match the pinned implementation in " + cases + " cases; prompts, constructors, discovery, trace readers, tool policy and error types passed without model calls");
      passed = true;
    } finally {
      if (passed) { try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); } }
      else System.err.println("Probe artifacts: " + root);
    }
  }
}
