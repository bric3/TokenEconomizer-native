//JAVA 25
//SOURCES ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.TimeUnit;

/** Checks response boundaries, owned process cleanup, signal deferral and no replay. */
class StandaloneTransportProbe {
  static int checks;
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static String sh(Object value){return "'"+value.toString().replace("'","'\\''")+"'";}
  static void server(Path root,String mode)throws Exception {
    Files.writeString(root.resolve("pid"),ProcessHandle.current().pid()+" "+ProcessHandle.current().parent().orElseThrow().pid());
    int requests=0;
    try(var input=new BufferedReader(new InputStreamReader(System.in,StandardCharsets.UTF_8))) {
      for(String line;(line=input.readLine())!=null;) {
        Files.writeString(root.resolve("requests"),String.valueOf(++requests));
        if(mode.equals("int")||mode.equals("term")) {
          Files.writeString(root.resolve("ready"),"ready");long deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(10);
          while(!Files.exists(root.resolve("continue"))){if(System.nanoTime()>deadline)throw new AssertionError("signal fixture was not released");Thread.sleep(5);}
        }
        if(mode.equals("eof"))return;
        var response=switch(mode) {
          case "json"->"not-json";
          case "array"->"[]";
          case "missing"->"{}";
          case "error"->"{\"error\":\"fixture error\"}";
          default->"{\"result\":{\"pid\":"+ProcessHandle.current().pid()+",\"sequence\":"+requests+",\"line\":"+ParserProbe.q(line)+"}}";
        };
        System.out.println(response);System.out.flush();
      }
    }
  }
  public static void main(String[] args)throws Exception {
    if(args.length>0){server(Path.of(args[1]),args[2]);return;}
    var root=Files.createTempDirectory(Path.of("validation/results"),"standalone-transport-").toAbsolutePath();boolean passed=false;
    try {
      for(var mode:List.of("normal","threads","json","array","missing","error","eof","int","term","gap","callback")) {
        var directory=Files.createDirectory(root.resolve(mode));var executable=directory.resolve("server");
        Files.writeString(executable,"#!/bin/sh\nexec "+sh(Path.of(System.getProperty("java.home"),"bin/java"))+" -cp "+sh(System.getProperty("java.class.path"))+" StandaloneTransportProbe --server "+sh(directory)+" "+sh(mode)+"\n");
        require(executable.toFile().setExecutable(true),"server fixture is not executable");
        var log=directory.resolve("out");var errors=directory.resolve("err");
        var process=new ProcessBuilder(Path.of(".venv/bin/python").toAbsolutePath().toString(),Path.of("validation/standalone_transport_host.py").toAbsolutePath().toString(),executable.toString(),mode,directory.toString()).redirectOutput(log.toFile()).redirectError(errors.toFile()).start();
        try {
          if(mode.equals("int")||mode.equals("term")||mode.equals("gap")) {
            long deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(10);
            while(!Files.exists(directory.resolve("ready"))){if(!process.isAlive()||System.nanoTime()>=deadline)throw new AssertionError("signal fixture did not become ready");Thread.sleep(5);}
            var kill=new ProcessBuilder("/bin/kill",mode.equals("term")?"-TERM":"-INT",String.valueOf(process.pid())).start();require(kill.waitFor()==0,"signal delivery failed");
            Thread.sleep(50);Files.writeString(directory.resolve("continue"),"continue");
          }
          if(!process.waitFor(15,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("transport client timed out: "+mode);}
          var output=Files.readString(log);require(process.exitValue()==0,Files.readString(errors));
          require(output.contains("\"restored\":true"),"signal handler was not restored: "+output);
          require(Files.readString(directory.resolve("requests")).equals(mode.equals("threads")?"32":"1"),"request was lost or replayed: "+mode);
          var identity=Files.readString(directory.resolve("pid")).split(" ");
          require(Long.parseLong(identity[1])==process.pid(),"fixture process ownership changed");
          require(ProcessHandle.of(Long.parseLong(identity[0])).map(handle->!handle.isAlive()).orElse(true),"owned engine was left alive: "+mode);
          var error=switch(mode){case "json"->"JSONDecodeError";case "array","missing","error"->"ValueError";case "eof"->"RuntimeError";case "int","gap","callback"->"KeyboardInterrupt";case "term"->"SystemExit";default->null;};
          if(error!=null)require(output.contains("\"raised\":\""+error+"\""),"wrong failure: "+output);
          else require(!output.contains("\"raised\""),"transport failed: "+output);
          if(List.of("int","term","gap","callback").contains(mode))require(output.contains("\"result\":"),"signal interrupted a committed frame: "+output);
          if(mode.equals("gap")||mode.equals("callback"))require(!output.contains("\"continued\":true"),"callback was not interruptible: "+output);
          if(mode.equals("threads"))require(output.contains("\"sequences\":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]"),"parallel response order changed: "+output);
        }finally{
          if(process.isAlive())process.destroyForcibly().waitFor();
          if(Files.exists(directory.resolve("pid"))) {
            var pid=Long.parseLong(Files.readString(directory.resolve("pid")).split(" ")[0]);
            ProcessHandle.of(pid).filter(handle->handle.info().commandLine().orElse("").contains(directory.toString())).ifPresent(ProcessHandle::destroyForcibly);
          }
        }
      }
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Standalone transport passed "+checks+" checks; concurrent frames, malformed responses, EOF, signal deferral, handler restoration, no replay and owned process shutdown passed");
  }
}
