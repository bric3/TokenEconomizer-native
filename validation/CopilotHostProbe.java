//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;

/** Checks Copilot binary discovery, installation validation, versions and environments. */
class CopilotHostProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"copilot-host-").toAbsolutePath();
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"copilot_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"copilot_host.py")) {
      for(var value:List.of("null","false","0","\"\"","\"relative/copilot\"","[]","{}","[1]"))for(boolean accessible:List.of(false,true))for(var which:List.of("null","\"/path/copilot\""))
        compare(nativeCode,source,"\"env\":{\"COPILOT_HOST_BIN\":"+value+"},\"accessible\":"+accessible+",\"which\":"+which);
      compare(nativeCode,source,"\"objects\":[\"COPILOT_HOST_BIN\"]");
      for(boolean seed:List.of(false,true))for(int mask=0;mask<8;mask++) {
        var paths=new java.util.ArrayList<String>();if((mask&1)>0)paths.add(q("/bin/copilot"));if((mask&2)>0)paths.add(q("/cache"));if((mask&4)>0)paths.add(q("/config"));
        compare(nativeCode,source,"\"method\":\"require_install\",\"arguments\":{\"need_container_seed\":"+seed+"},\"missing\":["+String.join(",",paths)+"]");
      }
      compare(nativeCode,source,"\"method\":\"require_install\",\"opaque_helpers\":true");
      compare(nativeCode,source,"\"method\":\"require_install\",\"helper_values\":{\"copilot_bin\":\"\\ud800\"},\"missing\":[\"\\ud800\"]");
      for(var value:List.of("null","false","0","1","\"\"","\"version\"","\"  é😀\\nextra  \"",q("x".repeat(130)),q("😀".repeat(130)),"\"\\ud800\"","[]","{}","[1]"))
        compare(nativeCode,source,"\"method\":\"cli_version\",\"arguments\":{\"binary\":\"fixture\"},\"stdout\":"+value+",\"stderr\":\" error version \"");
      for(var stdout:List.of("", " \n\t", " \u001cversion\u001f "))for(var stderr:List.of("", " \n\t", "stderr version"))
        compare(nativeCode,source,"\"method\":\"cli_version\",\"arguments\":{\"binary\":\"fixture\"},\"stdout\":"+q(stdout)+",\"stderr\":"+q(stderr));
      for(var boundary:List.of("\n","\r","\r\n","\u000b","\u000c","\u001c","\u001d","\u001e","\u0085","\u2028","\u2029"))
        compare(nativeCode,source,"\"method\":\"cli_version\",\"arguments\":{\"binary\":\"fixture\"},\"stdout\":"+q("first"+boundary+"second"));
      compare(nativeCode,source,"\"method\":\"cli_version\",\"arguments\":{\"binary\":\"fixture\"},\"opaque_stdout\":true");
      for(var extra:List.of("null","false","0","1","\"\"","\"bad\"","[]","{}","[1]","[[\"EXTRA\",23]]","{\"SAME\":\"extra\",\"COPILOT_AUTO_UPDATE\":\"true\"}"))
        compare(nativeCode,source,"\"method\":\"run_env\",\"arguments\":{\"extra\":"+extra+"}");
      compare(nativeCode,source,"\"method\":\"run_env\",\"opaque_helpers\":true");
      compare(nativeCode,source,"\"method\":\"run_env\",\"stub_helpers\":false,\"env\":{\"GH_TOKEN\":\"gh\",\"COPILOT_PROVIDER_BASE_URL\":\"http://fixture\",\"COPILOT_PROVIDER_API_KEY\":\"key\"}");
      for(var failure:List.of("get","expanduser","access","which","realpath","copilot_bin","copilot_cache","copilot_config","exists","run","stdout","stderr","strip","splitlines","slice","auth_env","provider_env"))
        for(var error:List.of("OSError","TimeoutExpired","SubprocessError","IndexError","RuntimeError","KeyboardInterrupt","SystemExit","ShadowOSError")) {
          String method=failure.startsWith("copilot_")||failure.equals("exists")?"require_install":List.of("run","stdout","stderr","strip","splitlines","slice").contains(failure)?"cli_version":failure.endsWith("_env")?"run_env":"copilot_bin";
          String extra=method.equals("require_install")?",\"arguments\":{\"need_container_seed\":true}":method.equals("cli_version")?",\"arguments\":{\"binary\":\"fixture\"},\"opaque_stdout\":"+!failure.equals("stderr")+",\"stdout\":\"\"":",\"which\":\"fixture\"";
          compare(nativeCode,source,"\"method\":"+q(method)+extra+",\"fail\":"+q(failure)+",\"error\":"+q(error));
        }
      var fixture=root.resolve("CopilotVersion.java");
      Files.writeString(fixture,"class CopilotVersion { public static void main(String[] args) { if(args.length!=1||!args[0].equals(\"--version\"))throw new AssertionError(); System.out.println(\"offline copilot 1.2.3\"); } }");
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),fixture.toString())!=0)throw new AssertionError("fixture compilation failed");
      var executable=root.resolve("copilot");
      Files.writeString(executable,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' CopilotVersion \"$@\"\n");
      if(!executable.toFile().setExecutable(true))throw new AssertionError("cannot make fixture executable");
      var link=root.resolve("copilot-link");Files.createSymbolicLink(link,executable.getFileName());
      for(var fields:List.of("\"env\":{\"COPILOT_HOST_BIN\":"+q(link)+"}","\"expanded\":"+q(link),"\"expanded\":"+q(root.resolve("missing"))+",\"path\":"+q(root))) {
        var result=compare(nativeCode,source,"\"real_paths\":true,"+fields);
        if(!result.contains("\"value\":"+q(executable)))throw new AssertionError("binary discovery failed: "+result);
      }
      var result=compare(nativeCode,source,"\"method\":\"cli_version\",\"real_process\":true,\"arguments\":{\"binary\":"+q(executable)+"}");
      if(!result.contains("\"value\":\"offline copilot 1.2.3\""))throw new AssertionError("version fixture failed: "+result);
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Copilot host helpers match the source in "+cases+" comparisons, including real symlink discovery, PATH lookup and offline version execution");
  }
}
