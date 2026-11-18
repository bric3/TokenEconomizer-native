//JAVA 25

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.ServerSocket;
import java.nio.file.Files;
import java.nio.file.Path;

/** Owned long-lived processes for process-group and interrupted-RPC cleanup checks. */
class WebCleanupFixture {
  public static void main(String[] args) throws Exception {
    if(args[0].equals("docker")) {
      Files.writeString(Path.of(args[1]),"removed\n");
      System.exit(39);
    }
    if(args[0].equals("rpc")) {
      var input=new BufferedReader(new InputStreamReader(System.in));
      if(input.readLine()==null)throw new AssertionError("missing RPC request");
      Files.writeString(Path.of(args[1]),"blocked\n");
      if(input.readLine()!=null)throw new AssertionError("unexpected second RPC request");
      return;
    }
    try(var server=new ServerSocket(0)) {
      System.out.println(ProcessHandle.current().pid()+" "+server.getLocalPort());
      System.out.flush();
      while(true)try(var connection=server.accept()) { }
    }
  }
}
