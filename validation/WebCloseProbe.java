//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares evaluator resource cleanup, process-group escalation and failure boundaries. */
class WebCloseProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_close_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_close_host.py")) {
      compare(nativeCode,source,"");
      for(var poll:List.of("null","false","true","0","1","\"\"","\"0\"","[]","{}"))compare(nativeCode,source,"\"poll\":"+poll);
      for(var options:List.of("\"opaque_poll\":true","\"opaque_pid\":true,\"opaque_group\":true","\"repeat\":true","\"mutate_kill\":true,\"waits\":[\"TimeoutExpired\",\"done\"]","\"mutate_signals\":true,\"waits\":[\"TimeoutExpired\",\"done\"]"))compare(nativeCode,source,options);
      for(var waits:List.of("[\"TimeoutExpired\",\"done\"]","[\"TimeoutExpired\",\"TimeoutExpired\"]","[\"SilentTimeout\",\"done\"]","[\"ShadowTimeoutExpired\"]","[\"RuntimeError\"]","[\"PermissionError\"]","[\"KeyboardInterrupt\"]"))compare(nativeCode,source,"\"waits\":"+waits);
      for(var field:List.of("httpd","proc","_log_fh")) {
        compare(nativeCode,source,"\"missing\":["+q(field)+"]");
        for(var value:List.of("null","false","0","[]","{}"))compare(nativeCode,source,"\"fields\":{"+q(field)+":"+value+"}");
      }
      for(var failure:List.of("discard","read:httpd","server.shutdown","shutdown","server.server_close","server_close","read:proc","proc.poll","poll","signal.SIGTERM","signal.SIGKILL","proc.pid","pid","getpgid","killpg","proc.wait","wait","read:_log_fh","file.close","close","write:finished"))for(var error:List.of("RuntimeError","OSError","PermissionError","ProcessLookupError","TimeoutExpired","SilentTimeout","ShadowPermissionError","KeyboardInterrupt","DetailError"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var failure:List.of("read:httpd","read:proc","pid","getpgid","killpg","wait"))compare(nativeCode,source,"\"waits\":[\"TimeoutExpired\",\"done\"],\"fail\":"+q(failure)+",\"fail_at\":2,\"error\":\"PermissionError\"");
    }
    System.out.println("Web-Bench evaluator cleanup matches the source in "+cases+" comparisons; registry order, HTTP shutdown, process groups, escalation, file close and exception boundaries passed");
  }
}
