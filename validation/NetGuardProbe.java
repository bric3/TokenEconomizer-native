//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/** Compares guard payloads, refusal telemetry and standalone process behaviour. */
class NetGuardProbe {
  static int cases;
  static String q(Object value) { return ParserProbe.q(value); }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String method,String args) throws Exception {
    var request="{\"method\":"+q(method)+",\"args\":{"+args+"}}";
    var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError(request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  record Output(int code,String stdout,String stderr,String log) {}
  static Output process(Path root,boolean nativeCode,byte[] input,Map<String,String> env,int layout) throws Exception {
    var log=root.resolve("guard.jsonl");
    if(Files.isDirectory(log))Files.delete(log);else Files.deleteIfExists(log);
    if(layout==1)Files.writeString(log,"previous record\n");
    if(layout==2)Files.createDirectory(log);
    var source=Path.of(".venv/bin/python").toAbsolutePath().toString();
    var command=nativeCode?List.of(Path.of("target/debug/te-net-guard").toAbsolutePath().toString()):List.of(source,Path.of("reference/upstream/experiments/te_harness/guards/net_guard.py").toAbsolutePath().toString());
    var inputPath=root.resolve("input");var out=root.resolve("out");var err=root.resolve("err");Files.write(inputPath,input);
    var builder=new ProcessBuilder(command).directory(root.toFile()).redirectInput(inputPath.toFile()).redirectOutput(out.toFile()).redirectError(err.toFile());
    builder.environment().remove("TE_ALLOW_NETWORK_TOOLS");builder.environment().remove("TE_NET_GUARD_LOG");
    builder.environment().put("PYTHONPATH",Path.of("reference/upstream/experiments").toAbsolutePath().toString());
    builder.environment().put("TE_NET_GUARD_LOG",log.toString());builder.environment().putAll(env);
    var child=builder.start();
    if(!child.waitFor(10,TimeUnit.SECONDS)){child.destroyForcibly().waitFor();throw new AssertionError("Guard process timed out");}
    return new Output(child.exitValue(),Files.readString(out),Files.readString(err),Files.isRegularFile(log)?Files.readString(log):null);
  }
  static void compareProcess(Path root,byte[] input,Map<String,String> env,int layout) throws Exception {
    var expected=process(root,false,input,env,layout);var actual=process(root,true,input,env,layout);
    if(!actual.equals(expected))throw new AssertionError("Process input: "+new String(input,StandardCharsets.UTF_8)+" env="+env+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  static String concurrent(Path root,boolean nativeCode) throws Exception {
    var log=root.resolve("concurrent.jsonl");Files.deleteIfExists(log);
    var input=root.resolve("concurrent-input");Files.writeString(input,"{\"command\":\"curl github\"}");
    var command=nativeCode?List.of(Path.of("target/debug/te-net-guard").toAbsolutePath().toString()):List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),Path.of("reference/upstream/experiments/te_harness/guards/net_guard.py").toAbsolutePath().toString());
    var children=new ArrayList<Process>();
    for(int i=0;i<24;i++) {
      var builder=new ProcessBuilder(command).redirectInput(input.toFile()).redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(ProcessBuilder.Redirect.DISCARD);
      builder.environment().remove("TE_ALLOW_NETWORK_TOOLS");builder.environment().put("TE_NET_GUARD_LOG",log.toString());
      children.add(builder.start());
    }
    for(var child:children)if(!child.waitFor(10,TimeUnit.SECONDS) || child.exitValue()!=2)throw new AssertionError("Concurrent refusal failed");
    var lines=Files.readAllLines(log);
    if(lines.size()!=24 || lines.stream().anyMatch(line->!line.equals("{\"command\": \"curl github\"}")))throw new AssertionError("Concurrent telemetry was damaged: "+lines);
    return Files.readString(log);
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory("te-net-guard-");boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"net_guard_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"net_guard_host.py")) {
      var baseline=process(root,false,"{\"command\":\"curl github\"}".getBytes(StandardCharsets.UTF_8),Map.of(),0);
      if(baseline.code()!=2 || baseline.stderr().isEmpty() || baseline.log()==null)throw new AssertionError("Source refusal fixture did not activate: "+baseline);
      compare(nativeCode,source,"hosts_block","");
      for(var command:List.of("", "pip install six", "curl https://github.com/a", "GITHUB", "gİthub", "gıthub", "ＧＩＴＨＵＢ", "git clone git@github.com:a/b", "localhost", "é😀 github"))
        compare(nativeCode,source,"decide","\"command\":"+q(command));
      for(var command:List.of("null","false","true","0","42","3.25","[]","[1]","{}","{\"x\":1}"))compare(nativeCode,source,"decide","\"command\":"+command);
      var values=List.of("null","false","0","\"\"","\"curl github\"","[\"curl\",\"github\"]","[null,false,42,{\"é😀\":\"github\"}]","42","{\"command\":\"github\"}");
      for(var value:values) {
        compare(nativeCode,source,"_as_command","\"value\":"+value);
        compare(nativeCode,source,"hook_command","\"payload\":"+value);
        for(var key:List.of("tool_input","toolArgs","input","arguments","params"))
          compare(nativeCode,source,"hook_command","\"payload\":{"+q(key)+":{\"command\":"+value+"},\"command\":\"fallback github\"}");
      }
      for(int depth:List.of(-1,0,1,2,3,4,5,9))for(int wrapped=0;wrapped<7;wrapped++) {
        var node="{\"command\":\"curl github\"}";
        for(int i=0;i<wrapped;i++)node=i%2==0?"{\"child\":"+node+"}":"["+node+"]";
        compare(nativeCode,source,"_find_command","\"node\":"+node+",\"depth\":"+depth);
        compare(nativeCode,source,"hook_command","\"payload\":"+node);
      }
      var payloads=List.of("not json","null","[]","{}","{\"command\":\"pip install six\"}","{\"toolArgs\":{\"command\":\"curl github\"}}","{\"tool_input\":{\"command\":[\"curl\",\"GITHUB\"]}}","{\"nested\":{\"command\":\"é😀 github\"}}","{\"command\":"+q("é😀".repeat(250)+" github")+"}");
      for(var payload:payloads)for(var lift:List.of("", "1"," true ","yes","TRUE"," 1 \u001f"))for(int layout:List.of(0,1,2))
        compareProcess(root,payload.getBytes(StandardCharsets.UTF_8),Map.of("TE_ALLOW_NETWORK_TOOLS",lift),layout);
      compareProcess(root,new byte[]{(byte)255},Map.of(),0);
      for(var number:List.of("NaN","Infinity","-Infinity","1e999","-1e999","1e-999","-0","-0.0","1e-7","1e-4","1e15","1e16","1.2345678901234567e20","1234567890123456789012345678901234567890"))
        compareProcess(root,("{\"command\":[\"github\","+number+"]}").getBytes(StandardCharsets.UTF_8),Map.of(),0);
      for(var payload:List.of("{\"command\":\"github\\ud800\"}","{\"command\":\"github\\udcff\"}","{\"command\":\"github\\ud800\\udc00\"}","{\"command\":\"github\\ud800\\u0061\"}","{\"command\":[\"github\",{\"\\ud800\":NaN,\"x\":[true,null,Infinity]}]}","{\"command\":\"github\",\"unused\":"+"[".repeat(150)+"0"+"]".repeat(150)+"}"))
        compareProcess(root,payload.getBytes(StandardCharsets.UTF_8),Map.of(),0);
      var malformed=new java.io.ByteArrayOutputStream();malformed.writeBytes("{\"command\":\"github".getBytes(StandardCharsets.UTF_8));malformed.write(255);malformed.writeBytes("\"}".getBytes(StandardCharsets.UTF_8));
      compareProcess(root,malformed.toByteArray(),Map.of(),0);
      for(var payload:List.of("{\"command\":\"github\",}","{\"command\":\"github\"} extra","{\"command\":[\"github\",-NaN]}","{\"command\":[\"github\",01]}","{\"command\":\"github\\uZZZZ\"}","{\"command\":\"github\\ud800\\uZZZZ\"}"))
        compareProcess(root,payload.getBytes(StandardCharsets.UTF_8),Map.of(),0);
      for(int depth:List.of(999,1000,1002,5000))compareProcess(root,("{\"command\":\"github\",\"unused\":"+"[".repeat(depth)+"0"+"]".repeat(depth)+"}").getBytes(StandardCharsets.UTF_8),Map.of(),0);
      for(int digits:List.of(4300,4301))compareProcess(root,("{\"command\":[\"github\","+"7".repeat(digits)+"]}").getBytes(StandardCharsets.UTF_8),Map.of(),0);
      for(var limit:List.of("0","640"))for(int digits:List.of(640,641,5000))compareProcess(root,("{\"command\":[\"github\","+"7".repeat(digits)+"]}").getBytes(StandardCharsets.UTF_8),Map.of("PYTHONINTMAXSTRDIGITS",limit),0);
      if(!concurrent(root,false).equals(concurrent(root,true)))throw new AssertionError("Concurrent log bytes differ");cases++;
      for(var payload:payloads) {
        var request="{\"method\":\"main\",\"input\":"+q(payload)+"}";
        var expected=source.call(request);var actual=nativeCode.call(request);
        if(!actual.equals(expected))throw new AssertionError("Imported main: "+request+"\nNative: "+actual+"\nSource: "+expected);
        cases++;
      }
      var transcript=root.resolve("transcript.jsonl");
      var traces=new ArrayList<String>();traces.add("");traces.add("malformed\r\n");
      for(var command:List.of("\"pip install six\"","\"curl github\"","[\"curl\",\"gİthub\"]","42","null"))
        traces.add("{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"x\",\"name\":\"Bash\",\"input\":{\"command\":"+command+"}}]}}\n");
      for(var trace:traces) {
        Files.writeString(transcript,trace);
        var request="{\"method\":\"answer_key_attempts\",\"root\":"+q(root)+"}";
        var expected=source.call(request);var actual=nativeCode.call(request);
        if(!actual.equals(expected))throw new AssertionError("Attempt counter: "+trace+"\nNative: "+actual+"\nSource: "+expected);
        cases++;
      }
      Files.delete(transcript);
      var request="{\"method\":\"answer_key_attempts\",\"root\":"+q(root)+"}";
      if(!source.call(request).equals(nativeCode.call(request)))throw new AssertionError("Missing transcript differs");cases++;
      Files.createDirectory(transcript);
      if(!source.call(request).equals(nativeCode.call(request)))throw new AssertionError("Directory transcript differs");cases++;
      passed=true;
      System.out.println("Answer-key guard matches the source in "+cases+" comparisons; payload priority, bounded search, Unicode matching, process refusal, logging and transcript attempts passed");
    } finally {if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
  }
}
