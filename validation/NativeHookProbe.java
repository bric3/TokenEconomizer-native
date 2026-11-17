//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/** Exercises the standalone hook and its native launcher without installed agent commands. */
class NativeHookProbe {
  static String q(Object value) {return ParserProbe.q(value);}
  static void require(boolean condition,String message) {CliProbe.require(condition,message);}
  static NativeCliProbe.Output hook(Path checkout,Path root,Map<String,String> env,boolean nativeHook,byte[] payload,String... args) throws Exception {
    var input=Files.createTempFile(root,"stdin-",".json");
    var out=Files.createTempFile(root,"stdout-",".log");
    var err=Files.createTempFile(root,"stderr-",".log");
    try {
      Files.write(input,payload);
      var command=new ArrayList<>(nativeHook?List.of(checkout.resolve("target/debug/te-hook").toString()):List.of(checkout.resolve(".venv/bin/python").toString(),"-m","tokeneconomizer.plugin.hook"));
      command.addAll(List.of(args));
      var builder=new ProcessBuilder(command).directory(root.toFile()).redirectInput(input.toFile()).redirectOutput(out.toFile()).redirectError(err.toFile());
      builder.environment().clear();builder.environment().putAll(env);
      var process=builder.start();
      if(!process.waitFor(30,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("Hook timed out: " + command);}
      var result=new NativeCliProbe.Output(process.exitValue(),Files.readString(out),Files.readString(err));
      require(result.code()==0,result.toString());
      return result;
    } finally {Files.delete(input);Files.delete(out);Files.delete(err);}
  }
  static NativeCliProbe.Output hook(Path checkout,Path root,Map<String,String> env,boolean nativeHook,String payload,String host) throws Exception {
    return hook(checkout,root,env,nativeHook,payload.getBytes(StandardCharsets.UTF_8),"--host",host);
  }
  static Map<String,String> environment(Path checkout,Path root,Path state) {
    return new HashMap<>(Map.of("HOME",root.toString(),"PATH","","TOKENECONOMIZER_HOME",state.toString(),
        "PYTHONPATH",checkout.resolve("reference/upstream").toString(),"TE_FOLD_SYNC","1","TE_MICROSKILLS","envfacts",
        "TE_TOOLPRUNE","off","TE_LOOPGUARD","off","GIT_CEILING_DIRECTORIES",root.toString(),"LANG","C.UTF-8"));
  }
  static Path base(Path state,String host) throws Exception {
    try(var repos=Files.list(state.resolve("repos"))) {return repos.filter(Files::isDirectory).findFirst().orElseThrow().resolve(host);}
  }
  static String payload(Path repo,Path transcript,String host,String sid,String event) {
    var fields="\"cwd\":"+q(repo)+",\"transcript_path\":"+q(transcript)+",\"prompt\":\"check é😀\"";
    if(host.equals("cursor")) {
      var nativeEvent=switch(event){case "SessionStart"->"sessionStart";case "UserPromptSubmit"->"beforeSubmitPrompt";case "PostToolUse"->"postToolUse";case "SessionEnd"->"sessionEnd";default->event.toLowerCase();};
      return "{"+fields+",\"hook_event_name\":"+q(nativeEvent)+",\"conversation_id\":"+q(sid)+",\"generation_id\":\"g1\",\"workspace_roots\":["+q(repo)+"]}";
    }
    if(host.equals("grok")) {
      var nativeEvent=switch(event){case "SessionStart"->"session_start";case "UserPromptSubmit"->"user_prompt_submit";case "PostToolUse"->"post_tool_use";case "SessionEnd"->"session_end";default->event.toLowerCase();};
      return "{"+fields+",\"hookEventName\":"+q(nativeEvent)+",\"sessionId\":"+q(sid)+",\"promptId\":\"p1\"}";
    }
    if(host.equals("antigravity"))return "{\"hook_event_name\":"+q(event.equals("UserPromptSubmit")?"PreInvocation":event)+",\"conversationId\":"+q(sid)+",\"workspacePaths\":["+q(repo)+"],\"transcriptPath\":"+q(transcript)+",\"lastUserInput\":\"check é😀\"}";
    return "{"+fields+",\"hook_event_name\":"+q(event)+",\"session_id\":"+q(sid)+"}";
  }
  static void hosts(Path checkout,Path root,Path repo) throws Exception {
    var transcript=root.resolve("empty.jsonl");
    for(var host:List.of("claude","codex","copilot","pi","opencode","hermes","droid","cursor","grok","antigravity")) {
      var nativeEnv=environment(checkout,root,root.resolve(host+"-native"));
      var sourceEnv=environment(checkout,root,root.resolve(host+"-source"));
      for(var env:List.of(nativeEnv,sourceEnv)) {
        boolean nativeHook=env==nativeEnv;
        var start=hook(checkout,root,env,nativeHook,payload(repo,transcript,host,"prime","SessionStart"),host);
        ContextProbe.expect(start.out(),"{}\n");ContextProbe.expect(start.err(),"");
        Files.writeString(base(Path.of(env.get("TOKENECONOMIZER_HOME")),host).resolve("injection.json"),"{\"rendered\":\"- Use python -m pytest for checks. é😀\",\"rendered_ids\":[\"fact\"]}");
      }
      boolean injected=false;
      for(var event:List.of("SessionStart","UserPromptSubmit","PostToolUse","Stop","SessionEnd")) {
        var body=payload(repo,transcript,host,"live",event);
        var actual=hook(checkout,root,nativeEnv,true,body,host);
        var expected=hook(checkout,root,sourceEnv,false,body,host);
        require(actual.out().equals(expected.out()),host+" "+event+"\nNative: "+actual+"\nUpstream: "+expected);
        ContextProbe.expect(actual.err(),expected.err());
        injected|=actual.out().contains("Use python -m pytest");
      }
      require(injected,host+" did not deliver the seeded fact");
      var nativeState=Path.of(nativeEnv.get("TOKENECONOMIZER_HOME"));
      var live=base(nativeState,host).resolve("sessions/live.json");
      var saved=Files.readString(live);
      require(saved.contains("\"session_id\": \"live\""),host+" did not persist the session");
      // Antigravity has no SessionEnd hook; a later SessionStart schedules its stale sessions.
      if(!host.equals("antigravity"))require(saved.contains("\"ended\""),host+" did not persist SessionEnd");
      require(!Files.exists(nativeState.resolve("errors.log")),host+" logged an error");
      var launcher=nativeState.resolve("bin/te");
      require(Files.isExecutable(launcher),host+" did not create the native launcher");
      require(Files.getPosixFilePermissions(launcher).equals(PosixFilePermissions.fromString("rwx------")),"Launcher permissions are not private");
      var before=Files.getLastModifiedTime(launcher);
      hook(checkout,root,nativeEnv,true,payload(repo,transcript,host,"last","SessionStart"),host);
      require(before.equals(Files.getLastModifiedTime(launcher)),"Unchanged launcher was rewritten");
      var status=NativeCliProbe.process(root,nativeEnv,List.of(launcher.toString(),"--host",host,"--cwd",repo.toString(),"status","--json"));
      require(status.code()==0 && status.out().contains("\"host\": \""+host+"\""),status.toString());
    }
  }
  static void boundaries(Path checkout,Path root,Path repo) throws Exception {
    var state=root.resolve("boundary-state");var env=environment(checkout,root,state);
    for(var input:List.of("null","[]","42","false","{\"hook_event_name\":\"SessionStart\"}"))ContextProbe.expect(hook(checkout,root,env,true,input,"claude").out(),"{}\n");
    env.put("TE_DISABLE","on");
    ContextProbe.expect(hook(checkout,root,env,true,"invalid","claude").out(),"{}\n");
    require(!Files.exists(state),"Ignored inputs created state");
    ContextProbe.expect(hook(checkout,root,env,true,new byte[]{(byte)255},"--host","claude").out(),"{}\n");
    var errors=state.resolve("errors.log");require(Files.readString(errors).contains("pid="),"Invalid UTF-8 was not logged");
    env.remove("TE_DISABLE");long before=Files.size(errors);
    ContextProbe.expect(hook(checkout,root,env,true,"{","claude").out(),"{}\n");
    require(Files.size(errors)>before,"Malformed JSON was not logged");
    Files.delete(errors);
    env.put("TE_HISTORY_IMPORT","on");
    var transcript=root.resolve("empty.jsonl");
    var imported=hook(checkout,root,env,true,payload(repo,transcript,"claude","history","SessionStart"),"claude");
    ContextProbe.expect(imported.out(),"{}\n");
    require(imported.err().contains("0 earlier session(s)"),"Synchronous history diagnostics did not reach stderr: "+imported);
    require(Files.readString(base(state,"claude").resolve("history.json")).contains("\"state\": \"done\""),"Native history import did not finish");
    env.remove("TE_HISTORY_IMPORT");
    env.put("TE_LOOPGUARD","on");
    hook(checkout,root,env,true,payload(repo,transcript,"claude","queued","SessionStart"),"claude");
    var queue=Files.createDirectories(state.resolve("locks/claude/queued.queue"));
    Files.writeString(queue.resolve("001.json"),payload(repo,transcript,"claude","queued","UserPromptSubmit"));
    var tool=payload(repo,transcript,"claude","queued","PostToolUse");
    Files.writeString(queue.resolve("002.json"),tool.substring(0,tool.length()-1)+",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"pwd\"},\"tool_response\":\"/repo\"}");
    hook(checkout,root,env,true,payload(repo,transcript,"claude","queued","Stop"),"claude");
    try(var entries=Files.list(queue)){require(entries.findAny().isEmpty(),"Native hook did not drain queued event");}
    var session=Files.readString(base(state,"claude").resolve("sessions/queued.json"));
    require(session.contains("\"tool_calls\": 1") && session.contains("\"turn\": null"),"Replay did not close the queued turn: "+session);
    env.put("TE_LOOPGUARD","off");env.put("TE_REPOMAP","start");
    var map=base(state,"claude").getParent().resolve("repomap.json");
    Files.writeString(map,"{\"commit\":\"old\",\"tokens_budget\":2048,\"map\":\"cached map é😀\"}");
    var mapped=hook(checkout,root,env,true,payload(repo,transcript,"claude","mapped","SessionStart"),"claude");
    require(mapped.out().contains("cached map") && Files.readString(map).contains("no Python with aider found"),"Failed native refresh did not retain the cached map: "+mapped);
    env.put("TE_REPOMAP","on");
    mapped=hook(checkout,root,env,true,payload(repo,transcript,"claude","prompt-map","UserPromptSubmit"),"claude");
    require(mapped.out().contains("cached map"),"Prompt lookup did not inject the cached map: "+mapped);
    require(!Files.exists(errors),"Replay logged an error");
  }
  static void background(Path checkout,Path root,Path repo) throws Exception {
    var state=root.resolve("background-state");var env=environment(checkout,root,state);env.remove("TE_FOLD_SYNC");
    var transcript=root.resolve("empty.jsonl");
    hook(checkout,root,env,true,payload(repo,transcript,"claude","background","SessionStart"),"claude");
    var base=base(state,"claude");var folded=base.resolve("folded.json");
    hook(checkout,root,env,true,payload(repo,transcript,"claude","background","SessionEnd"),"claude");
    long deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(20);
    while(!Files.exists(folded) || !Files.readString(folded).contains("background")) {
      if(System.nanoTime()>deadline)throw new AssertionError("Hook's detached native fold did not finish: "+(Files.exists(base.resolve("fold.log"))?Files.readString(base.resolve("fold.log")):"no log"));
      Thread.sleep(50);
    }
    // The folded marker is written after the ledger record, so it also marks a completed write.
    require(Files.readAllLines(base.resolve("ledger.jsonl")).size()==1,"Background fold lost or duplicated its record");
    require(!Files.exists(state.resolve("errors.log")),"Background hook logged an error");
  }
  static void launchers(Path checkout,Path root) throws Exception {
    var state=root.resolve("quoted-state");var target=root.resolve("native's directory/te");
    Files.createDirectories(target.getParent());Files.copy(checkout.resolve("target/debug/te"),target);
    Files.setPosixFilePermissions(target,PosixFilePermissions.fromString("rwx------"));
    var launcher=state.resolve("bin/te");var exe=launcher.resolveSibling("te.exe");
    try(var client=new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      var args="\"env\":{\"TOKENECONOMIZER_HOME\":"+q(state)+"},\"home\":"+q(root)+",\"target\":"+q(target)+",\"windows\":true";
      ContextProbe.expect(client.call("paths.ensure_native_launcher",args),"{\"result\":"+q(launcher)+"}");
      require(Arrays.equals(Files.readAllBytes(target),Files.readAllBytes(exe)),"Windows launcher did not copy native executable");
      var env=environment(checkout,root,state);
      require(NativeCliProbe.process(root,env,List.of(launcher.toString(),"--help")).out().startsWith("usage: te"),"Quoted launcher could not invoke the native CLI");
      var stamp=Files.getLastModifiedTime(exe);client.call("paths.ensure_native_launcher",args);
      require(stamp.equals(Files.getLastModifiedTime(exe)),"Unchanged executable was rewritten");
      var binary=Files.readAllBytes(target);binary[binary.length-1]^=1;
      var modified=Files.getLastModifiedTime(target);Files.write(target,binary);Files.setLastModifiedTime(target,modified);
      client.call("paths.ensure_native_launcher",args);
      require(Arrays.equals(binary,Files.readAllBytes(exe)),"Same-size executable replacement was missed");
      var self="\"env\":{\"TOKENECONOMIZER_HOME\":"+q(state)+"},\"home\":"+q(root)+",\"target\":"+q(launcher);
      var body=Files.readString(launcher);client.call("paths.ensure_native_launcher",self);
      ContextProbe.expect(Files.readString(launcher),body);
    }
  }
  public static void main(String[] args) throws Exception {
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory(checkout.resolve("validation/results"),"native-hook-");
    try {
      var repo=Files.createDirectories(root.resolve("repo/.git/info")).getParent().getParent();
      Files.writeString(repo.resolve(".git/HEAD"),"ref: refs/heads/main\n");
      Files.writeString(repo.resolve(".git/config"),"[remote \"origin\"]\nurl = https://github.com/acme/widget.git\n");
      Files.writeString(root.resolve("empty.jsonl"),"");
      boundaries(checkout,root,repo);launchers(checkout,root);background(checkout,root,repo);hosts(checkout,root,repo);
      System.out.println("Standalone native hooks match upstream for ten hosts; fail-open input, launchers, queued replay, history, maps and detached folding passed without Python on PATH");
    } finally {try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.deleteIfExists(path);}}
  }
}
