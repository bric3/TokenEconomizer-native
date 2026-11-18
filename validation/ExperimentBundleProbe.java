//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java AdapterProbe.java ExtractProposerPrompts.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;
import java.util.zip.ZipFile;

/** Exercises fresh experiment payloads, relocation, archive bytes and input/output guards. */
class ExperimentBundleProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static NativeCliProbe.Output run(Path root,Path exe,String... args)throws Exception {
    var command=new java.util.ArrayList<>(List.of(exe.toString()));command.addAll(List.of(args));
    return NativeCliProbe.process(root,Map.of("HOME",root.toString(),"PATH","","TOKENECONOMIZER_HOME",root.resolve("state").toString(),"TE_MICROSKILLS","envfacts","TE_LOOPGUARD","off","TE_TOOLPRUNE","off"),command,120);
  }
  static void copy(Path source,Path target)throws Exception {
    Files.createDirectories(target.getParent());Files.copy(source,target,StandardCopyOption.COPY_ATTRIBUTES);
  }
  static void hashes(Path bundle)throws Exception {
    var manifest=Files.readString(bundle.resolve("manifest.json"));
    var matches=Pattern.compile("\"([^\"]+)\": \"([0-9a-f]{64})\"").matcher(manifest);int count=0;
    while(matches.find()) {
      if(matches.group(1).equals("cargo_lock_sha256"))continue;
      var hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(bundle.resolve(matches.group(1)))));
      require(hash.equals(matches.group(2)),"manifest hash differs: "+matches.group(1));count++;
    }
    require(count==18,"manifest omitted files: "+count);
  }
  static void engine(Path bundle)throws Exception {
    try(var client=new ContextProbe.Client(bundle.resolve("bin/te-bridge"),"experiment.swebench")) {
      var decision=client.call("experiment.net_guard.decide","\"command\":\"echo https://github.com/probe\"");
      require(decision.contains("[true,"),"relocated guard policy failed: "+decision);
      var result=client.call("experiment.swebench.guard.install","\"name\":\"owned-fixture\"");boolean copied=false;
      while(result.contains("\"callback\"")) {
        var method=Pattern.compile("\"method\":\"([^\"]+)\"").matcher(result);require(method.find(),"callback method missing: "+result);
        var name=method.group(1);
        String value=switch(name) {
          case "config_function"->"[\"WebSearch\"]";
          case "config_attr"->result.contains("NET_GUARD_PROBE")?q("echo https://github.com/probe"):result.contains("\"kind\"")?q("claude"):q("/tmp/task-home");
          case "_exec"->{var out=result.contains("uname -m")?"aarch64\n":result.contains("mkdir -p")?"ready\n":result.contains("--install-hook")?"armed\n":"rc=2\n";yield "{\"returncode\":0,\"stdout\":"+q(out)+",\"stderr\":\"\"}";}
          case "container_run"->{require(result.contains(bundle.resolve("bin/container-helpers/arm64/te-net-guard").toString()),"helper discovery used another checkout: "+result);copied=true;yield "{\"returncode\":0,\"stdout\":\"\",\"stderr\":\"\"}";}
          default->throw new AssertionError("unexpected callback: "+result);
        };
        result=client.resume(result,name,"{\"value\":"+value+"}");
      }
      require(copied&&result.equals("{\"result\":true}"),"relocated helper discovery failed: "+result);
    }
  }
  public static void main(String[] args)throws Exception {
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory("te-experiment-bundle-").toAbsolutePath();boolean passed=false;
    try {
      var binaries=Files.createDirectories(root.resolve("inputs/bin"));
      for(var name:List.of("te","te-hook","te-bridge","te-net-guard","te-repomap-gen","te-package"))copy(checkout.resolve("target/debug/"+name),binaries.resolve(name));
      var helpers=Files.createDirectory(root.resolve("inputs/helpers"));
      for(var architecture:List.of("arm64","amd64"))for(var name:List.of("te-net-guard","manifest.json","toolchain.txt","THIRD-PARTY-NOTICES.txt"))copy(checkout.resolve("target/container-helpers").resolve(architecture).resolve(name),helpers.resolve(architecture).resolve(name));
      var tool=binaries.resolve("te-package");var bundle=root.resolve("runtime payload.v1");
      var built=run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",bundle.toString(),"--zip");require(built.code()==0,built.toString());hashes(bundle);
      var defaults=run(root,tool,"--experiments","--container-helpers",helpers.toString());
      require(defaults.code()==0&&Files.isRegularFile(root.resolve("dist/experiments/manifest.json")),"default output depended on the build checkout: "+defaults);
      require(Files.mismatch(bundle.resolve("assets/codex_base_prompt/0.153.1.md"),checkout.resolve("reference/upstream/experiments/te_harness/data/codex_base_prompt/0.153.1.md"))==-1,"captured Codex prompt changed");
      require(Files.mismatch(bundle.resolve("assets/webbench_undici_preload.mjs"),checkout.resolve("reference/upstream/experiments/te_harness/webbench_undici_preload.mjs"))==-1,"WebBench preload changed");
      require(Files.mismatch(bundle.resolve("LICENSE"),checkout.resolve("LICENSE"))==-1,"licence changed");
      try(var zip=new ZipFile(bundle.resolveSibling("runtime payload.v1.zip").toFile());var paths=Files.walk(bundle)) {
        for(var path:paths.filter(Files::isRegularFile).toList()) {
          var entry=zip.getEntry("runtime payload.v1/"+bundle.relativize(path).toString().replace('\\','/'));require(entry!=null,"archive omitted "+path);
          try(var input=zip.getInputStream(entry)){require(java.util.Arrays.equals(input.readAllBytes(),Files.readAllBytes(path)),"archive changed "+path);}
        }
      }
      require(run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",bundle.toString()).code()!=0,"existing output was overwritten");hashes(bundle);
      var moved=root.resolve("relocated runtime's directory");Files.move(bundle,moved);engine(moved);
      require(run(root,moved.resolve("bin/te"),"--help").code()==0,"relocated native CLI failed");
      var patch=root.resolve("patch.json");Files.writeString(patch,"{\"keep\":1}");var settings=root.resolve("settings.json");
      require(run(root,moved.resolve("bin/te-net-guard"),"--merge-json",settings.toString(),patch.toString()).out().equals("merged\n"),"relocated native merge failed");
      var invalid=helpers.resolve("amd64/manifest.json");var before=Files.readString(invalid);Files.writeString(invalid,before.replace("\"sha256\": \"","\"sha256\": \"bad"));
      var rejected=root.resolve("rejected");require(run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",rejected.toString()).code()!=0&&!Files.exists(rejected),"invalid helper created an output");Files.writeString(invalid,before);
      require(run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",helpers.resolve("overlap").toString()).code()!=0,"input/output overlap was accepted");
      for(var options:List.of(List.of("--experiments"),List.of("--experiments=true"),List.of("--experiments","--droid"),List.of("--experiments","--source","missing"),List.of("--container-helpers",helpers.toString())))require(run(root,tool,options.toArray(String[]::new)).code()!=0,"invalid arguments accepted: "+options);
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Experiment runtime bundle passed "+checks+" checks; embedded source assets, manifest hashes, ZIP bytes, relocation, native services, default helper discovery and output guards passed with an empty PATH");
  }
}
