//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ProviderProbe.java

import java.nio.file.Path;
import java.util.List;
import java.util.ArrayList;

/** Compares source JSON exceptions supplied to HTTP context managers. */
class JsonDiagnosticProbe {
  static List<String> syntaxExamples() {
    return List.of(""," ","bad","tru","true false","-","+1","01","1.","1e","1e+","[1.]","[1e+]","[","[1","[1 2]","[1,]","{","{x:1}","{\"x\"}","{\"x\":}","{\"x\":1,}","{\"x\":1 2}","\"abc","\"abc\\","\"a\\x\"","\"a\n\"","\"\\uZZZZ\"","\"\\u12\"","\"\\ud800\\uZZZZ\"","\ufeff{}","{\"é😀\":\n bad}");
  }
  static List<String> examples() {
    var cases=new ArrayList<>(syntaxExamples());
    for(var atom:List.of("", " ","0","true","null","NaN","Infinity","-Infinity","1e999","[1]","{\"x\":1}","1.","-01","1E+","é😀","\"x\"","\"\\ud800\"","\"\\u１１２３\"","\"\\u0x12\"","\"\\x\""))
      for(var prefix:List.of("", "[", "{\"é😀\":\n "))for(var suffix:List.of("", "]", "}", ",]", ",}", " x"))cases.add(prefix+atom+suffix);
    return cases;
  }
  static String request(String body) {return "\"agent\":\"codex\",\"name\":\"fetch_catalog\",\"arguments\":{\"base_url\":\"http://fixture\",\"api_key\":\"fixture\"},\"exit_details\":true,\"body\":"+ParserProbe.q(body);}
  public static void main(String[] args) throws Exception {
    try(var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"provider_host.py")) {
      if(List.of(args).contains("--source-only")) {
        for(var body:examples()) {
          var result=source.call("{"+request(body)+"}");
          System.out.println(ParserProbe.q(body)+": "+result);
        }
        return;
      }
      try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"provider_host.py")) {
        for(var body:examples())ProviderProbe.compare(nativeCode,source,request(body));
      }
    }
    System.out.println("JSON diagnostic parity passed "+ProviderProbe.cases+" source comparisons");
  }
}
