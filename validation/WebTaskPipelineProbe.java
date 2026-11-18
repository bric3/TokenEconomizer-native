//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.TimeUnit;
import javax.tools.ToolProvider;

/** Runs one source and one native task against an owned offline HTTP evaluator. */
class WebTaskPipelineProbe {
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"web-task-pipeline-").toAbsolutePath();boolean passed=false;
    try {
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),"validation/WebEvaluatorFixture.java","validation/WebAgentFixture.java")!=0)throw new AssertionError("fixture compilation failed");
      var project=Files.createDirectories(root.resolve("repo/projects/project"));
      Files.writeString(project.resolve("tasks.jsonl"),"{\"id\":\"init\"}\n");
      Files.createDirectories(root.resolve("repo/apps/eval/dist"));Files.writeString(root.resolve("repo/apps/eval/dist/index.js"),"");
      var binary=root.resolve("node");
      Files.writeString(binary,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' WebEvaluatorFixture \"$@\"\n");
      if(!binary.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      var agentBinary=root.resolve("agent");
      Files.writeString(agentBinary,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' WebAgentFixture \"$@\"\n");
      if(!agentBinary.toFile().setExecutable(true))throw new AssertionError("agent fixture is not executable");
      for(var agent:List.of("claude","copilot","codex"))for(boolean nativeCode:List.of(false,true)) {
        var directory=Files.createDirectories(root.resolve((nativeCode?"native-":"source-")+agent));
        var command=new ArrayList<>(List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),"validation/web_task_pipeline_host.py",binary.toString(),root.resolve("repo").toString(),directory.resolve("run").toString()));
        if(nativeCode)command.add("--native");
        var builder=new ProcessBuilder(command).redirectOutput(directory.resolve("stdout").toFile()).redirectError(directory.resolve("stderr").toFile());
        builder.environment().put("TE_FIXTURE_TASK_LOOP","1");
        builder.environment().put("TE_FIXTURE_AGENT",agent);
        builder.environment().put("TE_FIXTURE_AGENT_BIN",agentBinary.toString());
        builder.environment().put("TE_FIXTURE_MODE","normal");
        var process=builder.start();
        try {
          if(!process.waitFor(20,TimeUnit.SECONDS))throw new AssertionError("offline task timed out: "+directory);
          if(process.exitValue()!=0)throw new AssertionError("offline task failed: "+Files.readString(directory.resolve("stderr")));
          var output=Files.readString(directory.resolve("stdout"));
          if(!output.startsWith("Offline Web-Bench task passed:"))throw new AssertionError("pipeline result missing: "+output);
        } finally {if(process.isAlive()){process.destroyForcibly();process.waitFor(5,TimeUnit.SECONDS);}}
      }
      passed=true;
    } finally {if(passed)try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Source and native Web-Bench tasks passed 6 offline HTTP/process pipelines across three agents; evaluator startup, real queues, agent segments, workspace round trip, usage accounting, wrappers, verdicts and cleanup passed without a model or Playwright");
  }
}
