//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Drives agent support comparisons against the pinned source. */
class WebAgentProbe {
  static String q(Object value){return ParserProbe.q(value);}
  static void remove(Path root)throws Exception{if(Files.exists(root))try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
  public static void main(String[] args) throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"web-agent-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_agent_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_agent_host.py")) {
      for(var agent:List.of("claude","copilot","codex"))for(var operation:List.of("home","bin","class_bin","env","prepare","copilot_home","config","merge","usage","row")) {
        if(operation.equals("config")&&!agent.equals("codex")||operation.equals("copilot_home")&&!agent.equals("copilot"))continue;
        var base="\"agent\":"+q(agent)+",\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"objects\":[\"home\"]","\"objects\":[\"run_dir_path\"]","\"objects\":[\"workspace\"]","\"result\":null","\"result\":{}","\"result\":false","\"result\":[]","\"objects\":[\"result\"]","\"objects\":[\"source\"],\"opaque_comparison\":true","\"objects\":[\"source\"],\"opaque_comparison\":true,\"booleans\":{\"comparison\":false}","\"objects\":[\"a\",\"b\"]","\"objects\":[\"_debug_log\"]","\"lean\":null","\"lean\":\"\"","\"objects\":[\"lean\"]","\"objects\":[\"cfg\"]","\"missing\":[\"base_prompt\"]","\"mutate_capabilities\":true","\"isdir\":{\"/run/.copilot-home\":true}","\"isdir\":{\"/operator/.cache/copilot\":true}","\"isdir\":{\"/operator/.cache/copilot\":true},\"exists\":{\"/run/home/.cache/copilot\":true}"))WebContractProbe.compare(nativeCode,source,base+","+option);
        var phases=switch(operation){
          case "config"->List.of("os.makedirs","mkdir","codex.lean_instructions","get:base_prompt","get:_capabilities","lean","set:_lean_instructions_tokens","os.path","path.join","join","codex.config_toml","get:routing","get:model","get:effort","get:system_skills","config_toml","open","enter","write","exit");
          case "copilot_home"->List.of("os.path","path.join","join","path.isdir","isdir","mkdir","copilot.copilot_config","copilot_config","isfile","shutil.copy2","copy2");
          case "prepare"->List.of("os.path","path.join","join","path.expanduser","expanduser","path.isdir","isdir","mkdir","exists","os.symlink","symlink");
          case "merge"->List.of("merge","a.bool","a.get_function","a.get","b.bool","b.get_function","b.get","merged.put");
          case "usage","row"->List.of("result.bool","result.get_function","result.get","claude_row","ne:source","format:source");
          default->List.of("os.path","path.join","join","get:agent_home","get:scaffold_copilot_home","isdir","mkdir");
        };
        for(var phase:phases)for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError","ShadowOSError"))WebContractProbe.compare(nativeCode,source,base+",\"isdir\":{\"/operator/.cache/copilot\":true},\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      }
      for(var value:List.of("null","false","0","\"\"","[]","{}","\"debug_log\"","\"stream_usage\"","\"no_turn_event\""))for(var agent:List.of("copilot","codex"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"row\",\"agent\":"+q(agent)+",\"result\":{\"tokens_source\":"+value+",\"usage_incomplete_reason\":\"existing\",\"output_tokens\":0}");
      for(var operation:List.of("usage","row"))for(var agent:List.of("claude","copilot","codex"))for(var object:List.of("usage","output_tokens","source"))WebContractProbe.compare(nativeCode,source,"\"operation\":"+q(operation)+",\"agent\":"+q(agent)+",\"objects\":["+q(object)+"]");
      for(var agent:List.of("copilot","codex")) {
        WebContractProbe.compare(nativeCode,source,"\"operation\":\"row\",\"agent\":"+q(agent)+",\"opaque_dict\":true");
        for(var phase:List.of("dict","row.get_function","row.get","row.put","row.item","row.setdefault_function","row.setdefault"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"row\",\"agent\":"+q(agent)+",\"opaque_dict\":true,\"fail\":"+q(phase)+",\"exception_chain\":true");
      }
      for(int at:List.of(1,2))for(boolean suppress:List.of(false,true))WebContractProbe.compare(nativeCode,source,"\"operation\":\"config\",\"fail\":\"write\",\"fail_at\":"+at+",\"error\":\"DetailError\",\"opaque_exit\":true,\"suppress\":"+suppress+",\"exception_chain\":true");
      for(var phase:List.of("exit","exit.bool"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"config\",\"opaque_exit\":true,\"failures\":[{\"fail\":\"write\",\"error\":\"DetailError\"},{\"fail\":"+q(phase)+",\"error\":\"RuntimeError\"}],\"exception_chain\":true");
      for(var phase:List.of("log","error.str"))WebContractProbe.compare(nativeCode,source,"\"agent\":\"copilot\",\"operation\":\"prepare\",\"isdir\":{\"/operator/.cache/copilot\":true},\"failures\":[{\"fail\":\"symlink\",\"error\":\"ObservedOSError\"},{\"fail\":"+q(phase)+",\"error\":\"RuntimeError\"}],\"exception_chain\":true");
      WebContractProbe.compare(nativeCode,source,"\"agent\":\"copilot\",\"operation\":\"prepare\",\"isdir\":{\"/operator/.cache/copilot\":true},\"fail\":\"symlink\",\"error\":\"ObservedOSError\"");
      for(var operation:List.of("usage","config"))for(var sequence:List.of("[true,false,true,false]","[false,true,false,true]"))WebContractProbe.compare(nativeCode,source,"\"operation\":"+q(operation)+",\"objects\":["+q(operation.equals("usage")?"result":"lean")+"],\"booleans\":{"+q(operation.equals("usage")?"result":"lean")+":"+sequence+"}");
      var root=WebContractProbe.artifacts;
      var configHome=Files.createDirectory(root.resolve("config"));
      WebContractProbe.compare(nativeCode,source,"\"operation\":\"config\",\"real_files\":true,\"home\":"+q(configHome));
      if(!Files.readString(configHome.resolve("config.toml")).equals("fixture config é😀\n")||!Files.readString(configHome.resolve("instructions.md")).equals("lean instructions é😀\n"))throw new AssertionError("real config or instructions changed");
      var auth=Files.createDirectory(root.resolve("operator-config"));
      for(var name:List.of("config.json","settings.json","mcp-config.json","auth.json"))Files.writeString(auth.resolve(name),"operator "+name+" é😀");
      Files.createDirectory(auth.resolve("session-state"));Files.writeString(auth.resolve("session-state/history"),"private history");
      var run=Files.createDirectory(root.resolve("run"));
      var request="{\"agent\":\"copilot\",\"operation\":\"copilot_home\",\"real_files\":true,\"run_dir\":"+q(run)+",\"src\":"+q(auth)+"}";
      var expected=source.call(request);remove(run.resolve(".copilot-home"));var actual=nativeCode.call(request);
      if(!actual.equals(expected)||actual.contains("\"raised\":"))throw new AssertionError("real Copilot home differs: "+actual+"\n"+expected);
      WebContractProbe.cases++;
      for(var name:List.of("config.json","settings.json","mcp-config.json"))if(!Files.readString(run.resolve(".copilot-home").resolve(name)).equals(Files.readString(auth.resolve(name))))throw new AssertionError("seeded config differs: "+name);
      if(Files.exists(run.resolve(".copilot-home/auth.json"))||Files.exists(run.resolve(".copilot-home/session-state")))throw new AssertionError("unexpected operator files copied");
      Files.writeString(run.resolve(".copilot-home/settings.json"),"task settings");
      WebContractProbe.compare(nativeCode,source,"\"agent\":\"copilot\",\"operation\":\"copilot_home\",\"real_files\":true,\"run_dir\":"+q(run)+",\"src\":"+q(auth));
      if(!Files.readString(run.resolve(".copilot-home/settings.json")).equals("task settings")||!Files.readString(auth.resolve("settings.json")).equals("operator settings.json é😀"))throw new AssertionError("task or operator settings overwritten");
      var operator=Files.createDirectory(root.resolve("operator"));var cache=Files.createDirectories(operator.resolve(".cache/copilot"));Files.writeString(cache.resolve("package"),"cached package");
      var taskHome=Files.createDirectory(root.resolve("task-home"));
      request="{\"agent\":\"copilot\",\"operation\":\"prepare\",\"real_files\":true,\"home\":"+q(taskHome)+",\"operator_home\":"+q(operator)+"}";
      expected=source.call(request);remove(taskHome.resolve(".cache"));actual=nativeCode.call(request);
      if(!actual.equals(expected)||!Files.isSymbolicLink(taskHome.resolve(".cache/copilot"))||!Files.isSameFile(cache,taskHome.resolve(".cache/copilot"))||!Files.readString(cache.resolve("package")).equals("cached package"))throw new AssertionError("cache link differs: "+actual+"\n"+expected);
      WebContractProbe.cases++;
      passed=true;
    } finally {if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Web-Bench agent support matches the source in "+WebContractProbe.cases+" comparisons; homes, cache isolation, configuration, accounting, raw values and failure order passed");
  }
}
