//JAVA 25
//SOURCES ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.List;

/** Native CLI dispatch, default-host selection and state permissions after failures. */
class CliEntryProbe {
  static String q(Object value) {return ParserProbe.q(value);}
  static void require(boolean condition,String message) {CliProbe.require(condition,message);}
  static String permissions(Path state) throws Exception {return PosixFilePermissions.toString(Files.getPosixFilePermissions(state));}
  static void open(Path state) throws Exception {Files.setPosixFilePermissions(state,PosixFilePermissions.fromString("rwxr-xr-x"));}
  static String start(ContextProbe.Client client, Path root, String env, List<String> argv) throws Exception {
    return client.call("cli.main","\"argv\":" + ParserProbe.array(argv) + ",\"process_cwd\":" + q(root) + ",\"home\":" + q(root) + ",\"process_env\":" + env);
  }
  static void adapter(Path checkout, Path root) throws Exception {
    var out=Files.createTempFile(root,"adapter-",".log");
    try {
      var builder=new ProcessBuilder(checkout.resolve(".venv/bin/python").toString(),"-c",
          "import native_plugin; native_plugin.pytest_configure(None); import os, sys; from tokeneconomizer.plugin import cli; parser=cli.build_parser(); ns=parser.parse_args(['--host','pi','facts','forget','a b']); print(ns.cmd,ns.action,ns.ids,ns.fn.__name__); original=ns.cwd; os.chdir(sys.argv[1]); print(parser.parse_args(['status']).cwd==original); sys.argv=['te','--host','claude','repomap']; print(cli.main())",root.toString()).redirectError(ProcessBuilder.Redirect.INHERIT).redirectOutput(out.toFile());
      builder.environment().put("PYTHONPATH",checkout.resolve("validation").toString());
      builder.environment().put("TOKENECONOMIZER_HOME",root.resolve("adapter-state").toString());
      var process=builder.start();
      require(process.waitFor()==0,"Compatibility entry failed");
      var text=Files.readString(out);
      require(text.startsWith("facts forget ['a b'] cmd_microskills\nTrue\n") && text.endsWith("0\n"),text);
    } finally {Files.delete(out);}
  }
  public static void main(String[] args) throws Exception {
    var checkout=Path.of("").toAbsolutePath();
    var root=Files.createTempDirectory(checkout.resolve("validation/results"),"cli-entry-");
    var state=Files.createDirectory(root.resolve("state"));
    var env="{\"TOKENECONOMIZER_HOME\":\"state\",\"TE_HOST\":\"pi\"}";
    try(var client=new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"),"cli")) {
      open(state);
      var result=start(client,root,env,List.of("status"));
      require(result.contains("[\"execute\",[") && result.contains("\"host\":\"pi\"") && result.contains("\"fn\":\"cmd_status\""),result);
      ContextProbe.expect(ContextProbe.value(client.resume(result,"global","{\"value\":null}")),"0");
      require(permissions(state).equals("rwx------"),permissions(state));
      open(state);
      result=start(client,root,env,List.of("--host","copilot","facts","forget","id"));
      require(result.contains("\"host\":\"copilot\"") && result.contains("\"fn\":\"cmd_microskills\""),result);
      result=client.resume(result,"global","{\"error\":\"failed\",\"kind\":\"ValueError\"}");
      require(result.contains("\"raised\":\"ValueError\"") && permissions(state).equals("rwx------"),result);
      for(String kind:List.of("SystemExit","KeyboardInterrupt")) {
        open(state);
        result=start(client,root,env,List.of("doctor"));
        result=client.resume(result,"global","{\"error\":\"2\",\"kind\":" + q(kind) + "}");
        require(result.contains("\"raised\":" + q(kind)) && permissions(state).equals("rwx------"),result);
      }
      open(state);
      result=start(client,root,env,List.of("--argline","facts forget 'unclosed"));
      require(result.contains("te: could not read the arguments: No closing quotation"),result);
      ContextProbe.expect(ContextProbe.value(client.resume(result,"say","{\"value\":null}")),"2");
      require(permissions(state).equals("rwxr-xr-x"),permissions(state));
      result=start(client,root,env,List.of("--help"));
      require(result.contains("\"method\":\"say\"") && result.contains("inspect and steer"),result);
      result=client.resume(result,"say","{\"value\":null}");
      require(result.contains("\"raised\":\"SystemExit\",\"message\":\"0\"") && permissions(state).equals("rwxr-xr-x"),result);
      result=start(client,root,env,List.of("judge"));
      require(result.contains("[\"stderr\",[") && result.contains("arguments are required: --session, --job"),result);
      result=client.resume(result,"global","{\"value\":null}");
      require(result.contains("\"raised\":\"SystemExit\",\"message\":\"2\"") && permissions(state).equals("rwxr-xr-x"),result);
      for(String value:List.of("null","false","0","7","\"result\"")) {
        result=start(client,root,env,List.of("status"));
        var returned=ContextProbe.value(client.resume(result,"global","{\"value\":" + value + "}"));
        ContextProbe.expect(returned,List.of("null","false","0").contains(value)?"0":value);
      }
      adapter(checkout,root);
      System.out.println("Native CLI dispatch, aliases, default host, exit codes and permission cleanup passed");
    } finally {
      try(var paths=Files.walk(root)) {for(var path:paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);}
    }
  }
}
