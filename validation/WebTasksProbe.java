//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Compares evaluator task-file loading, context exits and retained ID values. */
class WebTasksProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_tasks_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_tasks_host.py")) {
      compare(nativeCode,source,"");
      for(var context:List.of("null","false","0","1","\"\"","[]","{}"))compare(nativeCode,source,"\"context\":"+context);
      for(var method:List.of("__enter__","__exit__"))compare(nativeCode,source,"\"missing_special\":"+q(method));
      for(var line:List.of(""," ","\n","\u001c\u001f\u0085\u3000","{}","null","true","1","1.5","NaN","\"x\"","[]","{","{\"id\":}","{\"id\":1} trailing","\ufeff{\"id\":1}","{\"id\":\"init\"}","{\"id\":\"first\",\"id\":\"last\"}","{\"id\":\"é😀\\ud800\\udc00\"}","{\"id\":\"é😀\ud800\udc00\"}"))
        compare(nativeCode,source,"\"lines\":["+q(line)+"]");
      for(var value:List.of("null","true","false","0","-0","1.5","1e309","NaN","Infinity","-Infinity","[]","{}","[1,{\"key\":\"value\"}]"))
        compare(nativeCode,source,"\"lines\":["+q("{\"id\":"+value+"}")+"]");
      for(int digits:List.of(640,641,650))for(int limit:List.of(0,640))compare(nativeCode,source,"\"int_max_digits\":"+limit+",\"lines\":["+q("{\"id\":"+"7".repeat(digits)+"}")+"]");
      for(var fields:List.of("\"lines\":[]","\"lines\":[null]","\"lines\":[42]","\"lines\":[[]]","\"lines\":[{}]","\"opaque_lines\":true","\"opaque_lines\":true,\"opaque_stripped\":true","\"opaque_decoded\":true","\"opaque_decoded\":true,\"opaque_lines\":true,\"opaque_stripped\":true","\"opaque_lines\":true,\"opaque_stripped\":true,\"mutate_decode\":true","\"mutate_exit\":true","\"mutate_path\":true","\"objects\":[\"repo\",\"project\"]"))compare(nativeCode,source,fields);
      for(var value:List.of("null","false","0","1","\"\"","[]","{}","[\"{\\\"id\\\":1}\"]"))compare(nativeCode,source,"\"enter_value\":"+value);
      for(var value:List.of("null","false","0","1","\"\"","\"/other\"","[]","{}"))for(var key:List.of("repo","project"))compare(nativeCode,source,"\"fields\":{"+q(key)+":"+value+"}");
      for(var key:List.of("repo","project"))compare(nativeCode,source,"\"missing\":["+q(key)+"]");
      for(var failure:List.of("path.join","read:repo","read:project","join","open","bind:exit","bind:enter","enter","iter","next","loads","exit"))for(var error:List.of("OSError","FileNotFoundError","PermissionError","RuntimeError","ValueError","KeyboardInterrupt","DetailError","ShadowOSError"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var failure:List.of("strip:line","bool:stripped","get_function","get","bool:exit_result"))for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"opaque_lines\":true,\"opaque_stripped\":true,\"opaque_decoded\":true,\"opaque_exit\":true,\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(boolean suppress:List.of(false,true))for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"suppress\":"+suppress+",\"fail\":\"next\",\"fail_at\":2,\"error\":"+q(error));
      compare(nativeCode,source,"\"opaque_exit\":true");
      for(boolean truthy:List.of(false,true))compare(nativeCode,source,"\"opaque_exit\":true,\"lines\":[\"bad\"],\"booleans\":{\"exit_result\":"+truthy+"}");
      for(var fields:List.of("\"fail\":\"next\",\"fail_at\":2","\"lines\":[\"bad\"]","\"lines\":[\"bad\"],\"suppress\":true","\"lines\":[\"bad\"],\"fail\":\"exit\""))
        compare(nativeCode,source,"\"exception_state\":true,"+fields);
      var root=Files.createTempDirectory(Path.of("validation/results"),"web-tasks-").toAbsolutePath();
      var project=Files.createDirectories(root.resolve("projects/project"));
      var path=project.resolve("tasks.jsonl");
      for(var content:List.of("{\"id\":\"init\"}\r\n\r\n{\"id\":\"task-1\"}\r{\"id\":null}\n","{\"id\":\"é😀\"}\n","bad\n")) {
        Files.writeString(path,content);
        compare(nativeCode,source,"\"real_files\":true,\"fields\":{\"repo\":"+q(root)+"}");
      }
      Files.write(path,new byte[]{'{','"','i','d','"',':','"',(byte)0xff,'"','}'});
      compare(nativeCode,source,"\"real_files\":true,\"fields\":{\"repo\":"+q(root)+"}");
      Files.delete(path);
      compare(nativeCode,source,"\"real_files\":true,\"fields\":{\"repo\":"+q(root)+"}");
      try(var paths=Files.walk(root)){for(var item:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(item);}
    }
    System.out.println("Web-Bench task-file loading matches the source in "+cases+" comparisons; path and file errors, context exits, JSON values, line filtering and opaque IDs passed with real files");
  }
}
