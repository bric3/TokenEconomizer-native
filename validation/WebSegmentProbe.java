//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Drives source comparisons for streaming, process cleanup and session file policy. */
class WebSegmentProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  public static void main(String[] args) throws Exception {
    WebContractProbe.artifacts = Files.createTempDirectory(Path.of("validation/results"), "web-segment-").toAbsolutePath();
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(Path.of("").toAbsolutePath(), true, "web_segment_host.py");
         var source = new AdapterProbe.Client(Path.of("").toAbsolutePath(), false, "web_segment_host.py")) {
      for (var agent : List.of("claude", "copilot", "codex")) {
        var base = "\"agent\":"+q(agent);
        WebContractProbe.compare(nativeCode, source, base);
        for (var option : List.of("\"guard\":false", "\"lines\":[]", "\"lines\":[\" \",\"bad\"]", "\"lines\":[\"null\"]", "\"lines\":[\"[]\"]", "\"lines\":[\"{\\\"type\\\":\\\"result\\\",\\\"x\\\":1}\",\"{\\\"type\\\":\\\"result\\\",\\\"x\\\":2}\"]", "\"stop_after\":1", "\"times\":[0,10,11]", "\"times\":[0,11],\"stderr\":\" failure é😀 \",\"usage\":{}", "\"stderr\":null", "\"stderr\":\" warning é😀 \",\"usage\":{}", "\"env\":{}", "\"exclude_tools\":null", "\"network\":[],\"exclude_tools\":[]", "\"fields\":{\"answer_key_ns\":true}", "\"prompt\":{}", "\"prompt\":{\"prompt\":\"resume é😀\",\"session_id\":\"session\",\"effort\":\"low\"}", "\"fatal\":\"model rejected\"", "\"stderr\":\"UNKNOWN CONFIGURATION x\"", "\"stderr\":\"Error Loading config.toml\"", "\"stderr\":"+q("x".repeat(1500)+" warning é😀 "), "\"argv\":[]", "\"fields\":{\"_net_guard_armed\":false}")) WebContractProbe.compare(nativeCode, source, base+","+option);
        for (var object : List.of("workspace", "prompt", "exclude_tools", "max_turns", "model", "effort", "protected_tools", "api_key", "argv", "_net_guard_armed", "answer_key_ns")) WebContractProbe.compare(nativeCode, source, base+",\"objects\":["+q(object)+"]");
        for (var phase : List.of("get:model", "get:effort", "get:protected_tools", "network", "os.environ", "get:agent_home", "dirname", "agent_home", "guard_log", "credentials", "mkdir", "bin_dir", "subprocess.Popen", "subprocess.PIPE", "popen", "proc.stdin", "stdin.write", "stdin.close", "time.time", "clock", "get:task_timeout", "recovery", "proc.stdout", "stdout.iter", "transcript.write", "transcript.flush", "loads", "observe", "guard.monitor", "monitor", "proc.wait", "wait", "stderr.read", "log", "session_log", "synthesize", "copilot.build_argv", "build_argv", "run_env", "copilot_home", "config", "resume_argv", "fatal_config", "read_log_dir", "row_usage", "usage.get", "usage.put")) {
          for (var error : List.of("RuntimeError", "OSError", "KeyboardInterrupt", "DetailError")) {
            WebContractProbe.compare(nativeCode, source, base+",\"lines\":[\"bad\"],\"usage\":{},\"stderr\":\"warning\",\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
          }
        }
        for (var phase : List.of("os.killpg", "os.getpgid", "proc.pid", "getpgid", "signal.SIGTERM", "killpg", "sleep", "signal.SIGKILL", "proc.kill", "kill", "wait")) for (var error : List.of("ProcessLookupError", "PermissionError", "OSError", "KeyboardInterrupt", "ShadowPermissionError")) for (int at : List.of(1,2)) WebContractProbe.compare(nativeCode, source, base+",\"stop_after\":1,\"fail\":"+q(phase)+",\"fail_at\":"+at+",\"error\":"+q(error)+",\"exception_chain\":true");
        WebContractProbe.compare(nativeCode, source, base+",\"fail\":\"wait\",\"error\":\"TimeoutExpired\"");
        for(var kind:List.of("Exception","OSError","KeyboardInterrupt","int"))WebContractProbe.compare(nativeCode,source,base+",\"fail\":\"wait\",\"error\":\"RuntimeError\",\"timeout_type\":"+q(kind)+",\"exception_chain\":true");
        for(var kind:List.of("Exception","OSError","KeyboardInterrupt","int"))WebContractProbe.compare(nativeCode,source,base+",\"lines\":[\"bad\"],\"exception_types\":{\"Exception\":"+q(kind)+"},\"exception_chain\":true");
        for(var phase:List.of("proc.kill","kill"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,base+",\"failures\":{\"wait\":\"TimeoutExpired\","+q(phase)+":"+q(error)+"},\"exception_chain\":true");
      }
      var session = "\"operation\":\"session_log\"";
      WebContractProbe.compare(nativeCode, source, session);
      for (var option : List.of("\"paths\":[]", "\"paths\":[\"a\",\"a\",\"b\"]", "\"contents\":{\"a.jsonl\":42}", "\"objects\":[\"home\"]", "\"home\":null")) WebContractProbe.compare(nativeCode, source, session+","+option);
      for (var phase : List.of("glob.glob", "os.path", "path.join", "join", "glob_call", "open", "enter", "read", "exit")) for (var error : List.of("RuntimeError", "OSError", "KeyboardInterrupt", "DetailError", "ShadowOSError")) for (boolean suppress : List.of(false,true)) WebContractProbe.compare(nativeCode, source, session+",\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"suppress\":"+suppress+",\"exception_chain\":true");
      for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt"))for(var phase:List.of("log","error.str"))WebContractProbe.compare(nativeCode,source,session+",\"failures\":{\"read\":\"DetailError\","+q(phase)+":"+q(error)+"},\"exception_chain\":true");
      for(var kind:List.of("Exception","KeyboardInterrupt","int"))WebContractProbe.compare(nativeCode,source,session+",\"fail\":\"read\",\"error\":\"OSError\",\"exception_types\":{\"OSError\":"+q(kind)+"},\"exception_chain\":true");
      for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,session+",\"failures\":{\"read\":\"DetailError\",\"exit\":"+q(error)+"},\"exception_chain\":true");
      var home=Files.createDirectories(WebContractProbe.artifacts.resolve("home/.claude/projects/project"));
      Files.writeString(home.resolve("b.jsonl"),"second é😀\n");
      Files.write(home.resolve("a.jsonl"),new byte[]{'f','i','r','s','t',(byte)0xff,'\r','\n'});
      Files.createDirectory(home.resolve("c.jsonl"));
      var real=WebContractProbe.compare(nativeCode,source,session+",\"real_files\":true,\"home\":"+q(WebContractProbe.artifacts.resolve("home")));
      if(!real.contains("first\\ufffd\\n\\nsecond"))throw new AssertionError("real session read lost replacement decoding or sorted concatenation: "+real);
      passed = true;
    } finally {
      if (passed) try (var paths = Files.walk(WebContractProbe.artifacts)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path); }
    }
    System.out.println("Web-Bench segments and session logs match the source in "+WebContractProbe.cases+" comparisons; command order, environments, streaming, guard stops, timeout cleanup and failure boundaries passed");
  }
}
