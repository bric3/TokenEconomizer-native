//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares RTK policy, commands and failure boundaries without downloads. */
class RtkScaffoldProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"rtk-scaffold-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"scaffold_rtk_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"scaffold_rtk_host.py")) {
      for(var operation:List.of("cache","bin_path","preflight","_hook_binary","_claude_hook","_copilot_hook_config","stage_host_binary","_codex_instructions","_install_copilot_hook","_probe_hook_rewrites","setup")) {
        var base="\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"kind\":\"copilot\"","\"kind\":\"codex\"","\"kind\":null","\"exists\":false","\"access\":false","\"exists\":false,\"rc\":1","\"exists\":false,\"objects\":[\"access\"]","\"rc\":1","\"registered\":false","\"registered\":false,\"rc\":1","\"stdout\":null","\"stdout\":42","\"stdout\":\"\"","\"stdout\":\"no rewrite\"","\"stdout\":\"RTK_HOOK_AT=/first\\nRTK_HOOK_AT=/second\\n\"","\"stdout\":\""+"é😀".repeat(150)+"\"","\"stderr\":null","\"stderr\":42","\"profile_env\":{}","\"profile_env\":{\"CODEX_HOME\":\"\"}","\"profile_env\":{\"CODEX_HOME\":\"/codex\",\"HOME\":\"/preset\"}","\"env\":{}","\"env\":{\"DATA\":\"/data\"}","\"env\":{\"TE_SCAFFOLD_CACHE\":\"\",\"DATA\":\"/data\"}","\"env_objects\":[\"TE_SCAFFOLD_CACHE\"]","\"env\":{},\"env_objects\":[\"DATA\"]","\"class_hook\":true","\"objects\":[\"bin_dir\",\"home\",\"agent\"]","\"objects\":[\"kind\"],\"opaque_comparison\":true","\"objects\":[\"registered\"]","\"exists\":false,\"objects\":[\"host_rc\"],\"opaque_comparison\":true","\"objects\":[\"exec1_rc\"],\"opaque_comparison\":true","\"objects\":[\"exec1_stdout\"]","\"objects\":[\"exec1_stdout\"],\"opaque_stripped\":true","\"objects\":[\"exec1_stdout\"],\"opaque_lines\":true","\"exists\":false,\"objects\":[\"RTK_URL\"]","\"exists\":false,\"objects\":[\"RTK_INSTALL_SH\",\"RTK_VERSION\"]","\"stubs\":[\"stage_host_binary\"],\"stub_stage_host_binary\":false","\"stubs\":[\"_install_copilot_hook\"],\"stub__install_copilot_hook\":{\"rtk_hook_registered\":true,\"rtk_installed\":false},\"kind\":\"copilot\""))WebContractProbe.compare(nativeCode,source,base+","+option);
        var phases=switch(operation) {
          case "cache"->List.of("env.get","os.path","path.join","join","fspath:DATA");
          case "bin_path","preflight"->List.of("os.path","path.join","env.get","join");
          case "_hook_binary","_claude_hook","_copilot_hook_config"->List.of("profile.get:bin_dir","format:bin_dir","class.hook","format:hook_binary");
          case "stage_host_binary"->List.of("scaffold.get:bin_path","env.get","exists","access","makedirs","dirname","context.get:host_run","host","host.returncode","host.stderr","context.get:log","log");
          case "_codex_instructions"->List.of("context.get:profile","profile.get:env","profile.get:home","context.get:exec_","exec","exec1.returncode","exec2.stdout","exec2.stderr","scaffold.get:RTK_RULE","scaffold.get:_hook_binary","profile.get:bin_dir","exec3.returncode","context.get:log","log");
          case "_install_copilot_hook"->List.of("json.dumps","scaffold.get:_copilot_hook_config","context.get:profile","profile.get:bin_dir","dumps","scaffold.get:COPILOT_HOOK_FILE","context.get:exec_","profile.get:home","exec","exec1.stdout","exec1.stderr");
          case "_probe_hook_rewrites"->List.of("context.get:exec_","shlex.quote","scaffold.get:_PROBE_PAYLOAD","quote","scaffold.get:_hook_binary","context.get:profile","profile.get:bin_dir","exec","exec1.stdout");
          default->List.of("scaffold.get:stage_host_binary","env.get","context.get:copy_in","copy","exec","exec1.stdout","profile.get:kind","scaffold.get:_codex_instructions","scaffold.get:_hook_binary","context.get:merge_json_file","merge","scaffold.get:_probe_hook_rewrites","context.get:log","log");
        };
        for(var phase:phases)for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError")) {
          var options=switch(phase){
            case "format:bin_dir"->",\"objects\":[\"bin_dir\"]";
            case "class.hook","format:hook_binary"->",\"class_hook\":true";
            case "makedirs","dirname","context.get:host_run","host","host.returncode","host.stderr"->",\"exists\":false,\"host_rc\":1";
            case "context.get:log","log","exec2.stderr"->",\"rc\":1,\"exists\":false,\"registered\":true";
            case "fspath:DATA"->",\"env\":{},\"env_objects\":[\"DATA\"]";
            case "scaffold.get:_codex_instructions"->",\"kind\":\"codex\"";
            default->"";
          };
          WebContractProbe.compare(nativeCode,source,base+options+",\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
        }
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("RTK scaffold matches the source in "+WebContractProbe.cases+" comparisons; cache, staging, hook channels, rewriting probes, Codex instructions and failure order passed without downloads");
  }
}
