//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares the complete task loop without launching an evaluator or a model. */
class WebTaskProbe {
  static int cases;
  static Path artifacts;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(cases==0&&(expected.contains("\"raised\":")||!expected.contains("\"resolved\":true")))throw new AssertionError("Default source fixture did not complete: "+expected);
    if(!actual.equals(expected)) {
      Files.writeString(artifacts.resolve("request.json"),request);Files.writeString(artifacts.resolve("expected.json"),expected);Files.writeString(artifacts.resolve("actual.json"),actual);
      int at=0;while(at<actual.length()&&at<expected.length()&&actual.charAt(at)==expected.charAt(at))at++;
      throw new AssertionError("Case "+cases+" differs at "+at+": "+request+"\nNative: "+actual.substring(Math.max(0,at-150),Math.min(actual.length(),at+350))+"\nSource: "+expected.substring(Math.max(0,at-150),Math.min(expected.length(),at+350))+"\nArtifacts: "+artifacts);
    }
    cases++;return actual;
  }
  static void remove(Path root) throws Exception {
    if(Files.exists(root))try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}
  }
  public static void main(String[] args) throws Exception {
    artifacts=Files.createTempDirectory(Path.of("validation/results"),"web-task-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_task_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_task_host.py")) {
      compare(nativeCode,source,"");
      var retry="\"events\":[{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":false},{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":true}]";
      for(boolean guard:List.of(false,true))for(int resumes:List.of(0,1,3))for(boolean scaffold:List.of(false,true))compare(nativeCode,source,retry+",\"guard\":"+guard+",\"resumes\":"+resumes+",\"scaffold\":"+scaffold);
      for(var option:List.of("\"fields\":{\"_stream_broken\":true}","\"finished\":true","\"events\":[]","\"events\":[null]","\"events\":[{}]","\"events\":[{\"task\":\"wrong\"}]","\"events\":[{\"task\":\"task-1\"},null]","\"events\":[{\"task\":\"task-1\"},{}]","\"events\":[{\"task\":\"task-1\"},{\"task\":\"wrong\",\"ok\":true}]","\"events\":[{\"task\":\"task-1\"},{\"task\":\"task-1\"}]","\"requests\":[null]","\"requests\":[{}]","\"requests\":[false]","\"missing_task_id\":true","\"task\":{\"repo\":null}","\"task\":{\"repo\":\"\"}","\"task\":{\"repo\":\"a/\"}","\"task\":{\"repo\":42}","\"opaque_id\":true","\"opaque_path\":true","\"opaque_prompt\":true","\"opaque_injected\":true","\"opaque_armed\":true","\"mutate_context\":true","\"guard\":true,\"falsey_guard\":true","\"guard\":true,\"opaque_stop\":true,\"booleans\":{\"continuation\":false}","\"guard\":true,\"resumes\":1,\"mapping_resume\":true","\"scaffold\":true,\"notes\":[{}]","\"fields\":{\"keep_workspaces\":true}"))compare(nativeCode,source,option);
      compare(nativeCode,source,retry+",\"scaffold\":true,\"notes\":[{}, {\"installed\":true}]");
      for(var retryCount:List.of("-1","0","1","2","3","null","\"2\"","2.5","7".repeat(700)))compare(nativeCode,source,"\"fields\":{\"retry\":"+retryCount+"}");
      for(var value:List.of("null","false","0","1","\"12\"","\"bad\"","[]","{}","7".repeat(700)))compare(nativeCode,source,"\"kwargs\":{\"max_turns\":"+value+"},\"env\":{\"TE_MAX_TURNS\":\"23\"}");
      for(var agent:List.of("claude","copilot","codex"))compare(nativeCode,source,"\"fields\":{\"agent\":"+q(agent)+"}");
      for(var option:List.of("\"events\":[{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":false},{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":false}]","\"task\":{\"instance_id\":\"init\"},\"events\":[{\"task\":\"init\"},{\"task\":\"init\",\"ok\":false},{\"task\":\"init\"},{\"task\":\"init\",\"ok\":false}]"))compare(nativeCode,source,option);
      for(var value:List.of("null","false","0","\"\"","[]","{}"))compare(nativeCode,source,"\"requests\":[{\"files\":"+value+"}]");
      for(var value:List.of("null","false","0","1","[]","{}"))compare(nativeCode,source,"\"segments\":["+value+"]");
      for(var phase:List.of("set:_current_task","runner.model","runner.agent","runner.effort","runner.cli_version","optimizer_context","cell","path.basename","basename","makedirs","clock","path.join","join","runner.repo","session_for","runner._stream_broken","session.finished","write_run_config","agent_home","install_net_guard","open","enter","next_event","next_request","req.get_function","req.get","make_guard","materialize","apply_scaffold","build_prompt","json.dump","write","flush","segment","guard.continuation","continuation","guard.segment","merge_segments","collect","segment_usage","reply.put_function","reply.put","rmtree","row_from_segment","net_guard_blocks","answer_key_attempts","scaffold.name","guard.aborted","guard.nudges_done","guard.abort_suppressed","guard.events","guard.judge_tokens","guard.nudge_writer_tokens","effective_tokens","log","exit"))for(var error:List.of("RuntimeError","OSError","ValueError","KeyboardInterrupt","DetailError","ShadowRuntimeError"))compare(nativeCode,source,retry+",\"guard\":true,\"resumes\":1,\"scaffold\":true,\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      for(var phase:List.of("json.dump","segment","collect","reply.put","next_event"))for(boolean suppress:List.of(false,true))compare(nativeCode,source,"\"guard\":true,\"fail\":"+q(phase)+",\"suppress_transcript\":"+suppress+",\"suppress_request\":"+suppress+",\"exception_chain\":true");
      compare(nativeCode,source,"\"failures\":[{\"fail\":\"segment\",\"error\":\"OSError\"},{\"fail\":\"exit\",\"fail_at\":2,\"error\":\"ValueError\"}],\"exception_chain\":true");
      for(var field:List.of("aborted","abort_suppressed","nudges_done","events","judge_tokens","nudge_writer_tokens"))compare(nativeCode,source,"\"guard\":true,\"missing_guard\":["+q(field)+"]");
      compare(nativeCode,source,"\"guard\":true,\"guard_fields\":{\"events\":null,\"abort_suppressed\":true,\"aborted\":true}");
      compare(nativeCode,source,"\"guard\":true,\"fields\":{\"retry\":3},\"events\":[{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":false},{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":false},{\"task\":\"task-1\"},{\"task\":\"task-1\",\"ok\":true}],\"per_guard\":[{\"judge_tokens\":1e16},{\"judge_tokens\":1},{\"judge_tokens\":-1e16}]");
      for(var option:List.of("\"guard\":true,\"guard_fields\":{\"nudges_done\":null}","\"guard\":true,\"guard_fields\":{\"judge_tokens\":\"bad\"}","\"collection\":null","\"collection\":[{},[],[]]","\"collection\":[{},null,[]]","\"collection\":[{},[],null]","\"collection\":[{},[[1]],[]]","\"row_fields\":{\"resolved\":\"override\",\"webbench_attempts\":77}","\"opaque_excluded\":true,\"booleans\":{\"excluded\":false}","\"opaque_excluded\":true"))compare(nativeCode,source,option);
      for(var phase:List.of("next_event","next_request","segment","collect","reply.put","rmtree","open","enter","write","exit"))for(int at:List.of(2,3))compare(nativeCode,source,retry+",\"guard\":true,\"resumes\":1,\"fail\":"+q(phase)+",\"fail_at\":"+at+",\"exception_chain\":true");
      for(var option:List.of("\"fields\":{\"_stream_broken\":true}","\"finished\":true"))for(var phase:List.of("open","enter","exit","row_from_segment","clock","log"))compare(nativeCode,source,option+",\"fail\":"+q(phase)+",\"fail_at\":"+(phase.equals("clock")?2:1)+",\"exception_chain\":true");
      var root=artifacts.resolve("real");var run=root.resolve("run");var project=Files.createDirectories(root.resolve("repo/projects/project"));
      Files.writeString(project.resolve(".evalignore"),"*.log\n");
      var real="\"real_files\":true,\"run_dir\":"+q(run)+",\"repo\":"+q(root.resolve("repo"))+",\"fields\":{\"keep_workspaces\":true},\"guard\":true,\"resumes\":1";
      var expected=source.call("{"+real+"}");
      if(!Files.readString(run.resolve("ws-attempt1/src/a.js")).equals("changed é😀\n")||!Files.readString(run.resolve("transcript.jsonl")).equals("segment 0\nsegment 1\n")||!Files.exists(run.resolve("agent_request_1.json")))throw new AssertionError("source filesystem task did not finish: "+expected);
      remove(run);
      var actual=nativeCode.call("{"+real+"}");
      if(!actual.equals(expected)||!Files.readString(run.resolve("ws-attempt1/src/a.js")).equals("changed é😀\n")||!Files.readString(run.resolve("transcript.jsonl")).equals("segment 0\nsegment 1\n"))throw new AssertionError("real filesystem task differs: "+actual+"\n"+expected);
      cases++;
      passed=true;
    } finally {if(passed)remove(artifacts);}
    System.out.println("Web-Bench task loop matches the source in "+cases+" comparisons; retries, stream failures, replies, raw references, nested contexts and per-attempt telemetry passed without an evaluator or model calls");
  }
}
