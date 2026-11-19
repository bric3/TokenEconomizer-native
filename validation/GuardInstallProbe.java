//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.List;
import java.util.concurrent.TimeUnit;

/** Compares runner guard commands and native hook installation with the source. */
class GuardInstallProbe {
  static int cases;
  static int selfChecks;
  static String q(Object value) {return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  record Output(int code,String out,String error,byte[] file) {}
  static Output run(Path root,List<String> argv,Path path) throws Exception {
    var out=root.resolve("out");var error=root.resolve("err");
    var process=new ProcessBuilder(argv).redirectOutput(out.toFile()).redirectError(error.toFile()).start();
    if(!process.waitFor(10,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("Installer timed out");}
    var diagnostic=Files.readString(error);var lines=diagnostic.strip().split("\n");
    return new Output(process.exitValue(),Files.readString(out),lines[lines.length-1],Files.isRegularFile(path)?Files.readAllBytes(path):null);
  }
  static void configure(Path path,String before) throws Exception {
    Files.deleteIfExists(path);
    if(before!=null) {Files.createDirectories(path.getParent());Files.writeString(path,before);}
  }
  static void config(Path root,AdapterProbe.Client source,String agent,String before) throws Exception {
    var program=Base64.getDecoder().decode(source.call("{\"mode\":\"program\",\"agent\":"+q(agent)+",\"python\":"+q(Path.of(".venv/bin/python").toAbsolutePath())+"}"));
    var script=root.resolve("install.py");Files.write(script,program);
    var path=root.resolve("config/settings.json");
    configure(path,before);
    var expected=run(root,List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),script.toString(),path.toString()),path);
    var expectedAgain=run(root,List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),script.toString(),path.toString()),path);
    configure(path,before);
    var argv=List.of(Path.of("target/debug/te-net-guard").toAbsolutePath().toString(),"--install-hook",agent,path.toString(),Path.of(".venv/bin/python").toAbsolutePath()+" /fixture/home/net_guard.py");
    var actual=run(root,argv,path);var actualAgain=run(root,argv,path);
    for(var pair:List.of(List.of(actual,expected),List.of(actualAgain,expectedAgain))) {
      var a=pair.get(0);var e=pair.get(1);
      if(a.code()!=e.code() || !a.out().equals(e.out()) || !a.error().equals(e.error()) || !java.util.Arrays.equals(a.file(),e.file()))
        throw new AssertionError("Hook config "+agent+": "+before+"\nNative: "+a+"\nSource: "+e+"\nNative file: "+(a.file()==null?null:new String(a.file(),java.nio.charset.StandardCharsets.UTF_8))+"\nSource file: "+(e.file()==null?null:new String(e.file(),java.nio.charset.StandardCharsets.UTF_8)));
      cases++;
    }
  }
  static byte[] docker(List<String> args,byte[] input) throws Exception {
    var argv=new java.util.ArrayList<String>();argv.add("docker");argv.addAll(args);
    var process=new ProcessBuilder(argv).redirectError(ProcessBuilder.Redirect.INHERIT).start();
    try(var stream=process.getOutputStream()){if(input!=null)stream.write(input);}
    if(!process.waitFor(30,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("Docker command timed out");}
    var output=process.getInputStream().readAllBytes();
    if(process.exitValue()!=0)throw new AssertionError("Docker command failed: "+argv);
    return output;
  }
  static void liveHosts(AdapterProbe.Client nativeCode,AdapterProbe.Client source) throws Exception {
    var name="te-guard-probe-"+java.util.UUID.randomUUID().toString().substring(0,8);
    docker(List.of("run","--rm","-d","--network","none","--hostname","te-guard-probe","--name",name,"eclipse-temurin:25-jdk-noble","sleep","180"),null);
    try {
      for(int pass=0;pass<2;pass++) {
        var before=docker(List.of("exec",name,"cat","/etc/hosts"),null);
        var request="{\"mode\":\"hosts\",\"live\":true,\"name\":"+q(name)+"}";
        var expected=source.call(request);var expectedHosts=docker(List.of("exec",name,"cat","/etc/hosts"),null);
        docker(List.of("exec","-i",name,"sh","-c","cat > /etc/hosts"),before);
        var actual=nativeCode.call(request);var actualHosts=docker(List.of("exec",name,"cat","/etc/hosts"),null);
        if(!expected.contains("\"value\":true") || !expected.equals(actual) || !java.util.Arrays.equals(expectedHosts,actualHosts))throw new AssertionError("Live DNS guard differs\nNative: "+actual+"\nSource: "+expected);
        cases++;
      }
    } finally {docker(List.of("rm","-f",name),null);}
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory("te-guard-install-");boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"guard_install_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"guard_install_host.py")) {
      compare(nativeCode,source,"");
      for(var agent:List.of("claude","copilot","codex"))for(var mode:List.of("hosts","verify","blocks","graded_diff"))
        compare(nativeCode,source,"\"agent\":"+q(agent)+",\"mode\":"+q(mode)+",\"home\":\"/fixture/é😀 home\"");
      for(var allow:List.of("1","true","yes"," TRUE ","0"," 1 \u001f"))compare(nativeCode,source,"\"allow\":"+q(allow));
      for(var fail:List.of("network","exec","excludes"))for(var error:List.of("RuntimeError","ValueError","OSError","KeyboardInterrupt","SystemExit"))for(var mode:List.of("hosts","verify","blocks","graded_diff"))
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(int at:List.of(0,1))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","SystemExit"))
        compare(nativeCode,source,"\"fail_at\":"+at+",\"error\":"+q(error)+",\"message\":"+q("é😀".repeat(150)));
      for(var stdout:List.of("null","false","0","42","[]","{}","[\"rc=2\"]","{\"rc=2\":1}",q(""),q("1\n"),q("1\r\n"),q("prefix\u20281\u001f"),q("rc=20"),q("é😀".repeat(100))))
        for(var mode:List.of("hosts","verify","blocks"))compare(nativeCode,source,"\"mode\":"+q(mode)+",\"responses\":[{\"returncode\":1,\"stdout\":"+stdout+",\"stderr\":\"stderr\"}]");
      for(var stdout:List.of(q("127.0.0.1 github.com"),q("127.0.0.10"),q("not blocked"),"null","42","[\"127.0.0.1\"]","{}"))
        compare(nativeCode,source,"\"responses\":[{\"stdout\":\"1\\n\"},{\"stdout\":"+stdout+"}]");
      for(var stderr:List.of("42","true","{\"x\":1}","[\"é😀\",null,false]",q("é😀".repeat(150))))compare(nativeCode,source,"\"responses\":[{\"stdout\":\"bad\",\"stderr\":"+stderr+"}]");
      for(var stdout:List.of("", " ", "0\n", "-12 rest\n", "١_٢\n", "bad", "7".repeat(300)))compare(nativeCode,source,"\"mode\":\"blocks\",\"responses\":[{\"stdout\":"+q(stdout)+"}]");
      for(var excludes:List.of("[]","[\"CLAUDE.md\",\"a b\",\"a'b\",\"é😀\"]","null","\"ab\"","[null,42,false]","0","false","true","{}","{\"a\":1,\"b\":2}"))compare(nativeCode,source,"\"mode\":\"graded_diff\",\"excludes\":"+excludes);
      for(var excludes:List.of("[]","[\"CLAUDE.md\",\"é😀\"]"))compare(nativeCode,source,"\"mode\":\"graded_diff\",\"iterator\":true,\"excludes\":"+excludes);
      for(int limit:List.of(0,640,4300))for(var number:List.of("7".repeat(5000),"0".repeat(5000)+"1","١".repeat(5000)))compare(nativeCode,source,"\"mode\":\"blocks\",\"int_max_digits\":"+limit+",\"responses\":[{\"stdout\":"+q(number)+"}]");
      for(var agent:List.of("claude","codex","copilot")) {
        config(root,source,agent,null);
        for(var before:List.of("{}","not json","[]","null","42","false","\"text\"","{\"hooks\":null}","{\"hooks\":[]}","{\"hooks\":{\"PreToolUse\":[],\"preToolUse\":[]},\"other\":\"é😀\"}"))config(root,source,agent,before);
        var event=agent.equals("copilot")?"preToolUse":"PreToolUse";
        for(var hooks:List.of("null","false","42","\"net_guard\"","{}","{\"net_guard\":1}","[{\"command\":\"rtk-rewrite\"}]","[{\"command\":\"already_net_guard\"}]"))config(root,source,agent,"{\"hooks\":{"+q(event)+":"+hooks+"}}");
        config(root,source,agent,"{\"metadata\":[NaN,Infinity,-Infinity,1e99,123456789012345678901234567890],\"unicode\":\"\\ud800\"}");
      }
      var unsupported=root.resolve("uncreated/settings.json");
      var rejected=run(root,List.of(Path.of("target/debug/te-net-guard").toAbsolutePath().toString(),"--install-hook","unknown",unsupported.toString(),"command"),unsupported);
      if(rejected.code()!=1 || Files.exists(unsupported.getParent()))throw new AssertionError("Unknown agent mutated the hook directory");selfChecks++;
      var ownPath=root.resolve("native/settings.json");var executable=Path.of("target/debug/te-net-guard").toAbsolutePath().toString();
      var ownArgs=List.of(executable,"--install-hook","claude",ownPath.toString(),executable);
      var first=run(root,ownArgs,ownPath);var second=run(root,ownArgs,ownPath);
      if(first.code()!=0 || second.code()!=0 || !java.util.Arrays.equals(first.file(),second.file()))throw new AssertionError("Native executable installation duplicated its hook");selfChecks++;
      var installed=nativeCode.call("{\"mode\":\"install\"}");
      if(!installed.contains("\"value\":true"))throw new AssertionError("Native helper fixture did not install: "+installed);selfChecks++;
      var missing=nativeCode.call("{\"mode\":\"install\",\"artifact_root\":"+q(root.resolve("missing-artifacts"))+"}");
      if(!missing.contains("\"value\":false")||!missing.contains("UNGUARDED"))throw new AssertionError("Native binding ignored the changed helper root: "+missing);selfChecks++;
      if(List.of(args).contains("--docker"))liveHosts(nativeCode,source);
      passed=true;System.out.println("Guard integration passes "+cases+" source comparisons and "+selfChecks+" native checks; DNS commands, refusal probes, exact counts, patch exclusions and hook-file merge passed");
    } finally {if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
  }
}
