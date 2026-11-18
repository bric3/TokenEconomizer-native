//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Checks task lifecycle ordering, cleanup, failure rows and exactly-once I/O. */
class TaskProbe {
  static String q(Object value) { return ParserProbe.q(value); }
  static int cases;
  static Path artifacts;
  static void clear(Path root) throws Exception { if (Files.exists(root)) try (var paths=Files.walk(root)) { for (var p:paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(p); } }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var root=artifacts.resolve("case-"+cases);
    var request="{\"root\":"+q(root)+(fields.isEmpty()?"":","+fields)+"}";
    var expected=source.call(request); clear(root);
    var actual=nativeCode.call(request);
    if (cases==0 && (expected.contains("\"raised\":") || !expected.contains("\"resolved\":true"))) throw new AssertionError("Default source fixture did not complete: "+expected);
    if (!expected.equals(actual)) {
      Files.writeString(artifacts.resolve("request.json"),request);Files.writeString(artifacts.resolve("expected.json"),expected);Files.writeString(artifacts.resolve("actual.json"),actual);
      int at=0;while(at<actual.length()&&at<expected.length()&&actual.charAt(at)==expected.charAt(at))at++;
      throw new AssertionError("Case "+cases+" differs at "+at+"\nNative: "+actual.substring(Math.max(0,at-100),Math.min(actual.length(),at+240))+"\nSource: "+expected.substring(Math.max(0,at-100),Math.min(expected.length(),at+240)));
    }
    cases++;
  }
  static void prompt() throws Exception {
    var source=Files.readString(Path.of("reference/upstream/experiments/te_harness/swebench.py"));
    int start=source.indexOf("TASK_TEMPLATE = \"\"\"")+"TASK_TEMPLATE = \"\"\"".length();
    var expected=source.substring(start,source.indexOf("\"\"\"",start));
    if(!Files.readString(Path.of("src/experiments/swebench/task-prompt.txt")).equals(expected))throw new AssertionError("Task prompt differs");
  }
  public static void main(String[] args) throws Exception {
    prompt(); artifacts=Files.createTempDirectory("te-task-probe-"); boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"task_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"task_host.py")) {
      compare(nativeCode,source,"");
      for(boolean guard:List.of(false,true))for(int resumes:List.of(0,1,3))for(boolean scaffold:List.of(false,true))compare(nativeCode,source,"\"guard\":"+guard+",\"resumes\":"+resumes+",\"scaffold\":"+scaffold+",\"injected\":\" manual \\n \",\"excluded\":[\"WebSearch\"]");
      compare(nativeCode,source,"\"guard\":true,\"falsey_guard\":true");
      for(var method:List.of("_container_name","_build_guard","_write_run_config","_start","_record_env_activation","_apply_scaffold","_install_net_guard","_install_answer_key_hosts","_segment","_guard_continuation","_graded_diff","_grade","_net_guard_blocks","_stop","_row_from_segment","_answer_key_attempts","_patch_excludes","_guard_row"))
        for(var kind:List.of("RuntimeError","TimeoutExpired","OSError","ValueError","KeyboardInterrupt","TransientFailure","ShadowRuntimeError"))compare(nativeCode,source,"\"guard\":true,\"scaffold\":true,\"fail\":"+q(method)+",\"error_kind\":"+q(kind));
      for(var count:List.of("0","1","\"12\"","null"))compare(nativeCode,source,"\"kwargs\":{\"max_turns\":"+count+"},\"max_turns_env\":\"23\"");
      for(var agent:List.of("claude","codex","copilot"))for(var result:List.of("{}","null","{\"tokens_source\":\"no_turn_event\",\"output_tokens\":null}","{\"tokens_source\":\"debug_log\",\"output_tokens\":5}","{\"tokens_source\":\"stream_usage\",\"output_tokens\":5}"))compare(nativeCode,source,"\"kind\":\"row\",\"agent\":"+q(agent)+",\"result\":"+result);
      for(var injected:List.of("null","\"\"",q(" é😀 manual \u001f\n")))compare(nativeCode,source,"\"kind\":\"prompt\",\"arguments\":{\"task\":{\"problem_statement\":\"Fix é😀\"},\"injected\":"+injected+"}");
      for(var path:List.of("/r/a/b/../c/","//r/a/b","relative/./run","/tmp/é😀/run"))compare(nativeCode,source,"\"kind\":\"name\",\"arguments\":{\"instance_id\":\"acme__widget-1\",\"arm\":\"TRIMECH-é😀\",\"run_dir_path\":"+q(path)+"}");
      System.out.println("SWE-bench lifecycle matches the source in "+cases+" cases; prompt, naming, resumes, artifacts, grading failures and cleanup order passed without Docker or model calls");passed=true;
    } finally {if(passed)clear(artifacts);else System.err.println("Probe artifacts: "+artifacts);}
  }
}
