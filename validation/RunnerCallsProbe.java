//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares runner transports, preflight wrappers and factory decisions across both domains. */
class RunnerCallsProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    boolean sdk=List.of(args).contains("--sdk");
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"runner_calls_host.py",sdk?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"runner_calls_host.py")) {
      for(boolean web:List.of(false,true))for(var agent:List.of("claude","copilot","codex")) {
        var base="\"webbench\":"+web+",\"agent\":"+q(agent);
        for(var mode:List.of("optimizer","preflight")) {
          var call=base+",\"mode\":"+q(mode);
          compare(nativeCode,source,call);
          compare(nativeCode,source,call+",\"result_tuple\":true");
          compare(nativeCode,source,call+",\"null_result\":true");
          for(var endpoint:List.of("null","false","0","\"abc\"","\"too long\"","[]","{}","[1]","[1,2]","[1,2,3]","[1,2,3,4]"))
            compare(nativeCode,source,call+",\"fields\":{\"optimizer_endpoint\":"+endpoint+"}");
          compare(nativeCode,source,call+",\"opaque_endpoint\":true");
          compare(nativeCode,source,call+",\"opaque_endpoint\":true,\"mutate_unpack\":true");
          for(var field:List.of("optimizer_model","optimizer_max_tokens","api_key","copilot","model","routing","optimizer_endpoint")) {
            compare(nativeCode,source,call+",\"missing\":["+q(field)+"]");
            compare(nativeCode,source,call+",\"mutate_field\":"+q(field));
          }
          for(var helper:List.of("anthropic","copilot","codex","preflight","codex_preflight"))for(var error:List.of("RuntimeError","KeyboardInterrupt","ShadowException","DetailError"))
            compare(nativeCode,source,call+",\"fail\":"+q(helper)+",\"error\":"+q(error));
        }
        for(var model:List.of("null","false","0","1","\"agent\"","\"claude\"","\"copilot\"","\"codex\"","\"AGENT\"","\"override\"","[]","{}","[1]"))
          compare(nativeCode,source,base+",\"arguments\":{\"model\":"+model+"}");
        for(boolean equal:List.of(false,true)) {
          compare(nativeCode,source,base+",\"objects\":[\"model\"],\"equal\":"+equal);
          compare(nativeCode,source,base+",\"objects\":[\"model\"],\"opaque_comparison\":true,\"truthy\":"+equal);
        }
        compare(nativeCode,source,base+",\"objects\":[\"model\"],\"mutate_model\":true");
        compare(nativeCode,source,base+",\"endpoints\":[[1,2,3],[4,5,6]]");
        for(var mode:List.of("optimizer","preflight"))
          compare(nativeCode,source,base+",\"mode\":"+q(mode)+",\"real_helpers\":true,\"fields\":{\"optimizer_endpoint\":[\"http://fixture\",\"fixture-key\",\"\"]}");
      }
      for(boolean web:List.of(false,true)) {
        var base="\"mode\":\"factory\",\"webbench\":"+web;
        for(var agent:List.of("null","false","0","1","\"\"","\"CLAUDE\"","\"copilot\"","\"Codex\"","\"missing\"","\" claude \"","\"é😀\"","[]","{}","[1]"))for(var env:List.of("{}","{\"TE_AGENT\":\"CoPiLoT\"}"))
          compare(nativeCode,source,base+",\"arguments\":{\"agent\":"+agent+"},\"env\":"+env);
        for(var scaffold:List.of("null","false","0","1","\"\"","\"rtk\"","\"repomap\"","\"serena\"","\"missing\"","[]","{}","[1]"))for(var env:List.of("{}","{\"TE_SCAFFOLD\":\"rtk\"}"))
          compare(nativeCode,source,base+",\"arguments\":{\"scaffold\":"+scaffold+"},\"env\":"+env);
        compare(nativeCode,source,base+",\"objects\":[\"agent\",\"extra\"]");
        compare(nativeCode,source,base+",\"objects\":[\"scaffold\"]");
        compare(nativeCode,source,base+",\"mutate_cleanup\":true");
        for(var keys:List.of("[]","[\"z\",\"a\"]","[\"é😀\",\"a\",\"z\"]","[1,2]","[\"a\",1]"))
          compare(nativeCode,source,base+",\"keys\":"+keys+",\"arguments\":{\"agent\":\"missing\"}");
        compare(nativeCode,source,base+",\"keys\":[\"z\",\"a\",\"c\"],\"foreign_keys\":true,\"arguments\":{\"agent\":\"missing\"}");
        compare(nativeCode,source,base+",\"paired_key\":true,\"arguments\":{\"agent\":\"missing\"}");
        for(var failure:List.of("env.get","registered","cleanup","lookup","keys","key_count","construct:claude","lower:agent"))for(var error:List.of("RuntimeError","TypeError","KeyboardInterrupt","DetailError")) {
          var fields=failure.equals("lower:agent")?",\"objects\":[\"agent\"]":failure.equals("keys")||failure.equals("key_count")?",\"arguments\":{\"agent\":\"missing\"}":"";
          compare(nativeCode,source,base+fields+",\"fail\":"+q(failure)+",\"error\":"+q(error));
        }
      }
    }
    System.out.println("Runner calls and factories match the source in "+cases+" comparisons; both domains, optimizer and preflight wrappers, inheritance, getter order, model routing and opaque identities passed");
  }
}
