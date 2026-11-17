//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Original SDK callbacks and the packaged native binding run against the same SQLite fixtures. */
class HermesGatewayProbe {
  static String q(Object value){return ParserProbe.q(value);}
  static void require(boolean condition,String message){CliProbe.require(condition,message);}
  static class Client implements AutoCloseable {
    final Process process;final PrintWriter input;final BufferedReader output;
    final Map<Long,String> received=new HashMap<>();long next=0;
    Client(Path checkout,Path root,Path binding,String name) throws Exception {
      var builder=new ProcessBuilder(checkout.resolve(".venv/bin/python").toString(),checkout.resolve("validation/hermes_sdk_host.py").toString(),binding.toString())
          .directory(root.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
      builder.environment().clear();builder.environment().putAll(Map.of("HOME",root.toString(),"PATH","","TOKENECONOMIZER_HOME",root.resolve(name+"-state").toString(),
          "PYTHONPATH",checkout.resolve("reference/upstream").toString(),"TE_FOLD_SYNC","1","TE_MICROSKILLS","envfacts","TE_LOOPGUARD","on","TE_TOOLPRUNE","off","GIT_CEILING_DIRECTORIES",root.toString()));
      process=builder.start();input=new PrintWriter(new OutputStreamWriter(process.getOutputStream(),StandardCharsets.UTF_8));
      output=new BufferedReader(new InputStreamReader(process.getInputStream(),StandardCharsets.UTF_8));
    }
    long send(String fields){long id=++next;input.println("{\"id\":"+id+","+fields+"}");input.flush();return id;}
    String receive(long id) throws Exception {
      long deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(30);
      while(!received.containsKey(id)) {
        if(System.nanoTime()>deadline)throw new AssertionError("SDK fixture remained pending");
        if(!output.ready()){if(!process.isAlive())throw new AssertionError("SDK fixture exited: "+process.exitValue());Thread.sleep(5);continue;}
        var line=output.readLine();var match=Pattern.compile("^\\{\"id\":(\\d+),").matcher(line);
        require(match.find(),line);received.put(Long.parseLong(match.group(1)),line);
      }
      var line=received.remove(id);require(!line.contains("\"error\":"),line);
      return line.substring(line.indexOf(",\"value\":")+9,line.length()-1);
    }
    String call(String fields)throws Exception{return receive(send(fields));}
    public void close() throws Exception {input.close();if(!process.waitFor(25,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("SDK fixture did not stop");}require(process.exitValue()==0,"SDK fixture failed");output.close();}
  }
  static String hook(String method,Path home,String args){return "\"method\":"+q(method)+",\"home\":"+q(home)+",\"args\":{"+args+"}";}
  static void compare(Client nativeClient,Client oracle,String fields,String expected)throws Exception {
    var actual=nativeClient.call(fields);var source=oracle.call(fields);
    ContextProbe.expect(actual,source);ContextProbe.expect(actual,expected);
  }
  static void sql(Client client,Path home,String sql)throws Exception {
    ContextProbe.expect(client.call("\"method\":\"sql\",\"db\":"+q(home.resolve("state.db"))+",\"sql\":"+q(sql)),"null");
  }
  static String sqlQuote(Path path){return "'"+path.toString().replace("'","''")+"'";}
  static void scenario(Path checkout,Path root,Path sdk) throws Exception {
    var first=Files.createDirectory(root.resolve("profile-a"));var second=Files.createDirectory(root.resolve("profile-b"));
    var a=Files.createDirectory(root.resolve("repo-a"));var b=Files.createDirectory(root.resolve("repo-b"));
    var fixture=Files.readString(checkout.resolve("reference/upstream/tests/test_hermes_plugin.py"));
    var schema=Pattern.compile("(?s)SCHEMA = \"\"\"(.*?)\"\"\"").matcher(fixture);require(schema.find(),"Original SQLite fixture missing");
    try(var nativeClient=new Client(checkout,root,sdk,"native");var oracle=new Client(checkout,root,checkout.resolve("reference/upstream/plugin/hermes/__init__.py"),"source")) {
      sql(nativeClient,first,schema.group(1)+"INSERT INTO sessions(id,source,created_source,cwd,started_at,ended_at,end_reason) VALUES('alpha','local','cli',"+sqlQuote(a)+",1700000000,1700000100,'completed');");
      sql(nativeClient,second,schema.group(1)+"INSERT INTO sessions(id,source,created_source,cwd,started_at,ended_at,end_reason) VALUES('beta','local','cli',"+sqlQuote(b)+",1700000000,1700000100,'completed');");
      var gone=Files.createDirectory(root.resolve("removed-gateway-cwd"));
      for(var client:List.of(nativeClient,oracle))ContextProbe.expect(client.call("\"method\":\"chdir\",\"path\":"+q(gone)),"null");
      Files.delete(gone);
      for(var client:List.of(nativeClient,oracle)) {
        long one=client.send(hook("section",first,"\"info\":{\"session_id\":\"alpha\",\"cwd\":\"/wrong\"}"));
        long two=client.send(hook("section",second,"\"info\":{\"session_id\":\"beta\"}"));
        ContextProbe.expect(client.receive(two),"\"\"");ContextProbe.expect(client.receive(one),"\"\"");
      }
      compare(nativeClient,oracle,hook("pre_llm_call",first,"\"session_id\":\"alpha\",\"user_message\":[{\"text\":\"SECRET prompt é😀\"},{}],\"turn_id\":\"t\""),"null");
      for(var client:List.of(nativeClient,oracle)) {
        var ids=new java.util.ArrayList<Long>();
        for(int i=0;i<16;i++)ids.add(client.send(hook("transform_tool_result",i%2==0?first:second,"\"session_id\":"+q(i%2==0?"alpha":"beta")+",\"tool_name\":\"terminal\",\"args\":{\"command\":\"pwd\"},\"result\":\"{\\\"output\\\":\\\"ok\\\"}\",\"tool_call_id\":"+q("call-"+i))));
        for(var id:ids)ContextProbe.expect(client.receive(id),"null");
        var snapshot=client.call(hook("snapshot",first,""));
        require(snapshot.contains(a.toString()) && snapshot.contains(b.toString()) && !snapshot.contains("SECRET prompt"),snapshot);
      }
      sql(nativeClient,first,"UPDATE sessions SET end_reason='compression' WHERE id='alpha'; INSERT INTO sessions(id,source,created_source,cwd,parent_session_id,started_at,ended_at,end_reason) VALUES('tip','local','cli',"+sqlQuote(a)+",'alpha',1700000101,1700000200,'completed');");
      compare(nativeClient,oracle,hook("transform_tool_result",first,"\"session_id\":\"tip\",\"tool_name\":\"read_file\",\"args\":{},\"result\":\"{}\""),"null");
      compare(nativeClient,oracle,hook("on_session_end",first,"\"session_id\":\"tip\",\"completed\":true,\"turn_id\":\"t\""),"null");
      compare(nativeClient,oracle,hook("on_session_finalize",first,"\"session_id\":\"tip\",\"reason\":\"closed\""),"null");
      compare(nativeClient,oracle,hook("on_session_finalize",second,"\"session_id\":\"beta\""),"null");
      compare(nativeClient,oracle,hook("snapshot",first,""),"{}");
      ContextProbe.expect(nativeClient.call("\"method\":\"kill_native\""),"null");
      compare(nativeClient,oracle,hook("section",first,"\"info\":{\"session_id\":\"alpha\"}"),"\"\"");
      compare(nativeClient,oracle,hook("on_session_finalize",first,"\"session_id\":\"alpha\""),"null");
    }
    for(var name:List.of("native","source")) {
      try(var paths=Files.walk(root.resolve(name+"-state"))) {
        var ledgers=paths.filter(p->p.getFileName().toString().equals("ledger.jsonl")).toList();
        require(ledgers.size()==2,"SDK did not separate its repositories: "+name);
        for(var ledger:ledgers) {
          require(Files.readAllLines(ledger).size()==1,"SDK folded more than once: "+ledger);
          var alpha=ledger.toString().contains("repo-a-");
          var session=Files.readString(ledger.getParent().resolve("sessions/"+(alpha?"alpha":"beta")+".json"));
          require(session.contains("\"tool_calls\": "+(alpha?9:8)),"SDK lost a tool callback: "+session);
        }
      }
      require(!Files.exists(root.resolve(name+"-state/errors.log")),"SDK recorded an error: "+name);
    }
  }
  static void protocol(Path checkout,Path root)throws Exception {
    var builder=new ProcessBuilder(checkout.resolve("target/debug/te").toString(),"--hermes-bridge").directory(root.toFile()).redirectError(ProcessBuilder.Redirect.INHERIT);
    builder.environment().clear();builder.environment().putAll(Map.of("HOME",root.toString(),"PATH",""));
    var process=builder.start();
    try(var input=new PrintWriter(new OutputStreamWriter(process.getOutputStream(),StandardCharsets.UTF_8));var output=new BufferedReader(new InputStreamReader(process.getInputStream(),StandardCharsets.UTF_8))) {
      input.println("{");input.flush();require(output.readLine().contains("\"error\""),"Malformed request was accepted");
      input.println("{\"id\":\"bad\"}");input.flush();require(output.readLine().contains("unsigned integer"),"Invalid request id was accepted");
      input.println("{\"id\":1,\"method\":\"section\",\"args\":{\"info\":{\"session_id\":\"off\"}},\"env\":{\"TE_DISABLE\":\"1\"}}");input.flush();
      ContextProbe.expect(output.readLine(),"{\"id\":1,\"value\":\"\"}");
      input.println("{\"id\":2,\"method\":\"section\",\"args\":{\"info\":{\"session_id\":\"waiting\"}},\"env\":{}}");input.flush();
      require(output.readLine().contains("\"callback\":\"home\""),"Profile lookup was not requested");
      input.println("{\"id\":2,\"cancel\":true}");input.flush();
      ContextProbe.expect(output.readLine(),"{\"id\":2,\"value\":\"\"}");
      input.println("{\"id\":3,\"method\":\"section\",\"args\":{\"info\":{\"session_id\":\"waiting\"}},\"env\":{}}");input.flush();
      require(output.readLine().contains("\"callback\":\"home\""),"Cancelled profile request blocked the next callback");
      process.getOutputStream().close();
      ContextProbe.expect(output.readLine(),"{\"id\":3,\"value\":\"\"}");
      require(process.waitFor(3,TimeUnit.SECONDS) && process.exitValue()==0,"Native server retained a cancelled profile request");
    } finally {if(process.isAlive())process.destroyForcibly().waitFor();}
  }
  static void brokenBackend(Path checkout,Path root,Path sdk)throws Exception {
    var executable=sdk.getParent().getParent().resolve("scripts/te");
    Files.writeString(executable,"#!/bin/sh\nexec '"+Path.of(System.getProperty("java.home"),"bin/java").toString().replace("'","'\\''")+"' -cp '"+System.getProperty("java.class.path").replace("'","'\\''")+"' HermesGatewayProbe --bad-backend\n");
    Files.setPosixFilePermissions(executable,PosixFilePermissions.fromString("rwx------"));
    try(var client=new Client(checkout,root,sdk,"broken")) {
      for(int i=0;i<2;i++)ContextProbe.expect(client.call(hook("section",root,"\"info\":{\"session_id\":\"s\"}")),"\"\"");
    }
  }
  public static void main(String[] args)throws Exception {
    if(args.length>0 && args[0].equals("--bad-backend")){System.out.println("invalid backend JSON");System.out.flush();Thread.sleep(30_000);return;}
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory(checkout.resolve("validation/results"),"hermes-gateway-");
    try {
      var built=NativeCliProbe.process(root,Map.of("HOME",root.toString(),"PATH",""),List.of(checkout.resolve("target/release/te-package").toString(),"--out",root.resolve("market").toString()));
      require(built.code()==0,built.toString());
      scenario(checkout,root,root.resolve("market/tokeneconomizer/hermes/__init__.py"));
      protocol(checkout,root);
      brokenBackend(checkout,root,root.resolve("market/tokeneconomizer/hermes/__init__.py"));
      System.out.println("Packaged Hermes SDK binding matches upstream through concurrent profiles, tool callbacks, compression, folding, privacy, deleted gateway cwd and protocol recovery without Python on PATH");
    } finally {try(var paths=Files.walk(root)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.deleteIfExists(path);}}
  }
}
