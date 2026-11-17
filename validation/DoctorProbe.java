//JAVA 25
//SOURCES CliProbe.java ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.StringJoiner;

/** Compares native installation diagnostics and URL guards with unchanged upstream code. */
class DoctorProbe {
  static String quote(Object value) { return CliProbe.quote(value); }
  static void require(boolean condition, String message) { CliProbe.require(condition,message); }
  static String object(Map<String,String> values) {
    var out = new StringJoiner(",","{","}");
    values.forEach((key,value) -> out.add(quote(key) + ":" + quote(value)));
    return out.toString();
  }
  static String environment(Path root, Map<String,String> env) {
    return "\"process_env\":" + object(env) + ",\"home\":" + quote(root) + ",\"process_cwd\":" + quote(root);
  }
  static CliProbe.Answer doctor(ContextProbe.Client client, Path root, Path repo, String ctx, Map<String,String> env, Set<String> agents, String info) throws Exception {
    var output = new StringBuilder();
    var result = client.call("cli.cmd_doctor","\"args\":{\"host\":\"claude\",\"cwd\":" + quote(repo) + "}," + environment(root,env));
    while (result.contains("\"callback\":\"cli\"")) {
      String method, value;
      if (result.contains("\"method\":\"context\"")) { method = "context"; value = ctx; }
      else if (result.contains("\"method\":\"say\"")) { method = "say"; output.append(CliProbe.stringArgument(result)).append('\n'); value = "null"; }
      else if (result.contains("\"method\":\"global\"")) {
        method = "global";
        if (result.contains("[\"runtime\",[")) value = info;
        else if (result.contains("[\"state_root\",[")) value = quote(env.get("TOKENECONOMIZER_HOME"));
        else if (result.contains("[\"claude_cli\",[")) value = "\"claude\"";
        else if (result.contains("[\"hermes_cli\",[")) value = "\"hermes\"";
        else if (result.contains("[\"which\",[")) {
          value = "null";
          for (String name : agents) if (result.contains("[" + quote(name) + "]")) value = quote(Path.of(env.get("PATH")).resolve(name));
        } else throw new AssertionError(result);
      } else {
        method = "service";
        if (result.contains(",\"codex_trusted\",[") || result.contains(",\"copilot_folder_trusted\",[")) value = "false";
        else if (result.contains(",\"repomap_python\",[")) value = "null";
        else throw new AssertionError(result);
      }
      result = client.resume(result,method,"{\"value\":" + value + "}");
    }
    return new CliProbe.Answer(result,output.toString());
  }
  static String source(Path checkout, Map<String,String> env, String... args) throws Exception {
    var command = new ArrayList<>(List.of(checkout.resolve(".venv/bin/python").toString()));
    command.addAll(List.of(args));
    var builder = new ProcessBuilder(command).directory(Path.of(env.get("HOME")).toFile());
    builder.environment().clear(); builder.environment().putAll(env);
    var process = builder.start();
    var out = new String(process.getInputStream().readAllBytes(),StandardCharsets.UTF_8);
    var error = new String(process.getErrorStream().readAllBytes(),StandardCharsets.UTF_8);
    int code = process.waitFor();
    return code == 0 ? out : "ERROR:" + error.lines().reduce((a,b) -> b).orElse("");
  }
  static void unicodeGuards(ContextProbe.Client client, Path root, Path checkout) throws Exception {
    var input = root.resolve("unicode-input");
    var output = root.resolve("unicode-output");
    var text = new StringBuilder();
    for (int cp=1; cp<=Character.MAX_CODE_POINT; cp++) {
      if (cp>=0xd800 && cp<=0xdfff) continue;
      text.append(Integer.toHexString(cp)).append('\t').appendCodePoint(cp).append('\0');
    }
    Files.writeString(input,text);
    var process = new ProcessBuilder(checkout.resolve(".venv/bin/python").toString(),"-c","import sys, unicodedata; sys.stdout.write(unicodedata.normalize('NFKC', sys.stdin.read()))")
        .redirectInput(input.toFile()).redirectOutput(output.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
    require(process.waitFor() == 0,"Unicode reference call failed");
    int guarded = 0;
    for (var record : Files.readString(output).split("\0")) {
      var parts = record.split("\t",2);
      int cp = Integer.parseInt(parts[0],16);
      if (cp>127 && parts[1].chars().anyMatch(c -> "/?#@:".indexOf(c)>=0)) {
        var result = client.call("cli.netloc","\"url\":" + quote("https://a" + Character.toString(cp) + "b/path"));
        require(result.contains("\"raised\":\"ValueError\"") && result.contains("NFKC"),result);
        guarded++;
      }
    }
    require(guarded == 19,"Unicode authority delimiter set changed: " + guarded);
    Files.delete(input); Files.delete(output);
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"doctor-");
    var repo = Files.createDirectory(root.resolve("repo"));
    var state = Files.createDirectory(root.resolve("state"));
    var bin = Files.createDirectory(root.resolve("bin"));
    var executable = checkout.resolve("target/debug/te-bridge");
    var agents = Set.of("agy","claude","codex","copilot","droid","grok","hermes","opencode","pi","rtk");
    var env = new HashMap<>(Map.of("TOKENECONOMIZER_HOME",state.toString(),"HOME",root.toString(),"PATH",bin.toString(),
        "PYTHONPATH",checkout.resolve("reference/upstream").toString(),"GIT_CEILING_DIRECTORIES",root.toString(),
        "COPILOT_PROVIDER_BASE_URL","https://[::1]:7331/v1","COPILOT_PROVIDER_TYPE","anthropic","TE_DROID_SIDECALL","1"));
    if (System.getenv("TZ") != null) env.put("TZ",System.getenv("TZ"));
    try (var client = new ContextProbe.Client(executable,"cli")) {
      for (String agent : agents) {
        var command = bin.resolve(agent);
        Files.writeString(command,"#!/bin/sh\nexit 0\n");
        Files.setPosixFilePermissions(command,PosixFilePermissions.fromString("rwx------"));
      }
      var version = source(checkout,env,"--version").strip().replace("Python ","");
      var parts = version.split("\\.");
      var info = "{\"name\":\"python\",\"version\":" + quote(version) + ",\"parts\":[" + parts[0] + "," + parts[1] + "],\"minimum\":[3,10],\"required\":\"3.10\"}";
      var ctx = CliProbe.context(executable,repo,state,env);
      Files.createDirectories(root.resolve(".claude"));
      Files.writeString(root.resolve(".claude/settings.json"),"{\"command\":\"rtk rewrite\"}");
      Files.createDirectories(root.resolve(".copilot/hooks"));
      Files.writeString(root.resolve(".copilot/hooks/b.json"),"rtk rewrite");
      Files.writeString(root.resolve(".copilot/hooks/.hidden.json"),"rtk rewrite");
      Files.createDirectory(root.resolve(".copilot/hooks/directory.json"));
      Files.writeString(root.resolve(".copilot/config.json"),"{\"trustedFolders\":[]}");
      Files.createDirectories(root.resolve(".codex"));
      Files.writeString(root.resolve(".codex/config.toml"),"[projects." + quote(repo) + "]\ntrust_level = \"untrusted\"\n");
      env.put("HERMES_HOME","relative-hermes");
      Files.createDirectories(root.resolve("relative-hermes/plugins/tokeneconomizer"));
      Files.createDirectories(root.resolve(".pi/agent"));
      Files.writeString(root.resolve(".pi/agent/settings.json"),"{\"packages\":[{\"source\":\"TOKENECONOMIZER\"}]}");
      Files.createDirectories(state.resolve("hosts/claude"));
      Files.writeString(state.resolve("hosts/claude/catalog.json"),"{}");
      Files.writeString(state.resolve("hosts/claude/sidecall.json"),"{\"at\":1700000000,\"reason\":\"private detail\"}");
      var log = "zero\r\none\rtwo\nthree\u0085four\u2028five\u2029six\nseven\u001deight é😀\n".getBytes(StandardCharsets.UTF_8);
      log = java.util.Arrays.copyOf(log,log.length+1); log[log.length-1] = (byte)0xff;
      Files.write(state.resolve("errors.log"),log);
      var actual = doctor(client,root,repo,ctx,env,agents,info);
      ContextProbe.expect(actual.result(),"{\"result\":{\"value\":0}}");
      ContextProbe.expect(actual.output(),source(checkout,env,"-m","tokeneconomizer.plugin.cli","--host","claude","--cwd",repo.toString(),"doctor"));
      require(!actual.output().contains("private detail") && actual.output().contains("BYOK anthropic at [::1]:7331"),actual.output());
      Files.writeString(root.resolve(".pi/agent/settings.json"),"[1,2]");
      Files.writeString(state.resolve("config.json"),"{\"droid_sidecall\":true}");
      var configured = CliProbe.context(executable,repo,state,env);
      var settings = doctor(client,root,repo,configured,env,agents,info);
      ContextProbe.expect(settings.output(),source(checkout,env,"-m","tokeneconomizer.plugin.cli","--host","claude","--cwd",repo.toString(),"doctor"));
      require(settings.output().contains("side calls on (the `droid_sidecall` setting)") && settings.output().contains("pi          package not in"),settings.output());
      var emptyBin = Files.createDirectory(root.resolve("empty-bin"));
      env.put("PATH",emptyBin.toString());
      var absent = doctor(client,root,repo,ctx,env,Set.of(),info);
      ContextProbe.expect(absent.output(),source(checkout,env,"-m","tokeneconomizer.plugin.cli","--host","claude","--cwd",repo.toString(),"doctor"));
      var unsupported = doctor(client,root,repo,ctx,env,Set.of(),"{\"name\":\"python\",\"version\":\"2.7.18\",\"parts\":[2,7],\"minimum\":[3,10],\"required\":\"3.10\"}");
      require(unsupported.result().contains("\"value\":1") && unsupported.output().startsWith("python      2.7.18 (needs >= 3.10)"),unsupported.toString());
      var nativeRuntime = doctor(client,root,repo,ctx,env,Set.of(),"{\"name\":\"native\",\"version\":\"0.1.0\",\"parts\":[],\"minimum\":[],\"required\":\"\"}");
      require(nativeRuntime.output().startsWith("native      0.1.0 ok"),nativeRuntime.toString());
      var readOnly = Files.createDirectory(root.resolve("read-only"));
      Files.setPosixFilePermissions(readOnly,PosixFilePermissions.fromString("r-x------"));
      try {
        var blockedEnv = new HashMap<>(env);
        blockedEnv.put("TOKENECONOMIZER_HOME",readOnly.toString());
        var blocked = doctor(client,root,repo,ctx,blockedEnv,Set.of(),info);
        require(blocked.result().contains("\"value\":1") && blocked.output().contains("NOT writable:"),blocked.toString());
      } finally { Files.setPosixFilePermissions(readOnly,PosixFilePermissions.fromString("rwx------")); }

      for (String url : new String[]{"https://example.test:123/v1","//[::1]:80/path"," localhost:123/a","\u0001\tHTTPS://user:pass@example.test\r\n:8/?x","https://é.test/v1","https://[v1.example]:x/path","https://[fe80::1%eth0]/","https://[127.0.0.1]/","https://[broken]/","https://host[::1]/","https://[::1]extra/","https://[vX.future]/","https://bad\uFF1Aname.test/"}) {
        var result = client.call("cli.netloc","\"url\":" + quote(url));
        var expected = source(checkout,env,"-c","import json, sys; from urllib.parse import urlparse; print(json.dumps(urlparse(sys.argv[1]).netloc or '', ensure_ascii=False))",url);
        if (expected.startsWith("ERROR:")) require(result.contains("\"raised\":\"ValueError\""),result + " versus " + expected);
        else ContextProbe.expect(ContextProbe.value(result),expected.strip());
      }
      unicodeGuards(client,root,checkout);

      var recentEnv = new HashMap<>(env);
      var recentRoot = root.resolve("recent-state");
      recentEnv.put("TOKENECONOMIZER_HOME",recentRoot.toString());
      ContextProbe.expect(ContextProbe.value(client.call("cli._recent_host","\"cwd\":" + quote(repo) + "," + environment(root,recentEnv))),"null");
      try (var context = new ContextProbe.Client(executable,"context")) {
        Path codex = null;
        for (String host : new String[]{"codex","claude"}) {
          var data = ContextProbe.value(context.call("context.build","\"host\":" + quote(host) + ",\"cwd\":" + quote(repo) + ",\"env\":" + object(recentEnv) + ",\"home\":" + quote(root) + ",\"settle\":false"));
          var base = Files.createDirectories(Path.of(CliProbe.field(data,"base")));
          if (host.equals("codex")) codex = base;
          Files.writeString(base.resolve("ledger.jsonl"),"");
          Files.setLastModifiedTime(base.resolve("ledger.jsonl"),FileTime.fromMillis(1700000000000L));
          Files.setLastModifiedTime(base,FileTime.fromMillis(1700000000000L));
          var selected = ContextProbe.value(client.call("cli._recent_host","\"cwd\":" + quote(repo) + "," + environment(root,recentEnv)));
          var original = source(checkout,recentEnv,"-c","import json, sys; from tokeneconomizer.plugin import cli; print(json.dumps(cli._recent_host(sys.argv[1])))",repo.toString());
          ContextProbe.expect(selected,original.strip());
          ContextProbe.expect(selected,quote(host));
        }
        Files.createSymbolicLink(codex.resolve("dangling"),root.resolve("absent"));
        var failed = client.call("cli._recent_host","\"cwd\":" + quote(repo) + "," + environment(root,recentEnv));
        require(failed.contains("\"raised\":\"FileNotFoundError\""),failed);
        require(source(checkout,recentEnv,"-c","import sys; from tokeneconomizer.plugin import cli; cli._recent_host(sys.argv[1])",repo.toString()).startsWith("ERROR:FileNotFoundError:"),failed);
      }
      System.out.println("Native doctor matches upstream; host recency, URL parsing, complete Unicode delimiter guard and runtime reporting passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
