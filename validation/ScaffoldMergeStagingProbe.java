//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java ContainerHelperProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.UUID;

/** Runs shared merging through actual WebBench and SWE-bench context callbacks. */
class ScaffoldMergeStagingProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static String request(String domain,String agent,Path directory,String home,String target,String extra) {
    var literal="$(touch '"+(home+"/must-not-run").replace("'","'\"'\"'")+"') `echo unsafe`\nTE_SCAFFOLD_PATCH";
    var patch="{\"nested\":{\"added\":2},\"hooks\":{\"PreToolUse\":[{\"command\":\"rtk-rewrite\"}],\"preToolUse\":[{\"command\":\"rtk-rewrite\"}]},\"literal\":"+q(literal)+"}";
    return "{\"domain\":"+q(domain)+",\"agent\":"+q(agent)+",\"directory\":"+q(directory)+",\"home\":"+q(home)+",\"path\":"+q(target)+",\"patch\":"+patch+extra+"}";
  }
  static void result(String value,String domain)throws Exception {
    if(value.contains("\"raised\"")||!value.contains("\"merged\":true")||!value.contains("\"temporary_files\":[]")||!value.contains("experiment.swebench.provider.scaffold.merge_json"))throw new AssertionError("merge pipeline failed: "+value);
    if(domain.equals("swe")&&(!value.contains("experiment.swebench.provider.swebench.scaffold_helper")||!value.contains("\"guarded\":true")))throw new AssertionError("native deployment was not exercised: "+value);
    checks++;
  }
  static void content(String text,boolean guard)throws Exception {
    if(!text.contains("\"keep\": 1")||!text.contains("\"old\": 1")||!text.contains("\"added\": 2")||!text.contains("rtk-rewrite")||!text.contains("$(touch ")||!text.contains("`echo unsafe`"))throw new AssertionError("merge changed or lost data: "+text);
    if(guard&&text.split("te-net-guard",-1).length!=2)throw new AssertionError("guard replaced the scaffold hook: "+text);checks++;
  }
  static String before="{\"keep\":1,\"nested\":{\"old\":1}}";
  static ContainerHelperProbe.Output docker(Path root,List<String> args,String input)throws Exception {
    var command=new java.util.ArrayList<>(List.of("docker"));command.addAll(args);
    return ContainerHelperProbe.command(root,command,input==null?null:input.getBytes(java.nio.charset.StandardCharsets.UTF_8));
  }
  static String ok(ContainerHelperProbe.Output result)throws Exception {
    if(result.code()!=0)throw new AssertionError("Docker failed: "+result);return result.out();
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"scaffold-merge-staging-").toAbsolutePath();boolean passed=false;
    try(var client=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"native_scaffold_pipeline_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        var directory=Files.createDirectory(root.resolve("web-"+agent));Files.createDirectory(directory.resolve("workspace"));
        var home=Files.createDirectory(directory.resolve("home '$(not-run)"));var path=home.resolve("config/settings 'quoted'.json");
        Files.createDirectories(path.getParent());Files.writeString(path,before);
        var result=client.call(request("web",agent,directory,home.toString(),path.toString(),""));result(result,"web");content(Files.readString(path),false);
        if(Files.exists(home.resolve("must-not-run")))throw new AssertionError("patch data executed as shell code");
      }
      for(var kind:List.of("RuntimeError","OSError","KeyboardInterrupt","SystemExit")) {
        var directory=Files.createDirectory(root.resolve("failure-"+kind));
        var value=client.call(request("swe","claude",directory,"/tmp/helper-failure","/tmp/helper-failure/settings.json",",\"container\":\"unused\",\"helper_error\":"+q(kind)));
        if(kind.equals("KeyboardInterrupt")||kind.equals("SystemExit")) {
          if(!value.contains("\"raised\":"+q(kind)))throw new AssertionError("process control was swallowed: "+value);
        }else if(!value.contains("\"merged\":false")||!value.contains("scaffold merge helper staging FAILED")||value.contains("\"raised\""))throw new AssertionError("helper failure lost the task: "+value);
        checks++;
      }
      if(List.of(args).contains("--docker")) {
        var name="te-scaffold-staging-"+UUID.randomUUID().toString().substring(0,8);
        try {
          ok(docker(root,List.of("run","--rm","-d","--network","none","--name",name,"eclipse-temurin:25-jdk-noble","sleep","240"),null));
          if(docker(root,List.of("exec",name,"bash","-lc","command -v python3 || command -v python"),null).code()!=1)throw new AssertionError("fixture image must have no Python");
          ok(docker(root,List.of("exec",name,"mkdir","-p","/testbed"),null));
          for(var agent:List.of("claude","copilot","codex")) {
            var directory=Files.createDirectory(root.resolve("swe-"+agent));var home="/tmp/home "+agent+" '$(not-run)";
            var path=agent.equals("codex")?"/opt/codex-home/hooks.json":home+(agent.equals("claude")?"/.claude/settings.json":"/.copilot/settings.json");
            ok(docker(root,List.of("exec",name,"mkdir","-p",Path.of(path).getParent().toString()),null));
            ok(docker(root,List.of("exec","-i",name,"tee",path),before));
            var result=client.call(request("swe",agent,directory,home,path,",\"container\":"+q(name)+",\"guard\":true"));result(result,"swe");content(ok(docker(root,List.of("exec",name,"cat",path),null)),true);
            var left=ok(docker(root,List.of("exec",name,"bash","-lc","compgen -G '/tmp/_scaffold_merge_*' || true"),null));
            if(!left.isBlank()||docker(root,List.of("exec",name,"test","-e",home+"/must-not-run"),null).code()!=1)throw new AssertionError("helper cleanup or quoting failed");checks++;
          }
          var directory=Files.createDirectory(root.resolve("retained-profile"));var home="/tmp/retained-profile";var path=home+"/settings.json";
          ok(docker(root,List.of("exec",name,"mkdir","-p",home),null));ok(docker(root,List.of("exec","-i",name,"tee",path),before));
          var retained=client.call(request("swe","claude",directory,home,path,",\"container\":"+q(name)+",\"rebind_profile\":true"));
          if(!retained.contains("\"merged\":true")||retained.contains("\"raised\""))throw new AssertionError("context lost its profile: "+retained);checks++;
          content(ok(docker(root,List.of("exec",name,"cat",path),null)),false);
        }finally{docker(root,List.of("rm","-f",name),null);}
      }
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Native scaffold merging passed "+checks+" harness checks; three WebBench agents and optional Python-free task-container merges preserve hooks, keys, quoting and cleanup");
  }
}
