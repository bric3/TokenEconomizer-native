//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares evaluator event and request waits with virtual clocks and local queues. */
class WebSessionProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_session_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_session_host.py")) {
      for(var method:List.of("next_event","next_request")) {
        var m="\"method\":"+q(method);
        compare(nativeCode,source,m);
        for(var timeout:List.of("null","false","true","-1","0","0.01","1","5","6","20","NaN","Infinity","-Infinity","\"1\"","[]","{}"))
          compare(nativeCode,source,m+",\"arguments\":{\"timeout\":"+timeout+"}");
        for(var times:List.of("[10,9,10,11,20]","[10,10.001,11,20]","[10,20]","[10,30]","[10,10,10,10,20]","[10,NaN,20]","[10,Infinity]","[10,-Infinity,20]"))
          compare(nativeCode,source,m+",\"times\":"+times);
        for(var values:List.of("[]","[null]","[false]","[0]","[\"x\"]","[[]]","[{}]","[{\"kind\":\"exit\"}]","[{\"kind\":\"verdict\",\"task\":\"task-1\",\"attempt\":2},{\"kind\":\"started\"}]","[{\"raise\":\"Empty\"},{\"kind\":\"started\"}]","[{\"raise\":\"SilentEmpty\"}]","[{\"raise\":\"ShadowEmpty\"}]"))for(boolean finished:List.of(false,true))
          compare(nativeCode,source,m+",\"queue\":"+values+",\"fields\":{\"finished\":"+finished+"}");
        for(var object:List.of("timeout","kind"))compare(nativeCode,source,m+",\"objects\":["+q(object)+"],\"binding_error\":true");
        for(var missing:List.of("timeout","kind"))compare(nativeCode,source,m+",\"omit\":["+q(missing)+"],\"binding_error\":true");
        compare(nativeCode,source,m+",\"arguments\":{\"unexpected\":true},\"binding_error\":true");
        compare(nativeCode,source,m+",\"opaque_clock\":true");
        for(boolean cap:List.of(false,true))compare(nativeCode,source,m+",\"objects\":[\"timeout\"],\"booleans\":{\"capped\":"+cap+"},\"mutate_get\":true");
        compare(nativeCode,source,m+",\"objects\":[\"timeout\"],\"booleans\":{\"expired\":true}");
        compare(nativeCode,source,m+",\"queue\":[{\"raise\":\"Empty\"}],\"field_objects\":[\"finished\"]");
        for(var failure:List.of("time","read:events","read:requests","queue.get_function","queue.get","read:finished"))for(var error:List.of("Empty","SilentEmpty","ShadowEmpty","RuntimeError","KeyboardInterrupt","DetailError"))
          compare(nativeCode,source,m+",\"queue\":[],\"fail\":"+q(failure)+",\"error\":"+q(error));
        for(var failure:List.of("radd:timeout","sub:deadline","le:left","bool:expired","gt:left","bool:capped"))for(var error:List.of("Empty","RuntimeError"))
          compare(nativeCode,source,m+",\"objects\":[\"timeout\"],\"fail\":"+q(failure)+",\"error\":"+q(error));
        for(var missing:List.of("events","requests","finished"))compare(nativeCode,source,m+",\"queue\":[],\"missing\":["+q(missing)+"]");
        for(var value:List.of("{\"kind\":\"started\",\"task\":\"task-1\"}","{\"kind\":\"exit\"}"))
          compare(nativeCode,source,m+",\"real_queue\":true,\"queue\":["+value+"]");
        var empty=compare(nativeCode,source,m+",\"real_queue\":true,\"queue\":[],\"times\":[10,10],\"arguments\":{\"timeout\":0.001}");
        if(!empty.contains(method.equals("next_event")?"\"value\":null":"\"value\":[null,null]"))throw new AssertionError("real queue timeout failed: "+empty);
      }
      for(var kind:List.of("null","false","0","\"started\"","\"verdict\"","\"exit\"","\"other\"","[]","{}"))
        compare(nativeCode,source,"\"arguments\":{\"kind\":"+kind+"},\"opaque_event\":true");
      compare(nativeCode,source,"\"opaque_event\":true,\"kind_values\":[\"other\",\"started\"]");
      compare(nativeCode,source,"\"opaque_event\":true,\"kind_values\":[\"other\",\"other\",\"changed\"]");
      compare(nativeCode,source,"\"event_objects\":[\"kind\"],\"opaque_comparison\":true,\"booleans\":{\"equal_exit\":false,\"equal_kind\":true}");
      var unexpected="\"opaque_event\":true,\"event_objects\":[\"kind\",\"task\",\"attempt\"],\"field_objects\":[\"project\"],\"queue\":[{\"kind\":\"other\"}],\"equal\":false";
      compare(nativeCode,source,unexpected+",\"mutate_log\":true");
      for(var failure:List.of("event.item","event.get","read:project","format:project","format:kind","format:task","format:attempt","log"))for(var error:List.of("Empty","RuntimeError","KeyboardInterrupt","DetailError"))
        compare(nativeCode,source,unexpected+",\"fail\":"+q(failure)+",\"error\":"+q(error));
      compare(nativeCode,source,"\"queue\":[{\"kind\":\"other é😀\\ud800\\udc00\",\"task\":\"é😀\\ud800\\udc00\"}]");
    }
    System.out.println("Web-Bench evaluator waits match the source in "+cases+" comparisons; deadlines, polling bounds, empty queues, event order, logs, exceptions and raw result identities passed with local queues");
  }
}
