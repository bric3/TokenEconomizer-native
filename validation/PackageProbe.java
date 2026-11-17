//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.security.MessageDigest;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;
import java.util.zip.ZipFile;

/** Native bundles run after relocation, with an empty PATH and no Python runtime payload. */
class PackageProbe {
  static void require(boolean condition,String message) {CliProbe.require(condition,message);}
  static String q(Object value) {return ParserProbe.q(value);}
  static NativeCliProbe.Output command(Path root,Map<String,String> env,Path exe,String... args) throws Exception {
    var command=new ArrayList<>(List.of(exe.toString()));command.addAll(List.of(args));
    return NativeCliProbe.process(root,env,command);
  }
  static NativeCliProbe.Output hook(Path root,Map<String,String> env,Path exe,String host,Path repo) throws Exception {
    var input=Files.createTempFile(root,"stdin-",".json");
    var out=Files.createTempFile(root,"stdout-",".log");var err=Files.createTempFile(root,"stderr-",".log");
    try {
      Files.writeString(input,"{\"hook_event_name\":\"SessionStart\",\"session_id\":\"packaged\",\"cwd\":"+q(repo)+"}");
      var builder=new ProcessBuilder(exe.toString(),"--host",host).directory(repo.toFile()).redirectInput(input.toFile()).redirectOutput(out.toFile()).redirectError(err.toFile());
      builder.environment().clear();builder.environment().putAll(env);
      var process=builder.start();
      if(!process.waitFor(30,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("Packaged hook timed out");}
      return new NativeCliProbe.Output(process.exitValue(),Files.readString(out),Files.readString(err));
    } finally {Files.delete(input);Files.delete(out);Files.delete(err);}
  }
  static Map<String,String> environment(Path root) {
    return new HashMap<>(Map.of("HOME",root.toString(),"PATH","","TOKENECONOMIZER_HOME",root.resolve("state").toString(),
        "TE_MICROSKILLS","envfacts","TE_LOOPGUARD","off","TE_TOOLPRUNE","off","TE_FOLD_SYNC","1","GIT_CEILING_DIRECTORIES",root.toString(),"TZ","UTC"));
  }
  static String withoutWindows(String text) {
    return text.replaceAll("(?m)^\\s*\"(?:commandWindows|windows)\":.*\\n","");
  }
  static void metadata(Path checkout,Path plugin) throws Exception {
    var hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(checkout.resolve("Cargo.lock"))));
    require(Files.readString(plugin.resolve("THIRD-PARTY-NOTICES.txt")).contains("Cargo.lock SHA-256: "+hash+"\n"),"Bundle notices do not match the lockfile");
    require(Arrays.equals(Files.readAllBytes(plugin.resolve("LICENSE")),Files.readAllBytes(checkout.resolve("reference/upstream/LICENSE"))),"Bundle lost the upstream licence");
    var manifest=Path.of(".claude-plugin/plugin.json");
    var sourceManifest=checkout.resolve("reference/upstream/plugin").resolve(manifest);
    require(Files.getLastModifiedTime(plugin.resolve(manifest)).equals(Files.getLastModifiedTime(sourceManifest)),"Copy changed a pinned asset's modification time");
    for(var host:List.of("codex","copilot")) {
      var path=Path.of("hooks",host+"-hooks.json");
      ContextProbe.expect(withoutWindows(Files.readString(plugin.resolve(path))),withoutWindows(Files.readString(checkout.resolve("reference/upstream/plugin").resolve(path))));
    }
    var windows=Files.readString(plugin.resolve("hooks/codex-hooks.json"));
    var matches=Pattern.compile("-EncodedCommand ([A-Za-z0-9+/=]+)").matcher(windows);
    int count=0;
    while(matches.find()) {
      var script=new String(Base64.getDecoder().decode(matches.group(1)),StandardCharsets.UTF_16LE);
      require(script.contains("$env:PLUGIN_ROOT") && script.contains("$env:CLAUDE_PLUGIN_ROOT") && script.contains("scripts/te-hook.exe") && script.contains("--host codex") && !script.contains("python"),script);
      count++;
    }
    require(count==5,"Codex Windows command missing from an event");
    var copilot=Files.readString(plugin.resolve("hooks/copilot-hooks.json"));
    require(copilot.contains("COPILOT_PLUGIN_ROOT") && copilot.contains("InputEncoding") && !copilot.contains("te_bootstrap"),"Copilot retained its Python launcher");
    for(var host:List.of("pi","opencode")) {
      var source=Files.readString(plugin.resolve(host+"/tokeneconomizer.ts"));
      require(source.contains("script + (platform === \"win32\" ? \".exe\" : \"\")") && !source.contains("te_bootstrap.py"),host+" did not select the native launcher");
    }
    var hermes=Files.readString(plugin.resolve("hermes/__init__.py"));
    require(hermes.contains("--hermes-bridge") && !hermes.contains("te_bootstrap.py") && !hermes.contains("sys.executable"),"Hermes did not select the native gateway");
  }
  static void archive(Path root,Path plugin,Path zipPath) throws Exception {
    try(var zip=new ZipFile(zipPath.toFile())) {
      var entries=zip.stream().toList();
      require(entries.stream().allMatch(entry->entry.getName().startsWith("tokeneconomizer/")),"ZIP has entries outside the plugin");
      try(var files=Files.walk(plugin)) {
        for(var file:files.filter(Files::isRegularFile).toList()) {
          var name="tokeneconomizer/"+plugin.relativize(file).toString().replace('\\','/');
          var entry=zip.getEntry(name);require(entry!=null,"ZIP omitted "+name);
          var modified=Files.getLastModifiedTime(file).toInstant().atOffset(ZoneOffset.UTC).toLocalDateTime().withNano(0);
          modified=modified.withSecond(modified.getSecond()/2*2);
          require(entry.getTimeLocal().equals(modified),"ZIP changed modification time for "+name);
          try(var input=zip.getInputStream(entry)){require(Arrays.equals(Files.readAllBytes(file),input.readAllBytes()),"ZIP changed "+name);}
        }
      }
    }
    var extracted=Files.createDirectory(root.resolve("extracted"));
    var unzipped=NativeCliProbe.process(extracted,environment(root),List.of("/usr/bin/unzip","-q",zipPath.toString()));
    require(unzipped.code()==0,unzipped.toString());
    require(Files.isExecutable(extracted.resolve("tokeneconomizer/scripts/te-hook")),"ZIP lost hook executable permissions");
    require(command(root,environment(root),extracted.resolve("tokeneconomizer/scripts/te"),"--help").code()==0,"Extracted native CLI failed");
  }
  static void relocation(Path root,Path plugin) throws Exception {
    var moved=root.resolve("installed plugin's directory");Files.move(plugin,moved);
    var env=environment(root);var repo=Files.createDirectory(root.resolve("repo"));
    require(!Files.exists(moved.resolve("lib")) && !Files.exists(moved.resolve("scripts/te_bootstrap.py")),"Native bundle retained the Python runtime payload");
    for(var host:List.of("claude","codex","copilot","cursor","hermes","pi","opencode","grok","antigravity","droid")) {
      var status=command(root,env,moved.resolve("scripts/te"),"--host",host,"--cwd",repo.toString(),"status","--json");
      require(status.code()==0 && status.out().contains("\"host\": \""+host+"\""),status.toString());
    }
    var result=hook(root,env,moved.resolve("scripts/te-hook"),"claude",repo);
    require(result.code()==0 && result.out().equals("{}\n") && result.err().isEmpty(),result.toString());
    var launcher=Path.of(env.get("TOKENECONOMIZER_HOME"),"bin/te");
    require(Files.isExecutable(launcher) && command(root,env,launcher,"--help").code()==0,"Installed hook did not create a usable native CLI launcher");
    var map=command(root,env,moved.resolve("scripts/te-repomap-gen"),"--python",root.resolve("missing-python").toString(),repo.toString(),"2048");
    require(map.code()==1 && !map.err().isEmpty(),map.toString());
    require(!Files.exists(root.resolve("state/errors.log")),"Installed hook logged an error");
  }
  public static void main(String[] args) throws Exception {
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory(checkout.resolve("validation/results"),"package-");
    var executable=checkout.resolve("target/release/te-package");
    try {
      var env=environment(root);var market=root.resolve("market");
      var built=command(root,env,executable,"--out="+market,"--zip");
      require(built.code()==0,built.toString());var plugin=market.resolve("tokeneconomizer");
      metadata(checkout,plugin);archive(root,plugin,root.resolve("tokeneconomizer-plugin.zip"));
      Files.writeString(plugin.resolve("stale"),"remove on rebuild");Files.writeString(market.resolve("keep"),"keep");
      require(command(root,env,executable,"--out",market.toString()).code()==0,"Rebuild failed");
      require(!Files.exists(plugin.resolve("stale")) && Files.readString(market.resolve("keep")).equals("keep"),"Rebuild crossed its output directory");
      relocation(root,plugin);
      var droid=root.resolve("droid");var made=command(root,env,executable,"--d","--ou",droid.toString());
      require(made.code()==0,made.toString());var droidPlugin=droid.resolve("tokeneconomizer");
      for(var path:List.of("hooks.json",".claude-plugin",".codex-plugin",".cursor-plugin",".github",".antigravity-plugin","pi","opencode","hermes","slash-commands","package.json","lib"))require(!Files.exists(droidPlugin.resolve(path)),"Droid included "+path);
      require(Files.exists(droidPlugin.resolve(".factory-plugin/plugin.json")) && Files.exists(droid.resolve(".factory-plugin/marketplace.json")),"Droid metadata missing");
      for(var name:List.of("config","facts","microskills","repos","status","toolprune","untool")) {
        var text=Files.readString(droidPlugin.resolve("commands/tokeneconomizer-"+name+".md"));
        require(text.contains("${DROID_PLUGIN_ROOT}") && text.contains("Execute tool") && !text.contains("Bash tool"),"Droid command changed: "+name);
      }
      var missing=Files.createDirectory(root.resolve("missing-binaries"));var guard=Files.createDirectories(root.resolve("guard/tokeneconomizer"));Files.writeString(guard.resolve("keep"),"keep");
      require(command(root,env,executable,"--binaries",missing.toString(),"--out",guard.getParent().toString()).code()!=0 && Files.exists(guard.resolve("keep")),"Missing binaries destroyed the prior bundle");
      for(var invalid:List.of(List.of("--out","--droid"),List.of("--zip=false"),List.of("--unknown"),List.of("--","positional"))) {
        require(command(root,env,executable,invalid.toArray(String[]::new)).code()!=0,"Invalid arguments were accepted: "+invalid);
      }
      var nested=Files.createDirectory(guard.resolve("binaries"));
      var overlap=command(root,env,executable,"--out",guard.getParent().toString(),"--binaries",nested.toString());
      require(overlap.code()!=0 && overlap.err().contains("contains its binary inputs") && Files.exists(guard.resolve("keep")),"Output check lost binary inputs");
      var source=Files.createDirectory(root.resolve("overlap-source"));
      overlap=command(root,env,executable,"--source",source.toString(),"--out",source.toString());
      require(overlap.code()!=0 && overlap.err().contains("overlaps its source"),"Output check accepted a source directory");
      var windows=Files.createDirectory(root.resolve("windows-binaries"));
      for(var name:List.of("te","te-hook","te-repomap-gen")){var file=windows.resolve(name+".exe");Files.writeString(file,"binary fixture");Files.setPosixFilePermissions(file,PosixFilePermissions.fromString("rwx------"));}
      var winOut=root.resolve("windows");var win=command(root,env,executable,"--binaries",windows.toString(),"--out",winOut.toString());
      require(win.code()==0,win.toString());
      var cmd=Files.readString(winOut.resolve("tokeneconomizer/scripts/te-hook.cmd"));
      require(cmd.contains("\"%~dp0te-hook.exe\" %*") && !cmd.contains("python"),cmd);
      require(Files.readString(winOut.resolve("tokeneconomizer/scripts/te-hook")).contains("$dir/te-hook.exe"),"Git Bash wrapper did not call the native executable");
      System.out.println("Native marketplace and Droid bundles, ZIP contents and permissions, relocation, host launchers and output guards passed without Python on PATH");
    } finally {try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.deleteIfExists(path);}}
  }
}
