//JAVA 25
//SOURCES CliProbe.java ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** Compares argument namespaces, help and parser errors with the pinned CLI. */
class ParserProbe {
  static String q(Object value) {
    var out=new StringBuilder("\"");
    for(char c:value.toString().toCharArray()) {
      switch(c) {
        case '"','\\' -> out.append('\\').append(c);
        case '\n' -> out.append("\\n");
        case '\r' -> out.append("\\r");
        case '\t' -> out.append("\\t");
        case '\b' -> out.append("\\b");
        case '\f' -> out.append("\\f");
        default -> {if(c<32)out.append(String.format("\\u%04x",(int)c));else out.append(c);}
      }
    }
    return out.append('"').toString();
  }
  static String array(List<String> values) {return "[" + String.join(",",values.stream().map(ParserProbe::q).toList()) + "]";}
  record Output(int code,String out,String err) {}
  static Output source(Path root, String width, List<String> args) throws Exception {
    String code="import json, sys; from tokeneconomizer.plugin import cli; ns=vars(cli.build_parser().parse_args(json.loads(sys.argv[1]))); ns['fn']=ns['fn'].__name__; print(json.dumps(ns, separators=(',', ':'), ensure_ascii=False))";
    var out=Files.createTempFile(root,"stdout-",".log");
    var err=Files.createTempFile(root,"stderr-",".log");
    try {
      var builder=new ProcessBuilder(Path.of(".venv/bin/python").toAbsolutePath().toString(),"-c",code,array(args)).redirectOutput(out.toFile()).redirectError(err.toFile());
      builder.environment().put("PYTHONPATH",Path.of("reference/upstream").toAbsolutePath().toString());
      builder.environment().put("COLUMNS",width);
      var process=builder.start();
      return new Output(process.waitFor(),Files.readString(out),Files.readString(err));
    } finally {Files.delete(out);Files.delete(err);}
  }
  static void compare(ContextProbe.Client client, Path root, int width, List<String> args) throws Exception {
    compare(client,root,Integer.toString(width),args);
  }
  static void compare(ContextProbe.Client client, Path root, String width, List<String> args) throws Exception {
    var expected=source(root,width,args);
    var actual=ContextProbe.value(client.call("cli.parse_args","\"argv\":" + array(args) + ",\"cwd\":" + q(Path.of("").toAbsolutePath()) + ",\"process_env\":{\"COLUMNS\":" + q(width) + "}"));
    String result;
    if (expected.code()==0 && expected.out().startsWith("{")) result="{\"args\":" + expected.out().stripTrailing() + ",\"exit\":null,\"stdout\":\"\",\"stderr\":\"\"}";
    else result="{\"args\":null,\"exit\":" + expected.code() + ",\"stdout\":" + q(expected.out()) + ",\"stderr\":" + q(expected.err()) + "}";
    if (!actual.equals(result)) throw new AssertionError("Arguments " + args + " at width " + width + "\nActual: " + actual + "\nExpected: " + result);
  }
  public static void main(String[] args) throws Exception {
    var checkout=Path.of("").toAbsolutePath();
    var root=Files.createTempDirectory(checkout.resolve("validation/results"),"parser-");
    int cases=0;
    try (var client=new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"),"cli")) {
      for (String command:List.of("status","microskills","facts","toolprune","untool","config","fold","doctor","repos","history","repomap","probe")) {
        compare(client,root,80,List.of(command));cases++;
        compare(client,root,80,List.of("--cwd","before","--host","pi",command,"--cwd","after","--host=codex"));cases++;
      }
      for (var input:List.of(
          List.<String>of(), List.of("--help"), List.of("-h"),List.of("bogus"),List.of("--bad"),List.of("--bad","status"),List.of("status","--bad"),
          List.of("--host","bad","status"),List.of("--host"),List.of("--cwd","--help"),List.of("--h"),List.of("--h=1"),
          List.of("status","--h"),List.of("--help","--h"),List.of("--cwd","--h"),List.of("--","status","--h"),List.of("--","status","--help","--h"),
          List.of("stats"),List.of("--host","claud","status"),List.of("facts","forgt"),
          List.of("--","status"),List.of("--cwd=space word","status"),List.of("status","--cwd=space word"),
          List.of("judge"),List.of("judge","--session","one"),List.of("judge","--session=one","--job","j"),
          List.of("fold","--session","one","--session=two","--verbose"),List.of("fold","--session"),
          List.of("probe","--sessions","-3","--model=space word","--keep"),List.of("probe","--sessions","+1_2"),List.of("probe","--sessions","٢٣"),
          List.of("probe","--sessions","1.5"),List.of("probe","--sessions","1__2"),List.of("probe","--sessions","-1e2"),
          List.of("microskills","forget","space word","é😀"),List.of("facts","unforget","--","-fact"),
          List.of("microskills","forget","one","--host","claude","two"),List.of("microskills","--host","claude","forget","one"),
          List.of("microskills","forget","--host","claude","one"),List.of("config","set","a=b","--global"),
          List.of("history","import","--auto","--dry-run","--status"),List.of("status","--json=1"),List.of("probe","--ses=7"),
          List.of("status","--ex"),List.of("-hh"),List.of("-hfoo"),List.of("-h=1"),List.of("status","-hfoo"),List.of("toolprune","-1"),List.of("status","--","--json"))) {
        compare(client,root,80,input);cases++;
      }
      for (int width:List.of(10,20,40,55,80,120,200)) {
        compare(client,root,width,List.of("--help"));cases++;
        for (String command:List.of("status","microskills","facts","toolprune","untool","config","fold","judge","doctor","repos","history","repomap","probe")) {
          compare(client,root,width,List.of(command,"--help"));cases++;
        }
      }
      for(String width:List.of("-1","0","bad","٤٠"," ٥٥ ","4_0","999999999999999999999999999999999")) {
        compare(client,root,width,List.of("status","--help"));cases++;
      }
      System.out.println("Native parser matches upstream for " + cases + " namespaces, help and error cases");
    } finally {
      try(var paths=Files.walk(root)) {for(var path:paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);}
    }
  }
}
