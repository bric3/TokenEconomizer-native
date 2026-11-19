//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.nio.file.Files;
import java.util.Comparator;
import java.util.List;

/** Compares built-in host discovery, version reads, grader imports and agent profiles. */
class SweHostProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields)throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args)throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"swe_host_host.py",List.of(args).contains("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"swe_host_host.py")) {
      compare(nativeCode,source,"\"method\":\"profiles\"");
      for(var value:List.of("null","false","0","1","\"\"","\"relative path/é😀\"","[]","{}")) {
        compare(nativeCode,source,"\"binary\":"+value);
        compare(nativeCode,source,"\"which\":"+value);
        compare(nativeCode,source,"\"method\":\"cli_version\",\"stdout\":"+value);
      }
      for(var method:List.of("claude_bin","cli_version","_import_swebench")) {
        var base="\"method\":"+q(method);
        compare(nativeCode,source,base);
        compare(nativeCode,source,base+",\"present\":true");
        for(boolean truth:List.of(false,true))for(var object:List.of("binary","which","argument","stdout","repo"))
          compare(nativeCode,source,base+",\"objects\":["+q(object)+"],\"truthy\":"+truth);
        for(var phase:List.of("env.get","bool:binary","which","bool:which","realpath","run","stdout","strip","bool:stdout","repo","contains","insert","import"))for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","SystemExit","DetailError"))
          compare(nativeCode,source,base+",\"objects\":[\"binary\",\"stdout\"],\"fail\":"+q(phase)+",\"error\":"+q(error));
      }
      for(var error:List.of("ImportError","ModuleNotFoundError","KeyboardInterrupt","DetailError"))
        compare(nativeCode,source,"\"method\":\"_import_swebench\",\"fail\":\"import\",\"fail_at\":2,\"error\":"+q(error));
      compare(nativeCode,source,"\"real\":true,\"binary\":"+q(Path.of(".venv/bin/python").toAbsolutePath()));
      var version=compare(nativeCode,source,"\"method\":\"cli_version\",\"real\":true,\"argument\":"+q(Path.of(".venv/bin/python").toAbsolutePath()));
      if(!version.contains("Python "))throw new AssertionError("owned version process did not run: "+version);
      var repo=Files.createTempDirectory(Path.of("validation/results"),"grader-import-").toAbsolutePath();
      try {
        var specification=Files.createDirectories(repo.resolve("swebench/harness/test_spec"));
        for(var directory:List.of(repo.resolve("swebench"),repo.resolve("swebench/harness"),specification))Files.writeString(directory.resolve("__init__.py"),"");
        Files.writeString(specification.resolve("test_spec.py"),"def make_test_spec(): return 'owned spec'\n");
        Files.writeString(repo.resolve("swebench/harness/grading.py"),"def get_eval_report(): return 'owned report'\n");
        var imported=compare(nativeCode,source,"\"method\":\"_import_swebench\",\"real_import\":true,\"repo\":"+q(repo));
        if(!imported.contains("owned spec")||!imported.contains("owned report"))throw new AssertionError("official grader fixture imports failed: "+imported);
      }finally{try(var paths=Files.walk(repo)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    }
    System.out.println("SWE-bench host helpers pass "+cases+" source comparisons; discovery, opaque values, exception boundaries, grader import order, real version processes and three agent profiles passed");
  }
}
