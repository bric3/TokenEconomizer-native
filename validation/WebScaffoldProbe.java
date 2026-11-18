//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.List;

/** Drives scaffold profile, context, callback and setup comparisons. */
class WebScaffoldProbe{
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception{
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"web-scaffold-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_scaffold_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_scaffold_host.py")){
      for(var agent:List.of("claude","copilot","codex"))for(var operation:List.of("profile","context","exec","host","copy","apply","excludes")){
        var base="\"agent\":"+q(agent)+",\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"objects\":[\"home\"]","\"objects\":[\"workspace\",\"run_dir_path\"]","\"objects\":[\"script\",\"timeout\",\"argv\",\"cfg\"]","\"objects\":[\"host_path\",\"dest_path\"]","\"objects\":[\"patch_excludes\"]","\"objects\":[\"exclude_name\"]","\"scaffold\":false","\"env\":null","\"env\":{}","\"env\":[]","\"env\":{\"PATH\":\"override\",\"HOME\":42}","\"excludes\":null","\"excludes\":[42]","\"lexists\":true","\"dest\":\"tool\"","\"retarget_home\":true"))WebContractProbe.compare(nativeCode,source,base+","+option);
        var phases=switch(operation){
          case "profile"->List.of("get:scaffold_profile_for","scaffolds_pkg.AgentProfile","get:scaffold_bin_dir","bin_dir","profile");
          case "context"->List.of("scaffolds_pkg.ScaffoldContext","get:scaffold_profile_for","profile","context");
          case "exec"->List.of("os.environ","os.pathsep","get:scaffold_bin_dir","bin_dir","get:scaffold_exec_env","exec_env","subprocess.run","run");
          case "host"->List.of("subprocess.run","run");
          case "copy"->List.of("get:scaffold_bin_dir","bin_dir","os.makedirs","path.dirname","dirname","makedirs","path.lexists","lexists","os.unlink","unlink","os.link","link","shutil.copy2","copy2","path.join","join","path.basename","basename","log","os.chmod","chmod","subprocess.CompletedProcess","complete");
          case "apply"->List.of("get:scaffold","os.makedirs","get:scaffold_bin_dir","bin_dir","makedirs","get:prepare_agent_home","prepare","scaffold.setup","get:_scaffold_context","context","setup","task.item","scaffold.name","log","error.str");
          default->List.of("get:scaffold","scaffold.patch_excludes","iter:patch_excludes","get:scaffold_profile_for","profile.patch_excludes","contains:exclude_name");
        };
        for(var phase:phases)for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt","DetailError","ShadowOSError"))WebContractProbe.compare(nativeCode,source,base+",\"lexists\":true,\"fail\":"+q(phase)+",\"error\":"+q(error)+",\"exception_chain\":true");
      }
      for(var phase:List.of("join","basename","log","makedirs","link","copy2","chmod"))for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt","DetailError"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"copy\",\"failures\":[{\"fail\":\"makedirs\",\"error\":\"ObservedOSError\"},{\"fail\":"+q(phase)+",\"fail_at\":"+(phase.equals("makedirs")?2:1)+",\"error\":"+q(error)+"}],\"exception_chain\":true");
      WebContractProbe.compare(nativeCode,source,"\"operation\":\"copy\",\"failures\":[{\"fail\":\"link\",\"error\":\"ObservedOSError\"},{\"fail\":\"copy2\",\"error\":\"PermissionError\"}],\"exception_chain\":true");
      for(boolean rename:List.of(false,true))WebContractProbe.compare(nativeCode,source,"\"operation\":\"apply\",\"fail\":\"setup\",\"error\":\"DetailError\",\"rename_error\":"+rename+",\"error_strings\":["+q("é😀".repeat(200))+",\"second failure\"]");
      WebContractProbe.compare(nativeCode,source,"\"operation\":\"apply\",\"fail\":\"setup\",\"error\":\"DetailError\",\"error_strings\":["+q("é😀".repeat(200))+"]");
      for(var phase:List.of("log","scaffold.name","error.str","task.item"))for(int at:List.of(1,2))WebContractProbe.compare(nativeCode,source,"\"operation\":\"apply\",\"failures\":[{\"fail\":\"setup\",\"error\":\"DetailError\"},{\"fail\":"+q(phase)+",\"fail_at\":"+at+",\"error\":\"RuntimeError\"}],\"exception_chain\":true");
      var root=Files.createDirectories(WebContractProbe.artifacts.resolve("real"));
      var src=root.resolve("source");Files.writeString(src,"fixture binary é😀\n");
      var outside=root.resolve("outside");Files.writeString(outside,"untouched");
      var bin=Files.createDirectory(root.resolve("bin"));var dest=Files.createDirectory(root.resolve("requested")).resolve("tool");
      for(var mode:List.of("hardlink","replace","copy","reroot","copy-reroot")){
        var target=mode.endsWith("reroot")?bin.resolve("tool"):dest;
        var fault=mode.equals("copy")?",\"fail\":\"link\",\"error\":\"OSError\"":mode.equals("reroot")?",\"fail\":\"makedirs\",\"error\":\"PermissionError\"":mode.equals("copy-reroot")?",\"failures\":[{\"fail\":\"link\",\"error\":\"OSError\"},{\"fail\":\"copy2\",\"error\":\"PermissionError\"}]":"";
        var request="{\"operation\":\"copy\",\"real_files\":true,\"src\":"+q(src)+",\"dest\":"+q(dest)+",\"bin_dir\":"+q(bin)+fault+"}";
        Files.deleteIfExists(dest);Files.deleteIfExists(bin.resolve("tool"));
        if(mode.equals("replace"))Files.createSymbolicLink(dest,outside);
        var expected=source.call(request);
        if(!Files.readString(target).equals("fixture binary é😀\n"))throw new AssertionError("source copy fixture failed");
        Files.delete(target);if(mode.equals("replace"))Files.createSymbolicLink(dest,outside);
        var actual=nativeCode.call(request);
        if(!actual.equals(expected)||actual.contains("\"raised\":"))throw new AssertionError("real placement differs: "+mode+"\n"+actual+"\n"+expected);
        if(Files.isSameFile(src,target)==mode.equals("copy")||Files.isSymbolicLink(target)||!Files.readString(outside).equals("untouched"))throw new AssertionError("link, copy or symlink handling changed: "+mode);
        if(!Files.getPosixFilePermissions(target).equals(PosixFilePermissions.fromString("rwxr-xr-x")))throw new AssertionError("executable mode changed");
        WebContractProbe.cases++;
      }
      var home=Files.createDirectory(root.resolve("home"));var workspace=Files.createDirectory(root.resolve("workspace"));
      var script="printf '%s\\n' \"$HOME\" \"${EXTRA-unset}\" \"$AGENT_ENV\"; pwd";
      for(boolean overrides:List.of(false,true)){
        var actual=WebContractProbe.compare(nativeCode,source,"\"operation\":\"exec\",\"real_process\":true,\"home\":"+q(home)+",\"workspace\":"+q(workspace)+",\"environment\":{\"PATH\":\"/usr/bin:/bin\",\"HOME\":\"/operator\"},\"env\":"+(overrides?"{\"EXTRA\":42}":"null")+",\"script\":"+q(script));
        if(!actual.contains("\"code\":0")||!actual.contains(q(home+"\n"+(overrides?"42":"unset")+"\nyes\n"+workspace+"\n")))throw new AssertionError("real callback environment differs: "+actual);
      }
      var actual=WebContractProbe.compare(nativeCode,source,"\"operation\":\"host\",\"real_process\":true,\"argv\":["+q(Path.of(System.getProperty("java.home"),"bin/java"))+",\"-version\"]");
      if(!actual.contains("\"code\":0"))throw new AssertionError("real host callback failed: "+actual);
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Web-Bench scaffold integration matches the source in "+WebContractProbe.cases+" comparisons; profiles, retained callbacks, environments, copy fallbacks, setup and exclusions passed");
  }
}
