//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Compares stream starts, ownership checks and cleanup without running evaluators. */
class WebStreamProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_stream_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_stream_host.py")) {
      for(var agent:List.of("claude","copilot","codex")) {
        var a="\"agent\":"+q(agent);
        for(var method:List.of("begin_stream","end_stream","session_for","close_session")) {
          var call=a+",\"method\":"+q(method);
          compare(nativeCode,source,call);
          for(var session:List.of("null","false","0","\"\"","[]","{}"))compare(nativeCode,source,call+",\"fields\":{\"_session\":"+session+"}");
          for(var missing:List.of("_session","_session_project","repo","retry","node","task_timeout","max_workers"))compare(nativeCode,source,call+",\"missing\":["+q(missing)+"]");
        }
        compare(nativeCode,source,a+",\"lifecycle\":true");
        compare(nativeCode,source,a+",\"override_close\":true");
      }
      for(var repo:List.of("null","false","0","1","\"\"","\"project\"","\"owner/project\"","\"owner/project/\"","\"/\"","\"project é😀\"","[]","{}","[1]"))for(var tasks:List.of("null","false","0","1","[]","{}","[{}]","[null]","[{\"repo\":\"owner/fallback\"}]","[{\"repo\":null}]","[{\"repo\":1}]"))
        compare(nativeCode,source,"\"arguments\":{\"repo\":"+repo+",\"tasks\":"+tasks+"}");
      for(var root:List.of("null","false","0","1","\"\"","\"relative\"","\"/results é😀\"","[]","{}","[1]"))compare(nativeCode,source,"\"arguments\":{\"results_root\":"+root+"}");
      for(var key:List.of("repo","retry","task_timeout","max_workers"))for(var value:List.of("null","false","0","1","-1","2.5","\"x\"","[]","[1]","{}"))compare(nativeCode,source,"\"fields\":{"+q(key)+":"+value+"}");
      for(var objects:List.of("[\"tasks\"]","[\"repo\"]","[\"results_root\"]","[\"repo\",\"results_root\"]"))compare(nativeCode,source,"\"objects\":"+objects);
      compare(nativeCode,source,"\"objects\":[\"tasks\"],\"mutate_list\":true");
      compare(nativeCode,source,"\"objects\":[\"tasks\"],\"booleans\":{\"tasks\":false}");
      compare(nativeCode,source,"\"opaque_stream\":true");
      compare(nativeCode,source,"\"opaque_exists\":true,\"booleans\":{\"exists\":false}");
      compare(nativeCode,source,"\"mutate_path\":true");
      compare(nativeCode,source,"\"field_objects\":[\"task_timeout\",\"retry\"],\"mutate_factory\":true");
      compare(nativeCode,source,"\"mutate_close\":true");
      for(var project:List.of("null","false","0","1","\"\"","\"project\"","\"other\"","\"é😀\\ud800\\udc00\"","[]","{}"))compare(nativeCode,source,"\"method\":\"session_for\",\"arguments\":{\"project\":"+project+"}");
      for(boolean different:List.of(false,true))compare(nativeCode,source,"\"method\":\"session_for\",\"field_objects\":[\"_session_project\"],\"different\":"+different+",\"opaque_comparison\":true,\"booleans\":{\"comparison\":"+different+"}");
      compare(nativeCode,source,"\"method\":\"session_for\",\"read_values\":{\"_session_project\":[\"other\",\"changed\"]}");
      for(var method:List.of("begin_stream","end_stream","session_for","close_session")) {
        var failures=switch(method) {
          case "begin_stream" -> List.of("list","read:_stream","write:_stream","path.isdir","path.join","join","isdir","read:close_session","read:_session","session.get.close","session.close","write:_session","write:_session_project","write:_stream_broken","getcwd","read:repo","read:retry","read:node","read:task_timeout","read:max_workers","factory");
          case "session_for" -> List.of("read:_session","read:_session_project");
          case "end_stream" -> List.of("read:close_session","read:_session","session.get.close","session.close","write:_session","write:_session_project");
          default -> List.of("read:_session","session.get.close","session.close","write:_session","write:_session_project");
        };
        for(var failure:failures)for(var error:List.of("RuntimeError","KeyboardInterrupt","DetailError"))
          compare(nativeCode,source,"\"method\":"+q(method)+",\"fail\":"+q(failure)+",\"error\":"+q(error));
      }
      for(var failure:List.of("bool:tasks","iter:tasks","len:tasks","split:repo","split:piece","bool:piece","bool:project","format:project","fspath:project","bool:results_root","fspath:results_root","add:retry","mul:task_timeout"))
        compare(nativeCode,source,"\"objects\":[\"tasks\",\"repo\",\"results_root\"],\"field_objects\":[\"retry\",\"task_timeout\"],\"fail\":"+q(failure));
      for(var failure:List.of("ne:_session_project","bool:comparison","repr:_session_project","repr:project"))
        compare(nativeCode,source,"\"method\":\"session_for\",\"field_objects\":[\"_session_project\"],\"objects\":[\"project\"],\"different\":true,\"opaque_comparison\":true,\"fail\":"+q(failure));
      for(var key:List.of("retry","task_timeout"))for(var value:List.of("7".repeat(650),"-"+"7".repeat(650)))compare(nativeCode,source,"\"fields\":{"+q(key)+":"+value+"}");
      compare(nativeCode,source,"\"method\":\"end_stream\",\"override_close\":true");
      compare(nativeCode,source,"\"arguments\":{\"repo\":\"é😀\\ud800\\udc00\",\"extra\":42}");
      compare(nativeCode,source,"\"arguments\":{\"tasks\":[{\"repo\":\"project\"}]}");
      var root=Files.createTempDirectory(Path.of("validation/results"),"web-stream-").toAbsolutePath();
      Files.createDirectories(root.resolve("projects/project"));
      for(var project:List.of("project","absent")) {
        var result=compare(nativeCode,source,"\"real_paths\":true,\"fields\":{\"repo\":"+q(root)+"},\"arguments\":{\"repo\":"+q(project)+"}");
        if(!result.contains(project.equals("project")?"\"session\":\"created\"":"no such project"))throw new AssertionError("real project check failed: "+result);
      }
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Web-Bench stream coordination matches the source in "+cases+" comparisons; project selection, evaluator arguments, ownership, cleanup, partial state and opaque identities passed without starting evaluators");
  }
}
