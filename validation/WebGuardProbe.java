//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Drives network guard setup comparisons against the pinned source. */
class WebGuardProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception{
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"web-guard-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_guard_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_guard_host.py")){
      for(var agent:List.of("claude","copilot","codex"))for(var operation:List.of("blocks","settings","entry","log","class_log","hosts","quote")){
        if(operation.equals("hosts")&&!agent.equals("codex"))continue;
        var base="\"agent\":"+q(agent)+",\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"objects\":[\"home\"]","\"objects\":[\"run_dir_path\"]","\"objects\":[\"command\"]","\"objects\":[\"NET_GUARD_PROBE\"]","\"objects\":[\"s\"]","\"opaque_verified\":true","\"verified\":false","\"rc\":0","\"rc\":[0,3]","\"opaque_rc\":true,\"opaque_comparison\":true","\"opaque_lines\":true","\"opaque_lines\":true,\"opaque_stripped\":true,\"booleans\":{\"stripped\":false}","\"lines\":[]","\"aggregate\":true","\"aggregate\":true,\"skip_generator\":true","\"settings\":null","\"settings\":[]","\"settings\":{\"hooks\":{\"PreToolUse\":[{\"command\":\"net_guard\"},42],\"preToolUse\":[{\"command\":\"net_guard\"},42]}}","\"settings\":{\"hooks\":{\"PreToolUse\":null,\"preToolUse\":null}}"))WebContractProbe.compare(nativeCode,source,base+","+option);
        var phases=switch(operation){
          case "blocks"->List.of("get:_net_guard_log","get:agent_home","agent_home","join","open","enter","file.iter","lines.next","exit");
          case "hosts"->List.of("path.join","join","open","enter","read","exit","net_guard.hosts_block","hosts_block","write");
          default->List.of("path.join","join","get:scaffold_copilot_home","copilot_home","str:s");
        };
        for(var phase:phases)for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError","ShadowOSError"))WebContractProbe.compare(nativeCode,source,base+",\"settings\":{\"hooks\":{\"PreToolUse\":[{}],\"preToolUse\":[{}]}},\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      }
      for(var operation:List.of("blocks","hosts"))for(var phase:List.of("read","load","write","file.iter","lines.next","exit"))for(boolean suppress:List.of(false,true))WebContractProbe.compare(nativeCode,source,"\"agent\":\"codex\",\"operation\":"+q(operation)+",\"opaque_exit\":true,\"suppress\":"+suppress+",\"fail\":"+q(phase)+",\"error\":\"ObservedOSError\",\"exception_chain\":true");
      for(var phase:List.of("strip:line","dumps","lines.next","pre.next","any","sum"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"blocks\",\"opaque_lines\":true,\"aggregate\":true,\"settings\":{\"hooks\":{\"PreToolUse\":[{}]}},\"fail\":"+q(phase)+",\"error\":\"StopIteration\",\"exception_chain\":true");
      for(var value:List.of("null","false","42","[]","{}","\"\"","\"a'b\"","\"\\ud800\\udc00\""))WebContractProbe.compare(nativeCode,source,"\"operation\":\"quote\",\"s\":"+value);
      for(boolean truthy:List.of(false,true))WebContractProbe.compare(nativeCode,source,"\"agent\":\"codex\",\"middle\":true,\"booleans\":{\"middle\":"+truthy+"}");
      for(var phase:List.of("middle","bool:middle","set:_net_guard_armed","get:_net_guard_armed"))for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))WebContractProbe.compare(nativeCode,source,"\"agent\":\"codex\",\"middle\":true,\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      for(var agent:List.of("claude","copilot","codex")){
        var run=Files.createDirectories(WebContractProbe.artifacts.resolve("real-"+agent));
        var home=Files.createDirectory(run.resolve("home"));var copilotHome=Files.createDirectory(run.resolve(".copilot-home"));
        var real="\"agent\":"+q(agent)+",\"real_files\":true,\"real_process\":true,\"home\":"+q(home)+",\"run_dir\":"+q(run)+",\"copilot_home\":"+q(copilotHome);
        var log=home.resolve("net_guard_blocks.jsonl");Files.writeString(log,"\n {} \n\t\né😀\n\u3000\n");
        var count=WebContractProbe.compare(nativeCode,source,"\"operation\":\"blocks\",\"agent\":"+q(agent)+",\"aggregate\":true,\"real_files\":true,\"agent_home\":"+q(home));
        if(!count.contains("\"value\":2"))throw new AssertionError("real refusal count differs: "+count);
        Files.write(log,new byte[]{(byte)0xff});
        WebContractProbe.compare(nativeCode,source,"\"operation\":\"blocks\",\"agent\":"+q(agent)+",\"real_files\":true,\"agent_home\":"+q(home));
        Files.delete(log);
        WebContractProbe.compare(nativeCode,source,"\"operation\":\"blocks\",\"agent\":"+q(agent)+",\"real_files\":true,\"agent_home\":"+q(home));
        if(agent.equals("codex")){
          WebContractProbe.compare(nativeCode,source,real+",\"operation\":\"hosts\"");
          var hosts=Files.readString(run.resolve(".hosts-block"));
          if(!hosts.startsWith(Files.readString(Path.of("/etc/hosts")).replaceFirst("\\n+$","")+"\n")||!hosts.contains("github.com"))throw new AssertionError("real hosts file lost host entries or answer-key block");
        }
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Web-Bench network guard APIs match the source in "+WebContractProbe.cases+" comparisons; hook shapes, settings paths, refusal counts, hosts files, raw values and failure boundaries passed");
  }
}
