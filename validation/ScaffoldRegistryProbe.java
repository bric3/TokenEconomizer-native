//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Checks registry lookup, constructor errors and handled exception scopes. */
class ScaffoldRegistryProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"scaffold-registry-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"scaffold_registry_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"scaffold_registry_host.py")) {
      for(var operation:List.of("get","for_arm")) {
        var base="\"operation\":"+q(operation);
        for(var option:List.of("\"name\":\"fixture\"","\"name\":\"missing\"","\"name\":null","\"name\":false","\"name\":0","\"name\":1","\"name\":[]","\"name\":[1]","\"name\":{}","\"name\":{\"key\":1}","\"opaque_name\":true","\"opaque_name\":true,\"truthy\":false","\"noncallable\":true","\"replace_registry\":true","\"arm\":null","\"arm\":\"missing\"","\"arm\":\"none\"","\"arm\":[1]","\"opaque_arm\":true","\"opaque_arm\":true,\"truthy\":false","\"stub_get\":true","\"keys\":[]","\"name\":\"missing\",\"keys\":[\"é\",\"😀\",\"Alpha\"]"))WebContractProbe.compare(nativeCode,source,base+","+option);
        for(var phase:List.of("bool:name","hash:name","repr:name","registry.item","registry.iter","constructor","error.new","arms.get","bool:arm","hash:arm","get"))for(var error:List.of("KeyError","RuntimeError","KeyboardInterrupt","DetailError")) {
          var options=switch(phase){case "bool:name","hash:name","repr:name"->",\"opaque_name\":true,\"keys\":[]";case "registry.iter","error.new"->",\"name\":\"missing\"";case "bool:arm","hash:arm"->",\"opaque_arm\":true";case "get"->",\"stub_get\":true";default->"";};
          WebContractProbe.compare(nativeCode,source,base+options+",\"fail\":"+q(phase)+",\"error\":"+q(error));
        }
      }
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Scaffold registry matches the source in "+WebContractProbe.cases+" comparisons; mappings, constructors, raw results and exception scopes passed");
  }
}
