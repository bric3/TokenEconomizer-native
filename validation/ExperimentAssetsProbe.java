//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;

/** Checks native prompt selection and actual native repo-map invocation without model calls. */
class ExperimentAssetsProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static String prompt(ContextProbe.Client client,Path assets,String version,String error)throws Exception {
    var result=client.call("experiment.swebench.install.base_prompt_for","\"cli_version\":"+q(version));
    var match=java.util.regex.Pattern.compile("[\\p{Nd}]+\\.[\\p{Nd}]+\\.[\\p{Nd}]+").matcher(version);
    Path path=match.find()?assets.resolve("codex_base_prompt/"+match.group()+".md"):null;
    while(result.contains("\"callback\"")) {
      require(path!=null&&result.contains(q(path)),"prompt looked outside its runtime assets: "+result);
      if(result.contains("\"method\":\"install_path\""))result=client.resume(result,"install_path","{\"value\":"+Files.exists(path)+"}");
      else if(error!=null)result=client.resume(result,"install_read","{\"error\":{\"kind\":"+q(error)+",\"message\":\"fixture read failure\"}}");
      else result=client.resume(result,"install_read","{\"value\":"+q(Files.readString(path))+"}");
    }
    return result;
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"experiment-assets-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"experiment_assets_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"experiment_assets_host.py")) {
      for(var version:List.of("","0.153.1","codex-cli 0.153.1 extra","9.9.9","9.9.9 and 0.153.1","١.٢.٣")) {
        var request="{\"operation\":\"prompt\",\"version\":"+q(version)+"}";
        require(nativeCode.call(request).equals(source.call(request)),"default prompt differs: "+version);
      }
      var runtime=Files.createDirectories(root.resolve("runtime/bin"));var assets=Files.createDirectories(root.resolve("runtime/assets/codex_base_prompt")).getParent();
      Files.copy(Path.of("target/debug/te-bridge"),runtime.resolve("te-bridge"),StandardCopyOption.COPY_ATTRIBUTES);
      Files.copy(Path.of("target/debug/te-repomap-gen"),runtime.resolve("te-repomap-gen"),StandardCopyOption.COPY_ATTRIBUTES);
      Files.writeString(runtime.getParent().resolve("manifest.json"),"{\"name\":\"tokeneconomizer-experiments\"}");
      var capture=assets.resolve("codex_base_prompt/0.153.1.md");Files.copy(Path.of("src/experiments/assets/codex_base_prompt/0.153.1.md"),capture);
      try(var client=new ContextProbe.Client(runtime.resolve("te-bridge"),"experiment.swebench")) {
        require(prompt(client,assets,"codex-cli 0.153.1",null).equals("{\"result\":"+q(Files.readString(capture))+"}"),"relocated capture differs");
        Files.delete(capture);require(prompt(client,assets,"0.153.1",null).equals("{\"result\":null}"),"missing bundled capture used another checkout");
        Files.writeString(assets.resolve("codex_base_prompt/1.2.3.md"),"custom é😀\n");
        require(prompt(client,assets,"codex-cli 1.2.3",null).equals("{\"result\":"+q("custom é😀\n")+"}"),"additional capture was ignored");
        Files.writeString(assets.resolve("codex_base_prompt/١.٢.٣.md"),"Arabic version\n");
        require(prompt(client,assets,"١.٢.٣",null).equals("{\"result\":"+q("Arabic version\n")+"}"),"Unicode version selection differs");
        for(var error:List.of("OSError","UnicodeError","KeyboardInterrupt"))require(prompt(client,assets,"1.2.3",error).contains("\"raised\":"+q(error)),"capture read error was swallowed");
      }
      var sdk=Files.createDirectories(root.resolve("sdk/aider"));Files.writeString(sdk.resolve("__init__.py"),"");
      Files.writeString(sdk.resolve("io.py"),"class InputOutput:\n    def __init__(self): print('IO WARNING')\n");
      Files.writeString(sdk.resolve("models.py"),"class Model:\n    def __init__(self,name): assert name == 'gpt-4o'\n");
      Files.writeString(sdk.resolve("repomap.py"),"""
          import json, os
          class RepoMap:
              TAGS_CACHE_DIR = 'DEFAULT'
              def __init__(self, **config): self.config=config; print('SCAN WARNING')
              def get_repo_map(self,chat,other,mentioned_idents):
                  if os.environ.get('ASSET_SDK_FAIL'): raise RuntimeError('SDK fixture failed')
                  return json.dumps(dict(files=sorted(os.path.relpath(p,self.config['root']) for p in other),
                      budget=self.config['map_tokens'], idents=sorted(mentioned_idents)),ensure_ascii=False)+'\\n'
          """);
      var repo=Files.createDirectory(root.resolve("repo 'é😀"));Files.writeString(repo.resolve("keep.py"),"class Widget: pass\n");
      Files.createDirectory(repo.resolve("nested"));Files.writeString(repo.resolve("nested/a b.py"),"class Other: pass\n");
      for(var budget:List.of("2048","512","١_٢","bad","0","-1"))for(var idents:List.of("[]","[\"Widget\",\"Other\"]","[\"Widget\",\"Widget\",\"é😀\"]")) {
        var request="{\"directory\":"+q(repo)+",\"sdk\":"+q(sdk.getParent())+",\"budget\":"+q(budget)+",\"idents\":"+idents+"}";
        var actual=nativeCode.call(request);var expected=source.call(request);
        require(actual.equals(expected),"native repo-map invocation differs: "+actual+"\n"+expected);
        if(budget.equals("bad"))require(actual.contains("\"raised\":\"ValueError\""),"invalid budget was accepted");
      }
      var relocated="{\"directory\":"+q(repo)+",\"sdk\":"+q(sdk.getParent())+",\"bridge\":"+q(runtime.resolve("te-bridge"))+"}";
      require(nativeCode.call(relocated).equals(source.call(relocated)),"relocated engine did not invoke its native generator");
      var promptRequest="{\"operation\":\"prompt\",\"version\":\"1.2.3\",\"bridge\":"+q(runtime.resolve("te-bridge"))+"}";
      require(nativeCode.call(promptRequest).equals("{\"value\":"+q("custom é😀\n")+"}"),"SDK binding read a capture outside its bundle");
      var badCapture=assets.resolve("codex_base_prompt/1.2.3.md");Files.write(badCapture,new byte[]{(byte)255});
      require(nativeCode.call(promptRequest).contains("\"raised\":\"UnicodeDecodeError\""),"invalid capture encoding was accepted");
      Files.delete(badCapture);Files.createDirectory(badCapture);
      require(nativeCode.call(promptRequest).contains("\"raised\":\"IsADirectoryError\""),"capture file error was swallowed");
      var failure="{\"directory\":"+q(repo)+",\"sdk\":"+q(sdk.getParent())+",\"fail\":true}";
      for(var client:List.of(source,nativeCode)) {
        var value=client.call(failure);
        require(value.startsWith("{\"value\":[\"\",\"")&&value.contains("RuntimeError: SDK fixture failed"),"SDK failure was lost: "+value);
      }
      Files.delete(runtime.resolve("te-repomap-gen"));
      var missing=nativeCode.call(relocated);
      require(missing.contains("\"raised\":\"FileNotFoundError\"")&&missing.contains(runtime.resolve("te-repomap-gen").toString()),"missing native generator used another checkout: "+missing);
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Experiment asset selection passed "+checks+" checks; embedded/bundled prompts, relocation, additional captures, Unicode versions, read errors and actual native repo-map execution passed with an isolated aider SDK");
  }
}
