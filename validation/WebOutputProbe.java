//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;

/** Compares evaluator stdout parsing, event publication and final state. */
class WebOutputProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_output_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_output_host.py")) {
      compare(nativeCode,source,"");
      for(var project:List.of("p","","a]b","a\nb","é😀\ud800\udc00"))for(var task:List.of("init","task-1","task-0","task-١٢","task-","task--1","other"))for(var mark:List.of("Started","✅","❌","Started extra","✅suffix","unknown"))
        compare(nativeCode,source,"\"lines\":["+q("["+project+"] "+task+" 2nd Attempt "+mark+"\n")+"]");
      for(var whitespace:List.of(" ","\t","\n","\r","\u001c","\u001d","\u001e","\u001f","\u0085","\u00a0","\u1680","\u2003","\u2028","\u2029","\u202f","\u205f","\u3000","\u200b"))
        compare(nativeCode,source,"\"lines\":["+q(whitespace+"[p]"+whitespace+"task-1"+whitespace+"2nd"+whitespace+"Attempt"+whitespace+"✅"+whitespace)+"]");
      for(var ordinal:List.of("0","001","-1","1.5","١٢","²","Ⅳ","１２","1".repeat(650)))for(int limit:List.of(0,640))
        compare(nativeCode,source,"\"int_max_digits\":"+limit+",\"lines\":["+q("[p] task-1 "+ordinal+"th Attempt ✅")+"]");
      int previous=-2;
      for(int point=0;point<=Character.MAX_CODE_POINT;point++)if(Character.getType(point)==Character.DECIMAL_DIGIT_NUMBER) {
        if(point!=previous+1)for(int offset:List.of(0,9)) {
          var digit=new String(Character.toChars(point+offset));
          compare(nativeCode,source,"\"lines\":["+q("[p] task-"+digit+" "+digit+"th Attempt ✅")+"]");
        }
        previous=point;
      }
      for(int point:List.of(0x11ddf,0x11de0,0x11de1,0x11de2,0x11de3,0x11de4,0x11de5,0x11de6,0x11de7,0x11de8,0x11de9,0x11dea,0x16d70,0x1e5f1)) {
        var digit=new String(Character.toChars(point));
        compare(nativeCode,source,"\"lines\":["+q("[p] task-"+digit+" "+digit+"th Attempt ✅")+"]");
      }
      for(var fields:List.of("\"lines\":[]","\"lines\":[null]","\"lines\":[42]","\"lines\":[{}]","\"lines\":[\"\",\" \"],\"opaque_code\":true","\"mutate_file\":true","\"opaque_lines\":true","\"opaque_lines\":true,\"mutate_pattern\":true","\"opaque_lines\":true,\"opaque_stripped\":true","\"ok_mark\":\"Started\"","\"opaque_mark\":true","\"field_objects\":[\"project\"],\"mutate_log\":true"))compare(nativeCode,source,fields);
      for(var fields:List.of("","\"opaque_lines\":true","\"no_match\":true","\"match_truthy\":false","\"group_objects\":[\"task\",\"ord\",\"mark\"]","\"group_objects\":[\"mark\"],\"opaque_comparison\":true","\"mutate_integer_at\":\"task\"","\"mutate_integer_at\":\"ord\"","\"mutate_mark\":true","\"opaque_lines\":true,\"mutate_put\":true"))
        compare(nativeCode,source,"\"custom_pattern\":true"+(fields.isEmpty()?"":","+fields));
      for(var pattern:List.of("(?P<task>task)-(?P<ord>2)-(?P<mark>✅)","(?P<task>.*)(?P<ord>.*)(?P<mark>.*)"))
        compare(nativeCode,source,"\"pattern\":"+q(pattern)+",\"lines\":[\"task-2-✅\"]");
      for(var missing:List.of("proc","_log_fh","events","project","exit_code","finished"))compare(nativeCode,source,"\"missing\":["+q(missing)+"]");
      for(var failure:List.of("read:proc","proc.stdout","iter","next","read:_log_fh","file.write","write","file.flush","flush","read:events","queue.put_function","put","proc.wait","wait","proc.returncode","write:exit_code","write:finished","read:exit_code","read:project","log"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var failure:List.of("pattern.match_function","match","match.bool","match.group_function","group","int:ord","bool:equals_started","rstrip:line","format:project","format:code"))
        compare(nativeCode,source,"\"custom_pattern\":true,\"opaque_lines\":true,\"group_objects\":[\"ord\",\"mark\"],\"opaque_comparison\":true,\"field_objects\":[\"project\"],\"opaque_code\":true,\"fail\":"+q(failure));
      for(int at:List.of(2,3))for(var failure:List.of("next","write","flush","put","group"))
        compare(nativeCode,source,"\"custom_pattern\":true,\"fail\":"+q(failure)+",\"fail_at\":"+at);
      var root=Files.createTempDirectory(Path.of("validation/results"),"web-output-").toAbsolutePath();
      var fixture=root.resolve("OutputFixture.java");
      Files.writeString(fixture,"class OutputFixture { public static void main(String[] args) { System.out.print(System.getenv(\"FIXTURE_OUTPUT\")); System.exit(Integer.parseInt(System.getenv(\"FIXTURE_EXIT\"))); } }");
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),fixture.toString())!=0)throw new AssertionError("fixture compilation failed");
      var binary=root.resolve("evaluator");Files.writeString(binary,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' OutputFixture\n");if(!binary.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      for(int code:List.of(0,7)) {
        var log=root.resolve("evaluator.log");
        var text="ordinary output é😀\n[p] init 1st Attempt Started\n[p] init 1st Attempt ✅\nlast partial line";
        var result=compare(nativeCode,source,"\"real_process\":true,\"binary\":"+q(binary)+",\"log_path\":"+q(log)+",\"code\":"+code+",\"lines\":["+q(text)+"]");
        if(!Files.readString(log).equals(text)||!result.contains("\"finished\":true"))throw new AssertionError("real process output failed: "+result);
      }
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Web-Bench stdout processing matches the source in "+cases+" comparisons; Unicode attempt lines, event order, log writes, final state, exceptions and an offline Java process passed");
  }
}
