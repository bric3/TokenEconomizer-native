//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares agent flags, resume parameters, environment additions and refusal detection. */
class InvocationProbe{
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception{
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"invocation-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"invocation_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"invocation_host.py")){
      for(var agent:List.of("copilot","codex"))for(var method:agent.equals("copilot")?List.of("build_argv","fatal_config_error"):List.of("build_argv","build_resume_argv","run_env")){
        var base="\"agent\":"+q(agent)+",\"method\":"+q(method);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"objects\":[\"prompt\",\"log_dir\"]","\"objects\":[\"model\",\"effort\"]","\"objects\":[\"session_id\"]","\"objects\":[\"exclude_tools\"]","\"objects\":[\"mcp_config\"]","\"objects\":[\"isolation\"]","\"objects\":[\"protected\"]","\"objects\":[\"hook\"]","\"objects\":[\"extra\"]","\"objects\":[\"C_BIN\",\"C_HOME\"]","\"objects\":[\"stderr_text\"]","\"objects\":[\"stderr_text\"],\"opaque_lines\":true,\"opaque_stripped\":true","\"objects\":[\"stderr_text\"],\"opaque_lines\":true,\"opaque_low\":true,\"contains\":true","\"objects\":[\"patterns\"]","\"opaque_dumps\":true","\"custom_str\":true","\"actual_effort\":true","\"effort_allowed\":false","\"opaque_allowed\":true","\"aggregate\":true,\"observed_patterns\":true","\"aggregate\":true,\"skip_generator\":true","\"isolation\":null","\"patterns\":[]","\"binary\":null","\"home\":false"))WebContractProbe.compare(nativeCode,source,base+","+option);
        for(var phase:List.of("bool:exclude_tools","iter:exclude_tools","exclude_tools.next","iter:isolation","len:isolation","contains:protected","bool:model","str:model","bool:effort","str:effort","effort_applies","bool:allowed","bool:hook","str:session_id","radd:mcp_config","dumps","format:encoded","bool:extra","keys:extra","item:extra","bool:stderr_text","splitlines:stderr_text","lines.iter","lines.next","lower:line","strip:line","item:stripped","patterns.iter","patterns.next","any","contains:low"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError","StopIteration")){
          var objects=switch(phase){case "iter:isolation","len:isolation"->"[\"isolation\"]";case "contains:protected"->"[\"protected\"]";case "radd:mcp_config"->"[\"mcp_config\"]";case "bool:extra","keys:extra","item:extra"->"[\"extra\"]";case "bool:hook"->"[\"hook\"]";case "str:session_id"->"[\"session_id\"]";case "splitlines:stderr_text","lines.iter","lines.next","lower:line","strip:line","item:stripped","bool:stderr_text","contains:low"->"[\"stderr_text\"]";default->"[\"model\",\"effort\",\"exclude_tools\"]";};
          WebContractProbe.compare(nativeCode,source,base+",\"objects\":"+objects+",\"opaque_lines\":true,\"opaque_stripped\":true,\"opaque_low\":"+phase.equals("contains:low")+",\"contains\":true,\"opaque_allowed\":true,\"opaque_dumps\":true,\"aggregate\":true,\"observed_patterns\":true,\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
        }
      }
      for(var value:List.of("null","false","0","\"\"","[]","{}","[1]","\"é😀\"")){
        WebContractProbe.compare(nativeCode,source,"\"arguments\":{\"prompt\":"+value+",\"log_dir\":"+value+",\"exclude_tools\":"+value+",\"model\":"+value+",\"effort\":"+value+",\"mcp_config\":"+value+"}");
        WebContractProbe.compare(nativeCode,source,"\"method\":\"fatal_config_error\",\"arguments\":{\"stderr_text\":"+value+"}");
        WebContractProbe.compare(nativeCode,source,"\"agent\":\"codex\",\"method\":\"build_resume_argv\",\"arguments\":{\"session_id\":"+value+",\"model\":"+value+",\"effort\":"+value+",\"hook\":"+value+"}");
        WebContractProbe.compare(nativeCode,source,"\"agent\":\"codex\",\"method\":\"run_env\",\"arguments\":{\"extra\":"+value+"}");
      }
      for(var text:List.of("","noise","is not available"," DOES NOT SUPPORT REASONING EFFORT ","first\u0085 second is not available\u2028third","is not available"+"😀".repeat(300),"x".repeat(400)+" is not available","is not available\nsecond does not support reasoning effort"))WebContractProbe.compare(nativeCode,source,"\"method\":\"fatal_config_error\",\"arguments\":{\"stderr_text\":"+q(text)+"}");
      for(var model:List.of("auto","AUTO","gpt-5.4",""))for(var gate:List.of("0","1",""))WebContractProbe.compare(nativeCode,source,"\"actual_effort\":true,\"arguments\":{\"prompt\":\"p\",\"log_dir\":\"logs\",\"model\":"+q(model)+",\"effort\":\"high\"},\"env\":{\"TE_EFFORT_APPLIES\":"+q(gate)+"}");
      WebContractProbe.compare(nativeCode,source,"\"agent\":\"codex\",\"method\":\"build_resume_argv\",\"points\":{\"session_id\":[55296,56320],\"effort\":[55296,56320]},\"observe_points\":true");
      WebContractProbe.compare(nativeCode,source,"\"isolation\":[\"first\",\"first\"],\"arguments\":{\"prompt\":\"p\",\"log_dir\":\"logs\",\"exclude_tools\":[\"bash\",\"Read\",\"Read\",null]}");
      for(boolean after:List.of(false,true))WebContractProbe.compare(nativeCode,source,"\"method\":\"fatal_config_error\",\"arguments\":{\"stderr_text\":\"old line\\nmodel is not available\"},\"patterns\":[\"is not available\"],\"retain_generators\":true,\"consume_after\":"+after);
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Agent invocation helpers match the source in "+WebContractProbe.cases+" comparisons; flags, raw values, resumes, environments, refusal patterns and failure order passed");
  }
}
