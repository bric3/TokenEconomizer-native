//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Files;
import java.util.List;
import java.util.ArrayList;
import com.sun.net.httpserver.HttpServer;

/** Compares HTTP optimizer requests, retry policy, response parsing and billing. */
class OptimizerHttpProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
    return actual;
  }
  static void localHttp(AdapterProbe.Client nativeCode,AdapterProbe.Client source) throws Exception {
    var received=new ArrayList<String>();var server=HttpServer.create(new InetSocketAddress("127.0.0.1",0),0);
    server.createContext("/",exchange->{
      var body=new String(exchange.getRequestBody().readAllBytes(),StandardCharsets.UTF_8);
      synchronized(received){received.add(exchange.getRequestURI()+" "+exchange.getRequestHeaders().getFirst("Authorization")+" "+exchange.getRequestHeaders().getFirst("X-api-key")+" "+body);}
      var response="{\"content\":[{\"type\":\"text\",\"text\":\"local-é😀\"}],\"output_text\":\"local-é😀\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}".getBytes(StandardCharsets.UTF_8);
      exchange.sendResponseHeaders(200,response.length);try(var output=exchange.getResponseBody()){output.write(response);}
    });server.start();
    try {
      var base="http://127.0.0.1:"+server.getAddress().getPort();
      var root=Files.createTempDirectory(Path.of("validation/results"),"optimizer-policy-").toAbsolutePath();
      for(var agent:List.of("claude","codex"))for(boolean policy:List.of(false,true)) {
        var result=compare(nativeCode,source,"\"real_http\":true,\"agent\":"+q(agent)+(policy?",\"policy_log\":"+q(root.resolve("optimizer.jsonl")):"")+",\"arguments\":{"+(agent.equals("codex")?"\"routing\":{\"base_url\":"+q(base+"/v1/")+",\"api_key\":\"fixture\"}":"\"base_url\":"+q(base+"/")+",\"api_key\":\"fixture\"")+"}");
        if(policy&&(!result.contains("\"optimizer_log\":[{")||!result.contains("\"value\":[\"local-")||result.contains("\"raised\"")))throw new AssertionError("HTTP response did not reach native pricing and logging: "+result);
        synchronized(received) {
          int size=received.size();if(size<2||!received.get(size-1).equals(received.get(size-2)))throw new AssertionError("Real request bytes or headers differ: "+received);
        }
      }
      Files.delete(root);
      synchronized(received){if(received.size()!=8)throw new AssertionError("Unexpected real HTTP request count: "+received.size());}
    } finally {server.stop(0);}
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"optimizer_http_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"optimizer_http_host.py")) {
      for(var agent:List.of("claude","codex")) {
        var a="\"agent\":"+q(agent);
        compare(nativeCode,source,a);
        compare(nativeCode,source,a+",\"opaque_effective\":true");
        compare(nativeCode,source,a+",\"mutate_log\":true");
        compare(nativeCode,source,a+",\"mutate_request\":true");
        compare(nativeCode,source,a+",\"mutate_usage\":{\"input_tokens\":123,\"output_tokens\":456}");
        for(var fail:List.of("urlopen","enter","read","exit","record","pricing","log","sleep"))for(var error:List.of("RuntimeError","ValueError","KeyboardInterrupt","SystemExit","ShadowException"))
          compare(nativeCode,source,a+",\"fail\":"+q(fail)+",\"error\":"+q(error));
        for(var code:List.of(400,401,429,500,502,503,504,529)) {
          compare(nativeCode,source,a+",\"replies\":[{\"http\":"+code+"}]");
          compare(nativeCode,source,a+",\"replies\":[{\"http\":"+code+"},{}]");
        }
        for(var error:List.of("OSError","TimeoutError","URLError","KeyboardInterrupt"))compare(nativeCode,source,a+",\"replies\":[{\"error\":"+q(error)+"},{}]");
        for(var body:List.of("", "bad","{\"usage\":", "NaN","null","[]","true","1","{}"))
          compare(nativeCode,source,a+",\"replies\":[{\"body\":"+q(body)+"}]");
        compare(nativeCode,source,a+",\"replies\":[{\"body\":\"bad\",\"suppress\":true}]");
        compare(nativeCode,source,a+",\"replies\":[{\"exit_error\":\"OSError\"},{\"body\":\"bad\",\"suppress\":true}]");
        compare(nativeCode,source,a+",\"replies\":[{\"bytes\":[255]},{}]");
        for(var errorBody:List.of("a'b", "a\"b'", "é😀".repeat(500),"line\nnext\t\u0000"))
          compare(nativeCode,source,a+",\"replies\":[{\"http\":400,\"error_body\":"+q(errorBody)+"}]");
      }
      for(var model:List.of("null","false","0","1","\"agent\"","\"claude-other\"","\"Claude-other\"","\"copilot\""))
        compare(nativeCode,source,"\"arguments\":{\"model\":"+model+"}");
      for(var auth:List.of("{\"api_key\":\"\",\"bearer\":\"fixture\"}","{\"api_key\":\"key\",\"bearer\":\"bearer\"}","{\"api_key\":\"\",\"bearer\":\"\"}"))
        compare(nativeCode,source,"\"arguments\":"+auth);
      compare(nativeCode,source,"\"objects\":[\"api_key\"]");
      compare(nativeCode,source,"\"arguments\":{\"api_key\":\"\"},\"objects\":[\"bearer\"]");
      for(var value:List.of("null","false","0","1","[]","{}","\"oops\"","[null,{\"type\":\"other\"},{\"type\":\"text\"},{\"type\":\"text\",\"text\":\"ok\"}]","[{\"type\":\"text\",\"text\":null}]"))
        compare(nativeCode,source,"\"replies\":[{\"data\":{\"content\":"+value+"}}]");
      compare(nativeCode,source,"\"replies\":[{\"data\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}],\"stop_reason\":\"max_tokens\"}}]");
      for(var data:List.of("null","[]","{}","{\"output_text\":\"direct é😀\"}","{\"output_text\":[null,1,\"x\",\"y\"]}","{\"output_text\":\" \n\",\"output\":[null,{\"content\":[{}, {\"text\":\"fallback\"}]}]}","{\"output\":true}","{\"output\":[{\"content\":42}]}"))
        compare(nativeCode,source,"\"mode\":\"text\",\"data\":"+data.replace("\n","\\n"));
      for(var value:List.of("null","false","0","\"3\"","\"١_٢\"","2.8","-0.2","\"bad\"","[]","[1]","{}"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"replies\":[{\"data\":{\"output_text\":\"ok\",\"usage\":{\"input_tokens\":"+value+",\"input_tokens_details\":{\"cached_tokens\":"+value+"},\"output_tokens\":"+value+"}}}]");
      for(var total:List.of("-10","0","10",q("7".repeat(650))))for(var cached:List.of("-20","0","20",q("6".repeat(650))))
        compare(nativeCode,source,"\"agent\":\"codex\",\"replies\":[{\"data\":{\"output_text\":\"ok\",\"usage\":{\"input_tokens\":"+total+",\"input_tokens_details\":{\"cached_tokens\":"+cached+"}}}}]");
      for(var value:List.of("NaN","Infinity","-Infinity","1e100","\"\\ud800\"",q("7".repeat(650))))
        compare(nativeCode,source,"\"agent\":\"codex\",\"int_max_digits\":640,\"replies\":[{\"data\":{\"output_text\":\"ok\",\"usage\":{\"output_tokens\":"+value+"}}}]");
      compare(nativeCode,source,"\"agent\":\"codex\",\"int_max_digits\":640,\"replies\":[{\"data\":{\"output_text\":\"ok\",\"usage\":{\"input_tokens\":"+q("7".repeat(640))+",\"input_tokens_details\":{\"cached_tokens\":"+q("-"+"6".repeat(640))+"}}}}]");
      for(var data:List.of("{\"status\":\"incomplete\",\"output_text\":\"ok\",\"incomplete_details\":{\"reason\":\"limit\"}}","{\"output_text\":\" \"}","{\"status\":\"incomplete\",\"output_text\":\"ok\"}","{\"output_text\":\"\\ud800\"}"))
        compare(nativeCode,source,"\"agent\":\"codex\",\"replies\":[{\"data\":"+data+"}]");
      compare(nativeCode,source,"\"agent\":\"codex\",\"mutate_response\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":3}}");
      compare(nativeCode,source,"\"agent\":\"codex\",\"mutate_raw_usage\":{\"output_tokens\":99}");
      localHttp(nativeCode,source);
    }
    System.out.println("Optimizer HTTP calls match the source in "+cases+" comparisons; exact requests, retries, response text, usage, observer mutations and real loopback HTTP passed without provider calls");
  }
}
