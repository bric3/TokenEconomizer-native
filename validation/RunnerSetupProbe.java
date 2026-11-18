//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares constructor state, dependency calls and failures with the original runners. */
class RunnerSetupProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"runner_setup_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"runner_setup_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        var a="\"agent\":"+q(agent);
        compare(nativeCode,source,a);
        for(var model:List.of("null","false","0","\"\"","\"auto\"","\"MODEL-é😀\""))
          compare(nativeCode,source,a+",\"arguments\":{\"agent\":\"custom-agent\",\"model\":"+model+",\"scaffold\":\"fixture\"}");
        for(var variable:List.of("ANTHROPIC_MODEL","COPILOT_MODEL","CODEX_MODEL","TE_OPTIMIZER_MODEL","TE_KEEP_CONTAINERS"))for(var value:List.of("","0","1","fixture"))
          compare(nativeCode,source,a+",\"env\":{"+q(variable)+":"+q(value)+"}");
        for(var variable:List.of("TE_EFFORT_LEVEL","CLAUDE_CODE_EFFORT_LEVEL","COPILOT_EFFORT_LEVEL","CODEX_EFFORT_LEVEL"))for(var effort:List.of("","high","none","minimal","bad","HIGH"))
          compare(nativeCode,source,a+",\"env\":{"+q(variable)+":"+q(effort)+"}");
        for(var variable:List.of("TE_OPTIMIZER_MAX_TOKENS","TE_TASK_TIMEOUT","TE_EVAL_TIMEOUT"))for(var value:List.of("","0","-3","+4","١_٢"," 8 ","2.5","bad"))
          compare(nativeCode,source,a+",\"env\":{"+q(variable)+":"+q(value)+"}");
        for(var variable:List.of("TE_OPTIMIZER_MAX_TOKENS","TE_TASK_TIMEOUT","TE_EVAL_TIMEOUT"))for(var limit:List.of(0,640))for(var length:List.of(610,650))
          compare(nativeCode,source,a+",\"int_max_digits\":"+limit+",\"env\":{"+q(variable)+":"+q("7".repeat(length))+"}");
        compare(nativeCode,source,a+",\"no_scaffold\":true");
        compare(nativeCode,source,a+",\"falsey_scaffold\":true");
        for(var fail:List.of("scaffold.get","scaffold.preflight","log","grader","preflight","set:model","set:scaffold","set:task_timeout"))for(var error:List.of("ValueError","OSError","KeyboardInterrupt","ShadowValueError"))
          compare(nativeCode,source,a+",\"fail\":"+q(fail)+",\"error\":"+q(error));
      }
      for(var fail:List.of("key","claude_bin","weights","claude_version"))for(var error:List.of("RuntimeError","SystemExit"))
        compare(nativeCode,source,"\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(var fail:List.of("normalize","priceable","effort_applies","copilot_install","provider_base","endpoint","copilot_version"))for(var error:List.of("RuntimeError","SystemExit"))
        compare(nativeCode,source,"\"agent\":\"copilot\",\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(var model:List.of("null","\"auto\"","\"fixture\""))for(boolean applies:List.of(false,true))for(var endpoint:List.of("null","[]","[\"http://provider\",\"key\",true]"))
        compare(nativeCode,source,"\"agent\":\"copilot\",\"arguments\":{\"model\":"+model+"},\"effort_applies\":"+applies+",\"endpoint\":"+endpoint+",\"env\":{\"TE_EFFORT_APPLIES\":\"0\"}");
      for(var fail:List.of("routing","priceable","codex_install","codex_version","skills","base_prompt","lean","config","validate_config","verify","codex_preflight"))for(var error:List.of("RuntimeError","SystemExit"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(var skills:List.of("[]","[\"é😀\",\"a\"]","\"ab\""))for(boolean tuple:List.of(false,true))
        compare(nativeCode,source,"\"agent\":\"codex\",\"skills\":"+skills+",\"skills_tuple\":"+tuple);
      for(var prompt:List.of("null","\"\"","\"é😀 abc\""))for(boolean mutate:List.of(false,true))
        compare(nativeCode,source,"\"agent\":\"codex\",\"base_prompt\":"+prompt+",\"mutate_lean\":"+mutate);
      for(var verification:List.of("[true,\"\"]","[false,\"not served\"]","[null,\"unreadable\"]","[0,\"zero\"]","[1,\"one\"]","[true]","[true,\"\",3]"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"verification\":"+verification);
      for(var pair:List.of("[]","[\"shim\"]","[\"shim\",\"vendor\",\"extra\"]","null"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"install_pair\":"+pair);
      for(var bad:List.of("", "invalid type é😀"))compare(nativeCode,source,"\"agent\":\"codex\",\"bad_config\":"+q(bad));
      for(var routing:List.of("{}","{\"base_url\":\"base\"}","null"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"routing\":"+routing);
      for(var agent:List.of("claude","copilot","codex"))for(var model:List.of("",agent.equals("codex")?"gpt-5.6-sol":"claude-sonnet-5","unknown-fixture"))for(var env:List.of("{}","{\"ANTHROPIC_API_KEY\":\"fixture\",\"OPENAI_API_KEY\":\"fixture\"}","{\"ANTHROPIC_API_KEY\":\"fixture\",\"OPENAI_API_KEY\":\"fixture\",\"COPILOT_PROVIDER_BASE_URL\":\"http://fixture\",\"COPILOT_PROVIDER_TYPE\":\"anthropic\",\"COPILOT_PROVIDER_BEARER_TOKEN\":\"fixture\"}"))
        compare(nativeCode,source,"\"real_helpers\":true,\"agent\":"+q(agent)+",\"arguments\":{\"model\":"+q(model)+"},\"env\":"+env);
      for(var prompt:List.of("null","\"\"",q("# Personality\nBrief.\n# Code\nKeep é😀.\n")))
        compare(nativeCode,source,"\"real_helpers\":true,\"real_config\":true,\"agent\":\"codex\",\"env\":{\"OPENAI_API_KEY\":\"fixture\"},\"base_prompt\":"+prompt);
    }
    System.out.println("SWE-bench constructors match the source in "+cases+" comparisons; partial state, setup order, object identity and callback mutations passed without paid calls");
  }
}
