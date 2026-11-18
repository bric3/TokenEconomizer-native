//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java ScaffoldMergeHelperProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Drives shared profile/context comparisons without importing a benchmark harness. */
class ScaffoldBaseProbe{
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception{
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"scaffold-base-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"scaffold_base_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"scaffold_base_host.py")){
      for(var operation:List.of("profile_new","profile_mcp","context_new","read","log","instructions","mcp","mounts","preflight","setup")){
        var base="\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"objects\":[\"home\",\"instructions_file\"]","\"objects\":[\"nav_tools\",\"patch_excludes\"]","\"objects\":[\"env\"]","\"objects\":[\"cfg_path\"]","\"objects\":[\"text\"]","\"objects\":[\"servers\",\"filename\"]","\"objects\":[\"path\"]","\"objects\":[\"patch\"]","\"opaque_run_dir\":true","\"no_read\":true","\"no_log\":true","\"constructor_empty\":true","\"opaque_mcp\":true","\"opaque_rc\":true,\"opaque_comparison\":true","\"rc\":2","\"stdout\":null","\"opaque_stdout\":true","\"opaque_hash\":true","\"instructions_file\":\"CLAUDE.md\"","\"instructions_file\":\"\"","\"profile_env\":{\"EXTRA\":42}"))WebContractProbe.compare(nativeCode,source,base+","+option);
        var phases=switch(operation){
          case "profile_new"->List.of("profile.set:kind","profile.set:home","profile.set:instructions_file","profile.set:nav_tools","profile.set:_mcp_args","profile.set:patch_excludes","profile.set:bin_dir","profile.set:env");
          case "profile_mcp"->List.of("profile.get:_mcp_args","call:mcp","iter:mcp_result");
          case "context_new"->List.of("context.set:profile","context.set:container","context.set:task","context.set:run_dir","context.set:exec_","context.set:copy_in","context.set:host_run","context.set:_read_bytes","context.set:_log","bool:logger");
          case "read"->List.of("context.get:_read_bytes","call:read");
          case "log"->List.of("context.get:_log","call:logger");
          case "instructions"->List.of("context.get:profile","profile.get:instructions_file","os.path","path.dirname","dirname","context.get:exec_","shlex.quote","quote","call:exec","path.join","context.get:run_dir","join","open","enter","write","exit","context.get:copy_in","call:copy","os.unlink","unlink","returncode");
          case "mcp"->List.of("context.get:profile","profile.get:home","path.join","join","open","enter","json.dump","dump","write","exit","context.get:exec_","shlex.quote","quote","call:exec","context.get:copy_in","call:copy","unlink","returncode");
          default->List.<String>of();
        };
        for(var phase:phases)for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,base+",\"opaque_mcp\":true,\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      }
      for(var operation:List.of("instructions","mcp"))for(var phase:List.of("write","dump","exit"))for(boolean suppress:List.of(false,true))WebContractProbe.compare(nativeCode,source,"\"operation\":"+q(operation)+",\"opaque_exit\":true,\"suppress\":"+suppress+",\"fail\":"+q(phase)+",\"error\":\"DetailError\",\"exception_chain\":true");
      for(var phase:List.of("exit","exit.bool"))WebContractProbe.compare(nativeCode,source,"\"opaque_exit\":true,\"failures\":[{\"fail\":\"write\",\"error\":\"DetailError\"},{\"fail\":"+q(phase)+",\"error\":\"RuntimeError\"}],\"exception_chain\":true");
      var run=Files.createDirectory(WebContractProbe.artifacts.resolve("run"));
      for(var operation:List.of("instructions","mcp")){
        WebContractProbe.compare(nativeCode,source,"\"operation\":"+q(operation)+",\"real_files\":true,\"run_dir\":"+q(run));
        try(var files=Files.list(run)){if(files.findAny().isPresent())throw new AssertionError("temporary file remains after success");}
      }
      for(var operation:List.of("instructions","mcp"))for(var phase:List.of("call:copy","unlink")){
        WebContractProbe.compare(nativeCode,source,"\"operation\":"+q(operation)+",\"real_files\":true,\"run_dir\":"+q(run)+",\"fail\":"+q(phase)+",\"error\":\"OSError\"");
        try(var files=Files.list(run)){
          var remaining=files.toList();
          if(remaining.size()!=1)throw new AssertionError("source failure retention changed");
          Files.delete(remaining.getFirst());
        }
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Shared scaffold classes match the source in "+WebContractProbe.cases+" comparisons; constructor state, raw callbacks, files, MCP configuration and failure order passed");
    ScaffoldMergeHelperProbe.main(new String[]{"--context"});
  }
}
