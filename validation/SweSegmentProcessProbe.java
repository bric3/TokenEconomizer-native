//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.TimeUnit;
import javax.tools.ToolProvider;

/** Runs Docker client and container agent fixtures as separate owned processes. */
class SweSegmentProcessProbe {
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"swe-segment-process-").toAbsolutePath();boolean passed=false;
    try {
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),"validation/SweDockerFixture.java","validation/WebAgentFixture.java")!=0)throw new AssertionError("fixture compilation failed");
      var docker=root.resolve("docker");
      Files.writeString(docker,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' SweDockerFixture \"$@\"\n");
      if(!docker.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      for(var agent:List.of("claude","copilot","codex"))for(var mode:agent.equals("claude")?List.of("normal","guard","timeout"):agent.equals("copilot")?List.of("normal","guard","timeout","config_error"):List.of("normal","guard","timeout","config_error","resume"))for(boolean nativeCode:List.of(false,true)) {
        var directory=Files.createDirectories(root.resolve((nativeCode?"native-":"source-")+agent+"-"+mode));
        Files.createDirectories(directory.resolve("workspace/src"));Files.writeString(directory.resolve("workspace/src/a.js"),"initial");
        var command=new ArrayList<>(List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),"validation/swe_segment_process_host.py",directory.toString()));if(nativeCode)command.add("--native");
        if(nativeCode&&List.of(args).contains("--sdk"))command.add("--sdk");
        var builder=new ProcessBuilder(command).redirectOutput(directory.resolve("stdout").toFile()).redirectError(directory.resolve("stderr").toFile());
        builder.environment().put("PATH",root+System.getProperty("path.separator")+builder.environment().get("PATH"));
        builder.environment().put("TE_FIXTURE_ROOT",directory.toString());builder.environment().put("TE_FIXTURE_AGENT",agent);builder.environment().put("TE_FIXTURE_MODE",mode);
        var process=builder.start();
        try {
          if(!process.waitFor(15,TimeUnit.SECONDS))throw new AssertionError("segment timed out: "+directory);
          if(process.exitValue()!=0)throw new AssertionError("segment failed: "+directory+"\n"+Files.readString(directory.resolve("stderr")));
          if(!Files.readString(directory.resolve("stdout")).startsWith("Owned SWE-bench segment passed:"))throw new AssertionError("result missing: "+directory);
          var pid=directory.resolve("agent.pid");
          if(Files.exists(pid)&&ProcessHandle.of(Long.parseLong(Files.readString(pid))).filter(ProcessHandle::isAlive).isPresent())throw new AssertionError("agent survived the segment: "+directory);
        }finally {
          if(process.isAlive()){process.destroyForcibly();process.waitFor(5,TimeUnit.SECONDS);}
          var pid=directory.resolve("agent.pid");
          if(Files.exists(pid)){var child=ProcessHandle.of(Long.parseLong(Files.readString(pid)));if(child.isPresent()&&child.get().isAlive()&&child.get().info().commandLine().orElse("").contains(root.toString()))child.get().destroyForcibly();}
        }
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Source and native SWE-bench segments passed 24 owned Docker-client/agent process checks; command flags, resumes, refusals, streams, accounting, guard stops, deadlines and internal agent termination passed without a model or Docker daemon");
  }
}
