//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares authored detector patterns and matching with the pinned Python compiler. */
class RegexProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String pattern,List<String> texts)throws Exception {
    var request="{\"pattern\":"+q(pattern)+",\"texts\":"+ParserProbe.array(texts)+"}";
    request(nativeCode,source,request);
  }
  static void request(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String request)throws Exception {
    var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args)throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"regex_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"regex_host.py")) {
      var texts=List.of("","A","a","Z","z","K","k","S","s","İ","ı","ſ","K","é","É","中","😀","0","٩","_"," ","\t","\n","\r","\u001c","\u0085","\u00a0","\u2003","éA","Aé","A_","__","aa","aA","aé","éÉ","éé","\nA\n");
      compare(nativeCode,source,"(?a)\\w",texts);
      for(var mode:List.of("","(?a)","(?i)","(?ai)"))for(var atom:List.of("\\w","\\W","\\d","\\D","\\s","\\S","\\b","\\B",".","[^a]","[a-z]","[A-Z]","é","k","s","\\x6b","\\u006b","\\u00e9","[\\w-]","[^\\W_]"))
        compare(nativeCode,source,mode+atom,texts);
      for(var pattern:List.of("(?a:\\w+)","(?a:(?u:\\w+))","(?i)(?a:k)","(?a)(?u:k)","(?a)(?u:\\w+)","(?ai)(.)\\1","(?ai)(?P<word>.)?(?(word)(?P=word)|k)","(?a)(?u:[a-z])","(?ai:é)","(?ai:[é])","(?ai:[À-Þ])","(?a)\\bA\\b","(?a)(?=\\w).","(?a)(?>\\w+)","(?a)\\w++","(?ax) a # flag ignored (?u)\n\\w","(?a)[()]\\w","(?a)(?# literal (?u)\\w","(?a)[\\]]","(?a)\\(","(?au)k","(?a-u:k)","(?-a:k)","x(?a)k","(?a)(?u)k","(?u)(?a)k","(?ai)(?u:é)"))
        compare(nativeCode,source,pattern,texts);
      for(var pattern:List.of("(?ai)(.+)\\1","(?ai)(?P<x>.+)(?P=x)","(?ai)(?u:(.))\\1","(?i)(?a:(.))(?u:\\1)","(?a)^\\w+$","(?a)^\\w+\\Z","(?a){}","(?a)a{,}","(?a)\\123","(?a)[\\123]","(?x)\u000ba","(?a)(?u)(","(?a)[^\\w\\W]","(?a)[a-b-c]","(?a)[[]","(?a)[a&&b]","(?a)[^]a]","(?a)\\b\\B"))
        compare(nativeCode,source,pattern,List.of("","aa","aA","aéAé","Aéaé","éé","éÉ","😀😀","KK","KK","a\n","a","aaa","aS","{}","S","[","&","]","b","\u000ba"));
      for(var pattern:List.of("\\1", "\\10", "(a)\\10", "(a)\\11", "(a)\\1", "(a\\1)", "\\1(a)", "(?P=x)(?P<x>a)", "(?P<x>a)(?P=x)", "(?P<x>(?P=x))", "(?P<x>a)(?P<x>b)", "(?P<é>a)(?P=é)", "(?P<1x>a)", "(?P<́>a)", "(?P<x>a)(?(x)b|c)", "(a)?(?(1)b|c)", "(?(1)a|b)(c)", "(a{1,2,3})", "a{,3}", "a{3,2}", "a{1,,2}"))
        compare(nativeCode,source,pattern,List.of("", "a", "aa", "ab", "c", "bc", "a{1,2,3}", "a{1,,2}"));
      for(var pattern:List.of("(?a)\\bpytest\\b","(?ai)\\bpytest\\b","(?u)\\bpytest\\b","(?a)(?u)")) {
        for(var blob:List.of("pytest","épytesté","PYTEST","pytest failed","none"))for(boolean features:List.of(false,true)) {
          var spec="{\"scope\":"+q(pattern)+",\"confirm\":"+q(pattern)+",\"contradict\":\"failed\",\"fired\":"+q(pattern)+"}";
          var digest="{\"_cmd_blob\":"+q(blob)+",\"_out_blob\":\"pytest passed\"}";
          request(nativeCode,source,"{\"mode\":\"detector\",\"spec\":"+spec+",\"digest\":"+digest+(features?",\"features\":{}":"")+"}");
        }
        for(var kind:List.of("cmd_regex","out_regex","cmd_count")) {
          var spec="{\"name\":\"feature\",\"kind\":"+q(kind)+",\"pattern\":"+q(pattern)+"}";
          request(nativeCode,source,"{\"mode\":\"extractor\",\"spec\":"+spec+",\"commands\":[[\"épytesté\",\"PYTEST\"],[\"pytest\",\"none\"],[\"none\",\"pytest\"]]}");
        }
      }
    }
    System.out.println("Detector regex compilation and search passed "+cases+" source comparisons");
  }
}
