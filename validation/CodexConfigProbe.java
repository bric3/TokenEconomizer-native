//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Compares Codex request contracts and scaffold orchestration without a container. */
class CodexConfigProbe {
  static int cases;
  static String q(Object value) { return ParserProbe.q(value); }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";
    var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"codex_config_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"codex_config_host.py")) {
      var capabilities=List.of("collaboration","goals","image_media","interactive_guidance","skills","web");
      for(int mask=0;mask<64;mask++) {
        var selected=new ArrayList<String>();
        for(int i=0;i<capabilities.size();i++)if((mask&(1<<i))!=0)selected.add(q(capabilities.get(i)));
        var caps="["+String.join(",",selected)+"]";
        for(var network:List.of("1","true","0"))for(var mode:List.of("config_toggles","config_toml"))
          compare(nativeCode,source,"\"mode\":"+q(mode)+",\"network\":"+q(network)+",\"kwargs\":{\"capabilities\":"+caps+"}");
      }
      for(var caps:List.of("null","[]","[\"unknown\",\"web\",\"web\",\"skills\"]","[\"apply_patch\"]","[\"planning\",\"exec_command\",\"planning\"]","\"web\""))
        compare(nativeCode,source,"\"kwargs\":{\"capabilities\":"+caps+"}");
      for(var network:List.of("", " 1 ","yes","1"))compare(nativeCode,source,"\"mode\":\"network_block_toggles\",\"network\":"+q(network));
      for(var skills:List.of("null","[]","[\"b\",\"a\",\"é😀\"]","\"ab\"","[0,false,null,{}]"))
        compare(nativeCode,source,"\"mode\":\"skills_config_entries\",\"kwargs\":{\"system_skills\":"+skills+"}");
      var prompt="preamble\n# Personality\nBe nice.\n\nWhen you run out of context, keep working.\n\n## Writing style\nBrief.\n\nEnd your turn after work.\n\n# Using skills\nDrop me.\n### How to use skills\nDrop too.\n# Code\nKeep é😀.\n";
      var prompts=new ArrayList<>(List.of("","plain",prompt,prompt.replace("\n","\r\n"),"# Personality\nonly\n","## Working with the user\nkeep\n"));
      try(var paths=Files.list(Path.of("reference/upstream/experiments/te_harness/data/codex_base_prompt"))) {
        for(var path:paths.filter(p->p.toString().endsWith(".md")).toList())prompts.add(Files.readString(path));
      }
      for(var text:prompts)for(var caps:List.of("[]","[\"skills\"]","[\"interactive_guidance\"]","[\"skills\",\"interactive_guidance\"]"))
        compare(nativeCode,source,"\"mode\":\"lean_instructions\",\"kwargs\":{\"base_text\":"+q(text)+",\"capabilities\":"+caps+"}");
      for(var timeout:List.of("90","null","true","-2.8","\"١_٢\"","\"bad\""))
        compare(nativeCode,source,"\"kwargs\":{\"instructions\":\"lean\",\"instructions_path\":\"/é/home\",\"capabilities\":[\"skills\",\"goals\"],\"mcp_servers\":{\"z\":{\"command\":\"run\",\"startup_timeout_sec\":"+timeout+",\"args\":[\"--x\",1,false],\"cwd\":\"/tmp\",\"env\":{\"Z\":null,\"A\":false,\"N\":3}},\"a\":{\"command\":\"other\"}}}");
      for(var caps:List.of("[]","[\"skills\",\"interactive_guidance\"]","[\"apply_patch\"]"))for(var stdout:List.of("wrote\n","already wrote config","failed"))
        compare(nativeCode,source,"\"mode\":\"write_config\",\"base_prompt\":"+q(prompt)+",\"capabilities\":"+caps+",\"stdout\":"+q(stdout)+",\"stderr\":"+q("é😀".repeat(150)));
      for(var mode:List.of("apply_scaffold","apply_codex_scaffold")) {
        compare(nativeCode,source,"\"mode\":"+q(mode));
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"no_scaffold\":true");
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"falsey_scaffold\":true");
        for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","SystemExit"))
          compare(nativeCode,source,"\"mode\":"+q(mode)+",\"error\":"+q(error)+",\"message\":"+q("é😀".repeat(180)));
        for(var notes:List.of("{}","null","[]","{\"mcp_servers\":{}}","{\"mcp_servers\":{\"z\":{\"command\":\"run\"},\"a\":{\"command\":\"a\"}},\"mcp_config\":\"/fixture/file\"}"))
          compare(nativeCode,source,"\"mode\":"+q(mode)+",\"notes\":"+notes);
      }
    }
    System.out.println("Codex configuration matches the source in "+cases+" cases; exact TOML, protected masks, prompt sections, writes and scaffold identity passed");
  }
}
