//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares optimizer log paths, cell keys, shared context and prompt classification. */
class OptimizerContextProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"optimizer_context_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"optimizer_context_host.py")) {
      for(var explicit:List.of("null","false","0","\"\"","\"relative/log\"","\"é😀\"","\"\\ud800\"","[]","{}","[1]"))for(var root:List.of("null","false","0","\"\"","\"relative/root\"","\"/absolute/root\"","[1]"))
        compare(nativeCode,source,"\"env\":{\"TE_OPTIMIZER_LOG\":"+explicit+",\"TOKENECONOMIZER_RESULTS\":"+root+"}");
      for(var key:List.of("TE_OPTIMIZER_LOG","TOKENECONOMIZER_RESULTS"))for(boolean truth:List.of(false,true))compare(nativeCode,source,"\"env_objects\":["+q(key)+"],\"truthy\":"+truth);
      for(var path:List.of("", "/", "//", "/results/arm/repo_s0/now", "arm/repo_s0/run/", "arm/repo_s0/../repo_s1/run", "file", ".", "../run", "é😀/cell/run", "bad\u0000/parent/run", "\ud800/parent/run"))for(boolean web:List.of(false,true))
        compare(nativeCode,source,"\"method\":\"_cell_of\",\"webbench\":"+web+",\"arguments\":{\"run_dir_path\":"+q(path)+"}");
      compare(nativeCode,source,"\"method\":\"_cell_of\",\"objects\":[\"run_dir_path\"]");
      compare(nativeCode,source,"\"method\":\"_cell_of\",\"mutate_paths\":true,\"arguments\":{\"run_dir_path\":\"/root/arm/cell/run\"}");
      for(var fields:List.of("{}","{\"a\":null,\"b\":false,\"c\":0,\"d\":\"\",\"e\":[],\"f\":{}}","{\"agent\":\"copilot\",\"cell\":\"arm/repo_s0\"}"))for(boolean web:List.of(false,true))for(boolean replace:List.of(false,true))
        compare(nativeCode,source,"\"method\":\"set_optimizer_context\",\"webbench\":"+web+",\"replace_context\":"+replace+",\"arguments\":"+fields);
      compare(nativeCode,source,"\"method\":\"set_optimizer_context\",\"objects\":[\"opaque\"]");
      for(int signature=0;signature<9;signature++)for(var padding:List.of("", "x".repeat(550), "x".repeat(599), "x".repeat(600), "😀".repeat(550)))
        compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"signature_index\":"+signature+",\"padding\":"+q(padding));
      for(var prompt:List.of("null","false","0","1","\"\"","\"unknown\"","[]","{}","[1]","[\"Reply with the single word: ok\"]","\"\\ud800\""))
        compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"arguments\":{\"prompt\":"+prompt+"}");
      for(boolean contains:List.of(false,true))compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"objects\":[\"prompt\"],\"contains\":"+contains);
      for(var kinds:List.of("null","[]","[[]]","[[\"kind\"]]","[[\"kind\",\"sig\",\"extra\"]]","[1]","[[\"kind\",\"\"]]","[[\"kind\",1]]","[[\"first\",\"match\"],[\"second\",\"match\"]]"))
        compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"kinds\":"+kinds+",\"arguments\":{\"prompt\":\"match\"}");
      compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"opaque_kind\":true,\"arguments\":{\"prompt\":\"match\"}");
      var preflight="Reply with the single word: ok";
      for(int padding:List.of(0,598-preflight.length(),599-preflight.length(),600-preflight.length()))
        compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"paired_prefix\":true,\"arguments\":{\"prompt\":"+q("x".repeat(padding)+preflight)+"}");
      compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"paired_prefix\":true,\"paired_signature\":true");
      compare(nativeCode,source,"\"method\":\"optimizer_call_kind\",\"paired_signature\":true,\"arguments\":{\"prompt\":"+q("\ud800\udc00")+"}");
      for(var failure:List.of("get","join","normpath","dirname","basename","clear","update","slice","contains","bool:prompt"))for(var error:List.of("RuntimeError","TypeError","KeyboardInterrupt")) {
        var method=List.of("clear","update").contains(failure)?"set_optimizer_context":List.of("slice","contains","bool:prompt").contains(failure)?"optimizer_call_kind":List.of("normpath","dirname","basename").contains(failure)?"_cell_of":"optimizer_log_path";
        var fields=method.equals("_cell_of")?",\"arguments\":{\"run_dir_path\":\"/arm/cell/run\"}":method.equals("optimizer_call_kind")?",\"objects\":[\"prompt\"]":",\"env\":{\"TOKENECONOMIZER_RESULTS\":\"root\"}";
        compare(nativeCode,source,"\"method\":"+q(method)+fields+",\"fail\":"+q(failure)+",\"error\":"+q(error));
      }
    }
    System.out.println("Optimizer metadata matches the source in "+cases+" comparisons; paths, Web-Bench aliases, context identity, callback replacement and all nine prompt signatures passed");
  }
}
