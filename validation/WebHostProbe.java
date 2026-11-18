//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;

/** Compares Web-Bench host discovery, timeout policy and namespace probing. */
class WebHostProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"web-host-").toAbsolutePath();
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_host_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_host_host.py")) {
      for(var method:List.of("webbench_undici_abort","webbench_task_timeout","webbench_undici_env")) {
        var a="\"method\":"+q(method);
        compare(nativeCode,source,a);
        for(var key:List.of("TE_WEBBENCH_UNDICI_ABORT","TE_WEBBENCH_TASK_TIMEOUT","TE_TASK_TIMEOUT","NODE_OPTIONS","TOKENECONOMIZER_WEBBENCH_REPO"))for(var value:List.of("null","false","0","1","2.9","NaN","Infinity","\"\"","\"0\"","\"-3\"","\"59\"","\"60\"","\"240\"","\"260\"","\"261\"","\"300\"","\"301\"","\"800\"","\"bad\"","[]","{}","[1]",q("١_٢")))
          compare(nativeCode,source,a+",\"env\":{"+q(key)+":"+value+"}");
        for(var abort:List.of(60,300,301,800))for(var timeout:List.of(0,1,240,260,261,900))compare(nativeCode,source,a+",\"env\":{\"TE_WEBBENCH_UNDICI_ABORT\":"+q(abort)+",\"TE_WEBBENCH_TASK_TIMEOUT\":"+q(timeout)+"}");
        for(var key:List.of("TE_WEBBENCH_UNDICI_ABORT","TE_WEBBENCH_TASK_TIMEOUT","TE_TASK_TIMEOUT")) {
          compare(nativeCode,source,a+",\"objects\":["+q(key)+"]");
          for(int digits:List.of(640,641,650))for(int limit:List.of(0,640))compare(nativeCode,source,a+",\"int_max_digits\":"+limit+",\"env\":{"+q(key)+":"+q("7".repeat(digits))+"}");
        }
        compare(nativeCode,source,a+",\"explicit\":true,\"env\":{\"TE_WEBBENCH_UNDICI_ABORT\":\"900\"},\"explicit_env\":{\"TE_WEBBENCH_UNDICI_ABORT\":\"60\"}");
        for(var error:List.of("TypeError","ValueError","OverflowError","RuntimeError","KeyboardInterrupt","ShadowValueError"))
          for(var failure:List.of("get","bool:TE_WEBBENCH_UNDICI_ABORT","int:TE_WEBBENCH_UNDICI_ABORT"))compare(nativeCode,source,a+",\"objects\":[\"TE_WEBBENCH_UNDICI_ABORT\"],\"fail\":"+q(failure)+",\"error\":"+q(error));
      }
      for(var key:List.of("TE_WEBBENCH_TASK_TIMEOUT","TE_TASK_TIMEOUT"))for(var error:List.of("TypeError","ValueError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"objects\":["+q(key)+"],\"fail\":"+q("bool:"+key)+",\"error\":"+q(error));
      for(var method:List.of("webbench_task_timeout","webbench_undici_env")) {
        var a="\"method\":"+q(method);
        for(var abort:List.of("-20","0","60","300","800","800.5"))compare(nativeCode,source,a+",\"abort\":"+abort+",\"env\":{\"TE_TASK_TIMEOUT\":\"900\"}");
        compare(nativeCode,source,a+",\"opaque_abort\":true,\"env\":{\"TE_TASK_TIMEOUT\":\"900\"},\"mutate_log\":true");
        compare(nativeCode,source,a+",\"abort\":"+"7".repeat(650)+",\"int_max_digits\":640");
      }
      compare(nativeCode,source,"\"method\":\"webbench_undici_abort\",\"constants\":{\"WEBBENCH_UNDICI_ABORT_DEFAULT_S\":"+"7".repeat(650)+"},\"int_max_digits\":640");
      for(var previous:List.of("null","false","0","1","\"\"","\" --trace-warnings \"","[]","[1]"))for(var repo:List.of("null","false","\"\"","\"/repo é😀\""))
        compare(nativeCode,source,"\"method\":\"webbench_undici_env\",\"env\":{\"TE_WEBBENCH_UNDICI_ABORT\":\"800\",\"NODE_OPTIONS\":"+previous+"},\"arguments\":{\"repo\":"+repo+"}");
      compare(nativeCode,source,"\"method\":\"webbench_undici_env\",\"env\":{\"TE_WEBBENCH_UNDICI_ABORT\":\"800\"},\"objects\":[\"NODE_OPTIONS\"],\"argument_objects\":[\"repo\"],\"opaque_uri\":true");
      for(var path:List.of("relative preload.mjs","/fixture/space é😀.mjs"))compare(nativeCode,source,"\"method\":\"webbench_undici_env\",\"real_uri\":true,\"env\":{\"TE_WEBBENCH_UNDICI_ABORT\":\"800\"},\"constants\":{\"UNDICI_PRELOAD\":"+q(path)+"}");
      for(var explicit:List.of("null","false","0","\"\"","\"/explicit/repo\"","[]","[1]"))for(var projects:List.of("null","\"\"","\"/repo/projects///\"","\"/\"","\"relative/projects\""))
        compare(nativeCode,source,"\"method\":\"webbench_repo\",\"env\":{\"TE_WEBBENCH_REPO\":"+explicit+",\"TOKENECONOMIZER_WEBBENCH_PROJECTS\":"+projects+"}");
      compare(nativeCode,source,"\"method\":\"webbench_repo\",\"objects\":[\"TOKENECONOMIZER_WEBBENCH_PROJECTS\"],\"mutate_paths\":true");
      for(var value:List.of("null","false","0","\"\"","\"relative/node\"","[]","[1]"))for(var which:List.of("null","\"/node\""))
        compare(nativeCode,source,"\"method\":\"node_bin\",\"env\":{\"TE_NODE_BIN\":"+value+"},\"which\":"+which);
      compare(nativeCode,source,"\"method\":\"node_bin\",\"objects\":[\"TE_NODE_BIN\"]");
      compare(nativeCode,source,"\"method\":\"node_bin\",\"opaque_which\":true");
      for(var code:List.of("null","false","true","0","1","-1","\"0\""))compare(nativeCode,source,"\"method\":\"_probe_namespace_guard\",\"returncode\":"+code);
      compare(nativeCode,source,"\"method\":\"_probe_namespace_guard\",\"opaque_returncode\":true");
      for(var error:List.of("OSError","TimeoutError","TimeoutExpired","SilentError","ValueError","RuntimeError","KeyboardInterrupt","ShadowOSError"))compare(nativeCode,source,"\"method\":\"_probe_namespace_guard\",\"fail\":\"run\",\"error\":"+q(error));
      for(var failure:List.of("returncode","ne:returncode","log"))for(var error:List.of("RuntimeError","KeyboardInterrupt"))compare(nativeCode,source,"\"method\":\"_probe_namespace_guard\",\"opaque_returncode\":true,\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var error:List.of("OSError","TimeoutExpired","SilentError"))compare(nativeCode,source,"\"method\":\"_probe_namespace_guard\",\"fail\":\"returncode\",\"error\":"+q(error));
      var repo=Files.createDirectories(root.resolve("repo/projects"));var link=root.resolve("repo-link");Files.createSymbolicLink(link,Path.of("repo"));
      for(var env:List.of("{\"TE_WEBBENCH_REPO\":"+q(link)+"}","{\"TOKENECONOMIZER_WEBBENCH_PROJECTS\":"+q(link.resolve("projects"))+"}")) {
        var result=compare(nativeCode,source,"\"method\":\"webbench_repo\",\"env\":"+env);
        if(!result.contains("\"value\":"+q(repo.getParent())))throw new AssertionError("Real repository discovery failed: "+result);
      }
      var fixture=root.resolve("NamespaceFixture.java");
      Files.writeString(fixture,"class NamespaceFixture { public static void main(String[] args) { if(!java.util.Arrays.asList(args).equals(java.util.List.of(\"--user\",\"--map-root-user\",\"--mount\",\"bash\",\"-c\",\"getent hosts github.com | head -1\")))throw new AssertionError(\"wrong probe arguments\"); System.exit(Integer.parseInt(System.getenv(\"FIXTURE_EXIT\"))); } }");
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),fixture.toString())!=0)throw new AssertionError("fixture compilation failed");
      var binary=root.resolve("node");Files.writeString(binary,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' NamespaceFixture \"$@\"\n");if(!binary.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      var discovered=compare(nativeCode,source,"\"method\":\"node_bin\",\"real_paths\":true,\"path\":"+q(root));
      if(!discovered.contains("\"value\":"+q(binary)))throw new AssertionError("Real PATH discovery failed: "+discovered);
      for(int code:List.of(0,1)) {
        var result=compare(nativeCode,source,"\"method\":\"_probe_namespace_guard\",\"real_process\":true,\"binary\":"+q(binary)+",\"fixture_exit\":"+code);
        if(!result.contains("\"value\":"+(code==0)))throw new AssertionError("Offline namespace result changed: "+result);
      }
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Web-Bench host helpers match the source in "+cases+" comparisons; discovery, timeout policy, evaluator environment and namespace decisions passed with offline I/O fixtures");
  }
}
