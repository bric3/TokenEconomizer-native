//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares controller construction, continuation and telemetry with SDK stand-ins. */
class RunnerControlProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"control_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"control_host.py")) {
      compare(nativeCode,source,"");
      for(var agent:List.of("claude","copilot","codex"))for(var value:List.of("null","false","true","0","1","[]","{}","{\"cold_start\":3}"))
        compare(nativeCode,source,"\"agent\":"+q(agent)+",\"kwargs\":{"+q(agent.equals("codex")?"codex_spiral_policy":"spiral_guard")+":"+value+"}");
      for(var agent:List.of("claude","copilot","codex"))for(var verbose:List.of("1","0","true"," 1 ",""))for(boolean supported:List.of(false,true))
        compare(nativeCode,source,"\"agent\":"+q(agent)+",\"verbose\":"+q(verbose)+",\"supports_never_abort\":"+supported+",\"kwargs\":{\"spiral_guard\":true,\"spiral_never_abort\":true},\"objects\":[\"abort_envelope\",\"spiral_judge\",\"spiral_thr\",\"spiral_nudge_writer\",\"codex_spiral_policy\"]");
      for(var budget:List.of("null","false","true","0","-3","2.8","-0.2","\"١_٢\"","\"bad\"","[]","{}",q("7".repeat(650))))for(boolean nudge:List.of(false,true))
        compare(nativeCode,source,"\"supports_never_abort\":true,\"int_max_digits\":640,\"kwargs\":{\"spiral_guard\":true,\"spiral_nudge\":"+nudge+",\"spiral_nudge_budget\":"+budget+"}");
      for(var key:List.of("spiral_guard","spiral_nudge","spiral_never_abort","spiral_nudge_budget"))for(boolean truth:List.of(false,true))
        compare(nativeCode,source,"\"supports_never_abort\":true,\"truth\":"+truth+",\"kwargs\":{\"spiral_guard\":true,\"spiral_nudge_budget\":2},\"objects\":["+q(key)+"]");
      for(var param:List.of("\"never_abort\"","\"envelope\"","\"unknown\"","null","[]","{}","42"))for(boolean supported:List.of(false,true))
        compare(nativeCode,source,"\"mode\":\"accepts\",\"param\":"+param+",\"supports_never_abort\":"+supported);
      for(var error:List.of("TypeError","ValueError","OSError","RuntimeError","KeyboardInterrupt","SystemExit","ShadowTypeError"))for(var mode:List.of("accepts","build"))
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"fail\":\"signature\",\"error\":"+q(error)+",\"kwargs\":{\"spiral_guard\":true}");
      for(var agent:List.of("claude","codex"))for(var error:List.of("RuntimeError","ValueError","KeyboardInterrupt","SystemExit"))
        compare(nativeCode,source,"\"agent\":"+q(agent)+",\"fail\":\"constructor\",\"error\":"+q(error)+",\"kwargs\":{\"spiral_guard\":true,\"codex_spiral_policy\":{} }");
      for(var agent:List.of("claude","copilot","codex")) {
        compare(nativeCode,source,"\"mode\":\"row\",\"agent\":"+q(agent));
        compare(nativeCode,source,"\"mode\":\"row\",\"agent\":"+q(agent)+",\"missing_suppressed\":true,\"changing_aborted\":true");
        compare(nativeCode,source,"\"mode\":\"continuation\",\"agent\":"+q(agent));
        for(var error:List.of("RuntimeError","KeyboardInterrupt"))compare(nativeCode,source,"\"mode\":\"continuation\",\"agent\":"+q(agent)+",\"fail\":\"continuation\",\"error\":"+q(error));
      }
      for(var attr:List.of("aborted","nudges_done","abort_suppressed","events","judge_tokens","nudge_writer_tokens"))for(var error:List.of("AttributeError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"mode\":\"row\",\"fail\":"+q("attr:"+attr)+",\"error\":"+q(error));
      for(var tel:List.of("null","[]","{}","{\"snapshot\":null}","{\"snapshot\":42}","{\"snapshot\":[],\"nudges\":1,\"events\":[],\"thread_id_captured\":false,\"paid_judge_effective_tokens\":0,\"policy\":{}}"))
        compare(nativeCode,source,"\"mode\":\"row\",\"agent\":\"codex\",\"telemetry\":"+tel);
      compare(nativeCode,source,"\"mode\":\"row\",\"agent\":\"codex\",\"falsey_snapshot\":true");
      for(var fail:List.of("telemetry","tel.get","snapshot.bool","snapshot.get","tel.item"))for(var error:List.of("AttributeError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"mode\":\"row\",\"agent\":\"codex\",\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(var agent:List.of("claude","copilot"))for(var action:List.of("nudge","abort",""))for(boolean enabled:List.of(false,true))
        compare(nativeCode,source,"\"mode\":\"real\",\"agent\":"+q(agent)+",\"action\":"+q(action)+",\"verbose\":\"0\",\"kwargs\":{\"spiral_guard\":"+enabled+",\"spiral_nudge\":true,\"spiral_thr\":{\"min_turns\":5}}");
      for(var reason:List.of("post_green_finalize","green_reverification_loop",""))for(var effort:List.of("","high"))
        compare(nativeCode,source,"\"mode\":\"real\",\"agent\":\"codex\",\"verbose\":\"0\",\"reason\":"+q(reason)+",\"kwargs\":{\"codex_spiral_policy\":{\"resume_effort\":"+q(effort)+"}}");
    }
    System.out.println("Runner controls match the source in "+cases+" comparisons; constructor order, object identity, budget conversion, optional parameters, continuation and telemetry passed");
  }
}
