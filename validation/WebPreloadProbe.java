//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Map;

/** Loads the selected preload in Node with an offline undici fixture. */
class WebPreloadProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static String uri(Path path){return path.toUri().toASCIIString().replace("'","%27");}
  static String request(Path bridge,int abort,String previous,Path repo){
    return "{\"operation\":\"web\",\"env\":{\"TE_WEBBENCH_UNDICI_ABORT\":"+q(abort)+",\"NODE_OPTIONS\":"+q(previous)+"},\"repo\":"+q(repo)+(bridge==null?"":",\"bridge\":"+q(bridge))+"}";
  }
  static String expected(Path preload,int abort,String previous,Path repo){
    if(abort<=300)return "{\"value\":{}}";
    return "{\"value\":{\"NODE_OPTIONS\":"+q(options(preload,previous))+",\"TE_WEBBENCH_REPO\":"+q(repo)+",\"TE_WEBBENCH_UNDICI_ABORT_MS\":"+q(abort*1000)+"}}";
  }
  static String options(Path preload,String previous){return (previous+" --import "+uri(preload)).strip();}
  static void node(Path root,Path preload,Path repo)throws Exception {
    var output=NativeCliProbe.process(root,Map.of("PATH","","NODE_OPTIONS",options(preload,"--trace-warnings"),"TE_WEBBENCH_UNDICI_ABORT_MS","900000","TE_WEBBENCH_REPO",repo.toString()),List.of("/opt/homebrew/bin/node","-e","if(JSON.stringify(globalThis.__fixtureAgent)!==JSON.stringify({headersTimeout:900000,bodyTimeout:900000}))throw Error('dispatcher was not raised')"));
    require(output.code()==0&&output.err().contains("headersTimeout=900000ms"),"Node did not load the selected preload: "+output);
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"web-preload-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"experiment_assets_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"experiment_assets_host.py")) {
      var repo=Files.createDirectories(root.resolve("repo's space é😀#%"));
      var undici=Files.createDirectories(repo.resolve("common/temp/node_modules/.pnpm/undici@1/node_modules/undici"));
      Files.writeString(undici.resolve("index.js"),"module.exports={Agent:class{constructor(options){this.options=options}},setGlobalDispatcher(agent){globalThis.__fixtureAgent=agent.options}};\n");
      var preload=Path.of("src/experiments/assets/webbench_undici_preload.mjs").toAbsolutePath();
      for(int abort:List.of(300,301,900))for(var previous:List.of(""," --trace-warnings ")) {
        var request=request(null,abort,previous,repo);var actual=nativeCode.call(request);
        require(actual.equals(expected(preload,abort,previous,repo)),"development preload selection differs: "+actual);
        require(actual.equals(source.call(request).replace(uri(Path.of("reference/upstream/experiments/te_harness/webbench_undici_preload.mjs").toAbsolutePath()),uri(preload))),"preload environment differs from source");
      }
      node(root,preload,repo);
      var runtime=Files.createDirectories(root.resolve("runtime's space é😀#%/bin"));
      Files.copy(Path.of("target/debug/te-bridge"),runtime.resolve("te-bridge"),StandardCopyOption.COPY_ATTRIBUTES);
      Files.writeString(runtime.getParent().resolve("manifest.json"),"{\"name\":\"tokeneconomizer-experiments\"}");
      var bundled=Files.createDirectories(runtime.getParent().resolve("assets")).resolve("webbench_undici_preload.mjs");Files.copy(preload,bundled);
      var bridge=runtime.resolve("te-bridge");
      for(int abort:List.of(300,301,900)) {
        var actual=nativeCode.call(request(bridge,abort,"--trace-warnings",repo));
        require(actual.equals(expected(bundled,abort,"--trace-warnings",repo)),"relocated preload selection differs: "+actual);
      }
      require(Files.mismatch(preload,bundled)==-1,"relocation changed preload bytes");node(root,bundled,repo);
      Files.delete(bundled);
      require(nativeCode.call(request(bridge,900,"",repo)).equals(expected(bundled,900,"",repo)),"missing bundled preload selected another checkout");
      var missing=NativeCliProbe.process(root,Map.of("PATH","","NODE_OPTIONS",options(bundled,"")),List.of("/opt/homebrew/bin/node","-e","throw Error('body must not run')"));
      require(missing.code()!=0&&missing.err().contains("ERR_MODULE_NOT_FOUND"),"Node accepted a missing preload: "+missing);
      Files.writeString(runtime.getParent().resolve("manifest.json"),"invalid JSON");
      require(nativeCode.call(request(bridge,300,"",repo)).equals("{\"value\":{}}"),"stock ceiling accessed runtime assets");
      require(nativeCode.call(request(bridge,900,"",repo)).contains("\"raised\":\"OSError\""),"invalid runtime manifest was ignored");
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("WebBench preload passed "+checks+" checks; native development/bundle paths, URI escaping, original environment parity, Node imports, dispatcher settings and missing assets passed offline");
  }
}
