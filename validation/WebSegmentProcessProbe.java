//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.TimeUnit;
import javax.tools.ToolProvider;

/** Exercises real process groups and pipes with owned offline agent executables. */
class WebSegmentProcessProbe {
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"web-segment-process-").toAbsolutePath();boolean passed=false;
    try {
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),"validation/WebAgentFixture.java")!=0)throw new AssertionError("fixture compilation failed");
      var binary=root.resolve("agent");
      Files.writeString(binary,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' WebAgentFixture \"$@\"\n");
      if(!binary.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      for(var agent:List.of("claude","copilot","codex"))for(var mode:agent.equals("claude")?List.of("normal","guard","timeout"):agent.equals("copilot")?List.of("normal","guard","timeout","config_error"):List.of("normal","guard","timeout","config_error","resume"))for(boolean nativeCode:List.of(false,true)) {
        var directory=Files.createDirectories(root.resolve((nativeCode?"native-":"source-")+agent+"-"+mode));
        Files.createDirectory(directory.resolve("workspace"));
        var command=new ArrayList<>(List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),"validation/web_segment_process_host.py",binary.toString(),directory.toString()));
        if(nativeCode)command.add("--native");
        var builder=new ProcessBuilder(command).redirectOutput(directory.resolve("stdout").toFile()).redirectError(directory.resolve("stderr").toFile());
        builder.environment().put("TE_FIXTURE_AGENT",agent);builder.environment().put("TE_FIXTURE_MODE",mode);
        var process=builder.start();
        try {
          if(!process.waitFor(12,TimeUnit.SECONDS))throw new AssertionError("segment timed out: "+directory);
          if(process.exitValue()!=0)throw new AssertionError("segment failed: "+directory+"\n"+Files.readString(directory.resolve("stderr")));
          if(!Files.readString(directory.resolve("stdout")).startsWith("Owned Web-Bench segment passed:"))throw new AssertionError("result missing: "+directory);
        } finally {if(process.isAlive()){process.destroyForcibly();process.waitFor(5,TimeUnit.SECONDS);}}
      }
      passed=true;
    } finally {if(passed)try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Source and native Web-Bench segments passed 24 owned process checks; invocation flags, resumes, configuration refusals, streaming, usage, guard stops, deadlines and process cleanup passed across three agents");
  }
}
