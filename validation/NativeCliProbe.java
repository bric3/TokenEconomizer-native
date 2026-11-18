//JAVA 25
//SOURCES CliProbe.java ContextProbe.java ParserProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.math.BigDecimal;
import java.util.regex.Pattern;

/** Standalone native CLI checks with isolated state and no installed model agents. */
class NativeCliProbe {
  static String q(Object value) {return ParserProbe.q(value);}
  static void require(boolean condition,String message) {CliProbe.require(condition,message);}
  record Output(int code,String out,String err) {}
  static String numbers(String json) {
    var out=new StringBuilder();
    boolean quoted=false,escaped=false;
    for(int i=0;i<json.length();i++) {
      char c=json.charAt(i);
      if(quoted) {
        out.append(c);
        if(escaped)escaped=false;else if(c=='\\')escaped=true;else if(c=='"')quoted=false;
      } else if(c=='"') {quoted=true;out.append(c);}
      else if(c=='-' || Character.isDigit(c)) {
        int end=i+1;
        while(end<json.length() && "0123456789.eE+-".indexOf(json.charAt(end))>=0)end++;
        out.append(new BigDecimal(json.substring(i,end)).stripTrailingZeros().toPlainString());i=end-1;
      } else out.append(c);
    }
    return out.toString();
  }
  static Output process(Path root, Map<String,String> env,List<String> argv) throws Exception {
    return process(root,env,argv,30);
  }
  static Output process(Path root, Map<String,String> env,List<String> argv,long timeoutSeconds) throws Exception {
    var out=Files.createTempFile(root,"stdout-",".log");
    var err=Files.createTempFile(root,"stderr-",".log");
    try {
      var builder=new ProcessBuilder(argv).directory(root.toFile()).redirectOutput(out.toFile()).redirectError(err.toFile());
      builder.environment().clear();builder.environment().putAll(env);
      var process=builder.start();
      if(!process.waitFor(timeoutSeconds,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("Command timed out: " + argv);}
      return new Output(process.exitValue(),Files.readString(out),Files.readString(err));
    } finally {Files.delete(out);Files.delete(err);}
  }
  static Output command(Path checkout,Path root,Map<String,String> env,boolean nativeCli,String host,String... args) throws Exception {
    var command=new ArrayList<>(nativeCli?List.of(checkout.resolve("target/debug/te").toString()):List.of(checkout.resolve(".venv/bin/python").toString(),"-m","tokeneconomizer.plugin.cli"));
    command.addAll(List.of("--host",host,"--cwd",root.resolve("repo").toString()));
    command.addAll(List.of(args));
    return process(root,env,command);
  }
  static Output compare(Path checkout,Path root,Map<String,String> env,int code,String host,String... args) throws Exception {
    var actual=command(checkout,root,env,true,host,args);
    var expected=command(checkout,root,env,false,host,args);
    require(actual.code()==code && expected.code()==code,List.of(args) + "\nNative: " + actual + "\nUpstream: " + expected);
    if(List.of(args).contains("--json"))ContextProbe.expect(numbers(actual.out()),numbers(expected.out()));
    else ContextProbe.expect(actual.out(),expected.out());
    ContextProbe.expect(actual.err(),expected.err());
    return actual;
  }
  static void worker(Path checkout,Path root,Map<String,String> env) throws Exception {
    var spec=root.resolve("worker.json");
    var log=root.resolve("worker.log");
    var command=checkout.resolve("target/debug/te");
    Files.writeString(spec,"{\"cmd\":[" + q(command) + ",\"--help\"],\"cwd\":" + q(root) + ",\"env\":{\"HOME\":" + q(root) + ",\"PATH\":\"\",\"TOKENECONOMIZER_HOME\":" + q(root.resolve("worker-state")) + "},\"log\":" + q(log) + "}");
    var result=process(root,env,List.of(command.toString(),"--run-spawn-spec",spec.toString()));
    require(result.code()==0 && !Files.exists(spec) && Files.readString(log).startsWith("usage: te"),result.toString());
  }
  public static void main(String[] args) throws Exception {
    var checkout=Path.of("").toAbsolutePath();
    var root=Files.createTempDirectory(checkout.resolve("validation/results"),"native-cli-");
    try {
      var repo=Files.createDirectory(root.resolve("repo"));
      var bin=Files.createDirectory(root.resolve("bin"));
      var state=root.resolve("state");
      var env=new HashMap<>(Map.of("HOME",root.toString(),"PATH",bin.toString(),"TOKENECONOMIZER_HOME",state.toString(),
          "PYTHONPATH",checkout.resolve("reference/upstream").toString(),"LANG","C.UTF-8","TZ","UTC","GIT_CONFIG_GLOBAL","/dev/null","GIT_CONFIG_NOSYSTEM","1"));
      Files.createSymbolicLink(bin.resolve("git"),Path.of("/usr/bin/git"));
      require(process(root,env,List.of("/usr/bin/git","init","-q",repo.toString())).code()==0,"Git fixture init failed");
      compare(checkout,root,env,0,"claude","config","show");
      compare(checkout,root,env,0,"claude","status");
      compare(checkout,root,env,0,"claude","status","--json");
      compare(checkout,root,env,0,"claude","status","--explain");
      compare(checkout,root,env,0,"claude","facts");
      compare(checkout,root,env,2,"claude","facts","forget");
      compare(checkout,root,env,1,"claude","facts","forget","missing");
      compare(checkout,root,env,0,"claude","untool");
      compare(checkout,root,env,0,"codex","toolprune","show");
      compare(checkout,root,env,0,"copilot","toolprune","show");
      compare(checkout,root,env,0,"claude","history");
      compare(checkout,root,env,0,"claude","history","import","--dry-run");
      compare(checkout,root,env,0,"claude","history","import");
      compare(checkout,root,env,0,"claude","repomap");
      compare(checkout,root,env,0,"claude","fold");
      compare(checkout,root,env,0,"claude","judge","--session","missing","--job","missing");
      compare(checkout,root,env,0,"claude","repos");
      compare(checkout,root,env,0,"claude","repos","--json");
      compare(checkout,root,env,0,"claude","repos","migrate");
      compare(checkout,root,env,2,"claude","repos","merge");
      compare(checkout,root,env,2,"claude","probe");
      compare(checkout,root,env,2,"claude","config","set","unknown=on");
      // Setting the same values twice produces identical output; each process uses its own runtime.
      compare(checkout,root,env,0,"claude","config","set","microskills=envfacts","toolprune=off","loopguard=off");
      compare(checkout,root,env,0,"claude","config","unset","microskills");
      compare(checkout,root,env,0,"claude","config","set","microskills=envfacts");
      var applied=command(checkout,root,env,true,"claude","toolprune","apply","Read");
      require(applied.code()==0 && Files.readString(repo.resolve(".claude/settings.local.json")).contains("Read"),applied.toString());
      require(command(checkout,root,env,true,"claude","toolprune","show").out().contains("Read"),"Native service did not record applied tools");
      var restored=command(checkout,root,env,true,"claude","toolprune","restore","Read");
      require(restored.code()==0 && !Files.readString(repo.resolve(".claude/settings.local.json")).contains("Read"),restored.toString());
      require(command(checkout,root,env,true,"claude","toolprune","show").out().contains("kept, as you gave them back"),"Native service lost kept tools");
      compare(checkout,root,env,1,"claude","repomap","refresh");
      var status=command(checkout,root,env,true,"claude","status","--json");
      var statePath=Pattern.compile("\"state\"\\s*:\\s*\"([^\"]+)\"").matcher(status.out());
      require(statePath.find(),status.toString());
      var base=Path.of(statePath.group(1));
      Files.createDirectories(base.resolve("sessions"));
      var transcript=root.resolve("session.jsonl");
      Files.writeString(transcript,"");
      Files.writeString(base.resolve("sessions/native-session.json"),"{\"transcript\":" + q(transcript) + ",\"cwd\":" + q(repo) + ",\"started\":1700000000,\"ended\":1700000100}");
      var folded=command(checkout,root,env,true,"claude","fold","--session","native-session","--verbose");
      require(folded.code()==0 && folded.out().contains("native-session"),folded.toString());
      require(Files.readString(base.resolve("folded.json")).contains("native-session") && Files.readString(base.resolve("injection.json")).contains("\"sessions\": 1"),"Native services did not persist the fold");
      long records=Files.readAllLines(base.resolve("ledger.jsonl")).size();
      require(command(checkout,root,env,true,"claude","fold","--session","native-session").code()==0 && Files.readAllLines(base.resolve("ledger.jsonl")).size()==records,"Session folded twice");
      var orphan=Files.createDirectories(state.resolve("repos/orphan/claude/sessions"));
      Files.writeString(orphan.resolve("background-session.json"),"{\"transcript\":" + q(transcript) + ",\"cwd\":" + q(repo) + ",\"started\":1700000200,\"ended\":1700000300}");
      Files.writeString(orphan.getParent().resolve("folded.json"),"{\"background-session\":1700000400}");
      var merged=command(checkout,root,env,true,"claude","repos","merge","orphan");
      require(merged.code()==0,merged.toString());
      long deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(20);
      while(!Files.readString(base.resolve("folded.json")).contains("background-session")) {
        if(System.nanoTime()>deadline)throw new AssertionError("Native background fold did not finish: " + merged + ", log=" + (Files.exists(base.resolve("fold.log"))?Files.readString(base.resolve("fold.log")):"missing"));
        Thread.sleep(50);
      }
      require(Files.readAllLines(base.resolve("ledger.jsonl")).size()==records+1,"Background fold lost its record");
      var doctor=command(checkout,root,env,true,"claude","doctor");
      require(doctor.code()==0 && doctor.out().startsWith("native      0.1.0 ok\n"),doctor.toString());
      require(doctor.out().contains("agents      none of"),doctor.toString());
      require(PosixFilePermissions.toString(Files.getPosixFilePermissions(state)).equals("rwx------"),"State root is not private");
      worker(checkout,root,env);
      var missingAgent=command(checkout,root,env,true,"copilot","probe");
      require(missingAgent.code()==1 && missingAgent.err().contains("`copilot` is not on PATH"),missingAgent.toString());
      var plugin=Files.createDirectories(root.resolve("plugin/hooks"));
      Files.writeString(plugin.resolve("copilot-hooks.json"),"{}");
      var copilot=bin.resolve("copilot");
      Files.writeString(copilot,"availability fixture; never executed\n");
      Files.setPosixFilePermissions(copilot,PosixFilePermissions.fromString("rwx------"));
      var tmp=Files.createDirectory(root.resolve("tmp"));
      env.put("TE_PLUGIN_ROOT",plugin.getParent().toString());env.put("TMPDIR",tmp.toString());
      var missingPython=command(checkout,root,env,true,"copilot","probe");
      require(missingPython.code()==1 && missingPython.err().contains("probe needs Python"),missingPython.toString());
      try(var files=Files.list(tmp)){require(files.findAny().isEmpty(),"Failed probe retained its temporary directory");}
      System.out.println("Standalone native CLI matches upstream across commands; native folding, runtime diagnostics and spawn-spec execution passed without model agents");
    } finally {
      try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.deleteIfExists(path);}
    }
  }
}
