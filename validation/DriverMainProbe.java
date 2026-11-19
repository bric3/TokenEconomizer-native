//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Compares the complete driver loop, actual stub artifacts and stream cleanup offline. */
class DriverMainProbe {
  static int checks;
  static Path root;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields)throws Exception {
    var request="{\"root\":"+q(root.resolve("case-"+checks))+(fields.isEmpty()?"":","+fields)+"}";
    var actual=nativeCode.call(request);var expected=source.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+checks+": "+request+"\nNative: "+actual+"\nSource: "+expected);checks++;return actual;
  }
  public static void main(String[] args)throws Exception {
    root=Files.createTempDirectory(Path.of("validation/results"),"driver-main-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"driver_main_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"driver_main_host.py")) {
      compare(nativeCode,source,"");
      for(var agent:List.of("claude","copilot","codex"))for(var index:List.of("0","1","-1","\"١_٢\"","99999999999999999999999999","\"bad\"","null"))for(var model:List.of("null","false","\"\"","\"agent\"","\"fixture-model\""))
        compare(nativeCode,source,"\"operation\":\"stub\",\"agent\":"+q(agent)+",\"task\":{\"_index\":"+index+",\"instance_id\":\"é😀\",\"repo\":\"org/pkg\"},\"run_kwargs\":{\"model\":"+model+"}");
      compare(nativeCode,source,"\"operation\":\"stub\",\"method\":\"optimizer\"");
      for(var arm:List.of("baseline","baseline-low","baseline-medium","inject-only","trimech","rtk","codex-trimech"))for(var agent:List.of("claude","codex"))
        compare(nativeCode,source,"\"arm\":"+q(arm)+",\"agent\":"+q(agent)+",\"env\":{\"TE_EFFORT_LEVEL\":\"inherited\"},\"real_stub\":true,\"srow\":{\"untool_judge_tokens\":17},\"opt_tokens\":40.0");
      for(var row:List.of("{}","{\"resolved\":1}","{\"resolved\":true,\"optimizer_tokens\":7,\"spiral_judge_tokens\":8}","{\"resolved\":false,\"seed\":9,\"run_dir\":\"preserved\"}"))
        compare(nativeCode,source,"\"rows\":["+row+"],\"srow\":{\"untool_judge_tokens\":17,\"spiral_judge_tokens\":0},\"opt_tokens\":40");
      for(boolean begin:List.of(false,true))for(boolean end:List.of(false,true))compare(nativeCode,source,"\"begin\":"+begin+",\"end\":"+end+",\"repo\":\"\",\"seed\":-1");
      compare(nativeCode,source,"\"tasks\":[]");
      for(var optimizer:List.of("gpt-fixture","claude-fixture","gemini-fixture","o1-fixture","o3-fixture","o4-fixture","other-label"))compare(nativeCode,source,"\"extra\":[\"--optimizer-model\","+q(optimizer)+"]");
      for(var argv:List.of("[]","[\"--help\"]","[\"--arm\",\"invalid\"]","[\"--seed\",\"bad\"]","[\"--unknown\"]"))compare(nativeCode,source,"\"argv\":"+argv);
      for(var failure:List.of("load_env","load_adapter","load_runner","load_tasks","makedirs","economizer","begin_stream","injection_prefix","build_tools","run_kwargs","run_task","after_task","open","stream.enter","stream.write","stream.exit","row.enter","row.write","row.exit","end_stream"))for(var error:List.of("RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var suppress:List.of("row","stream"))for(var fail:List.of("run_task","after_task","row.write","stream.write"))compare(nativeCode,source,"\"fail\":"+q(fail)+",\"suppress\":"+q(suppress));
      compare(nativeCode,source,"\"run_kwargs\":{\"arm\":\"duplicate\"}");
      compare(nativeCode,source,"\"run_kwargs\":[[\"model\",\"supplied\"]]");
    }
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"driver_e2e_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"driver_e2e_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        var result=compare(nativeCode,source,"\"agent\":"+q(agent));
        if(!result.contains("\"value\":null")||!result.contains("tasks=3 resolved=2"))throw new AssertionError("real offline baseline did not complete: "+result);
      }
      var free=compare(nativeCode,source,"\"agent\":\"codex\",\"arm\":\"codex-inject-envfacts\"");
      if(!free.contains("\"value\":null")||!free.contains("tasks=3 resolved=2"))throw new AssertionError("real envfacts run did not complete: "+free);
      var paid=compare(nativeCode,source,"\"arm\":\"inject-only\",\"count\":10");
      if(!paid.contains("\"value\":null")||!paid.contains("tasks=10 resolved=6")||!paid.contains("\"optimizer_tokens\":40.0"))throw new AssertionError("stub optimizer overhead was not exercised: "+paid);
      try(var client=new ContextProbe.Client(Path.of("target/debug/te-bridge").toAbsolutePath(),"experiment.swebench")) {
        var invalid=client.call("experiment.swebench.provider.driver.stub_missing","");
        if(!invalid.contains("\"raised\":\"ValueError\"")||invalid.contains("\"callback\""))throw new AssertionError("unknown stub operation requested I/O: "+invalid);
        checks++;
      }
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Benchmark main and stub passed "+checks+" checks ("+(checks-1)+" source comparisons and one protocol guard); argument parsing, setup order, real stub bytes, row/stream accounting, strict resolve counts and cleanup failures passed offline");
  }
}
