//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares driver factory selection and overhead copying with the pinned source. */
class DriverSelectionProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields)throws Exception {
    var request="{"+fields+"}";var actual=nativeCode.call(request);var expected=source.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args)throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"driver_selection_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"driver_selection_host.py")) {
      for(var factory:List.of("zero","agent","model","scaffold","all","kwargs","positional","dictionary"))
        for(var model:List.of("null","false","0","\"\"","\"agent\"","\"gpt-fixture\""))for(var scaffold:List.of("null","\"\"","\"rtk\""))
          compare(nativeCode,source,"\"spec\":"+q("fixture:"+factory)+",\"model\":"+model+",\"scaffold\":"+scaffold);
      for(var spec:List.of("null","42","[]","{}",q(""),q("invalid"),q("fixture:missing"),q(":all"),q("fixture:all:extra")))compare(nativeCode,source,"\"spec\":"+spec);
      for(var agent:List.of("null","false","\"\"","\"claude\"","\"copilot\"","\"codex\""))for(var scaffold:List.of("null","\"rtk\"","\"é😀'\""))
        compare(nativeCode,source,"\"spec\":\"stub\",\"agent\":"+agent+",\"scaffold\":"+scaffold);
      for(var env:List.of("{}","{\"TE_AGENT\":\"inherited\",\"TE_SCAFFOLD\":\"inherited\"}","{\"TE_AGENT\":\"\",\"TE_SCAFFOLD\":\"\"}"))
        compare(nativeCode,source,"\"env\":"+env+",\"scaffold\":\"rtk\",\"model\":\"gpt-fixture\"");
      for(var failure:List.of("import","setdefault","signature","factory"))for(var error:List.of("TypeError","ValueError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error)+",\"scaffold\":\"rtk\"");
      for(var error:List.of("TypeError","ValueError")) {
        var result=compare(nativeCode,source,"\"fail\":\"signature\",\"error\":"+q(error));
        if(!result.contains("\"same_error\":true"))throw new AssertionError("signature fallback lost the active exception");
      }
      for(var name:List.of("null","[]","{}","17","\"unknown\"","\"swebench\"","\"webbench\"","\"creddata\"","\"reposvul\""))
        for(var repo:List.of("null","17","\"\"","\"org/project\"","\"org/\"","\"é😀\""))
          compare(nativeCode,source,"\"operation\":\"adapter\",\"name\":"+name+",\"repo\":"+repo+",\"tasks\":\"/fixture/tasks.json\"");
      var keys=List.of("optimizer_tokens","untool_classify_tokens","untool_judge_tokens","codex_untool_adapt_tokens","spiral_judge_tokens","spiral_nudge_writer_tokens");
      for(var key:keys)for(var prior:List.of("","null","0","7"))for(var value:List.of("null","0","false","17","\"tokens\"","[]","{\"nested\":[1,2]}"))
        compare(nativeCode,source,"\"operation\":\"overhead\",\"row\":{"+(prior.isEmpty()?"":q(key)+":"+prior)+"},\"srow\":{"+q(key)+":"+value+",\"resolved\":true}");
      for(var failure:List.of("row.contains","srow.get","srow.item","row.set"))for(var error:List.of("KeyError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"operation\":\"overhead\",\"srow\":{\"optimizer_tokens\":7},\"fail\":"+q(failure)+",\"error\":"+q(error));
      compare(nativeCode,source,"\"operation\":\"overhead\",\"keys\":[\"custom\",\"custom\"],\"srow\":{\"custom\":[1,2],\"optimizer_tokens\":5}");
    }
    System.out.println("Experiment driver selection matches the source in "+cases+" comparisons; signatures, factory arguments, environment precedence, stub diagnostics, active exceptions and overhead identity passed offline");
  }
}
