//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;

/** Compares Codex installation policy and checks real offline probe cleanup. */
class CodexInstallProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  static String mode(String name) {return "\"mode\":"+q(name);}
  static void real(AdapterProbe.Client nativeCode,AdapterProbe.Client source) throws Exception {
    var root=Files.createTempDirectory("te-codex-install-");
    try {
      var fixture=root.resolve("Fixture.java");
      Files.writeString(fixture,"""
          import java.nio.file.*;
          public class Fixture {
            public static void main(String[] args) throws Exception {
              var home=Path.of(System.getenv("CODEX_HOME"));
              var child=new ProcessBuilder("/bin/sleep","60").start();
              Files.writeString(home.getParent().resolve("child.pid"),Long.toString(child.pid()));
              if(java.util.Arrays.asList(args).contains("--strict-config")) {
                if(Files.readString(home.resolve("config.toml")).equals("hang"))Thread.sleep(60000);
                else System.err.println("Error loading config.toml: unknown configuration field fixture");
              } else {
                var skills=home.resolve("skills/.system");
                for(var name:java.util.List.of("zeta","alpha")) {
                  Files.createDirectories(skills.resolve(name));
                  Files.writeString(skills.resolve(name+"/SKILL.md"),"fixture");
                }
                Files.createFile(skills.resolve(".codex-system-skills.marker"));
                Thread.sleep(60000);
              }
            }
          }
          """);
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),fixture.toString())!=0)
        throw new AssertionError("Could not compile the offline CLI fixture");
      var shim=root.resolve("codex");
      Files.writeString(shim,"#!/bin/sh\nexec '"+System.getProperty("java.home")+"/bin/java' -cp '"+root+"' Fixture \"$@\"\n");
      shim.toFile().setExecutable(true);
      for(var scenario:List.of("skills","invalid","hang")) {
        var operation=scenario.equals("skills")?"bundled_system_skills":"validate_config";
        var request="{\"real\":true,"+mode(operation)+",\"kwargs\":{\"binary\":"+q(shim)+",\"home\":"+q(root)+",\"timeout\":"+(scenario.equals("skills")?5:1)+(scenario.equals("skills")?"":",\"text\":"+q(scenario))+"}}";
        String expected=null;
        for(var client:List.of(source,nativeCode)) {
          var output=client.call(request);
          if(output.contains("\"raised\""))throw new AssertionError("Real probe failed: "+output);
          if(expected==null)expected=output;
          else if(!expected.equals(output))throw new AssertionError("Real probe differs\nNative: "+output+"\nSource: "+expected);
          if(Files.exists(root.resolve("work")))throw new AssertionError("The probe left its temporary home");
          var pid=Long.parseLong(Files.readString(root.resolve("child.pid")));
          var deadline=System.nanoTime()+2_000_000_000L;
          while(ProcessHandle.of(pid).map(ProcessHandle::isAlive).orElse(false)&&System.nanoTime()<deadline)Thread.sleep(10);
          if(ProcessHandle.of(pid).map(ProcessHandle::isAlive).orElse(false)) {
            ProcessHandle.of(pid).ifPresent(ProcessHandle::destroyForcibly);
            throw new AssertionError("The probe's child outlived its process group: "+pid);
          }
        }
        if(operation.equals("bundled_system_skills")&&!expected.contains("[\"alpha\",\"zeta\"]"))throw new AssertionError(expected);
        if(scenario.equals("invalid")&&!expected.contains("unknown configuration field fixture"))throw new AssertionError(expected);
        if(scenario.equals("hang")&&!expected.contains("could not run the config validator"))throw new AssertionError(expected);
        cases++;
      }
      var packageRoot=root.resolve("pkg");
      Files.createDirectories(packageRoot.resolve("bin"));
      Files.writeString(packageRoot.resolve("bin/codex.js"),"fixture");
      var vendor=packageRoot.resolve("node_modules/@openai/codex-arm64/vendor/aarch64-linux-musl");
      Files.createDirectories(vendor.resolve("bin"));
      Files.writeString(vendor.resolve("bin/codex"),"fixture");
      var linked=root.resolve("codex-linked");
      Files.createSymbolicLink(linked,packageRoot.resolve("bin/codex.js"));
      compare(nativeCode,source,"\"real\":true,"+mode("codex_vendor")+",\"kwargs\":{\"binary\":"+q(linked)+"}");
      for(var version:List.of("codex-cli 0.153.1","9.9.9",""))
        compare(nativeCode,source,"\"real\":true,"+mode("base_prompt_for")+",\"kwargs\":{\"cli_version\":"+q(version)+"}");
    } finally {
      try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.deleteIfExists(path);}
    }
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"codex_install_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"codex_install_host.py")) {
      for(var env:List.of("{}","{\"TE_CODEX_BIN\":\"\"}","{\"TE_CODEX_BIN\":\"/é/bin/codex\"}"))for(var which:List.of("null","\"/fixture/bin/codex\""))
        compare(nativeCode,source,"\"env\":"+env+",\"which\":"+which);
      for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))compare(nativeCode,source,"\"fail\":\"which\",\"error\":"+q(error));
      for(var value:List.of("", "/fixture/vendor","relative", "é😀"))for(boolean present:List.of(false,true))
        compare(nativeCode,source,mode("codex_vendor")+",\"env\":{\"TE_CODEX_VENDOR\":"+q(value)+"},\"isfile\":["+(present?q(value+"/bin/codex"):"")+"],\"kwargs\":{\"binary\":\"/fixture/pkg/bin/codex.js\"}");
      var base="/fixture/pkg/node_modules/@openai";
      var directories="{ "+q(base)+":[\"other\",\"codex-z\",\"codex-a\",\"codex-missing\"],"+q(base+"/codex-a/vendor")+":[\"z\",\"a\"],"+q(base+"/codex-z/vendor")+":[\"z\"]}";
      var vendor=mode("codex_vendor")+",\"kwargs\":{\"binary\":\"/fixture/pkg/bin/codex.js\"},\"directories\":"+directories;
      for(var files:List.of("[]","["+q(base+"/codex-a/vendor/z/bin/codex")+"]","["+q(base+"/codex-z/vendor/z/bin/codex")+"]","["+q(base+"/codex-a/vendor/a/bin/codex")+","+q(base+"/codex-a/vendor/z/bin/codex")+"]"))compare(nativeCode,source,vendor+",\"isfile\":"+files);
      for(var fail:List.of("realpath","listdir","isfile"))for(var error:List.of("FileNotFoundError","PermissionError","RuntimeError","KeyboardInterrupt","ShadowOSError"))
        compare(nativeCode,source,vendor+",\"fail\":"+q(fail)+",\"error\":"+q(error));
      compare(nativeCode,source,mode("codex_vendor"));
      compare(nativeCode,source,mode("require_install"));
      compare(nativeCode,source,mode("require_install")+",\"env\":{\"TE_CODEX_BIN\":\"shim\",\"TE_CODEX_VENDOR\":\"vendor\"},\"isfile\":[\"vendor/bin/codex\"]");
      for(var out:List.of("", "  ","codex-cli 0.153.1\nsecond", "é😀".repeat(70),"first\r\nsecond","one\u0085two","first\u2028second"))for(var err:List.of("","stderr"))
        compare(nativeCode,source,mode("cli_version")+",\"kwargs\":{\"binary\":\"shim\"},\"stdout\":"+q(out)+",\"stderr\":"+q(err));
      for(var error:List.of("OSError","ValueError","KeyboardInterrupt","SystemExit"))compare(nativeCode,source,mode("cli_version")+",\"kwargs\":{\"binary\":\"shim\"},\"fail\":\"run\",\"error\":"+q(error));
      for(var version:List.of("null","false","3.14","\"\"","\"codex 0.153.1 extra\"","\"١.٢.٣\"","\"9.9.9 and 0.153.1\""))for(boolean exists:List.of(false,true))
        compare(nativeCode,source,mode("base_prompt_for")+",\"kwargs\":{\"cli_version\":"+version+"},\"exists\":["+(exists?q("/fixture/te_harness/data/codex_base_prompt/"+(version.contains("١")?"١.٢.٣":version.contains("9.9.9")?"9.9.9":"0.153.1")+".md"):"")+"]");
      for(var fail:List.of("exists","open","read"))for(var error:List.of("OSError","UnicodeError","KeyboardInterrupt"))
        compare(nativeCode,source,mode("base_prompt_for")+",\"kwargs\":{\"cli_version\":\"0.153.1\"},\"exists\":[\"/fixture/te_harness/data/codex_base_prompt/0.153.1.md\"],\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(int failures=0;failures<=4;failures++)for(var error:List.of("OSError","PermissionError","RuntimeError","KeyboardInterrupt","ShadowOSError"))
        compare(nativeCode,source,mode("_rmtree_quietly")+",\"kwargs\":{\"path\":\"/fixture/home\"},\"fail\":\"rmtree\",\"fail_count\":"+failures+",\"error\":"+q(error));
      compare(nativeCode,source,mode("_kill_process_group"));
      for(var fail:List.of("pid","killpg","wait"))for(var error:List.of("ProcessLookupError","PermissionError","AttributeError","OSError","TimeoutExpired","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,mode("_kill_process_group")+",\"fail\":"+q(fail)+",\"error\":"+q(error));
      var skills=mode("bundled_system_skills")+",\"kwargs\":{\"binary\":\"shim\"},\"isdir\":[\"/fixture/tmp/skills/.system\"],\"directories\":{\"/fixture/tmp/skills/.system\":[\"zeta\",\"alpha\",\".codex-system-skills.marker\"]},\"isfile\":[\"/fixture/tmp/skills/.system/zeta/SKILL.md\",\"/fixture/tmp/skills/.system/alpha/SKILL.md\"]";
      compare(nativeCode,source,skills);
      compare(nativeCode,source,skills+",\"exists\":[\"/fixture/tmp/skills/.system/.codex-system-skills.marker\"]");
      compare(nativeCode,source,skills+",\"polls\":[0]");
      for(var timeout:List.of("0","-1","1","2","5","true","false","null","\"1\""))compare(nativeCode,source,mode("bundled_system_skills")+",\"kwargs\":{\"binary\":\"shim\",\"timeout\":"+timeout+"}");
      for(var fail:List.of("mkdtemp","open","write","spawn","clock","poll","exists","sleep","killpg","wait","isdir","listdir","isfile","rmtree"))for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,skills+",\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(var out:List.of("","valid","unknown configuration field: x","unknown variant é😀","invalid type", "Error loading config.toml", "Unknown configuration field", "invalid type"+" é😀".repeat(350)))for(var err:List.of("","\nmore"))
        compare(nativeCode,source,mode("validate_config")+",\"kwargs\":{\"binary\":\"shim\",\"text\":\"config\"},\"stdout\":"+q(out)+",\"stderr\":"+q(err));
      for(var fail:List.of("mkdtemp","open","write","spawn","communicate","killpg","wait","rmtree"))for(var error:List.of("OSError","TimeoutExpired","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,mode("validate_config")+",\"kwargs\":{\"binary\":\"shim\",\"text\":\"config\"},\"fail\":"+q(fail)+",\"error\":"+q(error));
      for(var operation:List.of("bundled_system_skills","validate_config"))for(var env:List.of("{}","{\"TE_CODEX_BIN\":\"shim\"}"))
        compare(nativeCode,source,mode(operation)+",\"env\":"+env+",\"kwargs\":{"+(operation.equals("validate_config")?"\"text\":\"config\"":"")+"}");
      real(nativeCode,source);
    }
    System.out.println("Codex installation matches the source in "+cases+" comparisons, including real skills discovery, config validation, timeout and child-process cleanup");
  }
}
