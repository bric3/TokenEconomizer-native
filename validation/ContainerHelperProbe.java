//JAVA 25

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.TimeUnit;

/** Runs native helper operations in a scratch image with no Python, shell or network. */
class ContainerHelperProbe {
  record Output(int code,String out,String error){}
  static Output command(Path root,List<String> args,byte[] input)throws Exception {
    var output=Files.createTempFile(root,"stdout-",".log");var error=Files.createTempFile(root,"stderr-",".log");
    var process=new ProcessBuilder(args).redirectOutput(output.toFile()).redirectError(error.toFile()).start();
    try {
      try(var stdin=process.getOutputStream()){if(input!=null)stdin.write(input);}
      if(!process.waitFor(60,TimeUnit.SECONDS))throw new AssertionError("command timed out: "+args);
      return new Output(process.exitValue(),Files.readString(output),Files.readString(error));
    }finally{if(process.isAlive()){process.destroyForcibly();process.waitFor(5,TimeUnit.SECONDS);}}
  }
  static Output run(Path root,Path data,String image,List<String> args,String input)throws Exception {
    var name="te-helper-probe-"+UUID.randomUUID().toString().substring(0,8);
    var command=new ArrayList<>(List.of("docker","run","--name",name,"--rm","-i","--network","none","--read-only","-v",data+":/data","-e","TE_NET_GUARD_LOG=/data/guard.log",image));command.addAll(args);
    try{return command(root,command,input==null?null:input.getBytes(StandardCharsets.UTF_8));}
    finally{new ProcessBuilder("docker","rm","-f",name).redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(ProcessBuilder.Redirect.DISCARD).start().waitFor();}
  }
  public static void main(String[] args)throws Exception {
    var architectures=args.length==0?List.of("arm64","amd64"):List.of(args);
    var root=Files.createTempDirectory(Path.of("validation/results"),"container-helper-").toAbsolutePath();boolean passed=false;int checks=0;
    try {
      for(var architecture:architectures) {
        if(!List.of("arm64","amd64").contains(architecture))throw new IllegalArgumentException("expected arm64 or amd64");
        var helper=Path.of("target/container-helpers",architecture,"te-net-guard");
        var digest=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(helper)));
        if(!Files.readString(helper.resolveSibling("manifest.json")).contains("\"sha256\": \""+digest+"\""))throw new AssertionError("helper manifest hash differs");
        if(!Files.readString(helper.resolveSibling("THIRD-PARTY-NOTICES.txt")).contains("Third-party notices for TokenEconomizer native"))throw new AssertionError("helper notices are missing");
        var context=Files.createDirectory(root.resolve(architecture));Files.copy(helper,context.resolve("te-net-guard"));
        if(!context.resolve("te-net-guard").toFile().setExecutable(true))throw new AssertionError("helper is not executable");
        Files.writeString(context.resolve("Dockerfile"),"FROM scratch\nCOPY te-net-guard /te-net-guard\nENTRYPOINT [\"/te-net-guard\"]\n");
        var image="te-native-helper-probe:"+UUID.randomUUID().toString().substring(0,8);
        try {
          var build=command(root,List.of("docker","build","--network","none","--platform","linux/"+architecture,"-t",image,context.toString()),null);
          if(build.code()!=0)throw new AssertionError("scratch image build failed: "+build.error());
          var data=Files.createDirectory(context.resolve("data"));
          var recorded=run(root,data,image,List.of("--catalog-recorder","/data/tools.json","https://api.fixture///"),"{\"op\":\"request\",\"method\":\"POST\",\"path\":\"/v1/messages\",\"document\":\"{\\\"tools\\\":[{\\\"name\\\":\\\"Write\\\",\\\"description\\\":\\\" write é😀 \\\"}]}\",\"headers\":[[\"authorization\",\"dummy\"]]}\n");
          if(recorded.code()!=0||!recorded.out().contains("https://api.fixture/v1/messages")||recorded.out().contains("dummy")||!Files.readString(data.resolve("tools.json")).equals("{\"Write\": \"write \\u00e9\\ud83d\\ude00\"}"))throw new AssertionError("catalogue recorder policy failed in scratch: "+recorded);checks++;
          Files.writeString(data.resolve("settings.json"),"{\"keep\":1,\"nested\":{\"old\":1}}");Files.writeString(data.resolve("patch.json"),"{\"nested\":{\"added\":2}}");
          var merged=run(root,data,image,List.of("--merge-json","/data/settings.json","/data/patch.json"),null);
          if(merged.code()!=0||!merged.out().equals("merged\n"))throw new AssertionError("merge failed: "+merged);
          var settings=Files.readString(data.resolve("settings.json"));
          if(!settings.contains("\"keep\": 1")||!settings.contains("\"old\": 1")||!settings.contains("\"added\": 2"))throw new AssertionError("merge lost existing values");checks++;
          for(var agent:List.of("claude","copilot","codex")) {
            var file=data.resolve(agent+".json");Files.writeString(file,"{\"other\":1,\"hooks\":{\"PreToolUse\":[{\"command\":\"rtk-rewrite\"}],\"preToolUse\":[{\"command\":\"rtk-rewrite\"}]}}");
            for(int repeat=0;repeat<2;repeat++) {
              var installed=run(root,data,image,List.of("--install-hook",agent,"/data/"+agent+".json","/te-net-guard"),null);
              if(installed.code()!=0||!installed.out().equals("armed\n"))throw new AssertionError("hook install failed: "+installed);
              var content=Files.readString(file);
              if(!content.contains("rtk-rewrite")||!content.contains("\"other\": 1")||content.split("\"command\": \"/te-net-guard\"",-1).length!=2)throw new AssertionError("hook merge lost data or appended twice");checks++;
            }
          }
          for(boolean marked:List.of(false,true)) {
            var file=data.resolve("entry-"+marked+".json");Files.writeString(file,"{\"keep\":1,\"hooks\":{\"PreToolUse\":[{\"command\":\"rtk-rewrite\"}]}}");
            var value=marked?"net_guard "+"x".repeat(200_000):"other hook";
            for(int repeat=0;repeat<2;repeat++) {
              var installed=run(root,data,image,List.of("--install-entry","PreToolUse","/data/"+file.getFileName()),"{\"command\":\""+value+"\"}");
              if(installed.code()!=0||!installed.out().equals("armed\n"))throw new AssertionError("entry install failed: "+installed);
              var content=Files.readString(file);int expected=marked?1:repeat+1;
              if(!content.contains("rtk-rewrite")||!content.contains("\"keep\": 1")||content.split(marked?"net_guard ":"other hook",-1).length!=expected+1)throw new AssertionError("entry merge changed source idempotence or lost data");checks++;
            }
          }
          var blocked=run(root,data,image,List.of(),"{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"echo https://github.com/probe\"}}");
          if(blocked.code()!=2||!blocked.error().contains("Blocked"))throw new AssertionError("guard did not refuse: "+blocked);checks++;
          var allowed=run(root,data,image,List.of(),"{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"echo ordinary\"}}");
          if(allowed.code()!=0||!allowed.out().isEmpty())throw new AssertionError("guard refused a control command: "+allowed);
          var log=Files.readAllLines(data.resolve("guard.log"));if(log.size()!=1||!log.getFirst().contains("github.com"))throw new AssertionError("guard telemetry differs");checks++;
        }finally{new ProcessBuilder("docker","image","rm","-f",image).redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(ProcessBuilder.Redirect.DISCARD).start().waitFor();}
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Native Linux helpers passed "+checks+" scratch-container checks across "+architectures+"; merge, hook preservation/idempotence, refusal and telemetry ran without Python, a shell or network");
  }
}
