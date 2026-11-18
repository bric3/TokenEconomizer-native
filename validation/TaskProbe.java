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
      for(var patch:List.of("", " \u001c\u001f\u0085\n", "fix é😀\n"))compare(nativeCode,source,"\"kind\":\"grade\",\"patch\":"+q(patch));
      for(int rc:List.of(0,1,-9))for(var output:List.of("", " \u001f\n", "tests passed\né😀\n"))
        compare(nativeCode,source,"\"kind\":\"grade\",\"exec_eval\":{\"returncode\":"+rc+",\"stdout\":"+q(output)+",\"stderr\":\"daemon failure\"}");
      compare(nativeCode,source,"\"kind\":\"grade\",\"exec_write\":{\"returncode\":1,\"stdout\":\"ignored\",\"stderr\":\"no container\"}");
      for(var method:List.of("eval_script","exec_write","exec_eval","_eval_exec_failed","score"))
        for(var kind:List.of("RuntimeError","TimeoutExpired","EvalTimeout","OSError","ValueError","KeyboardInterrupt","SystemExit","TransientFailure","ShadowRuntimeError","ShadowTimeoutExpired"))
          compare(nativeCode,source,"\"kind\":\"grade\",\"fail\":"+q(method)+",\"error_kind\":"+q(kind)+(method.equals("_eval_exec_failed")?",\"exec_write\":{\"returncode\":1,\"stdout\":\"\",\"stderr\":\"exec failed\"}":""));
      for(var method:List.of("exec_write","exec_eval"))for(var output:List.of("null",q("partial é😀\n"),"false","\"\""))
        compare(nativeCode,source,"\"kind\":\"grade\",\"fail\":"+q(method)+",\"error_kind\":\"TimeoutExpired\",\"partial_output\":"+output);
      for(var bytes:List.of("[]","[195,169,240,159,152,128,10]","[255,237,160,128,226,130,13,10]"))
        compare(nativeCode,source,"\"kind\":\"grade\",\"fail\":\"exec_eval\",\"error_kind\":\"EvalTimeout\",\"stdout_bytes\":"+bytes);
      for(boolean missing:List.of(false,true))for(var stderr:List.of("", " \u001f daemon failure é😀 \n", "é😀".repeat(400)))
        compare(nativeCode,source,"\"kind\":\"eval_exec_failed\",\"missing_run_dir\":"+missing+",\"out\":\"partial output\\n\",\"stderr\":"+q(stderr));
      for(var patch:List.of("", "fix é😀\n"))compare(nativeCode,source,"\"with_grading\":true,\"guard\":true,\"patch\":"+q(patch));
      for(var method:List.of("exec_write","exec_eval","score"))for(var kind:List.of("RuntimeError","TimeoutExpired","KeyboardInterrupt"))
        compare(nativeCode,source,"\"with_grading\":true,\"guard\":true,\"fail\":"+q(method)+",\"error_kind\":"+q(kind)+",\"partial_output\":\"partial é😀\\n\"");
      for(var rc:List.of(0,1))compare(nativeCode,source,"\"with_grading\":true,\"exec_eval\":{\"returncode\":"+rc+",\"stdout\":\"\",\"stderr\":\"container unavailable\"}");
      compare(nativeCode,source,"\"kind\":\"eval_exec_failed\",\"log_is_directory\":true,\"stderr\":\"write failed\"");
      for(var kind:List.of("task","grade"))compare(nativeCode,source,"\"kind\":"+q(kind)+",\"cwd\":"+q(artifacts)+",\"run_dir_relative\":true,\"with_grading\":true");
      System.out.println("SWE-bench task lifecycle and grading match the source in "+cases+" cases; prompts, names, resumes, artifacts, partial timeout output, failure reports and cleanup passed without Docker or model calls");passed=true;
    } finally {if(passed)clear(artifacts);else System.err.println("Probe artifacts: "+artifacts);}
  }
}
