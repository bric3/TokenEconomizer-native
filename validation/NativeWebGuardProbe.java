//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Verifies native WebBench hooks through their installed launcher and source file effects. */
class NativeWebGuardProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static String call(AdapterProbe.Client client,String fields,String expected)throws Exception {
    var value=client.call("{"+fields+"}");
    if(!value.contains(expected))throw new AssertionError("native guard result: "+value);checks++;return value;
  }
  static String command(String result){return CliProbe.stringArgument(result.replace("\"guard_command\":","\"args\":"));}
  static String encoded(String value){
    var quoted=q(value);var result=new StringBuilder();
    for(char c:quoted.substring(1,quoted.length()-1).toCharArray()) {
      if(c>=128)result.append(String.format("\\u%04x",(int)c));else result.append(c);
    }
    return result.toString();
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"native-web-guard-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_guard_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_guard_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        for(boolean verified:List.of(false,true))call(nativeCode,"\"agent\":"+q(agent)+",\"verified\":"+verified,"\"value\":"+verified);
        for(int rc:List.of(0,1,2,3,20))call(nativeCode,"\"agent\":"+q(agent)+",\"operation\":\"verify\",\"rc\":"+rc,"\"value\":"+(rc==2));
      }
      call(nativeCode,"\"agent\":\"codex\",\"opaque_armed_get\":true","\"value\":{\"ref\":\"armed\"}");
      for(var phase:List.of("open","enter","write","exit","chmod","run","returncode","remove","log"))for(var kind:List.of("OSError","RuntimeError","KeyboardInterrupt","SystemExit")) {
        boolean raised=List.of("KeyboardInterrupt","SystemExit").contains(kind)||phase.equals("returncode")||phase.equals("log")||phase.equals("remove")&&!kind.equals("OSError");
        var expected=raised?"\"raised\":"+q(kind):"\"value\":"+phase.equals("remove");
        var result=call(nativeCode,"\"operation\":\"verify\",\"fail\":"+q(phase)+",\"error\":"+q(kind)+(phase.equals("log")?",\"rc\":0":""),expected);
        if(!raised&&!phase.equals("remove")&&!result.contains("UNGUARDED"))throw new AssertionError("failed probe was not logged");
      }
      for(var agent:List.of("claude","copilot","codex"))for(var suffix:List.of("plain","quote '$(not-run)","é😀")) {
        var run=Files.createDirectory(root.resolve(agent+"-"+suffix));var home=Files.createDirectory(run.resolve("home"));var copilot=Files.createDirectory(run.resolve("copilot"));
        var path=agent.equals("claude")?home.resolve(".claude/settings.json"):agent.equals("copilot")?copilot.resolve("settings.json"):home.resolve("hooks.json");
        Files.createDirectories(path.getParent());
        var fields="\"agent\":"+q(agent)+",\"home\":"+q(home)+",\"run_dir\":"+q(run)+",\"copilot_home\":"+q(copilot)+",\"real_files\":true,\"real_process\":true,\"inspect\":true";
        for(var before:List.of("not json","{}","{\"keep\":\"é😀\",\"hooks\":{\"PreToolUse\":[{\"command\":\"rtk-rewrite\"}],\"preToolUse\":[{\"command\":\"rtk-rewrite\"}]}}")) {
          Files.writeString(path,before);var expected=call(source,fields,"\"value\":true");var expectedFile=Files.readString(path);
          Files.writeString(path,before);var actual=call(nativeCode,fields,"\"value\":true");var actualFile=Files.readString(path);
          if(!actualFile.replace(encoded(command(actual)),"<guard>").equals(expectedFile.replace(encoded(command(expected)),"<guard>")))throw new AssertionError("settings differ beyond guard command: "+actualFile+"\n"+expectedFile);
          var launcher=home.resolve("net_guard.py");var program=Files.readString(launcher);
          if(!Files.isExecutable(launcher)||!program.startsWith("#!/bin/sh\nexec ")||!program.contains("te-net-guard")||program.contains("python"))throw new AssertionError("launcher does not execute the native guard: "+program);
          if(Files.exists(home.resolve("net_guard_blocks.jsonl")))throw new AssertionError("verification inflated telemetry");checks++;
          call(nativeCode,fields,"\"value\":true");
          if(!Files.readString(path).equals(actualFile))throw new AssertionError("guard installation duplicated its entry");checks++;
        }
        for(boolean blocked:List.of(false,true)) {
          var payload="{\"tool_name\":\"Bash\",\"toolName\":\"bash\",\"tool_input\":{\"command\":"+q(blocked?"echo https://github.com/probe":"echo ordinary")+"},\"toolArgs\":{\"command\":"+q(blocked?"echo https://github.com/probe":"echo ordinary")+"}}";
          call(nativeCode,fields+",\"payload\":"+payload,"\"dispatch_code\":"+(blocked?2:0));
          var log=home.resolve("net_guard_blocks.jsonl");
          if(blocked?Files.readAllLines(log).size()!=1:Files.exists(log))throw new AssertionError("hook telemetry differs");checks++;
        }
      }
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Native WebBench guard wiring passed "+checks+" checks; source settings effects, three agents, executable launchers, quoted paths, idempotence, refusal, telemetry and failure boundaries passed");
  }
}
