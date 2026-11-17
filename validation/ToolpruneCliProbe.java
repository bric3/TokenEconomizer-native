//JAVA 25
//SOURCES LearningCliProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Native tool-pruning output, error boundaries and waiter lifecycle. No agent or model calls. */
class ToolpruneCliProbe {
  static String quote(Object value) { return CliProbe.quote(value); }
  static void require(boolean condition, String message) { CliProbe.require(condition,message); }
  static String value(String value) { return "{\"value\":" + value + "}"; }
  static String context(Path executable, Path repo, Path root, String host) throws Exception {
    try (var client = new ContextProbe.Client(executable,"context")) {
      return ContextProbe.value(client.call("context.build","\"host\":" + quote(host) + ",\"cwd\":" + quote(repo) + ",\"env\":{\"TOKENECONOMIZER_HOME\":" + quote(root.resolve("state")) + "},\"home\":" + quote(root) + ",\"settle\":true"));
    }
  }
  record Answer(String result, String output, List<String> calls, List<String> requests) {}
  static Answer run(ContextProbe.Client client, String op, String args, String ctx, Map<String,String> replies) throws Exception {
    var output = new StringBuilder();
    var calls = new ArrayList<String>();
    var requests = new ArrayList<String>();
    var result = client.call(op,args);
    while (result.contains("\"callback\":\"cli\"")) {
      String method, reply;
      if (result.contains("\"method\":\"context\"")) { method = "context"; reply = value(ctx); }
      else if (result.contains("\"method\":\"say\"")) { method = "say"; output.append(CliProbe.stringArgument(result)).append('\n'); reply = value("null"); }
      else {
        method = result.contains("\"method\":\"global\"") ? "global" : "service";
        reply = null;
        for (var entry : replies.entrySet()) {
          if (result.contains((method.equals("global") ? "\"args\":[\"" : ",\"") + entry.getKey() + "\",[")) {
            calls.add(entry.getKey());
            requests.add(result);
            reply = entry.getValue();
            break;
          }
        }
        require(reply != null,"Unexpected native callback: " + result);
      }
      result = client.resume(result,method,reply);
    }
    return new Answer(result,output.toString(),calls,requests);
  }
  static Answer command(ContextProbe.Client client, String ctx, String args, Map<String,String> replies) throws Exception {
    return run(client,"cli.cmd_toolprune","\"args\":" + args,ctx,replies);
  }
  static void code(Answer actual, int code) { ContextProbe.expect(actual.result,"{\"result\":{\"value\":" + code + "}}"); }
  static void waiter(Path executable, String ctx, Path base) throws Exception {
    var beat = base.resolve("toolprune-waiter");
    Files.deleteIfExists(beat);
    try (var client = new ContextProbe.Client(executable,"cli"); var other = new ContextProbe.Client(executable,"cli")) {
      var result = client.call("cli._wait_and_apply","\"args\":{}");
      int contextCalls = 0, attempts = 0, sleeps = 0, touches = 0;
      while (result.contains("\"callback\":\"cli\"")) {
        String method, reply;
        if (result.contains("\"method\":\"context\"")) { method = "context"; contextCalls++; reply = value(ctx); }
        else if (result.contains("\"apply_pending\",[")) { method = "service"; reply = value(switch (attempts++) {case 0 -> "0.01"; case 1 -> "10000"; default -> "null";}); }
        else {
          method = "global";
          if (result.contains("[\"time\",[")) reply = value(Double.toString(System.currentTimeMillis()/1000.0));
          else if (result.contains("[\"pid\",[")) reply = value("1234");
          else if (result.contains("[\"sleep\",[")) {
            require(result.contains(sleeps == 0 ? "[1.0]" : "[600.0]"),result);
            require(Files.readString(beat).equals("1234"),"Waiter PID changed");
            if (sleeps++ == 0) {
              var duplicate = run(other,"cli._wait_and_apply","\"args\":{}",ctx,Map.of("time",value(Double.toString(System.currentTimeMillis()/1000.0))));
              code(duplicate,0);
              require(duplicate.calls.equals(List.of("time")) && Files.exists(beat),duplicate.toString());
            }
            Files.setLastModifiedTime(beat,FileTime.fromMillis(1));
            reply = value("null");
          } else if (result.contains("[\"touch\",[")) {
            touches++;
            Files.setLastModifiedTime(beat,FileTime.from(Instant.now()));
            reply = value("null");
          } else throw new AssertionError(result);
        }
        result = client.resume(result,method,reply);
      }
      require(result.equals("{\"result\":{\"value\":0}}") && contextCalls == 3 && attempts == 3 && sleeps == 2 && touches == 2 && !Files.exists(beat),result);
      var failure = run(client,"cli._wait_and_apply","\"args\":{}",ctx,Map.of("time",value(Double.toString(System.currentTimeMillis()/1000.0)),"pid",value("1234"),"apply_pending","{\"kind\":\"RuntimeError\",\"error\":\"cannot apply\"}"));
      require(failure.result.contains("cannot apply") && !Files.exists(beat),"Failed waiter retained its heartbeat: " + failure);
      var sleeping = run(client,"cli._wait_and_apply","\"args\":{}",ctx,Map.of("time",value(Double.toString(System.currentTimeMillis()/1000.0)),"pid",value("1234"),"apply_pending",value("30"),"sleep","{\"kind\":\"RuntimeError\",\"error\":\"sleep interrupted\"}"));
      require(sleeping.result.contains("sleep interrupted") && !Files.exists(beat) && !sleeping.calls.contains("touch"),sleeping.toString());
      var missingClock = run(client,"cli._wait_and_apply","\"args\":{}",ctx,Map.of("time","{\"kind\":\"ProbeOSError\",\"parents\":[\"OSError\"],\"error\":\"clock unavailable\"}","pid",value("1234"),"apply_pending",value("null")));
      code(missingClock,0);
      require(!Files.exists(beat),missingClock.toString());
    }
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"toolprune-cli-");
    var repo = Files.createDirectory(root.resolve("repo"));
    var state = Files.createDirectory(root.resolve("state"));
    var executable = checkout.resolve("target/debug/te-bridge");
    var env = Map.of("TOKENECONOMIZER_HOME",state.toString(),"HOME",root.toString(),"PATH",System.getenv("PATH"),"PYTHONPATH",checkout.resolve("reference/upstream").toString(),"GIT_CEILING_DIRECTORIES",root.toString());
    try (var client = new ContextProbe.Client(executable,"cli")) {
      String claude = null;
      for (var host : new String[]{"claude","codex","copilot"}) {
        var ctx = context(executable,repo,root,host);
        var base = Files.createDirectories(Path.of(CliProbe.field(ctx,"base")));
        var streaks = new StringBuilder("{\"FirstTie\":50,\"SecondTie\":50");
        for (int i = 0; i < 35; i++) streaks.append(',').append(quote("Tool" + i)).append(':').append(i);
        streaks.append('}');
        String proposal = host.equals("codex") ? "[\"goals\"]" : "[\"UnusedWidget\"]";
        String unt = "{\"sessions\":4,\"prompts\":12,\"catalog_source\":\"observed\",\"proposal\":" + proposal
            + ",\"imported_sessions\":1,\"imported_prompts\":2,\"needs_apply\":" + proposal + ",\"auto_pending\":{\"items\":" + proposal + "},\"streaks\":" + streaks
            + ",\"prompt_streaks\":{\"FirstTie\":9,\"SecondTie\":9},\"used_tools\":[\"Tool34\"],\"capabilities\":{\"goals\":{\"loaded_sessions\":4,\"unused_sessions\":3,\"used\":false},\"collaboration\":{\"loaded_sessions\":4,\"loaded_prompts\":12,\"unused_sessions\":2,\"used\":true}},\"servers\":{\"服务\":{\"tools\":3,\"unused_sessions\":2,\"unused_prompts\":9,\"used\":false},\"used\":{\"tools\":1,\"used\":true}}}";
        Files.writeString(base.resolve("untool.json"),unt);
        var wanted = ContextProbe.value(client.call("cli._proposal","\"ctx\":" + ctx + ",\"unt\":" + unt));
        var replies = Map.of("applied",value("[]"),"proposal",value(wanted),"kept",value("[]"),"codex_trusted",value("null"));
        var shown = command(client,ctx,"{\"action\":\"show\"}",replies);
        code(shown,0);
        var expectedEnv = new HashMap<>(env);
        String expected = sourceCommand(checkout,repo,expectedEnv,host);
        ContextProbe.expect(shown.output,expected);
        if (host.equals("claude")) {
          claude = ctx;
          require(shown.output.indexOf("FirstTie") < shown.output.indexOf("SecondTie") && shown.output.lines().filter(line -> line.matches("  +[0-9]+ sessions.*")).count() == 30,shown.output);
        }
        if (host.equals("codex")) {
          var malformed = run(client,"cli._toolprune_show","\"ctx\":" + ctx + ",\"unt\":" + unt.replaceFirst(",\"used\":false", ""),ctx,replies);
          require(malformed.result.contains("\"raised\":\"KeyError\"") && malformed.result.contains("used"),malformed.toString());
          var codeReplies = new HashMap<String,String>();
          codeReplies.put("refuse_unwritable",value("0"));
          var bad = command(client,ctx,"{\"action\":\"apply\",\"tools\":[\"web\"]}",codeReplies);
          code(bad,2);
          require(bad.output.contains("not a removable Codex capability") && bad.calls.equals(List.of("refuse_unwritable")),bad.toString());
          codeReplies.put("deny_apply",value("[\"goals\"]"));
          codeReplies.put("set_kept",value("null"));
          codeReplies.put("codex_trusted",value("false"));
          codeReplies.put("applied",value("[\"goals\"]"));
          codeReplies.put("verify_codex",value("{\"goals\":false,\"collaboration\":true}"));
          var changed = command(client,ctx,"{\"action\":\"apply\",\"tools\":[\"goals\"]}",codeReplies);
          code(changed,0);
          require(changed.output.contains("warning: this project is not trusted") && changed.output.contains("codex features list: collaboration=on, goals=off"),changed.toString());
          var refreshed = command(client,ctx,"{\"action\":\"refresh-catalog\"}",Map.of());
          code(refreshed,0);
          require(refreshed.output.startsWith("Codex needs no catalog:"),refreshed.toString());
        }
        if (host.equals("copilot")) {
          var codeReplies = Map.of("refuse_unwritable",value("0"),"deny_apply",value("[\"project\"]"),"set_kept",value("null"),"copilot_trusted",value("false"),"applied",value("[\"project\"]"),"verify_copilot",value("{\"project\":0,\"other\":2}"));
          var changed = command(client,ctx,"{\"action\":\"apply\",\"tools\":[\"project\"]}",codeReplies);
          code(changed,0);
          require(changed.output.contains("warning: Copilot reads a repository's") && changed.output.contains("checked with a local capture: other still loaded (2 tools), project removed"),changed.toString());
          var refreshed = command(client,ctx,"{\"action\":\"refresh-catalog\"}",Map.of("refresh_catalog",value("[\"catalog.json\",[\"A\",\"B\"]]")));
          code(refreshed,0);
          ContextProbe.expect(refreshed.output,"2 tools recorded in catalog.json:\n  A, B\n");
        }
      }
      var base = Path.of(CliProbe.field(claude,"base"));
      var replies = new HashMap<String,String>();
      replies.put("refuse_unwritable",value("0"));
      replies.put("deny_apply",value("[\"Workflow\"]"));
      replies.put("deny_restore",value("[\"Workflow\"]"));
      replies.put("set_kept",value("null"));
      var applied = command(client,claude,"{\"action\":\"apply\",\"tools\":[\"Workflow\"]}",replies);
      code(applied,0);
      require(applied.calls.equals(List.of("refuse_unwritable","deny_apply","set_kept")) && applied.requests.getLast().contains("\"set_kept\",[[],[\"Workflow\"]]"),applied.toString());
      require(applied.output.startsWith("added: Workflow\n"),applied.toString());
      var restored = command(client,claude,"{\"action\":\"restore\",\"tools\":[\"Workflow\"]}",replies);
      code(restored,0);
      require(restored.requests.getLast().contains("\"set_kept\",[[\"Workflow\"],[]]") && restored.output.contains("kept from now on"),restored.toString());
      replies.put("deny_apply","{\"kind\":\"ProbeFailure\",\"parents\":[\"RuntimeError\"],\"error\":\"rejected\"}");
      var rejected = command(client,claude,"{\"action\":\"apply\",\"tools\":[\"Workflow\"]}",replies);
      code(rejected,1);
      ContextProbe.expect(rejected.output,"not applied: rejected\n");
      require(!rejected.calls.contains("set_kept"),rejected.toString());
      replies.put("deny_restore","{\"kind\":\"OSError\",\"error\":\"disk failure\"}");
      var failed = command(client,claude,"{\"action\":\"restore\",\"tools\":[\"Workflow\"]}",replies);
      require(failed.result.contains("\"raised\":\"OSError\"") && failed.output.isEmpty() && !failed.calls.contains("set_kept"),failed.toString());
      replies.put("proposal",value("[]"));
      replies.put("applied",value("[\"Workflow\"]"));
      replies.put("measure",value("{\"baseline\":100,\"proposal\":90,\"saved\":10}"));
      var measured = command(client,claude,"{\"action\":\"measure\"}",replies);
      code(measured,0);
      require(measured.calls.equals(List.of("proposal","applied","measure")) && measured.output.contains("\"saved\": 10"),measured.toString());
      replies.put("measure","{\"kind\":\"RuntimeError\",\"error\":\"probe unavailable\"}");
      var unavailable = command(client,claude,"{\"action\":\"measure\",\"tools\":[\"Workflow\"]}",replies);
      code(unavailable,1);
      ContextProbe.expect(unavailable.output,"not measured: probe unavailable\n");
      var rejectedConfig = run(client,"cli.cmd_config","\"args\":{\"action\":\"set\",\"pairs\":[\"mode=bad\"]}",claude,
          Map.of("refuse_unwritable",value("0"),"write_config","{\"kind\":\"ProbeValueError\",\"parents\":[\"ValueError\"],\"error\":\"bad setting\"}"));
      code(rejectedConfig,2);
      ContextProbe.expect(rejectedConfig.output,"rejected: bad setting\n");
      waiter(executable,claude,base);
      System.out.println("Native tool-pruning output matches upstream for three hosts; apply/restore ordering, error classes, measurement and waiter lifecycle passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
  static String sourceCommand(Path checkout, Path repo, Map<String,String> env, String host) throws Exception {
    var command = List.of(checkout.resolve(".venv/bin/python").toString(),"-m","tokeneconomizer.plugin.cli","--cwd",repo.toString(),"--host",host,"toolprune","show");
    var builder = new ProcessBuilder(command).directory(checkout.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().clear(); builder.environment().putAll(env);
    var process = builder.start();
    var output = new String(process.getInputStream().readAllBytes(),java.nio.charset.StandardCharsets.UTF_8);
    require(process.waitFor() == 0,output);
    return output;
  }
}
