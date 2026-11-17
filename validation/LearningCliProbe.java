//JAVA 25
//SOURCES CliProbe.java ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Checks native learning commands and their unchanged upstream output. */
class LearningCliProbe {
  static String quote(Object value) { return CliProbe.quote(value); }
  static void require(boolean condition, String message) { CliProbe.require(condition,message); }
  record Answer(String result, String output, List<String> calls) {}
  static Answer run(ContextProbe.Client client, String op, String args, String ctx, String pinned, Map<String,String> services) throws Exception {
    var output = new StringBuilder();
    var calls = new ArrayList<String>();
    String result = client.call(op,args);
    while (result.contains("\"callback\":\"cli\"")) {
      String method, value;
      if (result.contains("\"method\":\"context\"")) { method = "context"; value = ctx; }
      else if (result.contains("\"method\":\"context_session\"")) { method = "context_session"; value = pinned; }
      else if (result.contains("\"method\":\"say\"")) { method = "say"; output.append(CliProbe.stringArgument(result)).append('\n'); value = "null"; }
      else {
        method = result.contains("\"method\":\"global\"") ? "global" : "service";
        value = null;
        for (var entry : services.entrySet()) {
          if (result.contains((method.equals("global") ? "\"args\":[\"" : ",\"") + entry.getKey() + "\",[")) {
            calls.add(entry.getKey());
            value = entry.getValue();
            break;
          }
        }
        require(value != null,"Unexpected native callback: " + result);
      }
      result = client.resume(result,method,"{\"value\":" + value + "}");
    }
    return new Answer(result,output.toString(),calls);
  }
  static Answer command(ContextProbe.Client client, String op, String args, String ctx, Map<String,String> services) throws Exception {
    return run(client,op,"\"args\":" + args,ctx,ctx,services);
  }
  static void code(Answer answer, int code) { require(answer.result.equals("{\"result\":{\"value\":" + code + "}}"),answer.toString()); }
  static String reference(Path checkout, Path repo, Map<String,String> env, int code, String... args) throws Exception {
    var command = new ArrayList<>(List.of(checkout.resolve(".venv/bin/python").toString(),"-m","tokeneconomizer.plugin.cli","--cwd",repo.toString(),"--host","claude"));
    command.addAll(List.of(args));
    var builder = new ProcessBuilder(command).directory(checkout.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().clear();
    builder.environment().putAll(env);
    var process = builder.start();
    var output = new String(process.getInputStream().readAllBytes(),StandardCharsets.UTF_8);
    require(process.waitFor() == code,output);
    return output;
  }
  static String sourceHelper(Path checkout, Map<String,String> env, String script, String... args) throws Exception {
    var command = new ArrayList<>(List.of(checkout.resolve(".venv/bin/python").toString(),"-c",script));
    command.addAll(List.of(args));
    var builder = new ProcessBuilder(command).directory(checkout.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().clear();
    builder.environment().putAll(env);
    var process = builder.start();
    var output = new String(process.getInputStream().readAllBytes(),StandardCharsets.UTF_8);
    require(process.waitFor() == 0,output);
    return output;
  }
  static String repo(String ctx) { int start = ctx.indexOf("\"repo\":") + 7; return ctx.substring(start,ctx.indexOf('}',start)+1); }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"learning-cli-");
    var repo = Files.createDirectory(root.resolve("repo"));
    var state = Files.createDirectory(root.resolve("state"));
    var executable = checkout.resolve("target/debug/te-bridge");
    var env = new HashMap<>(Map.of("TOKENECONOMIZER_HOME",state.toString(),"HOME",root.toString(),"PATH",System.getenv("PATH"),
        "PYTHONPATH",checkout.resolve("reference/upstream").toString(),"GIT_CEILING_DIRECTORIES",root.toString()));
    if (System.getenv("TZ") != null) env.put("TZ",System.getenv("TZ"));
    try (var client = new ContextProbe.Client(executable,"cli")) {
      Files.writeString(state.resolve("config.json"),"{\"microskills\":\"envfacts\",\"toolprune\":\"off\",\"repomap\":\"off\"}");
      var ctx = CliProbe.context(executable,repo,state,env);
      var base = Files.createDirectories(Path.of(CliProbe.field(ctx,"base")));
      Files.writeString(base.resolve("injection.json"),"{\"rendered\":\"- rule é😀\\r\\n \",\"facts\":{\"z\":{\"state\":\"ACTIVE\",\"fire\":\"1/2\",\"inject\":true},\"a\":{\"state\":\"候補\",\"fire\":\"5/6\"},\"_task\":\"metadata\"}}");
      var services = new HashMap<String,String>();
      services.put("refuse_unwritable","0");
      var facts = command(client,"cli.cmd_microskills","{\"action\":\"show\",\"ids\":[]}",ctx,services);
      code(facts,0);
      ContextProbe.expect(facts.output,reference(checkout,repo,env,0,"microskills"));
      var unknown = command(client,"cli.cmd_microskills","{\"action\":\"forget\",\"ids\":[\"missing\"]}",ctx,services);
      code(unknown,1);
      ContextProbe.expect(unknown.output,reference(checkout,repo,env,1,"microskills","forget","missing"));
      var missing = command(client,"cli.cmd_microskills","{\"action\":\"unforget\",\"ids\":[]}",ctx,services);
      code(missing,2);
      ContextProbe.expect(missing.output,reference(checkout,repo,env,2,"microskills","unforget"));
      for (int n : new int[]{-9,-1,0,1,4,80}) {
        var text = "\u001cé😀 \t a\u2003b\n c\u001f";
        var actual = ContextProbe.value(client.call("cli._short","\"text\":" + quote(text) + ",\"n\":" + n));
        var expected = sourceHelper(checkout,env,"import json, sys; from tokeneconomizer.plugin import cli; print(json.dumps(cli._short(sys.argv[1], int(sys.argv[2])), ensure_ascii=False, separators=(',', ':')))",text,Integer.toString(n)).strip();
        ContextProbe.expect(actual,expected);
      }
      for (String ts : new String[]{"0","-0.5","1","1712345678.9"}) {
        var actual = ContextProbe.value(client.call("cli._when","\"ts\":" + ts));
        var expected = sourceHelper(checkout,env,"import json, sys; from tokeneconomizer.plugin import cli; print(json.dumps(cli._when(float(sys.argv[1]))))",ts).strip();
        ContextProbe.expect(actual,expected);
      }

      var map = "{\"map\":" + quote("é😀".repeat(1001)) + ",\"chars\":2002,\"tokens_budget\":512,\"commit\":\"1234567890abcdef\",\"secs\":0.25,\"python\":\"sdk\",\"error\":\"last attempt failed\"}";
      Files.writeString(base.getParent().resolve("repomap.json"),map);
      services.put("repomap_state",map);
      services.put("head_commit","\"\"");
      var shown = command(client,"cli.cmd_repomap","{\"action\":\"show\"}",ctx,services);
      code(shown,0);
      ContextProbe.expect(shown.output,reference(checkout,repo,env,0,"repomap"));
      services.put("repomap_refresh","{\"error\":\"SDK unavailable\"}");
      var refresh = command(client,"cli.cmd_repomap","{\"action\":\"refresh\"}",ctx,services);
      code(refresh,1);
      require(refresh.output.contains("SDK unavailable") && refresh.output.contains("the map is off"),refresh.toString());

      String summary = "{\"state\":\"done\",\"imported\":5,\"skipped\":{\"z\":1,\"a\":2},\"over_limit\":3}";
      Files.writeString(base.resolve("history.json"),summary);
      services.put("history_summary",summary);
      var history = command(client,"cli.cmd_history","{\"action\":\"status\"}",ctx,services);
      code(history,0);
      ContextProbe.expect(history.output,reference(checkout,repo,env,0,"history"));
      services.put("history_plan","{\"sessions\":[{\"started\":1},{\"started\":1712345678}],\"skipped\":{\"old\":2},\"over_limit\":4}");
      services.put("history_refusal","\"already learned\"");
      var dry = command(client,"cli.cmd_history","{\"action\":\"import\",\"dry_run\":true}",ctx,services);
      code(dry,0);
      require(dry.output.contains("2 earlier session(s)") && dry.output.contains("older_than_the_newest 4") && dry.output.contains("not importable now: already learned"),dry.toString());
      services.put("history_run","{\"state\":\"partial\",\"imported\":3,\"left\":2}");
      var imported = command(client,"cli.cmd_history","{\"action\":\"import\"}",ctx,services);
      code(imported,0);
      require(imported.output.contains("3 earlier session(s)") && imported.output.contains("2 to go"),imported.toString());
      services.put("history_run","{\"state\":\"busy\",\"reason\":\"held\"}");
      var busy = command(client,"cli.cmd_history","{\"action\":\"import\"}",ctx,services);
      code(busy,1);
      ContextProbe.expect(busy.output,"not imported: held\n");
      var auto = command(client,"cli.cmd_history","{\"action\":\"import\",\"auto\":true}",ctx,services);
      code(auto,0);
      require(auto.output.isEmpty(),auto.toString());

      services.put("pending_sessions","[\"s\",\"t\"]");
      services.put("fold_session","{\"kind\":\"session\"}");
      var folded = command(client,"cli.cmd_fold","{\"verbose\":true}",ctx,services);
      code(folded,0);
      ContextProbe.expect(folded.output,"{\"kind\": \"session\"}\n".repeat(2));
      require(folded.calls.equals(List.of("pending_sessions","fold_session","fold_session")),folded.toString());
      services.put("fold_session","null");
      var explicit = command(client,"cli.cmd_fold","{\"session\":[\"explicit\"],\"verbose\":true}",ctx,services);
      code(explicit,0);
      require(explicit.output.isEmpty() && explicit.calls.equals(List.of("fold_session")),explicit.toString());
      services.put("judge_job","null");
      var judged = command(client,"cli.cmd_judge","{\"session\":\"s\",\"job\":\"job\"}",ctx,services);
      code(judged,0);
      require(judged.output.isEmpty() && judged.calls.equals(List.of("judge_job")),judged.toString());

      var otherRepo = Files.createDirectory(root.resolve("other"));
      var other = CliProbe.context(executable,otherRepo,state,env);
      var sessions = Files.createDirectories(base.resolve("sessions"));
      Files.writeString(sessions.resolve("pinned.json"),"{}");
      client.call("repos.pin","\"root\":" + quote(state) + ",\"host\":\"claude\",\"session_id\":\"pinned\",\"repo\":" + repo(ctx) + ",\"cwd\":" + quote(repo));
      String request = "\"args\":{\"host\":\"claude\",\"cwd\":" + quote(otherRepo) + "},\"session_id\":\"pinned\"";
      var pinned = run(client,"cli._session_ctx",request,other,ctx,services);
      require(pinned.result.contains("\"base\":" + quote(base)),pinned.toString());
      Files.delete(sessions.resolve("pinned.json"));
      var fallback = run(client,"cli._session_ctx",request,other,ctx,services);
      require(fallback.result.contains("\"base\":" + quote(CliProbe.field(other,"base"))),fallback.toString());

      services.put("state_root",quote(state));
      services.put("repos_run_migration","null");
      var migrating = command(client,"cli.cmd_repos","{\"action\":\"migrate\"}",ctx,services);
      code(migrating,1);
      require(migrating.output.contains("another migration"),migrating.toString());
      services.put("repos_run_migration","[]");
      var quiet = command(client,"cli.cmd_repos","{\"action\":\"migrate\",\"quiet\":true}",ctx,services);
      code(quiet,0);
      require(quiet.output.isEmpty(),quiet.toString());
      var empty = command(client,"cli.cmd_repos","{\"action\":\"migrate\"}",ctx,services);
      code(empty,0);
      require(empty.output.startsWith("nothing to move: "),empty.toString());
      services.put("repos_listing","[[{\"label\":\"old\",\"key\":\"key\",\"current\":true,\"how\":\"\",\"remote\":\"\",\"agents\":{\"claude\":2,\"codex\":0},\"last\":0,\"imported\":{\"claude\":1},\"checkouts\":[],\"cwds\":[\"a\",\"b\",\"c\",\"d\"],\"notes\":[\"keep\"]}],1]");
      var listing = command(client,"cli.cmd_repos","{\"action\":\"list\"}",ctx,services);
      code(listing,0);
      ContextProbe.expect(listing.output,"old  [key]  <- this directory\n  identified   recorded by an earlier version\n  sessions     claude 2 (+1 from earlier history), codex 0; last -\n  ran in       a, b, c (+1)\n  note         keep\n\n1 earlier state directory merged into the above, kept under " + state.resolve("merged") + "\n");
      for (String key : new String[]{"", ".", "..", "../other", "a/b", "a\\b"}) {
        var invalid = command(client,"cli.cmd_repos","{\"action\":\"merge\",\"keys\":[" + quote(key) + "]}",ctx,services);
        code(invalid,2);
        require(invalid.output.contains("not a state directory key") && invalid.calls.equals(List.of("state_root","refuse_unwritable")),invalid.toString());
      }
      var moves = "[{\"key\":\"old\",\"into\":\"new\",\"moves\":[{\"into\":\"new\",\"host\":\"claude\",\"moved\":2},{\"into\":\"other\",\"host\":\"codex\",\"moved\":1,\"refold\":1}]}]";
      var printed = run(client,"cli._print_moves","\"done\":" + moves,ctx,ctx,services);
      code(printed,2);
      ContextProbe.expect(printed.output,sourceHelper(checkout,env,"import json, sys; from tokeneconomizer.plugin import cli; cli._print_moves(json.loads(sys.argv[1]))",moves));
      System.out.println("Native fact/history/map output matches upstream; import outcomes, session routing, fold output and merge guards passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
