//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares Docker agent launch, stream handling, recovery and termination policy. */
class SweSegmentProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"swe-segment-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"swe_segment_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"swe_segment_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        var base="\"agent\":"+q(agent);
        var normal=WebContractProbe.compare(nativeCode,source,base);
        if(normal.contains("\"raised\"")||!normal.contains("\"launched\":1")||!normal.contains("\"num_turns\":2"))throw new AssertionError("default segment failed: "+normal);
        for(var option:List.of("\"guard\":false","\"lines\":[]","\"lines\":[\" \",\"bad\"]","\"lines\":[\"null\"]","\"lines\":[\"[]\"]","\"lines\":[\"{\\\"type\\\":\\\"result\\\",\\\"x\\\":1}\",\"{\\\"type\\\":\\\"result\\\",\\\"x\\\":2}\"]","\"stop_after\":1","\"times\":[0,10,11]","\"times\":[0,11],\"stderr\":\" failure é😀 \",\"usage\":{}","\"stderr\":null","\"stderr\":\" warning é😀 \",\"usage\":{}","\"env\":{}","\"exclude_tools\":null","\"network\":[],\"exclude_tools\":[]","\"prompt\":{}","\"prompt\":{\"prompt\":\"resume é😀\",\"session_id\":\"session\",\"effort\":\"low\"}","\"fatal\":\"model rejected\"","\"stderr\":\"Error loading config.toml\"","\"stderr\":\"Error Loading config.toml\"","\"stderr\":\"unknown configuration field\"","\"stderr\":\"UNKNOWN CONFIGURATION x\"","\"stderr\":"+q("x".repeat(1500)+" warning é😀 "),"\"argv\":[]","\"fields\":{\"_guard_python\":null}","\"activation\":null","\"activation\":42","\"activation\":{\"key\":\"value\"}","\"mcp_args\":null","\"mcp_args\":[\"--mcp-config\",\"/fixture/config é😀\"]","\"mcp_config\":null","\"secrets\":[]","\"secrets\":[\"FIRST_KEY\",\"SECOND_KEY\"]","\"run_env\":{}","\"run_env\":null"))WebContractProbe.compare(nativeCode,source,base+","+option);
        for(var object:List.of("name","prompt","exclude_tools","max_turns","model","effort","protected_tools","api_key","argv","_guard_python","NET_GUARD_LOG","mcp_config"))WebContractProbe.compare(nativeCode,source,base+",\"objects\":["+q(object)+"]");
        for(var phase:List.of("net_guard.LOG_ENV","get:NET_GUARD_LOG","get:_env_activation_args","activation","get:model","get:effort","get:protected_tools","network","get:scaffold_profile","profile.mcp_args","mcp_args","os.environ","get:api_key","subprocess.Popen","subprocess.PIPE","popen","proc.stdin","stdin.write","stdin.close","time.time","clock","get:task_timeout","recovery","proc.stdout","stdout.iter","transcript.write","transcript.flush","loads","observe","guard.monitor","monitor","proc.wait","wait","stderr.read","log","get:_session_log_text","session_log","synthesize","copilot.CONTAINER_LOG_DIR","copilot.build_argv","build_argv","copilot.provider_secret_names","secrets","run_env","get:_guard_python","resume_argv","fatal_config","row_usage","usage.get","usage.put","run","host_result.stdout"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,base+",\"lines\":[\"bad\"],\"usage\":{},\"stderr\":\"warning\",\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
        for(var phase:List.of("run","sleep","proc.kill","kill","wait"))for(var error:List.of("ProcessLookupError","PermissionError","OSError","KeyboardInterrupt","DetailError"))for(int at:List.of(1,2,3))WebContractProbe.compare(nativeCode,source,base+",\"stop_after\":1,\"fail\":"+q(phase)+",\"fail_at\":"+at+",\"error\":"+q(error)+",\"exception_chain\":true");
        WebContractProbe.compare(nativeCode,source,base+",\"fail\":\"wait\",\"error\":\"TimeoutExpired\",\"exception_chain\":true");
        for(var kind:List.of("Exception","OSError","KeyboardInterrupt","int"))WebContractProbe.compare(nativeCode,source,base+",\"fail\":\"wait\",\"error\":\"RuntimeError\",\"timeout_type\":"+q(kind)+",\"exception_chain\":true");
        for(var phase:List.of("proc.kill","kill"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,base+",\"failures\":{\"wait\":\"TimeoutExpired\","+q(phase)+":"+q(error)+"},\"exception_chain\":true");
      }
      var session="\"operation\":\"session_log\"";
      WebContractProbe.compare(nativeCode,source,session);
      for(var option:List.of("\"home\":null","\"log_text\":null","\"log_text\":0","\"objects\":[\"home\"]","\"objects\":[\"name\"]"))WebContractProbe.compare(nativeCode,source,session+","+option);
      for(var phase:List.of("get:scaffold_profile","profile.home","get:_exec","format:home","exec","exec_result.stdout"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,session+",\"objects\":[\"home\"],\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))for(var phase:List.of("log","error.str"))WebContractProbe.compare(nativeCode,source,session+",\"failures\":{\"exec\":\"DetailError\","+q(phase)+":"+q(error)+"},\"exception_chain\":true");
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("SWE-bench streaming matches the source in "+WebContractProbe.cases+" comparisons; Docker arguments, secret forwarding, transcripts, guard stops, usage recovery, session logs and failure boundaries passed without a model or Docker");
  }
}
