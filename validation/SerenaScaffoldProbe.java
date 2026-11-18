//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares Serena policy using object protocols and callback failures, without downloads. */
class SerenaScaffoldProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"serena-scaffold-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"scaffold_serena_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"scaffold_serena_host.py")) {
      for(var operation:List.of("setup","mcp_server","nudge")) {
        var base="\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"kind\":\"copilot\"","\"kind\":\"codex\"","\"kind\":null","\"rc\":1","\"rc\":1,\"objects\":[\"stderr\"]","\"stdout\":null","\"stdout\":false","\"stdout\":42","\"stdout\":\""+"é😀".repeat(70)+"\"","\"rc\":1,\"stderr\":null","\"rc\":1,\"stderr\":42","\"rc\":1,\"stderr\":\""+"é😀".repeat(120)+"\"","\"nav_tools\":[]","\"nav_tools\":null","\"nav_tools\":[1]","\"nav_tools\":\"shell\"","\"tools\":[]","\"tools\":[1]","\"tools\":null","\"opaque_tools\":true","\"opaque_tool\":true","\"opaque_tool\":true,\"opaque_added\":true","\"opaque_install\":true","\"stub_mcp_server\":true","\"stub_mcp_server\":true,\"stub_nudge\":true,\"kind\":\"codex\"","\"cfg\":null","\"objects\":[\"cfg\",\"nudged\"]","\"objects\":[\"home\"]","\"objects\":[\"kind\"]","\"objects\":[\"nav_tools\"]","\"objects\":[\"stdout\"]","\"objects\":[\"stdout\"],\"opaque_stripped\":true","\"objects\":[\"rc\"],\"opaque_comparison\":true","\"objects\":[\"rc\"],\"opaque_comparison\":true,\"booleans\":{\"comparison\":false}","\"objects\":[\"kind\"],\"opaque_comparison\":true","\"tool_points\":[55296,56320]"))WebContractProbe.compare(nativeCode,source,base+","+option);
        for(var phase:List.of("context.get:profile","profile.get:home","profile.get:kind","profile.get:nav_tools","context.get:exec_","exec","returncode","stdout","context.get:log","log","stderr","scaffold.get:mcp_server","scaffold.get:nudge","context.get:write_mcp_config","write_mcp","context.get:write_instructions","write_instructions","format:home","eq:kind","bool:comparison","strip:stdout","slice:stripped","next:tools","iter:nav_tools","radd:tool","add:added"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError")) {
          var options=switch(phase){
            case "context.get:log","log","stderr"->",\"rc\":1";
            case "format:home"->",\"objects\":[\"home\"]";
            case "eq:kind","bool:comparison"->",\"objects\":[\"kind\"],\"opaque_comparison\":true";
            case "strip:stdout","slice:stripped"->",\"objects\":[\"stdout\"],\"opaque_stripped\":true";
            case "next:tools"->",\"opaque_tools\":true";
            case "iter:nav_tools"->",\"objects\":[\"nav_tools\"]";
            case "radd:tool","add:added"->",\"opaque_tool\":true,\"opaque_added\":true";
            default->"";
          };
          WebContractProbe.compare(nativeCode,source,base+options+",\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
        }
      }
      for(var agent:List.of("claude","copilot","codex")) {
        var run=Files.createDirectories(WebContractProbe.artifacts.resolve(agent));Files.createDirectory(run.resolve("home"));
        WebContractProbe.compare(nativeCode,source,"\"real_files\":true,\"kind\":"+q(agent)+",\"run_dir\":"+q(run));
        var config=Files.readString(run.resolve("home/scaffold.mcp.json"));
        var instructions=run.resolve("workspace").resolve(switch(agent){case "codex"->"AGENTS.md";case "copilot"->".github/copilot-instructions.md";default->"CLAUDE.md";});
        if(!config.contains("start-mcp-server")||!Files.readString(instructions).contains("find_symbol"))throw new AssertionError("Serena files missing");
        try(var files=Files.list(run)){if(files.anyMatch(path->path.getFileName().toString().startsWith("_scaffold")))throw new AssertionError("temporary file retained");}
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Serena scaffold matches the source in "+WebContractProbe.cases+" comparisons; install results, raw server references, Codex handoff, instructions and failure order passed without downloads");
  }
}
