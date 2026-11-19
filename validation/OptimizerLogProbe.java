//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Checks optimizer record bytes, failure handling, verbosity and real file/stream I/O. */
class OptimizerLogProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"optimizer-log-").toAbsolutePath();
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"optimizer_log_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"optimizer_log_host.py")) {
      compare(nativeCode,source,"");
      compare(nativeCode,source,"\"exception_state\":true");
      for(var context:List.of("null","false","0","1","\"\"","[]","{}"))compare(nativeCode,source,"\"file_context\":"+context);
      for(var method:List.of("__enter__","__exit__"))compare(nativeCode,source,"\"missing_special\":"+q(method));
      for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt"))for(boolean suppress:List.of(false,true))
        compare(nativeCode,source,"\"exception_state\":true,\"fail\":\"write\",\"error\":"+q(error)+",\"suppress\":"+suppress);
      compare(nativeCode,source,"\"exception_state\":true,\"fail\":\"write\",\"exit_error\":\"KeyboardInterrupt\"");
      for(var path:List.of("null","false","0","\"\"","[]","{}","[1]"))compare(nativeCode,source,"\"path\":"+path);
      for(var context:List.of("null","false","[]","[[\"field\",1]]","{}","{\"ts\":\"overridden\",\"kind\":\"old\",\"model\":\"old\",\"response\":\"old\"}","{\"shared\":[1],\"usage\":{\"old\":true}}"))compare(nativeCode,source,"\"context\":"+context);
      compare(nativeCode,source,"\"context\":{\"shared\":[1]},\"mutate_context\":true");
      for(var options:List.of("\"mutate_makedirs\":true","\"mutate_time\":true","\"mutate_write\":true","\"mutate_exit\":true","\"opaque_path\":true","\"opaque_exit\":true","\"static_context\":true","\"opaque_timestamp\":true","\"opaque_timestamp\":true,\"context\":{\"ts\":\"overridden\"}"))compare(nativeCode,source,options);
      for(int size:List.of(0,1,399,400,401,799,800,801,3999,4000,4001))for(var character:List.of("a","😀","\ud800"))
        compare(nativeCode,source,"\"arguments\":{\"prompt\":"+q(character.repeat(size))+",\"text\":"+q(character.repeat(size))+"}");
      for(var slot:List.of("prompt","text","usage","model","stop_reason"))for(var value:List.of("null","false","0","1","\"\"","[]","{}","[1]","NaN","Infinity","-Infinity"))
        compare(nativeCode,source,"\"arguments\":{"+q(slot)+":"+value+"}");
      for(var object:List.of("prompt","text","usage","model","stop_reason"))compare(nativeCode,source,"\"objects\":["+q(object)+"]");
      compare(nativeCode,source,"\"objects\":[\"prompt\"],\"opaque_slice\":true");
      compare(nativeCode,source,"\"objects\":[\"stop_reason\"],\"opaque_equality\":true");
      for(var failure:List.of("path","dirname","makedirs","open","enter","write.lookup","gmtime","strftime","kind","write","exit","error.log","slice:prompt","len:prompt"))
        for(var error:List.of("RuntimeError","OSError","StopIteration","KeyboardInterrupt","SystemExit","GeneratorExit","ShadowException")) {
          String extra=failure.contains(":prompt")?",\"objects\":[\"prompt\"]":failure.equals("error.log")?",\"objects\":[\"usage\"]":"";
          compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error)+extra);
        }
      for(var error:List.of("RuntimeError","StopIteration","KeyboardInterrupt"))for(var suppress:List.of("\"suppress\":true","\"opaque_exit\":true"))
        compare(nativeCode,source,"\"fail\":\"write\",\"error\":"+q(error)+","+suppress);
      for(var failure:List.of("write","kind"))for(var error:List.of("RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"exit_error\":"+q(error));
      for(var options:List.of("",",\"mutate_error\":true",",\"error_text\":\"\\ud800\"",",\"fail_format\":true"))
        compare(nativeCode,source,"\"fail\":\"write\",\"error\":\"DetailError\""+options);
      for(var gate:List.of("null","false","0","1","\"\"","\"0\"","\"1\"","\"true\"","[]","{}"))for(boolean web:List.of(false,true))
        compare(nativeCode,source,"\"mode\":\"verbose\",\"webbench\":"+web+",\"env\":{\"TE_VERBOSE\":"+gate+"}");
      for(var options:List.of("",",\"objects\":[\"msg\"]",",\"objects\":[\"msg\"],\"mutate_print\":true",",\"mutate_stdout\":true",",\"objects\":[\"msg\"],\"formatted\":\"\\ud800\""))
        compare(nativeCode,source,"\"mode\":\"verbose\""+options);
      for(var failure:List.of("get","format:msg","print","flush:original"))for(var error:List.of("RuntimeError","StopIteration","KeyboardInterrupt"))
        compare(nativeCode,source,"\"mode\":\"verbose\",\"objects\":[\"msg\"],\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var options:List.of("",",\"webbench\":true",",\"objects\":[\"msg\"],\"mutate_print\":true")) {
        var result=compare(nativeCode,source,"\"mode\":\"verbose\",\"real_print\":true"+options);
        if(!result.contains("[te_harness] ")||!result.contains("flush:original"))throw new AssertionError("Real printing failed: "+result);
      }
      for(var initial:List.of("","existing row\n")) {
        var result=compare(nativeCode,source,"\"real_file\":true,\"path\":"+q(root.resolve("logs/optimizer.jsonl"))+",\"initial_file\":"+q(initial)+",\"arguments\":{\"stop_reason\":\"max_tokens\"}");
        if(!result.contains("\\\"truncated\\\": true")||result.contains("error.log"))throw new AssertionError("Real file logging failed: "+result);
      }
      for(int padding:List.of(0,398,399,400,3998,3999,4000))compare(nativeCode,source,"\"paired_strings\":true,\"arguments\":{\"prompt\":"+q("x".repeat(padding))+",\"text\":"+q("x".repeat(padding))+"}");
      compare(nativeCode,source,"\"mode\":\"verbose\",\"real_print\":true,\"paired_strings\":true");
      compare(nativeCode,source,"\"fail\":\"write\",\"error\":\"DetailError\",\"paired_error\":true");
      for(var options:List.of("\"numeric_keys\":true","\"cyclic_usage\":true","\"descriptor_context\":true","\"descriptor_context\":true,\"fail\":\"lookup:enter\"","\"descriptor_context\":true,\"fail\":\"lookup:exit\"","\"fail\":\"write\",\"error\":\"FormatError\"","\"fail\":\"path\",\"error\":\"DetailError\""))compare(nativeCode,source,options);
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Optimizer logging matches the source in "+cases+" comparisons; exact JSON rows, Unicode truncation, resource failures, callback mutations and real file/print I/O passed");
  }
}
