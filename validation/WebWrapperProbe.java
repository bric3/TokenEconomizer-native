//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares agent task wrappers and Codex-home pruning with the pinned source. */
class WebWrapperProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  static void remove(Path root) throws Exception {
    if(Files.exists(root))try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}
  }
  static void seed(Path root) throws Exception {
    remove(root);
    var home=Files.createDirectories(root.resolve(".codex-home"));
    for(var name:List.of("config.toml","instructions.md","AGENTS.md","RTK.md"))Files.writeString(home.resolve(name),"provenance "+name);
    Files.writeString(home.resolve("state.sqlite"),"state");
    Files.createDirectory(home.resolve("sessions"));Files.writeString(home.resolve("sessions/roll.jsonl"),"{}");
    Files.createDirectory(root.resolve(".host-home"));Files.writeString(root.resolve(".host-home/state"),"throwaway");
    Files.createDirectory(root.resolve("outside"));Files.writeString(root.resolve("outside/keep"),"untouched");
    Files.createSymbolicLink(home.resolve("directory-link"),root.resolve("outside"));
    Files.createSymbolicLink(home.resolve("file-link"),root.resolve("outside/keep"));
  }
  static void checkPruned(Path root,boolean keep) throws Exception {
    var home=root.resolve(".codex-home");
    for(var name:List.of("config.toml","instructions.md","AGENTS.md","RTK.md"))if(!Files.readString(home.resolve(name)).equals("provenance "+name))throw new AssertionError("provenance changed: "+name);
    if(Files.exists(home.resolve("state.sqlite"))!=keep||Files.exists(home.resolve("sessions"))!=keep||Files.exists(root.resolve(".host-home"))!=keep)throw new AssertionError("cleanup retention changed");
    if(!Files.isSymbolicLink(home.resolve("directory-link"))||Files.isSymbolicLink(home.resolve("file-link"))!=keep||!Files.readString(root.resolve("outside/keep")).equals("untouched"))throw new AssertionError("symlink handling changed");
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_wrapper_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_wrapper_host.py")) {
      for(var operation:List.of("copilot","codex","prune")) {
        var base="\"operation\":"+q(operation);
        compare(nativeCode,source,base);
        for(var option:List.of("\"opaque_path\":true","\"opaque_out\":true","\"opaque_task\":true","\"opaque_debug\":true","\"opaque_caps\":true","\"kwargs\":{}","\"fields\":{\"scaffold_name\":\"rtk\"}","\"fields\":{\"scaffold_name\":\"serena\"}","\"fields\":{\"keep_workspaces\":true}","\"rewrite_caps\":[\"rewritten\"]","\"row\":{}","\"row\":{\"_debug_log\":null}","\"row\":{\"_debug_log\":false}","\"row\":{\"_debug_log\":42}","\"row\":{\"_debug_log\":\"\"}","\"isdir\":{\"/run/.codex-home\":false}","\"names\":[]","\"names\":null"))compare(nativeCode,source,base+","+option);
        var phases=operation.equals("copilot")?List.of("middle","parent","row.pop","pop","path.join","join","open","enter","write","exit"):operation.equals("codex")?List.of("set:_capabilities","middle","parent","row.set","get:_capabilities","get:_lean_instructions_tokens","get:answer_key_ns","get:scaffold_name","row.update","compliance","path.join","join","get:_prune_agent_home","prune_agent_home"):List.of("get:agent_home","agent_home","get:keep_workspaces","path.isdir","isdir","listdir","get:_AGENT_HOME_KEEP","path.join","join","rmtree","remove");
        for(var phase:phases)for(var error:List.of("RuntimeError","OSError","PermissionError","KeyboardInterrupt","DetailError","ShadowOSError","ObservedOSError"))compare(nativeCode,source,base+",\"fields\":{\"scaffold_name\":\"rtk\"},\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      }
      for(var value:List.of("null","false","0","1","\"\"","\"12\"","\"bad\"","[]","{}","7".repeat(700)))compare(nativeCode,source,"\"fields\":{\"_lean_instructions_tokens\":"+value+"}");
      for(var value:List.of("null","false","0","1","\"ab\"","[]","{}","{\"a\":1}","[null,{},42]"))compare(nativeCode,source,"\"kwargs\":{\"codex_config_capabilities\":"+value+"}");
      for(var value:List.of("null","false","42","[]","[1]","[1,2,3]"))compare(nativeCode,source,"\"parent_result\":"+value);
      for(var option:List.of("\"objects\":[\"_lean_instructions_tokens\"]","\"objects\":[\"answer_key_ns\"]","\"compliance\":[[\"first\",1],[\"bad\"]]"))compare(nativeCode,source,"\"fields\":{\"scaffold_name\":\"rtk\"},"+option);
      for(var phase:List.of("write","exit","join"))for(boolean suppress:List.of(false,true))compare(nativeCode,source,"\"operation\":\"copilot\",\"fail\":"+q(phase)+",\"error\":\"ObservedOSError\",\"suppress\":"+suppress+",\"exception_chain\":true");
      for(boolean suppress:List.of(false,true))compare(nativeCode,source,"\"operation\":\"copilot\",\"fail\":\"write\",\"error\":\"ObservedOSError\",\"opaque_exit\":true,\"suppress\":"+suppress+",\"exception_chain\":true");
      for(var phase:List.of("error.str","log","exit.bool"))compare(nativeCode,source,"\"operation\":\"copilot\",\"opaque_exit\":true,\"exception_chain\":true,\"failures\":[{\"fail\":\"write\",\"error\":\"ObservedOSError\"},{\"fail\":"+q(phase)+",\"error\":\"RuntimeError\"}]");
      for(var phase:List.of("isdir","join","rmtree","remove"))for(int at:List.of(2,3,4))compare(nativeCode,source,"\"operation\":\"prune\",\"fail\":"+q(phase)+",\"fail_at\":"+at+",\"error\":\"OSError\"");
      for(var names:List.of("[null]","[42]","[[]]","[\"config.toml\",\"state.sqlite\",\"config.toml\"]"))compare(nativeCode,source,"\"operation\":\"prune\",\"names\":"+names);
      var root=Files.createTempDirectory(Path.of("validation/results"),"web-wrapper-").toAbsolutePath();
      try {
        var logs=Files.createDirectories(root.resolve("logs"));
        compare(nativeCode,source,"\"operation\":\"copilot\",\"real_files\":true,\"out_dir\":"+q(logs));
        if(!Files.readString(logs.resolve("copilot_debug.log")).equals("debug é😀\n"))throw new AssertionError("debug log changed");
        var run=root.resolve("run");
        for(boolean keep:List.of(false,true)) {
          var request="{\"operation\":\"prune\",\"real_files\":true,\"run_dir\":"+q(run)+",\"fields\":{\"keep_workspaces\":"+keep+"}}";
          seed(run);var expected=source.call(request);checkPruned(run,keep);
          seed(run);var actual=nativeCode.call(request);checkPruned(run,keep);
          if(!actual.equals(expected)||actual.contains("\"raised\":"))throw new AssertionError("real pruning differs: "+actual+"\n"+expected);
          cases++;
        }
      } finally {remove(root);}
    }
    System.out.println("Web-Bench agent wrappers match the source in "+cases+" comparisons; superclass dispatch, references, capabilities, debug logs, cleanup and exception boundaries passed");
  }
}
