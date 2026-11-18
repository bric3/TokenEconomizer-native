//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Compares workspace construction and answer collection with the pinned implementation. */
class WebWorkspaceProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  static void remove(Path path) throws Exception {
    if(Files.exists(path))try(var paths=Files.walk(path)){for(var entry:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(entry);}
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_workspace_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_workspace_host.py")) {
      for(var operation:List.of("materialize","collect")) {
        var base="\"operation\":"+q(operation);
        compare(nativeCode,source,base);
        for(var option:List.of("\"opaque_workspace\":true","\"isdir\":false","\"files\":null","\"files\":{}","\"files\":true","\"files\":[]","\"opaque_files\":true","\"opaque_files\":true,\"files_truthy\":false","\"request_files\":null","\"request_files\":{}","\"request_files\":[]","\"request_files\":42","\"missing_repo\":true","\"skip_dirs\":[]","\"skip_files\":[]","\"cap\":0","\"cap\":1","\"cap\":null","\"scaffold_files\":[]","\"globs\":[]","\"walk\":[]","\"enter_value\":null","\"context\":false"))compare(nativeCode,source,base+","+option);
        var phases=operation.equals("materialize")?List.of("path.isdir","isdir","rmtree","makedirs","files.bool","files.items","path.join","join","path.dirname","dirname","open","enter","write","exit"):List.of("ignore_globs","runner.repo","runner.scaffold_answer_excludes","scaffold_excludes","ancestors","walk","path.relpath","relpath","path.join","join","ignored","path.getsize","getsize","open","enter","read","exit");
        for(var phase:phases)for(var error:List.of("OSError","UnicodeDecodeError","ValueError","RuntimeError","KeyboardInterrupt","DetailError","ShadowOSError")) {
          var extra=phase.startsWith("files.")?",\"opaque_files\":true":"";
          compare(nativeCode,source,base+",\"fail\":"+q(phase)+",\"error\":"+q(error)+extra);
        }
        for(var phase:List.of("open","enter",operation.equals("materialize")?"write":"read","exit"))for(boolean suppress:List.of(false,true))compare(nativeCode,source,base+",\"fail\":"+q(phase)+",\"error\":\"RuntimeError\",\"suppress\":"+suppress+",\"exception_state\":true");
      }
      for(var rel:List.of("","/absolute","..","../escape","nested/../escape","nested//file","./file","back\\slash","é😀/file","trailing/",".../file"))compare(nativeCode,source,"\"operation\":\"materialize\",\"files\":{"+q(rel)+":\"text\"}");
      for(var value:List.of("null","false","0","1","[]","{}","\"text\\ud800\\udc00\""))compare(nativeCode,source,"\"operation\":\"materialize\",\"files\":{\"content\":"+value+"}");
      for(var rel:List.of("null","false","0","[]","{}"))compare(nativeCode,source,"\"operation\":\"materialize\",\"opaque_files\":true,\"pairs\":[["+rel+",\"text\"]]");
      for(var files:List.of("{\"node_modules/a\":\"old\",\"assets/a\":\"old\"}","{\".hidden/a\":\"old\",\".new\":\"old\",\"package-lock.json\":\"old\"}","{\"big\":\"old\",\"binary\":\"old\",\"generated\":\"old\",\"skip.log\":\"old\"}"))compare(nativeCode,source,"\"request_files\":"+files);
      for(var value:List.of("null","false","0","[]","{}","\"é😀\\ud800\\udc00\""))compare(nativeCode,source,"\"contents\":{\"new\":"+value+"}");
      for(var directory:List.of("node_modules","assets","dist",".hidden","src"))for(boolean supplied:List.of(false,true))compare(nativeCode,source,"\"walk\":[[\"/ws\",["+q(directory)+"],[]]],\"request_files\":"+(supplied?"{"+q(directory+"/entry")+":\"old\"}":"{}"));
      for(var option:List.of("\"fail\":\"read\",\"suppress\":true","\"fail\":\"read\",\"fail_at\":2,\"suppress\":true","\"fail\":\"read\",\"opaque_exit\":true,\"booleans\":{\"exit_result\":false}","\"fail\":\"read\",\"opaque_exit\":true,\"booleans\":{\"exit_result\":true}"))compare(nativeCode,source,option);
      compare(nativeCode,source,"\"observe_format\":true");
      for(var error:List.of("OSError","UnicodeDecodeError"))compare(nativeCode,source,"\"observe_format\":true,\"exception_state\":true,\"fail\":\"read\",\"error\":"+q(error));
      compare(nativeCode,source,"\"observe_format\":true,\"fail\":\"read\",\"error\":\"OSError\",\"walk\":[[\"/ws\",[],[\"one\"]],[\"/ws/src\",[\"nested\"],[\"two\"]]]");
      var root=Files.createTempDirectory(Path.of("validation/results"),"web-workspace-").toAbsolutePath();
      var workspace=root.resolve("workspace");var project=Files.createDirectories(root.resolve("repo/projects/project"));
      Files.writeString(project.resolve(".evalignore"),"*.log\n");
      try {
        var request="{\"operation\":\"materialize\",\"real_files\":true,\"workspace\":"+q(workspace)+",\"files\":{\"a.txt\":\"é😀\\n\",\"src/b.js\":42,\"../escape\":\"bad\"}}";
        Files.createDirectory(workspace);Files.writeString(workspace.resolve("old"),"old");
        var expected=source.call(request);
        if(!Files.readString(workspace.resolve("a.txt")).equals("é😀\n")||Files.size(workspace.resolve("src/b.js"))!=0||Files.exists(root.resolve("escape")))throw new AssertionError("source materialization fixture failed");
        remove(workspace);Files.createDirectory(workspace);Files.writeString(workspace.resolve("old"),"old");
        var actual=nativeCode.call(request);
        if(!actual.equals(expected)||!Files.readString(workspace.resolve("a.txt")).equals("é😀\n")||Files.size(workspace.resolve("src/b.js"))!=0||Files.exists(workspace.resolve("old"))||Files.exists(root.resolve("escape")))throw new AssertionError("real materialization differs: "+actual+"\n"+expected);
        cases++;
        Files.writeString(workspace.resolve("skip.log"),"ignored");Files.write(workspace.resolve("binary"),new byte[]{(byte)0xff});
        Files.createDirectories(workspace.resolve(".tracked"));Files.writeString(workspace.resolve(".tracked/a"),"changed");
        Files.createDirectories(workspace.resolve("node_modules"));Files.writeString(workspace.resolve("node_modules/new"),"skipped");
        var collected=compare(nativeCode,source,"\"real_files\":true,\"workspace\":"+q(workspace)+",\"repo\":"+q(root.resolve("repo"))+",\"project_dir\":"+q(project)+",\"request_files\":{\"a.txt\":\"é😀\\n\",\".tracked/a\":\"old\",\"missing\":\"gone\"}");
        if(!collected.contains("\".tracked/a\":\"changed\"")||!collected.contains("skip.log (evaluator ignores it)")||!collected.contains("binary (unreadable/binary)")||!collected.contains("[\"missing\"]"))throw new AssertionError("real answer collection did not exercise filtering: "+collected);
      } finally {remove(root);}
    }
    System.out.println("Web-Bench workspace policy matches the source in "+cases+" comparisons; path validation, directory pruning, changed files, caps, unreadable files, context exits and real filesystem round trips passed");
  }
}
