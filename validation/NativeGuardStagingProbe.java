//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java ContainerHelperProbe.java

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.UUID;

/** Exercises the normal SWE-bench installer through its native process binding. */
class NativeGuardStagingProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static String install(AdapterProbe.Client client,String fields,boolean expected)throws Exception {
    var result=client.call("{\"mode\":\"install\""+(fields.isEmpty()?"":","+fields)+"}");
    if(!result.contains("\"value\":"+expected)||result.contains("net_guard.py")||result.contains("command -v python"))throw new AssertionError("native installer result: "+result);
    if(!expected&&!fields.contains("\"allow\"")&&!result.contains("row runs UNGUARDED"))throw new AssertionError("failed installation was not logged: "+result);
    checks++;return result;
  }
  static void artifact(AdapterProbe.Client client,Path root,byte[] binary,String manifest,String diagnostic)throws Exception {
    var directory=Files.createDirectories(root.resolve("target/container-helpers/arm64"));
    Files.write(directory.resolve("te-net-guard"),binary);
    var hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(binary));
    Files.writeString(directory.resolve("manifest.json"),manifest.replace("HASH",hash));
    var result=install(client,"\"artifact_root\":"+q(root),false);
    if(!result.contains(diagnostic)||result.contains("\"run\""))throw new AssertionError("invalid artifact reached Docker copy: "+result);
  }
  static ContainerHelperProbe.Output docker(Path root,List<String> args,String input)throws Exception {
    var command=new java.util.ArrayList<>(List.of("docker"));command.addAll(args);
    return ContainerHelperProbe.command(root,command,input==null?null:input.getBytes(java.nio.charset.StandardCharsets.UTF_8));
  }
  static String ok(ContainerHelperProbe.Output result)throws Exception {
    if(result.code()!=0)throw new AssertionError("Docker failed: "+result);return result.out();
  }
  static void live(AdapterProbe.Client client,Path root)throws Exception {
    var name="te-native-staging-"+UUID.randomUUID().toString().substring(0,8);
    try {
      ok(docker(root,List.of("run","--rm","-d","--network","none","--name",name,"eclipse-temurin:25-jdk-noble","sleep","240"),null));
      var python=docker(root,List.of("exec",name,"bash","-lc","command -v python3 || command -v python"),null);
      if(python.code()!=1)throw new AssertionError("fixture image must have no Python: "+python);
      var architecture=ok(docker(root,List.of("exec",name,"uname","-m"),null)).strip();
      for(var agent:List.of("claude","copilot","codex")) {
        var home="/tmp/guard "+agent+" '$(not-run)";
        var hook="'"+(home+"/te-net-guard").replace("'","'\"'\"'")+"'";
        var settings=agent.equals("codex")?"/opt/codex-home/hooks.json":home+(agent.equals("claude")?"/.claude/settings.json":"/.copilot/settings.json");
        var before="{\"keep\":1,\"hooks\":{\"PreToolUse\":[{\"command\":\"rtk-rewrite\"}],\"preToolUse\":[{\"command\":\"rtk-rewrite\"}]}}";
        ok(docker(root,List.of("exec",name,"mkdir","-p",Path.of(settings).getParent().toString()),null));
        ok(docker(root,List.of("exec","-i",name,"tee",settings),before));
        String first=null;
        for(int repeat=0;repeat<2;repeat++) {
          var result=install(client,"\"agent\":"+q(agent)+",\"name\":"+q(name)+",\"home\":"+q(home)+",\"live\":true",true);
          if(agent.equals("codex")&&!result.contains("\"hook_trust\":"+q(home+"/te-net-guard")))throw new AssertionError("Codex hook trust was not enabled: "+result);
          var content=ok(docker(root,List.of("exec",name,"cat",settings),null));
          if(!content.contains("rtk-rewrite")||!content.contains("\"keep\": 1")||content.split("te-net-guard",-1).length!=2)throw new AssertionError("hook preservation: "+content);
          if(!content.contains("\"command\": "+q(hook)))throw new AssertionError("hook command quoting differs: "+content);
          if(first!=null&&!first.equals(content))throw new AssertionError("reinstallation changed hook settings");first=content;
          checks++;
        }
        var blocked=docker(root,List.of("exec","-i","-e","TE_NET_GUARD_LOG="+home+"/blocks.jsonl",name,"bash","-lc",hook),"{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"echo https://github.com/probe\"}}");
        if(blocked.code()!=2||!blocked.error().contains("Blocked"))throw new AssertionError("installed guard did not refuse: "+blocked);
        var allowed=docker(root,List.of("exec","-i",name,home+"/te-net-guard"),"{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"echo ordinary\"}}");
        if(allowed.code()!=0)throw new AssertionError("installed guard refused a control command");
        var log=ok(docker(root,List.of("exec",name,"cat",home+"/blocks.jsonl"),null));
        if(log.lines().count()!=1||!log.contains("github.com"))throw new AssertionError("installed guard telemetry differs: "+log);checks++;
      }
      System.out.println("Normal installer ran in a real "+architecture+" task container with no Python or network");
    }finally{docker(root,List.of("rm","-f",name),null);}
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"native-guard-staging-").toAbsolutePath();boolean passed=false;
    try(var client=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"guard_install_host.py")) {
      for(var agent:List.of("claude","copilot","codex"))for(var arch:List.of("aarch64","arm64","x86_64","amd64")) {
        var result=install(client,"\"agent\":"+q(agent)+",\"responses\":[{\"stdout\":"+q(arch+"\n")+"}]",true);
        if(!result.contains("--install-hook "+agent)||!result.contains("te-net-guard"))throw new AssertionError(result);
      }
      for(var allow:List.of("1","true","yes")) {
        var result=install(client,"\"allow\":"+q(allow),false);
        if(result.contains("\"exec\"")||result.contains("\"run\""))throw new AssertionError("network escape hatch touched Docker");
      }
      for(int index=0;index<4;index++)for(var kind:List.of("RuntimeError","OSError","ValueError"))install(client,"\"fail_at\":"+index+",\"error\":"+q(kind),false);
      for(var kind:List.of("KeyboardInterrupt","SystemExit")) {
        var result=client.call("{\"mode\":\"install\",\"fail_at\":0,\"error\":"+q(kind)+"}");
        if(!result.contains("\"raised\":"+q(kind)))throw new AssertionError("process control was swallowed: "+result);checks++;
      }
      for(var responses:List.of("[{\"stdout\":\"riscv64\"}]","[{\"returncode\":1}]","[{}, {\"returncode\":1}]","[{},{},{\"returncode\":1}]","[{},{},{\"stdout\":\"not armed\"}]","[{},{},{},{\"stdout\":\"rc=20\"}]","[{},{},{},{\"returncode\":1}]"))install(client,"\"responses\":"+responses,false);
      install(client,"\"copy_response\":{\"returncode\":1,\"stderr\":\"copy failed\"}",false);
      var original=Files.readAllBytes(Path.of("target/container-helpers/arm64/te-net-guard"));
      var manifest="{\"platform\":\"linux/arm64\",\"binary\":\"te-net-guard\",\"sha256\":\"HASH\"}";
      artifact(client,root,original,manifest.replace("HASH","bad"),"manifest does not match");
      artifact(client,root,new byte[32],manifest,"ELF64");
      var bytes=original.clone();bytes[18]=62;bytes[19]=0;artifact(client,root,bytes,manifest,"ELF64");
      bytes=original.clone();var elf=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);elf.putInt(Math.toIntExact(elf.getLong(32)),3);artifact(client,root,bytes,manifest,"dynamic interpreter");
      bytes=original.clone();ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).putLong(32,Long.MAX_VALUE);artifact(client,root,bytes,manifest,"program headers");
      artifact(client,root,original,"{", "invalid container helper manifest");
      Files.delete(root.resolve("target/container-helpers/arm64/manifest.json"));install(client,"\"artifact_root\":"+q(root),false);
      if(List.of(args).contains("--docker"))live(client,root);
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Normal native guard staging passed "+checks+" checks; architecture selection, artifact validation, failure reporting and Codex trust passed");
  }
}
