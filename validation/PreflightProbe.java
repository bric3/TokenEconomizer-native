//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares preflight gates, call arguments, failures, aliases and cost formatting. */
class PreflightProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"preflight_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"preflight_host.py")) {
      for(var agent:List.of("claude","codex")) {
        var a="\"agent\":"+q(agent);
        compare(nativeCode,source,a);
        for(var gate:List.of("null","0","false","\"\"","\"0\"","\"1\"","\" 0 \""))for(var probe:List.of("null","false","0","1","[]","[1]","{}","\"\"","\" \"","\"  ok é😀 \\n\""))
          compare(nativeCode,source,a+",\"env\":{\"TE_OPTIMIZER_PREFLIGHT\":"+gate+"},\"reply\":["+probe+",1.5]");
        for(var fail:List.of("get","optimizer","log"))for(var error:List.of("RuntimeError","ValueError","OSError","KeyboardInterrupt","SystemExit","ShadowValueError"))
          compare(nativeCode,source,a+",\"env\":{\"TE_OPTIMIZER_PREFLIGHT\":\"0\"},\"fail\":"+q(fail)+",\"error\":"+q(error));
        for(var reply:List.of("null","[]","[\"ok\"]","[\"ok\",1,2]"))compare(nativeCode,source,a+",\"reply\":"+reply);
        for(boolean truth:List.of(false,true))for(var stripped:List.of("\"ok\"","\"\"","null","1"))
          compare(nativeCode,source,a+",\"objects\":[\"probe\",\"effective\",\"configured_model\"],\"truth\":"+truth+",\"stripped\":"+stripped);
        for(var reply:List.of("[\"ok\",2.5]","[\"\",2.5]"))compare(nativeCode,source,a+",\"reply\":"+reply+",\"mutate_log\":true");
        for(var error:List.of("RuntimeError","ValueError","OSError","KeyboardInterrupt","SystemExit","ShadowValueError"))compare(nativeCode,source,a+",\"fail\":\"optimizer\",\"error\":"+q(error));
      }
      for(var cost:List.of("0","-1.5","0.5","1.5","2.5","1e100","NaN","Infinity","null","\"3\"","false"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"reply\":[\"ok\","+cost+"]");
      for(var routing:List.of("null","{}","{\"base_url\":null}"))for(var probe:List.of("\"ok\"","\"\""))
        compare(nativeCode,source,"\"agent\":\"codex\",\"arguments\":{\"routing\":"+routing+"},\"reply\":["+probe+",1]");
      compare(nativeCode,source,"\"objects\":[\"label\",\"remedy\",\"base_url\"],\"reply\":[\"\",0]");
      compare(nativeCode,source,"\"objects\":[\"label\"],\"env\":{\"TE_OPTIMIZER_PREFLIGHT\":\"0\"}");
      compare(nativeCode,source,"\"objects\":[\"label\"],\"env\":{\"TE_OPTIMIZER_PREFLIGHT\":\"0\"},\"format_mutate_log\":true");
      compare(nativeCode,source,"\"agent\":\"codex\",\"objects\":[\"configured_model\"],\"format_mutate_log\":true");
      compare(nativeCode,source,"\"opaque_log_result\":true,\"env\":{\"TE_OPTIMIZER_PREFLIGHT\":\"0\"}");
      compare(nativeCode,source,"\"agent\":\"codex\",\"opaque_log_result\":true");
    }
    System.out.println("Endpoint preflights match the source in "+cases+" comparisons; gating, argument identity, failure ordering, logger capture and cost formatting passed without provider calls");
  }
}
