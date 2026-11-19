//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;

/** Checks Copilot optimizer process policy without a paid CLI or provider. */
class OptimizerCliProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
    return actual;
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"optimizer-cli-").toAbsolutePath();
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"optimizer_cli_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"optimizer_cli_host.py")) {
      compare(nativeCode,source,"");
      for(var model:List.of("null","false","0","1","\"agent\"","\"AGENT\"","\"copilot\"","\"Copilot\"","\"claude-sonnet-5\"","\"é😀\"","\"\\ud800\""))compare(nativeCode,source,"\"arguments\":{\"model\":"+model+"}");
      for(var options:List.of("\"opaque_effective\":true","\"opaque_usage\":true","\"objects\":[\"binary\"]","\"objects\":[\"model\"]","\"objects\":[\"prompt\"]","\"objects\":[\"model\"],\"strings\":[\"agent\"]","\"objects\":[\"model\"],\"strings\":[\"other\",\"selected\"]","\"mutate_run\":true","\"mutate_environ\":true","\"mutate_sum\":true","\"deny\":[],\"exclude\":[]","\"deny\":[\"a\",\"a\"],\"exclude\":[\"b\"]","\"env\":{\"FIXTURE\":\"override\"}"))compare(nativeCode,source,options);
      for(var failure:List.of("binary","temp:neutral","temp:logdir","run_env","run","stdout","remove:neutral","read_log","sum","remove:logdir","record","pricing"))
        for(var error:List.of("RuntimeError","OSError","TimeoutExpired","SubprocessError","KeyboardInterrupt","SystemExit","ShadowException"))compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(var stdout:List.of("null","false","0","1","[]","{}","\"\"",q("noise\r\n{\"type\":\"assistant.message\",\"data\":{\"content\":\"one\"}}\r{\"type\":\"assistant.message\",\"data\":{\"content\":\"two\"}}"),q("null"),q("[]"),q("true"),q("{\"type\":\"assistant.message\",\"data\":true}"),q("{\"type\":\"assistant.message\",\"data\":{\"content\":[1]}}")))compare(nativeCode,source,"\"stdout\":"+stdout);
      for(var content:List.of("null","false","0","1","[]","{}","[1]","\"\"","\"last\"","\"\\ud800\"")) {
        var lines="{\"type\":\"assistant.message\",\"data\":{\"content\":\"first\"}}\n{\"type\":\"assistant.message\",\"data\":{\"content\":"+content+"}}";
        compare(nativeCode,source,"\"stdout\":"+q(lines));
      }
      for(var usage:List.of("null","false","0","1","[]","{}","[1]","\"opaque\""))compare(nativeCode,source,"\"usage\":"+usage);
      compare(nativeCode,source,"\"mutate_usage\":{\"output_tokens\":23}");
      compare(nativeCode,source,"\"mutate_text\":true,\"stdout\":"+q("{\"type\":\"assistant.message\",\"data\":{\"content\":[1]}}"));
      compare(nativeCode,source,"\"opaque_stdout\":true");
      compare(nativeCode,source,"\"opaque_stdout\":true,\"truthy\":false");
      compare(nativeCode,source,"\"stdout\":"+q("{\"type\":\"assistant.message\",\"data\":{\"content\":\""+(char)0xd800+"\"}}"));
      for(var boundary:List.of("\n","\r","\r\n","\u000b","\u000c","\u001c","\u001d","\u001e","\u0085","\u2028","\u2029"))
        compare(nativeCode,source,"\"stdout\":"+q("noise"+boundary+"{\"type\":\"assistant.message\",\"data\":{\"content\":\"split\"}}"));
      for(var tools:List.of("null","false","1","\"ab\"","{\"tool\":0}"))compare(nativeCode,source,"\"deny\":"+tools);
      var fixture=root.resolve("OfflineCopilot.java");
      Files.writeString(fixture,"""
          import java.nio.file.*;
          class OfflineCopilot {
            public static void main(String[] args) throws Exception {
              var flags=java.util.Arrays.asList(args);
              if(!flags.contains("--deny-tool")||!flags.contains("--excluded-tools")||!flags.contains("--no-custom-instructions")||!flags.contains("--disable-builtin-mcps"))throw new AssertionError("missing isolation flags");
              if(!"false".equals(System.getenv("COPILOT_AUTO_UPDATE")))throw new AssertionError("environment override missing");
              var neutral=Path.of(args[flags.indexOf("-C")+1]);
              if(!Files.isDirectory(neutral))throw new AssertionError("neutral directory missing");
              if(args[1].equals("hang"))Thread.sleep(30000);
              var logs=Path.of(args[flags.indexOf("--log-dir")+1]);
              Files.writeString(logs.resolve("fixture.log"),"\\\"usage\\\": {\\\"prompt_tokens\\\":10,\\\"completion_tokens\\\":2,\\\"total_tokens\\\":12,\\\"prompt_tokens_details\\\":{\\\"cached_tokens\\\":3,\\\"cache_creation_tokens\\\":2}}");
              System.out.println("{\\"type\\":\\"assistant.message\\",\\"data\\":{\\"content\\":\\"local answer\\"}}");
            }
          }
          """);
      if(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),fixture.toString())!=0)throw new AssertionError("fixture compilation failed");
      var executable=root.resolve("copilot-fixture");
      Files.writeString(executable,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java")+"' -cp '"+root+"' OfflineCopilot \"$@\"\n");
      if(!executable.toFile().setExecutable(true))throw new AssertionError("cannot make fixture executable");
      var success=compare(nativeCode,source,"\"real\":true,\"arguments\":{\"binary\":"+q(executable)+"}");
      if(!success.contains("\"value\":[\"local answer\",1.5]")||!success.contains("\"remaining\":[]"))throw new AssertionError("Offline process did not complete and clean up: "+success);
      var policy=compare(nativeCode,source,"\"real\":true,\"policy_log\":"+q(root.resolve("optimizer.jsonl"))+",\"arguments\":{\"binary\":"+q(executable)+",\"model\":\"claude-sonnet-5\"}");
      if(!policy.contains("\"optimizer_log\":[{")||!policy.contains("\"tokens_total\":12")||!policy.contains("\"value\":[\"local answer\",")||policy.contains("\"local answer\",null")||policy.contains("\"raised\"")||!policy.contains("\"remaining\":[]"))throw new AssertionError("Offline CLI did not reach native accounting, pricing and logging: "+policy);
      for(var arguments:List.of("\"binary\":"+q(executable)+",\"prompt\":\"hang\",\"timeout\":0.2","\"binary\":"+q(root.resolve("missing-copilot")))) {
        var failure=compare(nativeCode,source,"\"real\":true,\"arguments\":{"+arguments+"}");
        if(!failure.contains("\"value\":[\"\",null]")||!failure.contains("\"remaining\":[\"logdir\"]"))throw new AssertionError("Offline failure did not preserve source cleanup: "+failure);
      }
      try(var files=Files.walk(root)){for(var path:files.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    }
    System.out.println("Copilot optimizer matches the source in "+cases+" comparisons, including real offline subprocesses and directory cleanup; no provider called");
  }
}
