//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.net.ServerSocket;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;

/** Compares evaluator setup, configuration, launch order and offline HTTP rendezvous. */
class WebStartProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    var reference=Path.of("reference/upstream/experiments/te_harness/webbench_undici_preload.mjs").toAbsolutePath();
    var asset=Path.of("src/experiments/assets/webbench_undici_preload.mjs").toAbsolutePath();
    var referenceUri=reference.toUri().toASCIIString();var assetUri=asset.toUri().toASCIIString();
    if(expected.contains(referenceUri)) {
      if(Files.mismatch(reference,asset)!=-1||!actual.contains(assetUri))throw new AssertionError("Native evaluator did not select the pinned runtime preload");
      expected=expected.replace(referenceUri,assetUri);
    }
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_start_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_start_host.py")) {
      compare(nativeCode,source,"");
      for(var full:List.of("null","false","0","1","\"\"","\"full\"","[]","{}","[\"init\",\"task-1\"]","[\"init\"]"))for(var tasks:List.of("null","false","0","1","\"\"","[]","{}","[{}]","[null]","[{\"instance_id\":\"init\"}]","[{\"instance_id\":\"task-1\"},{\"instance_id\":\"other\"}]"))
        compare(nativeCode,source,"\"full\":"+full+",\"arguments\":{\"tasks\":"+tasks+"}");
      for(var key:List.of("project","repo","session_dir","retry","reply_timeout","log_level","max_workers","node"))for(var value:List.of("null","false","0","1","-1","2.5","\"\"","\"é😀\\ud800\\udc00\"","[]","{}"))
        compare(nativeCode,source,"\"arguments\":{"+q(key)+":"+value+"}");
      for(var key:List.of("project","repo","session_dir","retry","reply_timeout","log_level","max_workers","node","tasks"))compare(nativeCode,source,"\"objects\":["+q(key)+"]");
      for(var full:List.of("","\"booleans\":{\"full\":false}","\"opaque_comparison\":true","\"mutate_ids\":true","\"length\":0","\"length\":-1"))compare(nativeCode,source,"\"opaque_full\":true"+(full.isEmpty()?"":","+full));
      for(var field:List.of("dir","httpd","config_path","log_path","_read_stdout"))compare(nativeCode,source,"\"mutate_on_get\":"+q(field));
      compare(nativeCode,source,"\"objects\":[\"project\"],\"mutate_log\":true,\"arguments\":{\"tasks\":[{\"instance_id\":\"task-1\"}]}");
      compare(nativeCode,source,"\"rewrites\":{\"project\":\"stored-project\",\"repo\":\"/stored/repo\",\"dir\":\"/stored/dir\",\"retry\":9,\"port\":23456,\"hash\":\"stored-hash\"}");
      for(var undici:List.of("null","false","0","[]","[[\"key\",\"value\"]]","{\"NODE_OPTIONS\":\"--trace-warnings\"}"))compare(nativeCode,source,"\"undici\":"+undici);
      compare(nativeCode,source,"\"exists\":false");
      compare(nativeCode,source,"\"opaque_exit\":true");
      for(boolean suppress:List.of(false,true))compare(nativeCode,source,"\"fail\":\"dump\",\"suppress\":"+suppress+",\"exception_state\":true");
      compare(nativeCode,source,"\"opaque_exit\":true,\"fail\":\"dump\",\"exception_state\":true");
      for(var key:List.of("retry","max_workers"))for(int limit:List.of(0,640))compare(nativeCode,source,"\"arguments\":{"+q(key)+":"+"7".repeat(650)+"},\"int_max_digits\":"+limit);
      for(var failure:List.of("write:project","write:repo","write:dir","write:retry","write:reply_timeout","queue","write:requests","write:events","write:finished","write:exit_code","strftime","write:hash","makedirs","path.join","join","write:log_path","server","write:httpd","server.server_address","write:port","thread","thread.start","full","read:port","read:hash","read:dir","write:config_path","open","enter","dump","exit","path.exists","exists","undici","write:_log_fh","popen","write:proc","register","read:_read_stdout","write:_reader","pid","log"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(int at:List.of(2,3))for(var failure:List.of("queue","join","thread","thread.start","open","read:dir","read:config_path"))compare(nativeCode,source,"\"fail\":"+q(failure)+",\"fail_at\":"+at);
      var root=Files.createTempDirectory(Path.of("validation/results"),"web-start-").toAbsolutePath();
      var project=Files.createDirectories(root.resolve("repo/projects/project"));
      Files.writeString(project.resolve("tasks.jsonl"),"{\"id\":\"init\"}\n{\"id\":\"task-1\"}\n");
      Files.createDirectories(root.resolve("repo/apps/eval/dist"));Files.writeString(root.resolve("repo/apps/eval/dist/index.js"),"");
      var fixture=Path.of("validation/WebEvaluatorFixture.java");
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),fixture.toString())!=0)throw new AssertionError("fixture compilation failed");
      var binary=root.resolve("node");Files.writeString(binary,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' WebEvaluatorFixture \"$@\"\n");if(!binary.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      int port;try(var socket=new ServerSocket(0)){port=socket.getLocalPort();}
      var io="\"real_io\":true,\"real_helpers\":true,\"port\":"+port;
      var arguments="\"repo\":"+q(root.resolve("repo"))+",\"session_dir\":"+q(root.resolve("session"))+",\"node\":"+q(binary);
      for(var extra:List.of("",",\"tasks\":[{\"instance_id\":\"task-1\"}],\"max_workers\":5")) {
        var env=extra.isEmpty()?"{}":"{\"TE_WEBBENCH_UNDICI_ABORT\":\"800\"}";
        var result=compare(nativeCode,source,io+",\"env\":"+env+",\"arguments\":{"+arguments+extra+"}");
        if(!result.contains("\"finished\":true")||!result.contains("\"exit_code\":0")||!result.contains("\"process_groups\":[true]")||!result.contains("\"kind\":\"started\"")||!result.contains("\"kind\":\"verdict\"")||!result.contains("\\u2705"))throw new AssertionError("offline evaluator startup failed: "+result);
        if(!extra.isEmpty()&&!result.contains("\"startTask\":\"task-1\""))throw new AssertionError("subset range missing: "+result);
      }
      var failure=compare(nativeCode,source,io+",\"arguments\":{\"repo\":"+q(root.resolve("repo"))+",\"session_dir\":"+q(root.resolve("session"))+",\"node\":"+q(root.resolve("absent-node"))+"}");
      if(!failure.contains("\"raised\":\"FileNotFoundError\"")||!failure.contains("\"registered\":false"))throw new AssertionError("launch error changed: "+failure);
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Web-Bench evaluator startup matches the source in "+cases+" comparisons; configuration, subset warnings, partial state, launch arguments and offline HTTP/process integration passed");
  }
}
