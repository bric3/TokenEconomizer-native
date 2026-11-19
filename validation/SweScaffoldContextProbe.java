//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import javax.tools.ToolProvider;

/** Checks retained Docker callback arguments and binary I/O without a Docker daemon. */
class SweScaffoldContextProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"swe-scaffold-context-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"swe_scaffold_context_host.py",List.of(args).contains("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"swe_scaffold_context_host.py")) {
      for(var operation:List.of("context","exec","exec_default","copy","read","read_default","host","log")) {
        var base="\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"objects\":[\"name\"]","\"objects\":[\"script\",\"timeout\"]","\"objects\":[\"host_path\",\"container_path\"]","\"objects\":[\"rc\"],\"opaque_comparison\":true","\"rc\":1","\"rc\":1,\"stderr\":null","\"rc\":1,\"stderr\":false","\"rc\":1,\"stderr\":\"unicode é😀\"","\"rc\":1,\"objects\":[\"stderr\"]","\"objects\":[\"stdout\"]","\"overrides\":true","\"overrides\":true,\"env_truthy\":false","\"opaque_values\":true","\"overrides\":true,\"pairs\":[[\"too short\"]]","\"overrides\":true,\"pairs\":[[\"a\",\"b\",\"c\"]]","\"env\":{\"FOO\":\"first\",\"BAR\":42}","\"env\":42","\"env\":[]","\"second_context\":true","\"second_context\":true,\"rebind_run\":true","\"rebind_run\":true"))WebContractProbe.compare(nativeCode,source,base+","+option);
        for(var phase:List.of("scaffolds.ScaffoldContext","profile","context","env.bool","env.items","format:value","format:name","format:container_path","host_run","subprocess.run","subprocess.PIPE","run","returncode","bool:comparison","scaffolds.ScaffoldError","stderr","slice:stderr","repr:sliced","stdout","log"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","DetailError")) {
          var options=switch(phase){case "env.bool","env.items"->",\"overrides\":true";case "format:value"->",\"opaque_values\":true";case "format:name"->",\"objects\":[\"name\"]";case "format:container_path"->",\"objects\":[\"container_path\"]";case "bool:comparison"->",\"objects\":[\"rc\"],\"opaque_comparison\":true";case "scaffolds.ScaffoldError","stderr"->",\"rc\":1";case "slice:stderr","repr:sliced"->",\"rc\":1,\"objects\":[\"stderr\"]";default->"";};
          WebContractProbe.compare(nativeCode,source,base+options+",\"fail\":"+q(phase)+",\"error\":"+q(error));
        }
      }
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",WebContractProbe.artifacts.toString(),"validation/DockerScaffoldFixture.java")!=0)throw new AssertionError("fixture compilation failed");
      var docker=WebContractProbe.artifacts.resolve("docker");
      Files.writeString(docker,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+WebContractProbe.artifacts+"' DockerScaffoldFixture \"$@\"\n");
      if(!docker.toFile().setExecutable(true))throw new AssertionError("fixture is not executable");
      for(var option:List.of("\"operation\":\"read\",\"script\":\"fixture-binary\"","\"operation\":\"read\",\"script\":\"fixture-error\"","\"operation\":\"exec\",\"script\":\"fixture-env\",\"env\":{\"FOO\":\"fixture value\",\"BAR\":42}")) {
        var result=WebContractProbe.compare(nativeCode,source,"\"real_process\":true,\"bin_dir\":"+q(WebContractProbe.artifacts)+","+option);
        if(option.contains("fixture-binary")&&!result.contains("\"bytes\":[0,255,128,10]"))throw new AssertionError("raw stdout changed: "+result);
        if(option.contains("fixture-error")&&!result.contains("\"raised\":\"ScaffoldError\""))throw new AssertionError("binary process failure missing: "+result);
        if(option.contains("fixture-env")&&(!result.contains("environment passed")||!result.contains("\"rc\":0")))throw new AssertionError("process environment failed: "+result);
      }
      var pipeline=WebContractProbe.artifacts.resolve("pipeline");
      var result=WebContractProbe.compare(nativeCode,source,"\"operation\":\"pipeline\",\"real_process\":true,\"bin_dir\":"+q(WebContractProbe.artifacts)+",\"pipeline_dir\":"+q(pipeline));
      if(!result.contains("\"map_ok\":true")||Files.exists(pipeline.resolve("_repomap_src"))||!Files.readString(pipeline.resolve("container/testbed/.github/instructions.md")).contains("keep.py: Widget"))throw new AssertionError("repo-map scaffold pipeline failed: "+result);
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("SWE-bench scaffold context matches the source in "+WebContractProbe.cases+" comparisons; retained callbacks, signatures, raw arguments, environment order and binary process I/O passed without Docker");
  }
}
