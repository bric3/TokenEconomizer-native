//JAVA 25
//SOURCES ContextProbe.java CliProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** Offline probe checks: upstream reports, fixture traps and restoration after failures. */
class ProbeProbe {
  static String q(Object value) { return CliProbe.quote(value); }
  static void require(boolean condition, String message) { CliProbe.require(condition,message); }
  static String array(List<String> strings) { return "[" + String.join(",",strings.stream().map(ProbeProbe::q).toList()) + "]"; }
  static String source(Path checkout, String code, String... args) throws Exception {
    var command = new ArrayList<>(List.of(checkout.resolve(".venv/bin/python").toString(),"-c",code));
    command.addAll(List.of(args));
    var builder = new ProcessBuilder(command).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().put("PYTHONPATH",checkout.resolve("reference/upstream") + java.io.File.pathSeparator + checkout.resolve("validation"));
    var process = builder.start();
    var output = new String(process.getInputStream().readAllBytes(),StandardCharsets.UTF_8);
    require(process.waitFor() == 0,"Upstream probe failed: " + code);
    return output.stripTrailing();
  }
  static String environment(Path home) { return "\"process_env\":{\"COPILOT_HOME\":" + q(home) + "},\"home\":" + q(home) + ",\"process_cwd\":" + q(home); }
  static String report(String session, boolean injected, int attempts, Integer tokens) {
    return "{\"session\":" + q(session) + ",\"injected\":" + injected + ",\"tests\":[\"python -m " + (attempts>0?"pytest":"unittest") + "\"],\"first_test\":\"python -m " + (attempts>0?"pytest":"unittest") + "\",\"tried_pytest\":" + attempts + ",\"tool_definition_tokens\":" + tokens + ",\"input_per_request\":10}";
  }
  static void helpers(ContextProbe.Client client, Path checkout, Path root) throws Exception {
    var commands = new ArrayList<String>();
    for (String prefix : List.of("","cd /r && ","(","rtk ","x;","x|","x&","x\u001c","x_"))
      for (String runner : List.of("pytest","py.test","python -m pytest","python3.15 -m unittest","python\u001f-m\u001cpytest","python -m unittest.mock","python3 -munittest","python -m pytest_extra","pytesté","pytest²","pytest\u0301")) commands.add(prefix + runner);
    var input = array(commands);
    var expected = source(checkout,"import json, sys; from tokeneconomizer.plugin import probe; print(json.dumps(probe.test_commands(json.loads(sys.argv[1])), separators=(',', ':'), ensure_ascii=False))",input);
    ContextProbe.expect(ContextProbe.value(client.call("probe.test_commands","\"cmds\":" + input)),expected);

    var env = "{\"PATH\":\"old\",\"TE_DISABLE\":\"1\",\"TE_SIDECALL\":\"1\",\"TERM_PROGRAM\":\"term\",\"CLAUDECODE\":\"1\",\"CLAUDE_PLUGIN_ROOT\":\"x\",\"CLAUDE_CODE_SESSION_ID\":\"s\",\"KEEP\":\"yes\"}";
    var sessionEnv = ContextProbe.value(client.call("probe._session_env","\"state\":\"state\",\"trap\":\"trap\",\"process_env\":" + env));
    var sourceEnv = source(checkout,"import json, os, sys; from tokeneconomizer.plugin import probe; os.environ.clear(); os.environ.update(json.loads(sys.argv[1])); print(json.dumps(probe._session_env('state','trap'), separators=(',', ':')))",env);
    ContextProbe.expect(sessionEnv,sourceEnv);

    var output = client.call("probe.run_session","\"repo\":\"repo\",\"env\":{},\"plugin\":\"plug\",\"model\":\"model\",\"cli\":\"cp\",\"timeout\":23");
    require(output.contains("[\"program\",[\"cp\"]]"),output);
    output = client.resume(output,"probe","{\"value\":[\"node\",\"cp.js\"]}");
    require(output.contains("[\"process\",[[\"node\",\"cp.js\",\"-p\"") && output.contains("\"--model\",\"model\"]") && output.contains(",23]"),output);
    var stdout = "null\n[1]\n{bad\n{\"type\":\"result\",\"sessionId\":\"first\"}\r\n{\"type\":\"assistant.message\",\"data\":{\"content\":\"  one test é😀  \"}}\u2028{\"type\":\"assistant.message\",\"data\":[]}\n{\"type\":\"result\",\"sessionId\":\"last\"}\n";
    output = client.resume(output,"probe","{\"value\":{\"stdout\":" + q(stdout) + ",\"stderr\":\"first\\r\\n  last error  \\n\"}}");
    ContextProbe.expect(ContextProbe.value(output),"[\"last\",\"one test é😀\",\"  last error\"]");
    ContextProbe.expect(source(checkout,"import native_plugin; native_plugin.pytest_configure(None); import subprocess, unittest; from unittest.mock import patch; from tokeneconomizer.plugin import probe; patch.object(probe.sidecall,'program',return_value=['copilot']).start(); patch.object(probe.subprocess,'run',side_effect=subprocess.TimeoutExpired('copilot',23)).start(); unittest.TestCase().assertRaises(subprocess.TimeoutExpired,probe.run_session,'repo',{},'plugin',''); print('timeout preserved')"),"timeout preserved");

    var plugin = Files.createDirectories(root.resolve("plugin/hooks"));
    Files.writeString(plugin.resolve("copilot-hooks.json"),"{}");
    ContextProbe.expect(ContextProbe.value(client.call("probe._plugin_root","\"module_file\":" + q(root.resolve("tokeneconomizer/plugin/probe.py")) + ",\"process_cwd\":" + q(root) + ",\"process_env\":{\"TE_PLUGIN_ROOT\":\"plugin\"}")),"\"plugin\"");
    var missing = client.call("probe._plugin_root","\"module_file\":" + q(root.resolve("absent/plugin/probe.py")));
    require(missing.contains("\"raised\":\"RuntimeError\"") && missing.contains("set TE_PLUGIN_ROOT"),missing);

    var base = Files.createDirectories(root.resolve("fold-store"));
    Files.writeString(base.resolve("folded.json"),"{\"s\":{}}");
    var waitArgs = "\"ctx\":{\"base\":" + q(base) + "},\"sid\":\"s\",\"timeout\":2";
    var wait = client.call("probe._wait_folded",waitArgs);
    wait = client.resume(wait,"probe","{\"value\":10}");
    wait = client.resume(wait,"probe","{\"value\":10}");
    ContextProbe.expect(ContextProbe.value(wait),"true");
    Files.delete(base.resolve("folded.json"));
    wait = client.call("probe._wait_folded",waitArgs);
    wait = client.resume(wait,"probe","{\"value\":10}");
    wait = client.resume(wait,"probe","{\"value\":10}");
    require(wait.contains("[\"sleep\",[1]]"),wait);
    wait = client.resume(wait,"probe","{\"value\":null}");
    wait = client.resume(wait,"probe","{\"value\":12}");
    ContextProbe.expect(ContextProbe.value(wait),"false");
  }
  static void inspect(ContextProbe.Client client, Path checkout, Path root) throws Exception {
    var log = root.resolve("session-state/s1/events.jsonl");
    Files.createDirectories(log.getParent());
    var command = "cd /r && python -m pytest | tail -1";
    Files.writeString(log,"{\"type\":\"user.message\",\"data\":{}}\n"
        + "{\"type\":\"tool.execution_start\",\"data\":{\"toolCallId\":\"a\",\"toolName\":\"bash\",\"arguments\":{\"command\":" + q(command) + "}}}\n"
        + "{\"type\":\"tool.execution_complete\",\"data\":{\"toolCallId\":\"a\",\"result\":{\"content\":\"127\"},\"success\":true}}\n"
        + "\"hook.end sessionStart\"\n{broken \"hook.end\" \"sessionStart\"\n"
        + "{\"type\":\"hook.end\",\"data\":{\"hookType\":\"sessionStart\",\"output\":{\"additionalContext\":\"pytest fails here\"}}}\n"
        + "{\"type\":\"session.shutdown\",\"data\":{\"modelMetrics\":{\"m\":{\"usage\":{\"inputTokens\":65,\"cacheReadTokens\":10,\"cacheWriteTokens\":5},\"requests\":{\"count\":2}}},\"toolDefinitionsTokens\":123}}\n");
    var actual = client.call("probe.inspect_session","\"sid\":\"s1\"," + environment(root));
    require(actual.contains("[\"settle\",[" + q(log)),actual);
    actual = ContextProbe.value(client.resume(actual,"probe","{\"value\":null}"));
    var expected = source(checkout,"import json, os, sys; from tokeneconomizer.plugin import probe; os.environ['COPILOT_HOME']=sys.argv[1]; print(json.dumps(probe.inspect_session('s1'), separators=(',', ':'), ensure_ascii=False))",root.toString());
    ContextProbe.expect(actual,expected);
    require(actual.contains("\"input_per_request\":32") && actual.contains("\"tried_pytest\":1"),actual);
    Files.delete(log);
    var missing = client.call("probe.inspect_session","\"sid\":\"s1\"," + environment(root));
    missing = client.resume(missing,"probe","{\"value\":null}");
    require(missing.contains("\"raised\":\"FileNotFoundError\""),missing);
  }
  static void fixture(ContextProbe.Client client, Path checkout, Path root) throws Exception {
    var fixture = Files.createDirectories(root.resolve("fixture's $HOME `quoted`"));
    var python = checkout.resolve(".venv/bin/python");
    var result = client.call("probe.make_repo","\"root\":" + q(fixture) + ",\"python\":" + q(python));
    result = client.resume(result,"probe","{\"value\":null}");
    require(result.contains("\"add\",\"-A\""),result);
    result = client.resume(result,"probe","{\"value\":null}");
    require(result.contains("\"commit\",\"-S\"") && result.contains("test: initialise probe fixture") && !result.contains("gpgsign=false"),result);
    result = client.resume(result,"probe","{\"value\":null}");
    ContextProbe.expect(ContextProbe.value(result),"[" + q(fixture.resolve("repo")) + "," + q(fixture.resolve("trap")) + "]");
    var trap = fixture.resolve("trap");
    for (String executable : List.of("python","python3","pytest")) require(Files.isExecutable(trap.resolve(executable)),executable);
    for (var command : List.of(List.of(trap.resolve("pytest").toString()),List.of(trap.resolve("python").toString(),"-m","pytest"),List.of(trap.resolve("python3").toString(),"-m","unittest"))) {
      var log = fixture.resolve("output.log");
      var process = new ProcessBuilder(command).directory(fixture.resolve("repo").toFile()).redirectErrorStream(true).redirectOutput(log.toFile()).start();
      var code = process.waitFor();
      var output = Files.readString(log);
      boolean success = command.contains("unittest");
      require(success ? code == 0 && output.contains("Ran 1 test") : code != 0 && output.contains("pytest"),command + ": " + code + " " + output);
    }
    var wrapper = Files.readString(trap.resolve("python"));
    require(wrapper.contains("'\\''") && wrapper.contains("${PYTHONPATH:+:$PYTHONPATH}"),wrapper);
  }
  static CliProbe.Answer run(ContextProbe.Client client, Path root, int sessions, boolean proposal, boolean keep, boolean fail, boolean noSession) throws Exception {
    var base = Files.createDirectories(root.resolve("store"));
    Files.writeString(base.resolve("untool.json"),proposal?"{\"proposal\":[\"unused\"]}":"{}");
    var output = new StringBuilder();
    var result = client.call("probe.run","\"sessions\":" + sessions + ",\"model\":null,\"keep\":" + keep + ",\"cli\":\"copilot\"," + environment(root));
    int session = 0, applied = 0, restored = 0;
    while (result.contains("\"callback\":\"cli\"")) {
      if (result.contains("\"method\":\"say\"")) {
        output.append(CliProbe.stringArgument(result)).append('\n');
        result = client.resume(result,"say","{\"value\":null}");
        continue;
      }
      require(result.contains("\"method\":\"probe\""),result);
      String value;
      if (result.contains("[\"which\",")) value = "\"copilot\"";
      else if (result.contains("[\"copilot_model\",")) value = "\"local/model\"";
      else if (result.contains("[\"plugin_root\",")) value = q(root.resolve("plugin"));
      else if (result.contains("[\"trusted_folders\",")) value = "[]";
      else if (result.contains("[\"mkdtemp\",")) value = q(root);
      else if (result.contains("[\"make_repo\",")) value = "[" + q(root.resolve("repo")) + "," + q(root.resolve("trap")) + "]";
      else if (result.contains("[\"refresh_catalog\",")) value = "[null,[\"a\",\"b\"]]";
      else if (result.contains("[\"context\",")) value = "{\"base\":" + q(base) + "}";
      else if (result.contains("[\"wait_folded\",")) value = "true";
      else if (result.contains("[\"run_session\",")) {
        session++;
        if (fail && session > sessions) {
          result = client.resume(result,"probe","{\"error\":\"session failed\",\"kind\":\"RuntimeError\"}");
          continue;
        }
        value = noSession?"[null,\"\",\"missing output\"]":"[\"s" + session + "\",\"one test\",\"\"]";
      }
      else if (result.contains("[\"inspect_session\",")) value = report("s" + session,session>1,session==1?1:0,session>sessions?50:100);
      else if (result.contains("[\"apply\",")) { value = "null"; applied++; }
      else if (result.contains("[\"restore\",")) { value = "null"; restored++; }
      else throw new AssertionError(result);
      result = client.resume(result,"probe","{\"value\":" + value + "}");
    }
    require(applied == (proposal && !noSession?1:0) && applied == restored,"Deny list was not restored");
    require(keep == Files.exists(root),"Temporary directory cleanup changed");
    return new CliProbe.Answer(result,output.toString());
  }
  static String referenceRun(Path checkout, Path root, int sessions, boolean proposal, boolean noSession, boolean nativeAdapter) throws Exception {
    var reports = new ArrayList<String>();
    var outputs = new ArrayList<String>();
    for (int i=1;i<=sessions+(proposal?1:0);i++) {
      reports.add(report("s" + i,i>1,i==1?1:0,i>sessions?50:100));
      outputs.add(noSession?"[null,\"\",\"missing output\"]":"[\"s" + i + "\",\"one test\",\"\"]");
    }
    var data = "{\"root\":" + q(root) + ",\"repo\":" + q(root.resolve("repo")) + ",\"trap\":" + q(root.resolve("trap")) + ",\"plugin\":" + q(root.resolve("plugin")) + ",\"base\":" + q(root.resolve("store")) + ",\"reports\":[" + String.join(",",reports) + "],\"outputs\":[" + String.join(",",outputs) + "],\"sessions\":" + sessions + "}";
    var prefix = nativeAdapter ? "import native_plugin; native_plugin.pytest_configure(None); " : "";
    return source(checkout,prefix + "import json, sys; from unittest.mock import Mock, patch; from types import SimpleNamespace; from tokeneconomizer.plugin import probe; from tokeneconomizer.plugin.store import Store; f=json.loads(sys.argv[1]); out=[]; patches=patch.multiple(probe, _plugin_root=Mock(return_value=f['plugin']), make_repo=Mock(return_value=(f['repo'],f['trap'])), run_session=Mock(side_effect=f['outputs']), inspect_session=Mock(side_effect=f['reports']), _wait_folded=Mock(return_value=True)); patches.start(); patch.object(probe.shutil,'which',return_value='copilot').start(); patch.object(probe.sidecall,'copilot_model',return_value='local/model').start(); patch.object(probe.denylist,'copilot_trusted_folders',return_value=[]).start(); patch.object(probe.tempfile,'mkdtemp',return_value=f['root']).start(); patch.object(probe.toolcheck,'refresh_catalog',return_value=(None,['a','b'])).start(); patch.object(probe.LiveContext,'build',return_value=SimpleNamespace(store=Store(f['base']))).start(); patch.object(probe.denylist,'apply',return_value=None).start(); patch.object(probe.denylist,'restore',return_value=None).start(); probe.run(sessions=f['sessions'],keep=True,out=out.append); print('\\n'.join(out))",data) + "\n";
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"probe-");
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"),"cli")) {
      helpers(client,checkout,root);
      inspect(client,checkout,root);
      fixture(client,checkout,root);
      for (boolean proposal : List.of(false,true)) {
        var dir = Files.createDirectory(root.resolve("run-" + proposal));
        var actual = run(client,dir,2,proposal,true,false,false);
        ContextProbe.expect(actual.result(),"{\"result\":{\"value\":0}}");
        ContextProbe.expect(actual.output(),referenceRun(checkout,dir,2,proposal,false,false));
        ContextProbe.expect(actual.output(),referenceRun(checkout,dir,2,proposal,false,true));
      }
      var absent = Files.createDirectory(root.resolve("no-session"));
      var actual = run(client,absent,2,true,true,false,true);
      ContextProbe.expect(actual.result(),"{\"result\":{\"value\":1}}");
      ContextProbe.expect(actual.output(),referenceRun(checkout,absent,2,true,true,false));
      ContextProbe.expect(actual.output(),referenceRun(checkout,absent,2,true,true,true));
      var failed = run(client,Files.createDirectory(root.resolve("failed")),1,true,false,true,false);
      require(failed.result().contains("\"raised\":\"RuntimeError\"") && failed.result().contains("session failed"),failed.toString());
      var zero = run(client,Files.createDirectory(root.resolve("zero")),0,false,false,false,false);
      require(zero.result().contains("\"value\":0") && zero.output().contains("with it: 0/0  (the fact never cleared"),zero.toString());
      var unsupported = client.call("cli.cmd_probe","\"args\":{\"host\":\"claude\"}");
      require(unsupported.contains("Copilot only"),unsupported);
      ContextProbe.expect(ContextProbe.value(client.resume(unsupported,"say","{\"value\":null}")),"2");
      System.out.println("Native probe helpers, upstream reports, fixture traps, fold deadlines and deny-list cleanup passed; no model calls");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
