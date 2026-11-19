//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares container orchestration through local process and SDK stand-ins. */
class ContainerProbe {
  static int cases;
  static Path artifacts;
  static String q(Object value) { return ParserProbe.q(value); }
  static void clear(Path root) throws Exception {
    if(Files.exists(root))try(var paths=Files.walk(root)) {
      for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);
    }
  }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var root=artifacts.resolve("case-"+cases);
    var request="{\"root\":"+q(root)+(fields.isEmpty()?"":","+fields)+"}";
    var expected=source.call(request);clear(root);
    var actual=nativeCode.call(request);
    if(cases==0 && (expected.contains("\"raised\":") || !expected.contains("\"verified\":true")))throw new AssertionError("Default source fixture did not activate: "+expected);
    if(!actual.equals(expected)) {
      Files.writeString(artifacts.resolve("request.json"),request);Files.writeString(artifacts.resolve("expected.json"),expected);Files.writeString(artifacts.resolve("actual.json"),actual);
      int at=0;while(at<actual.length()&&at<expected.length()&&actual.charAt(at)==expected.charAt(at))at++;
      throw new AssertionError("Case "+cases+" differs at "+at+"\nNative: "+actual.substring(Math.max(0,at-100),Math.min(actual.length(),at+280))+"\nSource: "+expected.substring(Math.max(0,at-100),Math.min(expected.length(),at+280)));
    }
    cases++;
  }
  public static void main(String[] args) throws Exception {
    artifacts=Files.createTempDirectory("te-container-probe-");boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"container_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"container_host.py")) {
      compare(nativeCode,source,"");
      for(var agent:List.of("claude","copilot","codex"))for(var mode:List.of("start","flow"))for(boolean scaffold:List.of(false,true))
        compare(nativeCode,source,"\"agent\":"+q(agent)+",\"mode\":"+q(mode)+",\"scaffold\":"+scaffold);
      for(var agent:List.of("claude","copilot","codex"))compare(nativeCode,source,"\"agent\":"+q(agent)+",\"mode\":\"start\",\"scaffold\":true,\"falsey_scaffold\":true");
      for(var raw:List.of("0","1","false"," 0 "))compare(nativeCode,source,"\"env\":{\"TE_ENV_ACTIVATION\":"+q(raw)+"}");
      for(var discovery:List.of("", "PATH=/usr/bin\n", "ENV=/a/bin\nENV=/b/bin\n", "ENV= \n", "ENV=/only/env/bin\nPATH= /usr/bin \nPATH=/bin\n", "PATH=/bin\r\nENV=/other/bin\u0085ENV=/x/testbed/bin\u2028"))
        compare(nativeCode,source,"\"responses\":{\"discovery\":{\"stdout\":"+q(discovery)+"}}");
      for(var check:List.of("", "/usr/bin/python\n/sys/prefix\n", "/opt/miniconda3/envs/testbed/bin/python", "/opt/miniconda3/envs/testbed/bin/python\u001f/prefix"))
        compare(nativeCode,source,"\"responses\":{\"check\":{\"stdout\":"+q(check)+"}}");
      for(var mode:List.of("info","args","record"))for(var state:List.of("{}","null","[]","{\"te-cell-abc\":{}}","{\"te-cell-abc\":false}","{\"te-cell-abc\":{\"mode\":\"none\"}}","{\"te-cell-abc\":{\"mode\":\"path-prepend\",\"env\":{\"PATH\":\"/é😀/bin\",\"ZERO\":0,\"BOOL\":false,\"NONE\":null}}}"))
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"state\":"+state);
      for(var raw:List.of("0","-1","١٢","1_2","bad","2.5"))for(var mode:List.of("stop","start"))
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"env\":{\"TE_DOCKER_START_TIMEOUT\":"+q(raw)+",\"TE_DOCKER_STOP_TIMEOUT\":"+q(raw)+"}");
      compare(nativeCode,source,"\"mode\":\"stop\",\"keep\":true");
      for(var mode:List.of("start","flow","stop","reap","activate"))for(var error:List.of("RuntimeError","TimeoutExpired","ValueError","OSError","KeyboardInterrupt","SystemExit","ShadowRuntimeError"))
        for(int at:List.of(0,1,2,3))compare(nativeCode,source,"\"mode\":"+q(mode)+",\"fail_at\":"+at+",\"error_kind\":"+q(error));
      for(var agent:List.of("claude","copilot","codex"))compare(nativeCode,source,"\"mode\":\"start\",\"agent\":"+q(agent)+",\"responses\":{\"start\":{\"returncode\":1,\"stderr\":"+q("é😀".repeat(400))+"}}");
      compare(nativeCode,source,"\"mode\":\"start\",\"agent\":\"copilot\",\"responses\":{\"seed\":{\"returncode\":1,\"stderr\":\"seed warning\"}}");
      for(var mode:List.of("exec","reap","copilot_cache","copilot_config","copilot_mounts","codex_mounts","seed"))compare(nativeCode,source,"\"mode\":"+q(mode));
      compare(nativeCode,source,"\"mode\":\"copilot_mounts\",\"env\":{\"COPILOT_HOST_CACHE\":\"/fixture/cache é😀\",\"COPILOT_HOME\":\"/fixture/config\"}");
      for(boolean check:List.of(false,true))for(var cmd:List.of("[\"fake\",\"a\",\"b\",\"c\",\"d\"]","\"fake\""))for(int rc:List.of(0,1))
        compare(nativeCode,source,"\"mode\":\"raw_run\",\"cmd\":"+cmd+",\"check\":"+check+",\"process\":{\"returncode\":"+rc+",\"stderr\":"+q("é😀".repeat(500))+"}");
      for(var config:List.of("{}","[]","\"x\"","null","false"))compare(nativeCode,source,"\"mode\":\"record\",\"config\":"+config);
      compare(nativeCode,source,"\"mode\":\"record\",\"missing_config\":true");
      compare(nativeCode,source,"\"mode\":\"record\",\"config_is_directory\":true");
      compare(nativeCode,source,"\"mode\":\"record\",\"local_cwd\":true,\"relative_path\":true");
      for(var agent:List.of("claude","copilot","codex"))for(var error:List.of("RuntimeError","TimeoutExpired","KeyboardInterrupt"))for(int at:List.of(0,1,2,3,4,5))
        compare(nativeCode,source,"\"mode\":\"start\",\"agent\":"+q(agent)+",\"fail_at\":"+at+",\"error_kind\":"+q(error));
      for(var agent:List.of("claude","codex"))for(var error:List.of("RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,"\"mode\":\"start\",\"scaffold\":true,\"agent\":"+q(agent)+",\"fail\":\"mounts\",\"error_kind\":"+q(error));
      for(var error:List.of("RuntimeError","KeyboardInterrupt"))compare(nativeCode,source,"\"mode\":\"start\",\"agent\":\"codex\",\"fail\":\"_write_config\",\"error_kind\":"+q(error));
      System.out.println("Container orchestration matches the source in "+cases+" cases; three agents, command order, process identity, activation, live tracking, cleanup and artifacts passed with local process stand-ins");
      passed=true;
    } finally {if(passed)clear(artifacts);else System.err.println("Probe artifacts: "+artifacts);}
  }
}
