//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.TimeUnit;

/** Compares native merge execution and destination bytes with the source-generated program. */
class ScaffoldMergeHelperProbe {
  static int cases;
  static AdapterProbe.Client context;
  static String q(Object value){return ParserProbe.q(value);}
  static Path prepare(Path root,String before,String mode)throws Exception {
    Files.createDirectories(root);var target=root.resolve(mode.equals("filename")?"settings.json":"data/settings.json");
    if(mode.equals("parent_file"))Files.writeString(root.resolve("data"),"keep parent");
    else if(mode.equals("directory"))Files.createDirectories(target);
    else if(before!=null){Files.createDirectories(target.getParent());Files.write(target,before.equals("INVALID_UTF8")?new byte[]{(byte)255}:before.getBytes(StandardCharsets.UTF_8));}
    return target;
  }
  static void compare(AdapterProbe.Client source,Path root,String before,String patch,String mode,int limit)throws Exception {
    var directory=Files.createDirectory(root.resolve("case-"+cases));var sourceRoot=directory.resolve("source");var nativeRoot=directory.resolve("native");
    var expectedPath=prepare(sourceRoot,before,mode);var actualPath=prepare(nativeRoot,before,mode);
    var patchFile=directory.resolve("patch.json");Files.writeString(patchFile,patch);
    var relative=nativeRoot.relativize(actualPath).toString();
    var expected=source.call("{\"directory\":"+q(sourceRoot)+",\"path\":"+q(relative)+",\"patch\":"+q(patchFile)+",\"int_max_digits\":"+limit+"}");
    String actual;
    if(context!=null)actual=context.call("{\"directory\":"+q(nativeRoot)+",\"path\":"+q(relative)+",\"patch\":"+q(patchFile)+",\"int_max_digits\":"+limit+"}");
    else {
      var builder=new ProcessBuilder(Path.of("target/debug/te-net-guard").toAbsolutePath().toString(),"--merge-json",relative,patchFile.toString()).directory(nativeRoot.toFile()).redirectOutput(directory.resolve("out").toFile()).redirectError(directory.resolve("err").toFile());
      builder.environment().put("PYTHONINTMAXSTRDIGITS",Integer.toString(limit));var process=builder.start();
      if(!process.waitFor(10,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("native merge timed out");}
      var stderr=Files.readString(directory.resolve("err")).strip();var lines=stderr.split("\n");
      actual="{\"code\":"+process.exitValue()+",\"error\":"+q(stderr.isEmpty()?"":lines[lines.length-1])+",\"stdout\":"+q(Files.readString(directory.resolve("out")))+"}";
    }
    if(!actual.equals(expected))throw new AssertionError("Merge case "+cases+" differs: before="+before+" patch="+patch+" mode="+mode+"\nNative: "+actual+"\nSource: "+expected+"\nArtifacts: "+directory);
    if(Files.isRegularFile(expectedPath)!=Files.isRegularFile(actualPath)||Files.isDirectory(expectedPath)!=Files.isDirectory(actualPath)||Files.exists(expectedPath.getParent())!=Files.exists(actualPath.getParent()))throw new AssertionError("destination state differs: "+directory);
    if(Files.isRegularFile(expectedPath)&&!Arrays.equals(Files.readAllBytes(expectedPath),Files.readAllBytes(actualPath)))throw new AssertionError("merged bytes differ: "+directory+"\nNative: "+Files.readString(actualPath)+"\nSource: "+Files.readString(expectedPath));
    cases++;
  }
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"scaffold-merge-helper-").toAbsolutePath();boolean passed=false;
    try(var nativeContext=List.of(args).contains("--context")?new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"scaffold_merge_source.py"):null;
        var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"scaffold_merge_source.py")) {
      context=nativeContext;
      for(var before:Arrays.asList(null,"","bad json","INVALID_UTF8","{}","{\"keep\":1,\"nested\":{\"old\":1,\"replace\":0}}","[]","[1]","null","true","42","\"text\"","{\"value\":NaN,\"nested\":{\"number\":Infinity}}","{\"\\ud800\":\"\\udc00\",\"nested\":{\"escaped\":\"é😀\"}}"))
        for(var patch:List.of("{}","{\"new\":2}","{\"nested\":{\"replace\":3,\"added\":4}}","{\"nested\":[]}","{\"nested\":null}","{\"new\":NaN,\"nested\":{\"n\":-Infinity}}","{\"é😀\":\"\\ud800\"}","[]","null","true","42","\"text\""))compare(source,root,before,patch,"normal",4300);
      for(var mode:List.of("filename","parent_file","directory"))for(var patch:List.of("{}","{\"nested\":{\"new\":2}}","[]"))compare(source,root,"{\"keep\":1}",patch,mode,4300);
      for(var patch:List.of("","{","{\"a\":}","{\"a\":1} trailing","{\"n\":"+"7".repeat(5000)+"}"))compare(source,root,null,patch,"normal",4300);
      compare(source,root,null,"{\"n\":"+"7".repeat(5000)+"}","normal",0);
      compare(source,root,"{\"n\":"+"7".repeat(5000)+"}","{\"keep\":true}","normal",640);
      compare(source,root,null,"{\"float\":1.0e-5,\"huge\":1e999,\"negative_zero\":-0.0}","normal",4300);
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Native scaffold merge "+(context==null?"execution":"context staging")+" matches the source in "+cases+" file/process comparisons; one-level updates, preserved keys, malformed files, Unicode, non-finite numbers, integer limits and failure effects passed");
  }
}
