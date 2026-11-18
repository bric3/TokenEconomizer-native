//JAVA 25
//SOURCES ContainerProbe.java AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.ArrayList;

/** Compares agent task wrappers and transcript measurements with local stand-ins. */
class RunnerWrapperProbe {
  static int cases;
  static Path artifacts;
  static String q(Object value) { return ParserProbe.q(value); }
  static void setup(Path root,byte[] trace,int layout) throws Exception {
    ContainerProbe.clear(root);
    if(layout!=3)Files.createDirectories(root);
    if(layout==1)Files.createDirectories(root.resolve("transcript.jsonl"));
    else if(trace!=null)Files.write(root.resolve("transcript.jsonl"),trace);
    if(layout==2)Files.createDirectory(root.resolve("copilot_debug.log"));
  }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields,byte[] trace,int layout) throws Exception {
    var root=artifacts.resolve("case-"+cases);
    var request="{\"root\":"+q(root)+(fields.isEmpty()?"":","+fields)+"}";
    setup(root,trace,layout);var expected=source.call(request);
    setup(root,trace,layout);var actual=nativeCode.call(request);
    if(!actual.equals(expected)) {
      Files.writeString(artifacts.resolve("request.json"),request);Files.writeString(artifacts.resolve("expected.json"),expected);Files.writeString(artifacts.resolve("actual.json"),actual);
      throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    }
    cases++;
  }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception { compare(nativeCode,source,fields,null,0); }
  static byte[] bytes(String text) { return text.getBytes(StandardCharsets.UTF_8); }
  static String event(String fields) { return "{\"type\":\"item.completed\",\"item\":{"+fields+"}}\n"; }
  public static void main(String[] args) throws Exception {
    artifacts=Files.createTempDirectory("te-runner-wrapper-");boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"runner_wrapper_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"runner_wrapper_host.py")) {
      for(var agent:List.of("codex","copilot")) {
        compare(nativeCode,source,"\"agent\":"+q(agent));
        for(var error:List.of("RuntimeError","OSError","KeyboardInterrupt","SystemExit"))compare(nativeCode,source,"\"agent\":"+q(agent)+",\"error\":"+q(error));
      }
      for(var text:List.of("null","\"\"","false","\"debug é😀\\nline two\\r\\n\""))for(int layout:List.of(0,2,3))for(boolean relative:List.of(false,true))
        compare(nativeCode,source,"\"agent\":\"copilot\",\"relative\":"+relative+",\"row\":{\"keep\":1,\"_debug_log\":"+text+"}",null,layout);
      for(var value:List.of("3","[1]","{\"x\":1}"))compare(nativeCode,source,"\"agent\":\"copilot\",\"row\":{\"_debug_log\":"+value+"}");
      for(var caps:List.of("null","[]","[\"skills\",\"collaboration\"]","\"web\"","{\"skills\":1}","3"))
        compare(nativeCode,source,"\"kwargs\":{\"codex_config_capabilities\":"+caps+",\"max_turns\":5}");
      compare(nativeCode,source,"\"capabilities_after\":[\"goals\"],\"kwargs\":{\"codex_config_capabilities\":[\"skills\"]}");
      for(var lean:List.of("null","0","false","12","-2.8","\"١_٢\"","\"bad\"","{}","[]"))compare(nativeCode,source,"\"lean_tokens\":"+lean);
      var traces=new ArrayList<byte[]>();
      traces.add(null);traces.add(bytes(""));traces.add(bytes("garbage mcp\n[]\nnull\n{\"mcp\":1}\n"));
      var shell=new StringBuilder();
      for(var command:List.of("rtk git status","/bin/bash -lc \"rtk ls\"","sh -c 'pytest'","/usr/bin/zsh -lc '/opt/bin/rtk proxy make'"," ","rtk", "rtk-ish x","echo rtk","sh\u001c-c\u001frtk ls")) {
        compare(nativeCode,source,"\"mode\":\"_unwrap_shell\",\"command\":"+q(command));
        shell.append(event("\"type\":\"command_execution\",\"command\":"+q(command)+",\"exit_code\":0"));
      }
      traces.add(bytes(shell.toString()));
      var mcp=event("\"type\":\"mcp_tool_call\",\"server\":\"serena\",\"tool\":\"find_symbol\"")
          +event("\"type\":\"mcp_tool_call\",\"invocation\":{\"server\":\"serena\",\"tool\":\"overview\"}")
          +event("\"type\":\"mcp_tool_call\",\"server_name\":\"codex\",\"tool_name\":\"list_mcp_resources\"")
          +"{\"type\":\"item.started\",\"item\":{\"type\":\"mcp_tool_call\",\"tool\":\"skip\"}}\n"
          +event("\"type\":\"mcp_tool_call\",\"name\":\"serena/search\"")
          +event("\"type\":\"mcp_tool_call\",\"weird\":"+q("é😀".repeat(300)))
          +event("\"type\":\"MCP_tool_call\",\"tool\":\"skip\"");
      traces.add(bytes(mcp));traces.add(bytes(mcp.replace("\n","\r\n")));traces.add(bytes(mcp.replace("\n","\r")));
      traces.add(bytes(event("\"type\":\"mcp_tool_call\",\"server\":[\"serena\"],\"tool\":42")));
      traces.add(bytes(event("\"type\":\"mcp_tool_call\",\"weird\":1")));
      traces.add(new byte[]{(byte)0xff,'m','c','p','\r','\n'});
      for(var command:List.of("[\"rtk\",\"ls\"]","42","false","\"/usr/bin/rtk git status\""))
        traces.add(bytes("{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"x\",\"name\":\"Bash\",\"input\":{\"command\":"+command+"}}]}}\n"));
      for(var trace:traces) {
        compare(nativeCode,source,"\"mode\":\"rtk_compliance\"",trace,0);
        for(var server:List.of("null","\"serena\"","\"codex\"","\"\""))compare(nativeCode,source,"\"mode\":\"mcp_tool_calls\",\"server\":"+server,trace,0);
        for(var scaffold:List.of("rtk","serena","repomap"))compare(nativeCode,source,"\"scaffold\":"+q(scaffold)+",\"relative\":true",trace,0);
      }
      for(var mode:List.of("rtk_compliance","mcp_tool_calls")) {
        compare(nativeCode,source,"\"mode\":"+q(mode),null,1);
        compare(nativeCode,source,"\"mode\":"+q(mode)+",\"null_path\":true");
      }
      passed=true;
      System.out.println("Agent task wrappers and scaffold measurements match the source in "+cases+" cases; task identity, state resets, debug files, shell prefixes and MCP events passed");
    } finally {if(passed)ContainerProbe.clear(artifacts);else System.err.println("Probe artifacts: "+artifacts);}
  }
}
