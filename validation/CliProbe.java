//JAVA 25
//SOURCES ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;

/** Compares native CLI presentation with the unchanged upstream CLI. */
class CliProbe {
  static String quote(Object value) {
    var out = new StringBuilder("\"");
    for (char c : value.toString().toCharArray()) {
      if (c == '\\' || c == '"') out.append('\\').append(c);
      else if (c < 32) out.append(String.format("\\u%04x",(int)c));
      else out.append(c);
    }
    return out.append('"').toString();
  }
  static String stringArgument(String request) {
    int start = request.indexOf("\"args\":\"") + 8;
    require(start >= 8,request);
    var out = new StringBuilder();
    for (int i = start; i < request.length(); i++) {
      char c = request.charAt(i);
      if (c == '"') return out.toString();
      if (c != '\\') { out.append(c); continue; }
      char escaped = request.charAt(++i);
      switch (escaped) {
        case 'n' -> out.append('\n');
        case 'r' -> out.append('\r');
        case 't' -> out.append('\t');
        case 'b' -> out.append('\b');
        case 'f' -> out.append('\f');
        case 'u' -> { out.append((char)Integer.parseInt(request.substring(i+1,i+5),16)); i += 4; }
        case '"', '\\', '/' -> out.append(escaped);
        default -> throw new AssertionError(request);
      }
    }
    throw new AssertionError(request);
  }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static String context(Path executable, Path repo, Path state, Map<String,String> env) throws Exception {
    try (var client = new ContextProbe.Client(executable,"context")) {
      return ContextProbe.value(client.call("context.build","\"host\":\"claude\",\"keeps_history\":true,\"cwd\":" + quote(repo)
          + ",\"env\":{\"TOKENECONOMIZER_HOME\":" + quote(state) + "},\"home\":" + quote(env.get("HOME")) + ",\"settle\":true"));
    }
  }
  static String field(String json, String name) {
    var match = Pattern.compile("\"" + name + "\":\"([^\"]+)\"").matcher(json);
    require(match.find(),json);
    return match.group(1);
  }
  static String reference(Path checkout, Path repo, Map<String,String> env, String... args) throws Exception {
    var command = new ArrayList<>(List.of(checkout.resolve(".venv/bin/python").toString(),"-m","tokeneconomizer.plugin.cli","--cwd",repo.toString(),"--host","claude"));
    command.addAll(List.of(args));
    var builder = new ProcessBuilder(command).directory(checkout.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().clear();
    builder.environment().putAll(env);
    var process = builder.start();
    var output = new String(process.getInputStream().readAllBytes(),StandardCharsets.UTF_8);
    require(process.waitFor() == 0,output);
    return output;
  }
  record Answer(String result, String output) {}
  static Answer command(ContextProbe.Client client, String op, String ctx, String args, String map, String writeReply) throws Exception {
    var output = new StringBuilder();
    String result = client.call(op,"\"args\":" + args);
    while (result.contains("\"callback\":\"cli\"")) {
      String method, reply;
      if (result.contains("\"method\":\"context\"")) { method = "context"; reply = "{\"value\":" + ctx + "}"; }
      else if (result.contains("\"method\":\"say\"")) { method = "say"; output.append(stringArgument(result)).append('\n'); reply = "{\"value\":null}"; }
      else if (result.contains("\"method\":\"service\"")) {
        method = "service";
        String value;
        if (result.contains(",\"session_note\",[") || result.contains(",\"paused\",[") || result.contains(",\"history_summary\",[")) value = "null";
        else if (result.contains(",\"applied\",[") || result.contains(",\"proposal\",[")) value = "[]";
        else if (result.contains(",\"refuse_unwritable\",[")) value = "0";
        else if (result.contains(",\"repomap_state\",[")) value = map;
        else if (result.contains(",\"resume\",[")) value = "true";
        else if (result.contains(",\"call_name\",[")) value = "\"claude -p\"";
        else if ((result.contains(",\"write_config\",[") || result.contains(",\"unset_config\",[")) && writeReply != null) {
          result = client.resume(result,method,writeReply);
          continue;
        } else throw new AssertionError(result);
        reply = "{\"value\":" + value + "}";
      } else throw new AssertionError(result);
      result = client.resume(result,method,reply);
    }
    return new Answer(result,output.toString());
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"cli-");
    var repo = Files.createDirectory(root.resolve("repository"));
    var state = root.resolve("state");
    var executable = checkout.resolve("target/debug/te-bridge");
    var env = Map.of("TOKENECONOMIZER_HOME",state.toString(),"HOME",root.toString(),"PATH",System.getenv("PATH"),
        "PYTHONPATH",checkout.resolve("reference/upstream").toString(),"GIT_CEILING_DIRECTORIES",root.toString());
    try (var client = new ContextProbe.Client(executable,"cli")) {
      Files.createDirectories(state);
      Files.writeString(state.resolve("config.json"),"{\"repomap\":\"on\"}");
      var ctx = context(executable,repo,state,env);
      var base = Files.createDirectories(Path.of(field(ctx,"base")));
      Files.writeString(base.resolve("injection.json"),"{\"rendered\":\"- first\\r\\n- é\\u2028- third\",\"sessions\":8,\"imported_sessions\":3}");
      Files.writeString(base.resolve("untool.json"),"{\"sessions\":5,\"imported_sessions\":2,\"prompts\":9,\"imported_prompts\":4}");
      Files.writeString(base.resolve("ledger.jsonl"),"{\"kind\":\"session\",\"effective_tokens\":900,\"turns\":8,\"requests\":12,\"nudges\":2,\"aborts\":1,\"overhead\":{\"spiral_judge_tokens\":100}}\n{\"kind\":\"session\",\"imported\":true,\"turns\":10}\n");
      String map = "{\"map\":\"files\",\"chars\":1024,\"commit\":\"1234567890abcdef\"}";
      Files.writeString(base.getParent().resolve("repomap.json"),map);
      for (boolean explain : new boolean[]{false,true}) {
        var actual = command(client,"cli.cmd_status",ctx,"{\"json\":false,\"explain\":" + explain + "}",map,null);
        ContextProbe.expect(actual.result,"{\"result\":{\"value\":0}}");
        String expected = explain ? reference(checkout,repo,env,"status","--explain") : reference(checkout,repo,env,"status");
        ContextProbe.expect(actual.output,expected);
      }
      var shown = command(client,"cli.cmd_config",ctx,"{\"action\":\"show\"}",map,null);
      ContextProbe.expect(shown.output,reference(checkout,repo,env,"config","show"));
      var pause = state.resolve("hosts/claude/sidecall.json");
      Files.createDirectories(pause.getParent());
      Files.writeString(pause,"{\"at\":1}");
      var set = command(client,"cli.cmd_config",ctx,"{\"action\":\"set\",\"pairs\":[\" loopguard = stop \",\"microskills=envfacts+distiller\",\"microskills_propose_every=2\"]}",map,
          "{\"value\":{\"microskills\":\"envfacts+distiller\",\"loopguard\":\"stop\",\"microskills_propose_every\":2}}");
      ContextProbe.expect(set.output,reference(checkout,repo,env,"config","set"," loopguard = stop ","microskills=envfacts+distiller","microskills_propose_every=2"));
      var unset = command(client,"cli.cmd_config",ctx,"{\"action\":\"unset\",\"pairs\":[\" loopguard \"]}",map,
          "{\"value\":{\"microskills\":\"envfacts+distiller\",\"microskills_propose_every\":2}}");
      ContextProbe.expect(unset.output,reference(checkout,repo,env,"config","unset"," loopguard "));
      var invalid = command(client,"cli.cmd_config",ctx,"{\"action\":\"set\",\"pairs\":[\"missing equals\"]}",map,null);
      require(invalid.result.contains("\"value\":2") && invalid.output.contains("expected KEY=VALUE"),invalid.toString());
      var rejected = command(client,"cli.cmd_config",ctx,"{\"action\":\"set\",\"pairs\":[\"mode=wrong\"]}",map,"{\"kind\":\"ValueError\",\"error\":\"bad mode\"}");
      require(rejected.result.contains("\"value\":2") && rejected.output.equals("rejected: bad mode\n"),rejected.toString());
      var failed = command(client,"cli.cmd_config",ctx,"{\"action\":\"set\",\"pairs\":[\"mode=auto\"]}",map,"{\"kind\":\"OSError\",\"error\":\"disk failure\"}");
      require(failed.result.contains("\"raised\":\"OSError\"") && failed.output.isEmpty(),failed.toString());
      var totals = client.call("accounting.summarize","\"records\":[{\"kind\":\"session\",\"requests\":9007199254740993},{\"kind\":\"session\",\"requests\":2}]");
      require(totals.contains("\"requests\":9007199254740995") && totals.contains("\"nudges\":0,"),totals);
      var overflow = client.call("accounting.summarize","\"records\":[{\"kind\":\"session\",\"turns\":9223372036854775807},{\"kind\":\"session\",\"turns\":1}]");
      require(overflow.contains("turns count exceeds i64"),overflow);
      ContextProbe.expect(client.call("cli._split_argline","\"line\":" + quote("microskills forget '' \"C:\\a b\" a#b a\tb")),"{\"result\":{\"value\":[\"microskills\",\"forget\",\"\",\"C:\\\\a b\",\"a#b\",\"a\",\"b\"]}}");
      var malformed = client.call("cli._split_argline","\"line\":" + quote("'unfinished"));
      require(malformed.contains("\"raised\":\"ValueError\"") && malformed.contains("No closing quotation"),malformed);
      Files.move(base.resolve("injection.json"),base.resolve("injection-saved.json"));
      Files.createDirectory(base.resolve("injection.json"));
      var directory = command(client,"cli.cmd_status",ctx,"{}",map,null);
      require(directory.result.contains("\"raised\":\"IsADirectoryError\"") && directory.output.isEmpty(),directory.toString());
      Files.delete(base.resolve("injection.json"));
      Files.move(base.resolve("injection-saved.json"),base.resolve("injection.json"));
      var transport = new ProcessBuilder(checkout.resolve(".venv/bin/python").toString(),"-c",
          "import native_plugin, os, sys; native_plugin.pytest_configure(None); from tokeneconomizer.plugin import cli; os.chdir(sys.argv[1]); print(cli._unwritable('relative-state')); native_plugin.PROCESS.stdin.close(); native_plugin.PROCESS.wait()",root.toString())
          .directory(checkout.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
      transport.environment().put("PYTHONPATH",checkout.resolve("validation").toString());
      var process = transport.start();
      var result = new String(process.getInputStream().readAllBytes(),StandardCharsets.UTF_8);
      require(process.waitFor() == 0 && result.equals("None\n") && Files.isDirectory(root.resolve("relative-state")),"Relative SDK write probe used the bridge directory: " + result);
      System.out.println("Native CLI status and config match upstream; rejection, I/O failures, exact ledger counts and argument quoting passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
