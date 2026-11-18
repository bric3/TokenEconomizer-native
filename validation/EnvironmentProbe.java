//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Compares native environment parsing, file selection and application with the source. */
class EnvironmentProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields)throws Exception {
    var request="{"+fields+"}";var actual=nativeCode.call(request);var expected=source.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+checks+": "+request+"\nNative: "+actual+"\nSource: "+expected);checks++;
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"environment-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"environment_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"environment_host.py")) {
      for(var text:List.of("","# comment\nA=value","export A=one\nA=two\nA=\nB=\"\""," A = ' space ' \r\nB=\"$HOME $(echo literal)\"\rC=left=right # literal","=bad\nexport =bad\nexport\tD=kept\nexport export E=value","QUOTES='mismatch\"\nHASH=#literal\nUNICODE=é😀"))
        for(boolean direct:List.of(false,true))compare(nativeCode,source,"\"text\":"+q(text)+",\"direct\":"+direct);
      for(var separator:List.of("\n","\r","\r\n","\u000b","\f","\u001c","\u001d","\u001e","\u0085","\u2028","\u2029"))
        for(var space:List.of(" ","\t","\u001f","\u00a0","\u2003"))for(boolean direct:List.of(false,true))
          compare(nativeCode,source,"\"text\":"+q(space+"A"+space+"="+space+"one"+space+separator+"B=two")+",\"direct\":"+direct);
      compare(nativeCode,source,"\"text\":\"KEY\\ud800=value\\udfff\"");
      for(var text:List.of("null","17","false","[]","{}"))compare(nativeCode,source,"\"text\":"+text);
      var experiments=Files.createDirectory(root.resolve("experiments"));var parent=root.resolve(".env");var local=experiments.resolve(".env");
      for(boolean hasParent:List.of(false,true))for(boolean hasLocal:List.of(false,true)) {
        if(hasParent)Files.writeString(parent,"A=parent\n");if(hasLocal)Files.writeString(local,"A=local\n");
        for(var path:List.of("null","\"\"",q(local)))for(var environment:List.of("{}","{\"TOKENECONOMIZER_ENV_FILE\":"+q(parent)+"}"))
          compare(nativeCode,source,"\"operation\":\"path\",\"root\":"+q(experiments)+",\"path\":"+path+",\"env\":"+environment);
        compare(nativeCode,source,"\"operation\":\"load\",\"root\":"+q(experiments));
        Files.deleteIfExists(parent);Files.deleteIfExists(local);
      }
      Files.writeString(local,"A=file\nB=file\nEMPTY=\nexport C='literal $A'\n");
      for(boolean override:List.of(false,true))for(var env:List.of("{}","{\"A\":\"exported\",\"B\":\"\"}","{\"A\":null,\"B\":false}"))
        compare(nativeCode,source,"\"operation\":\"load\",\"path\":"+q(local)+",\"env\":"+env+",\"override\":"+override);
      Files.write(local,new byte[]{(byte)255});compare(nativeCode,source,"\"operation\":\"load\",\"path\":"+q(local));
      compare(nativeCode,source,"\"operation\":\"load\",\"path\":"+q(experiments));
      for(var names:List.of("[]","[\"A\"]","[\"A\",\"B\",\"A\"]","[17]","[[]]"))for(var env:List.of("{}","{\"A\":\"exported\",\"B\":\"\"}"))
        compare(nativeCode,source,"\"operation\":\"require\",\"names\":"+names+",\"env\":"+env);
      for(var failure:List.of("open","enter","read","exit","get","set"))for(var error:List.of("FileNotFoundError","OSError","RuntimeError","KeyboardInterrupt"))for(boolean suppress:List.of(false,true))
        compare(nativeCode,source,"\"operation\":\"load\",\"path\":\"fixture\",\"file_fixture\":true,\"document\":\"A=value\",\"fail\":"+q(failure)+",\"error\":"+q(error)+",\"suppress\":"+suppress);
      for(var document:List.of("A=one\nB=two\nA=three\nA=\nEMPTY=\"\"", "GOOD=value\nBAD\u0000KEY=value\nAFTER=skip", "GOOD=value\nBAD=zero\u0000byte\nAFTER=skip", "A='$HOME $(echo literal)'\nUNICODE_é=😀"))for(boolean override:List.of(false,true))
        compare(nativeCode,source,"\"operation\":\"load\",\"path\":\"fixture\",\"file_fixture\":true,\"real_env\":true,\"env\":{\"A\":\"exported\"},\"document\":"+q(document)+",\"override\":"+override);
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Environment loader matches the source in "+checks+" comparisons; native parser and callback binding, literal values, Unicode, precedence, file selection, credential diagnostics and cleanup passed offline");
  }
}
