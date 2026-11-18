import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;

/** Owned Docker client and container process adapter for streaming integration checks. */
class SweDockerFixture {
  public static void main(String[] args)throws Exception {
    var root=Path.of(System.getenv("TE_FIXTURE_ROOT"));
    if(!root.startsWith(Path.of(System.getProperty("java.class.path"))))throw new AssertionError("fixture root escaped");
    if(args.length<3||!args[0].equals("exec"))throw new AssertionError("Docker exec required");
    var environment=new LinkedHashMap<String,String>();int at=1;
    while(at<args.length&&args[at].startsWith("-")) {
      switch(args[at++]) {
        case "-i"->{}
        case "-w"->{if(!args[at++].equals("/testbed"))throw new AssertionError("working directory changed");}
        case "-e"->{var pair=args[at++].split("=",2);environment.put(pair[0],pair.length==2?pair[1]:System.getenv(pair[0]));}
        default->throw new AssertionError("unexpected Docker flag");
      }
    }
    if(!args[at++].equals("fixture-container"))throw new AssertionError("container changed");
    var command=args[at++];
    if(command.equals("mkdir")) {
      if(!args[at++].equals("-p"))throw new AssertionError("mkdir arguments changed");
      Files.createDirectories(root.resolve("container").resolve(args[at].replaceFirst("^/","")));return;
    }
    if(command.equals("pkill")) {
      var signal=args[at++];var matcher=args[at++];var agent=args[at++];
      if(!matcher.equals(agent.equals("codex")?"-x":"-f")||at!=args.length)throw new AssertionError("kill matcher changed");
      Files.writeString(root.resolve("signals"),signal+" "+matcher+" "+agent+"\n",StandardOpenOption.CREATE,StandardOpenOption.APPEND);
      var pid=Long.parseLong(Files.readString(root.resolve("agent.pid")));
      var process=ProcessHandle.of(pid);
      if(process.isPresent()&&process.get().isAlive()) {
        var child=process.get();
        if(!child.info().commandLine().orElse("").contains(System.getProperty("java.class.path")))throw new AssertionError("process escaped the fixture");
        if(signal.equals("-KILL"))child.destroyForcibly();else if(signal.equals("-TERM"))child.destroy();else throw new AssertionError("unexpected signal");
      }
      return;
    }
    if(command.equals("bash")) {
      if(!args[at++].equals("-lc"))throw new AssertionError("shell arguments changed");
      var script=args[at];
      if(script.contains(".claude/projects"))System.out.write(Files.readAllBytes(root.resolve("home/.claude/projects/fixture/session.jsonl")));
      else if(script.startsWith("cat ")&&script.contains("/*.log")) {
        var logs=root.resolve("container").resolve(script.substring(4,script.indexOf("/*.log")).replaceFirst("^/",""));
        try(var files=Files.list(logs)){for(var file:files.sorted().toList())if(file.toString().endsWith(".log"))System.out.write(Files.readAllBytes(file));}
      }else throw new AssertionError("unexpected container script: "+script);
      return;
    }
    var agent=System.getenv("TE_FIXTURE_AGENT");var mode=System.getenv("TE_FIXTURE_MODE");
    if(!Path.of(command).getFileName().toString().equals(agent))throw new AssertionError("agent executable changed: "+command);
    if(Arrays.stream(args).anyMatch(arg->arg.contains("fixture-private-key")))throw new AssertionError("secret reached argv");
    if(agent.equals("codex")&&mode.equals("config_error")){System.err.println("unknown configuration field fixture");System.exit(2);}
    var cli=new ArrayList<>(List.of(Path.of(System.getProperty("java.home"),"bin/java").toString(),"-cp",System.getProperty("java.class.path"),"WebAgentFixture"));
    cli.addAll(Arrays.asList(args).subList(at,args.length));
    if(agent.equals("copilot")) {
      int log=cli.indexOf("--log-dir")+1;
      var destination=root.resolve("container").resolve(cli.get(log).replaceFirst("^/",""));Files.createDirectories(destination);cli.set(log,destination.toString());
    }
    var builder=new ProcessBuilder(cli).directory(root.resolve("workspace").toFile());
    builder.environment().clear();
    for(var entry:environment.entrySet())if(entry.getValue()!=null)builder.environment().put(entry.getKey(),entry.getValue());
    builder.environment().put("TE_FIXTURE_AGENT",agent);builder.environment().put("TE_FIXTURE_MODE",mode);builder.environment().put("HOME",root.resolve("home").toString());
    if(agent.equals("claude")&&!builder.environment().get("ANTHROPIC_API_KEY").equals("fixture-private-key"))throw new AssertionError("Claude credential forwarding changed");
    if(agent.equals("codex")&&!builder.environment().getOrDefault("OPENAI_API_KEY","").equals("fixture-private-key"))throw new AssertionError("Codex credential forwarding changed");
    var child=builder.start();Files.writeString(root.resolve("agent.pid"),Long.toString(child.pid()));
    try {
      child.getOutputStream().write(System.in.readAllBytes());child.getOutputStream().close();
      child.getInputStream().transferTo(System.out);child.getErrorStream().transferTo(System.err);
      int code=child.waitFor();System.exit(code);
    }finally{if(child.isAlive())child.destroyForcibly();}
  }
}
