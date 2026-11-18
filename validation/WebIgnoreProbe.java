//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares ignore-file loading, path ancestors and native wildcard matching. */
class WebIgnoreProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_workspace_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_workspace_host.py")) {
      for(var rel:List.of("","file","/","//","a/","a//b/","a/b/c","é😀/a","\\windows\\file","\ud800/\udc00/a"))compare(nativeCode,source,"\"operation\":\"ancestors\",\"rel\":"+q(rel));
      for(var value:List.of("null","false","42","[]","{}"))for(var op:List.of("ancestors","ignored"))compare(nativeCode,source,"\"operation\":"+q(op)+",\"rel\":"+value);
      for(var rel:List.of("","a","b","c","z","-","[","]","!","src/a.log","src/a.txt","nested/src/a","a/b","a/x/b","a/x/y/b",".hidden/a","é😀","\n","a\nb","\ud800","\udc00","\ud800\udc00"))for(var pattern:List.of("","/","//","*","**","?","a?","*.log","src/*","a/**/b","[a-z]","[z-a]","[!a]","[]]","[!]]","[[]","[","[]","[!]","[a-b-c]","[--a]","[a--]","[a&&b]","[a||b]","[a~~b]","é*","[\ud800-\udfff]","\ud800?","\\*"))compare(nativeCode,source,"\"operation\":\"ignored\",\"rel\":"+q(rel)+",\"globs\":["+q(pattern)+"]");
      for(var globs:List.of("null","false","42","[null]","[false]","[42]","[[]]","[{}]","[\"/\",\"nomatch\",\"*.log\"]"))compare(nativeCode,source,"\"operation\":\"ignored\",\"globs\":"+globs);
      for(var option:List.of("\"foreign_match\":true","\"foreign_match\":true,\"match_result\":true","\"foreign_match\":true,\"opaque_match\":true","\"foreign_match\":true,\"opaque_match\":true,\"booleans\":{\"match\":false}"))compare(nativeCode,source,"\"operation\":\"ignored\","+option);
      for(var option:List.of("\"foreign_case\":true","\"foreign_case\":true,\"match_result\":true","\"observe_normcase\":true","\"mutate_case\":true","\"mutate_case\":true,\"match_result\":true"))compare(nativeCode,source,"\"operation\":\"ignored\","+option);
      for(var option:List.of("","\"ignore_lines\":[\" # comment\\n\",\"\\n\",\" dist/ \\n\",\"/root///\\n\",\"!keep\\n\",\"é😀\\n\"]","\"ignore_lines\":[null]","\"ignore_lines\":[42]","\"ignore_lines\":[\"\\u001c\\u001f\\u0085\\u3000\"]","\"context\":false","\"enter_value\":null"))compare(nativeCode,source,"\"operation\":\"load_ignore_globs\""+(option.isEmpty()?"":","+option));
      for(var phase:List.of("path.join","join","open","enter","file.iter","exit"))for(var error:List.of("OSError","UnicodeDecodeError","RuntimeError","KeyboardInterrupt","ShadowOSError","DetailError"))compare(nativeCode,source,"\"operation\":\"load_ignore_globs\",\"ignore_lines\":[\"one\\n\"],\"fail\":"+q(phase)+",\"error\":"+q(error));
      for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))for(boolean suppress:List.of(false,true))compare(nativeCode,source,"\"operation\":\"load_ignore_globs\",\"fail\":\"file.iter\",\"error\":"+q(error)+",\"suppress\":"+suppress+",\"exception_state\":true");
    }
    System.out.println("Web-Bench ignore policy matches the source in "+cases+" comparisons; ancestors, eager file paths, line filtering, wildcard classes, surrogate code points, foreign matchers and exception boundaries passed");
  }
}
