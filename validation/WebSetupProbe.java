//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares Web-Bench constructors, partial state and shared stream initialisation. */
class WebSetupProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_setup_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_setup_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        var a="\"agent\":"+q(agent);
        compare(nativeCode,source,a);
        compare(nativeCode,source,a+",\"getters\":true");
        compare(nativeCode,source,a+",\"stream_only\":true");
        compare(nativeCode,source,a+",\"stream_override\":true");
        for(var model:List.of("null","false","0","1","\"\"","\"auto\"","\"MODEL-é😀\""))
          compare(nativeCode,source,a+",\"arguments\":{\"model\":"+model+"}");
        for(var scaffold:List.of("null","false","0","1","\"\"","\"rtk\"","\"repomap\"","[]","[1]"))
          compare(nativeCode,source,a+",\"arguments\":{\"scaffold\":"+scaffold+"}");
        for(var variable:List.of("ANTHROPIC_MODEL","COPILOT_MODEL","CODEX_MODEL","TE_OPTIMIZER_MODEL","TE_KEEP_WORKSPACES"))for(var value:List.of("","0","1","fixture"))
          compare(nativeCode,source,a+",\"env\":{"+q(variable)+":"+q(value)+"}");
        for(var variable:List.of("TE_EFFORT_LEVEL","CLAUDE_CODE_EFFORT_LEVEL","COPILOT_EFFORT_LEVEL","CODEX_EFFORT_LEVEL"))for(var effort:List.of("","high","none","minimal","bad","HIGH"))
          compare(nativeCode,source,a+",\"env\":{"+q(variable)+":"+q(effort)+"}");
        for(var variable:List.of("TE_OPTIMIZER_MAX_TOKENS","TE_WEBBENCH_RETRY","TE_WEBBENCH_MAX_WORKERS","TE_WEBBENCH_VERDICT_TIMEOUT","TE_WEBBENCH_START_TIMEOUT")) {
          for(var value:List.of("\"\"","\"0\"","\"-3\"","\"+4\"",q("١_٢"),"\" 8 \"","\"2.5\"","\"bad\"","null","true","2.9","NaN","Infinity","[]","\"\\ud800\""))
            compare(nativeCode,source,a+",\"env\":{"+q(variable)+":"+value+"}");
          for(int digits:List.of(640,641,650))for(int limit:List.of(0,640))
            compare(nativeCode,source,a+",\"int_max_digits\":"+limit+",\"env\":{"+q(variable)+":"+q("7".repeat(digits))+"}");
          compare(nativeCode,source,a+",\"env_objects\":["+q(variable)+"]");
        }
        for(var option:List.of("\"no_scaffold\":true","\"falsey_scaffold\":true","\"mutate_profile\":true","\"mutate_format\":true,\"objects\":[\"model\"]","\"objects\":[\"repo\"]"))compare(nativeCode,source,a+","+option);
        for(var failure:List.of("key","weights","scaffold.get","profile","scaffold.preflight","node","task_timeout","stream","preflight","namespace","set:model","set:repo","set:retry","set:start_timeout","set:_stream"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError","ShadowException"))
          compare(nativeCode,source,a+",\"fail\":"+q(failure)+",\"error\":"+q(error));
      }
      for(var failure:List.of("normalize","priceable","effort_applies","copilot_install","provider_base","endpoint","copilot_version"))for(var error:List.of("RuntimeError","SystemExit"))
        compare(nativeCode,source,"\"agent\":\"copilot\",\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var model:List.of("null","\"auto\"","\"fixture\""))for(boolean applies:List.of(false,true))for(var endpoint:List.of("null","[]","[\"http://fixture\",\"key\",true]"))
        compare(nativeCode,source,"\"agent\":\"copilot\",\"getters\":true,\"arguments\":{\"model\":"+model+"},\"effort_applies\":"+applies+",\"endpoint\":"+endpoint+",\"env\":{\"TE_EFFORT_APPLIES\":\"0\"}");
      for(var failure:List.of("routing","priceable","verify","codex_install","vendor","codex_bin","codex_version","base_prompt","skills"))for(var error:List.of("RuntimeError","SystemExit"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var skills:List.of("null","[]","false","\"ab\"","[\"z\",\"a\"]"))compare(nativeCode,source,"\"agent\":\"codex\",\"skills\":"+skills);
      for(var verified:List.of("null","false","[true,\"\"]","[false,\"not served\"]"))compare(nativeCode,source,"\"agent\":\"codex\",\"verification\":"+verified);
      for(var routing:List.of("null","{}","{\"api_key\":\"fixture\"}","{\"base_url\":\"base\"}"))compare(nativeCode,source,"\"agent\":\"codex\",\"routing\":"+routing);
      for(var agent:List.of("claude","copilot","codex"))for(var model:List.of("",agent.equals("codex")?"gpt-5.6-sol":"claude-sonnet-5","unknown-fixture"))for(var env:List.of("{}","{\"ANTHROPIC_API_KEY\":\"fixture\",\"OPENAI_API_KEY\":\"fixture\"}","{\"ANTHROPIC_API_KEY\":\"fixture\",\"OPENAI_API_KEY\":\"fixture\",\"COPILOT_PROVIDER_BASE_URL\":\"http://fixture\",\"COPILOT_PROVIDER_TYPE\":\"anthropic\",\"COPILOT_PROVIDER_BEARER_TOKEN\":\"fixture\"}"))
        compare(nativeCode,source,"\"real_helpers\":true,\"agent\":"+q(agent)+",\"arguments\":{\"model\":"+q(model)+"},\"env\":"+env);
    }
    System.out.println("Web-Bench setup matches the source in "+cases+" comparisons; three constructors, partial state, stream reset, evaluation order and opaque helper identities passed without paid calls");
  }
}
