//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares run configuration, task prompts and both guard constructors. */
class WebContractProbe {
  static int cases;
  static Path artifacts;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected)) {
      Files.writeString(artifacts.resolve("request.json"),request);Files.writeString(artifacts.resolve("expected.json"),expected);Files.writeString(artifacts.resolve("actual.json"),actual);
      int at=0;while(at<actual.length()&&at<expected.length()&&actual.charAt(at)==expected.charAt(at))at++;
      throw new AssertionError("Case "+cases+" differs at "+at+": "+request+"\nNative: "+actual.substring(Math.max(0,at-100),Math.min(actual.length(),at+350))+"\nSource: "+expected.substring(Math.max(0,at-100),Math.min(expected.length(),at+350))+"\nArtifacts: "+artifacts);
    }
    cases++;return actual;
  }
  static void prompts() throws Exception {
    var source=Files.readString(Path.of("reference/upstream/experiments/te_harness/webbench.py"));
    for(var field:List.of("TASK_TEMPLATE","ERROR_TEMPLATE")) {
      var marker=field+" = \"\"\"";int start=source.indexOf(marker)+marker.length();
      var expected=source.substring(start,source.indexOf("\"\"\"",start));
      var actual=Files.readString(Path.of("src/experiments/swebench",field.equals("TASK_TEMPLATE")?"web-task-prompt.txt":"web-error-prompt.txt"));
      if(!actual.equals(expected))throw new AssertionError("Native prompt asset differs: "+field);
    }
  }
  public static void main(String[] args) throws Exception {
    prompts();artifacts=Files.createTempDirectory(Path.of("validation/results"),"web-contract-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_contract_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_contract_host.py")) {
      for(var operation:List.of("config","prompt","strip","guard","codex_guard")) {
        var base="\"operation\":"+q(operation);
        compare(nativeCode,source,base);
        for(var option:List.of("\"kwargs\":{}","\"kwargs\":{\"spiral_guard\":false}","\"kwargs\":{\"codex_spiral_policy\":false}","\"kwargs\":{\"codex_spiral_policy\":{}}","\"accepts\":false","\"opaque_accepts\":true","\"env\":{\"TE_VERBOSE\":\"0\"}","\"objects\":[\"verbosity\"],\"opaque_comparison\":true","\"injected\":null","\"injected\":\"\"","\"scaffold\":true","\"mutate_protected\":true","\"excluded\":[]","\"network\":[\"WebSearch\",\"WebSearch\"]","\"objects\":[\"injected\"]","\"objects\":[\"workspace\"]","\"objects\":[\"project\"]","\"objects\":[\"spiral_judge\",\"abort_envelope\",\"spiral_thr\",\"spiral_nudge_writer\"]"))compare(nativeCode,source,base+","+option);
      }
      for(var budget:List.of("null","false","0","-1","2.8","\"١_٢\"","\"bad\"","[]","{}","7".repeat(700)))for(var op:List.of("guard","config"))compare(nativeCode,source,"\"operation\":"+q(op)+",\"kwargs\":{\"spiral_guard\":true,\"spiral_nudge\":true,\"spiral_nudge_budget\":"+budget+"}");
      for(var text:List.of("","\n","Add filename after code block."," keep \r\n Add filename after code block. \r other\u0085more\u2028last","The filename should appear on the line following the language specifier in your code block.","é😀","\u001c\u001f\u0085\u3000"))compare(nativeCode,source,"\"operation\":\"strip\",\"description\":"+q(text));
      for(var value:List.of("null","false","42","[]","{}"))for(var key:List.of("description","injected"))compare(nativeCode,source,"\"operation\":"+q(key.equals("description")?"strip":"prompt")+","+q(key)+":"+value);
      for(var option:List.of("\"request\":{}","\"request\":{\"task\":null,\"error\":null}","\"request\":{\"task\":false,\"error\":false}","\"request\":{\"task\":42}","\"request\":{\"error\":42}","\"task\":{},\"request\":{}","\"objects\":[\"task_text\"]","\"objects\":[\"task_text\"],\"opaque_stripped\":true","\"objects\":[\"description\"],\"opaque_stripped\":true","\"foreign_templates\":[\"TASK_TEMPLATE\",\"ERROR_TEMPLATE\"]","\"TASK_TEMPLATE\":\"{cwd}|{description}|{error_block}|{cwd}\"","\"TASK_TEMPLATE\":\"{missing}\"","\"ERROR_TEMPLATE\":\"{error!r}\"","\"injected_points\":[55296,56320],\"observe_points\":true","\"error_points\":[55296,56320],\"observe_points\":true"))compare(nativeCode,source,"\"operation\":\"prompt\","+option);
      for(var op:List.of("config","guard"))for(var object:List.of("spiral_guard","spiral_nudge","spiral_nudge_budget","spiral_never_abort","protected_tools","run_dir_path"))compare(nativeCode,source,"\"operation\":"+q(op)+",\"objects\":["+q(object)+"]");
      for(var phase:List.of("kwargs.get_function","kwargs.get","guard_accepts","env.get","nudge_guard","codex_guard"))for(var error:List.of("RuntimeError","OSError","ValueError","KeyboardInterrupt","DetailError"))for(var op:List.of("guard","codex_guard"))compare(nativeCode,source,"\"operation\":"+q(op)+",\"kwargs\":{\"spiral_guard\":true,\"codex_spiral_policy\":{}},\"fail\":"+q(phase)+",\"error\":"+q(error));
      for(var phase:List.of("network_tool_block","get:protected_tools","task.get","get:agent","get:model","get:effort","get:cli_version","get:agent_bin","get:routing_base","get:optimizer_model","get:optimizer_transport","get:scaffold","scaffold.name","get:scaffold_answer_excludes","scaffold_excludes","sha256","hash.hexdigest","hexdigest","guard_accepts","session.config_path","session.log_path","session.hash","get:retry","get:max_workers","get:node","get:task_timeout","get:verdict_timeout","get:start_timeout","path.join","join","open","enter","json.dump","write","exit"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))compare(nativeCode,source,"\"scaffold\":true,\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      for(boolean suppress:List.of(false,true))compare(nativeCode,source,"\"fail\":\"json.dump\",\"suppress\":"+suppress+",\"exception_chain\":true");
      for(var points:List.of("[55296]","[55296,56320]","[233,128512]"))compare(nativeCode,source,"\"injected_points\":"+points);
      var boundary="["+String.join(",",java.util.Collections.nCopies(7999,"97"))+",55296,56320]";
      compare(nativeCode,source,"\"operation\":\"prompt\",\"error_points\":"+boundary+",\"observe_points\":true");
      compare(nativeCode,source,"\"objects\":[\"injected\"],\"booleans\":{\"injected\":[true,false,true]}");
      compare(nativeCode,source,"\"operation\":\"prompt\",\"objects\":[\"workspace\"],\"foreign_templates\":[\"TASK_TEMPLATE\"]");
      var run=Files.createDirectories(artifacts.resolve("run"));
      var real="{\"real_files\":true,\"run_dir\":"+q(run)+",\"scaffold\":true}";
      var expected=source.call(real);var expectedFile=Files.readString(run.resolve("run_config.json"));
      var actual=nativeCode.call(real);var actualFile=Files.readString(run.resolve("run_config.json"));
      if(!actual.equals(expected)||!actualFile.equals(expectedFile)||!actualFile.contains("\"WebSearch\",\n      \"WebSearch\"")||!actualFile.contains("\"guard_per_attempt\": true"))throw new AssertionError("real configuration output differs: "+actual+"\n"+expected);
      cases++;
      passed=true;
    } finally {if(passed)try(var paths=Files.walk(artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Web-Bench run contracts match the source in "+cases+" comparisons; configuration order, tool masks, prompts, guard arguments, raw references and failure boundaries passed");
  }
}
