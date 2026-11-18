//JAVA 25

import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import javax.tools.ToolProvider;

/** Verifies native cleanup can interrupt a locked RPC without losing the signal status. */
class WebCloseSignalProbe {
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);}
  static void signal(String name,long target) throws Exception {
    var result=new ProcessBuilder("/bin/kill","-"+name,Long.toString(target)).start();
    require(result.waitFor(5,TimeUnit.SECONDS)&&result.exitValue()==0,"signal failed: "+name+" "+target);
  }
  static boolean open(int port) {
    try(var socket=new Socket()){socket.connect(new InetSocketAddress("127.0.0.1",port),100);return true;}
    catch(java.io.IOException expected){return false;}
  }
  static void gone(ProcessHandle handle) throws Exception {
    var end=System.nanoTime()+Duration.ofSeconds(5).toNanos();
    while(handle.isAlive()&&System.nanoTime()<end)Thread.sleep(10);
    require(!handle.isAlive(),"owned process survived: "+handle.pid());
  }
  public static void main(String[] args) throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"web-close-signals-").toAbsolutePath();
    var javaExecutable=Path.of(System.getProperty("java.home"),"bin/java").toString();
    require(ToolProvider.getSystemJavaCompiler().run(null,null,null,"-d",root.toString(),"validation/WebCleanupFixture.java")==0,"fixture compilation failed");
    int cases=0;
    for(boolean nativeCode:List.of(false,true))for(var kind:List.of("TERM","INT","HUP","GROUP_TERM")) {
      var directory=Files.createDirectory(root.resolve((nativeCode?"native-":"source-")+kind));
      var command=new ArrayList<>(List.of(Path.of(".venv/bin/python").toAbsolutePath().toString(),"validation/web_close_signal_host.py",javaExecutable,root.toString(),directory.toString()));
      if(nativeCode)command.add("--native");
      var process=new ProcessBuilder(command).redirectOutput(directory.resolve("stdout").toFile()).redirectError(directory.resolve("stderr").toFile()).start();
      ProcessHandle node=null,primary=null;
      try {
        var ready=directory.resolve("ready");var blocked=directory.resolve("blocked");
        var deadline=System.nanoTime()+Duration.ofSeconds(10).toNanos();
        while((!Files.exists(ready)||!Files.exists(blocked))&&process.isAlive()&&System.nanoTime()<deadline)Thread.sleep(10);
        require(Files.exists(ready)&&Files.exists(blocked)&&process.isAlive(),"signal fixture did not block: "+Files.readString(directory.resolve("stderr")));
        var fields=Files.readString(ready).strip().split(" ");
        require(Long.parseLong(fields[0])==process.pid()&&Long.parseLong(fields[1])==process.pid(),"fixture group ownership changed");
        node=ProcessHandle.of(Long.parseLong(fields[2])).orElseThrow();primary=ProcessHandle.of(Long.parseLong(fields[5])).orElseThrow();
        require(node.parent().orElseThrow().pid()==process.pid()&&primary.parent().orElseThrow().pid()==process.pid(),"fixture child ownership changed");
        require(open(Integer.parseInt(fields[3]))&&open(Integer.parseInt(fields[4])),"fixture resources are not live");
        var name=kind.equals("GROUP_TERM")?"TERM":kind;
        signal(name,kind.equals("GROUP_TERM")?-process.pid():process.pid());
        require(process.waitFor(10,TimeUnit.SECONDS),"signal cleanup deadlocked: "+directory);
        int number=name.equals("TERM")?15:name.equals("INT")?2:1;
        require(process.exitValue()==128+number,"signal status changed: "+process.exitValue()+" "+directory);
        gone(node);gone(primary);
        require(!open(Integer.parseInt(fields[3]))&&!open(Integer.parseInt(fields[4])),"owned listener survived cleanup");
        var events=Files.readAllLines(directory.resolve("events"));
        var semantic=events.stream().filter(line->!line.startsWith("bridge ")).toList();
        require(semantic.size()==5&&semantic.get(0).startsWith("discard ")&&semantic.subList(1,5).equals(List.of("shutdown","server_close","close","finished True")),"cleanup sequence changed: "+events);
        if(!kind.equals("GROUP_TERM"))require(semantic.get(0).equals("discard True"),"RPC lock was not held during signal cleanup");
        var bridges=events.stream().filter(line->line.startsWith("bridge ")).toList();
        require(bridges.size()==(nativeCode?4:0),"unexpected cleanup channel count: "+events);
        for(var bridge:bridges) {
          var parts=bridge.split(" ");require(parts[1].equals(parts[2]),"cleanup bridge shared the interrupted group");
          var handle=ProcessHandle.of(Long.parseLong(parts[1]));if(handle.isPresent())gone(handle.get());
        }
        cases++;
      } finally {
        if(process.isAlive()){signal("KILL",-process.pid());process.waitFor(5,TimeUnit.SECONDS);}
        if(node!=null&&node.isAlive()){signal("KILL",-node.pid());gone(node);}
        if(primary!=null&&primary.isAlive()){primary.destroyForcibly();gone(primary);}
      }
    }
    try(var paths=Files.walk(root)){for(var path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
    System.out.println("Source and native evaluator cleanup passed "+cases+" real signal cases; TERM/INT/HUP, process-group TERM, held RPC locks, listener shutdown, child reaping and original signal exit statuses passed");
  }
}
