//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import com.sun.net.httpserver.HttpServer;

/** Compares provider policy, mutable mappings, HTTP failures and catalogue caching. */
class ProviderProbe {
  static int cases;
  static String q(Object value) {return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!expected.equals(actual))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  static String method(String agent,String name) {return "\"agent\":"+q(agent)+",\"name\":"+q(name);}
  static void http(AdapterProbe.Client nativeCode,AdapterProbe.Client source) throws Exception {
    var requests=new AtomicInteger();var server=HttpServer.create(new InetSocketAddress("127.0.0.1",0),0);
    server.createContext("/v1/models",exchange->{
      requests.incrementAndGet();
      if(!"Bearer fixture".equals(exchange.getRequestHeaders().getFirst("Authorization")))throw new AssertionError("Wrong credential header");
      var body="{\"data\":[{\"id\":\"é😀\"},{\"id\":\"model-a\"}]}".getBytes(StandardCharsets.UTF_8);
      exchange.sendResponseHeaders(200,body.length);try(var output=exchange.getResponseBody()){output.write(body);}
    });server.start();
    try {
      var args="{\"base_url\":"+q("http://127.0.0.1:"+server.getAddress().getPort()+"/v1/")+",\"api_key\":\"fixture\"}";
      compare(nativeCode,source,method("codex","fetch_catalog")+",\"real_http\":true,\"arguments\":"+args);
      if(requests.get()!=2)throw new AssertionError("The HTTP fixture did not receive both requests");
      compare(nativeCode,source,method("codex","fetch_catalog")+",\"real_http\":true,\"keep_cache\":true,\"assert_cached_identity\":true,\"arguments\":"+args);
      if(requests.get()!=2)throw new AssertionError("A cache hit repeated the HTTP request");
    } finally {server.stop(0);}
  }
  public static void main(String[] args) throws Exception {
    boolean standalone=args.length==1&&args[0].equals("--sdk");
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"provider_host.py",standalone?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"provider_host.py")) {
      for(var agent:List.of("claude","codex"))for(var key:List.of("null","false","0","\"\"","\"fixture\"","[]","{}","[1]"))for(boolean explicit:List.of(false,true))
        compare(nativeCode,source,method(agent,agent.equals("claude")?"resolve_anthropic_key":"resolve_codex_routing")+",\"explicit\":"+explicit+",\"env\":{"+q(agent.equals("claude")?"ANTHROPIC_API_KEY":"OPENAI_API_KEY")+":"+key+"}");
      for(var base:List.of("", " https://region/v1/// ","\u001c https://api/v1/\u001f","/","é😀"))for(var agent:List.of("claude","codex"))
        compare(nativeCode,source,method(agent,agent.equals("claude")?"resolve_anthropic_key":"resolve_codex_routing")+",\"env\":{\"ANTHROPIC_API_KEY\":\"fixture\",\"OPENAI_API_KEY\":\"fixture\","+q(agent.equals("claude")?"ANTHROPIC_BASE_URL":"OPENAI_BASE_URL")+":"+q(base)+"}");
      for(var truth:List.of(false,true))for(var agent:List.of("claude","codex"))
        compare(nativeCode,source,method(agent,agent.equals("claude")?"resolve_anthropic_key":"resolve_codex_routing")+",\"truth\":"+truth+",\"objects\":["+q(agent.equals("claude")?"ANTHROPIC_API_KEY":"OPENAI_API_KEY")+"]");
      compare(nativeCode,source,method("claude","agent_credential_env")+",\"opaque_key\":true,\"env\":{\"ANTHROPIC_BASE_URL\":\"base\",\"ANTHROPIC_AUTH_TOKEN\":\"token\",\"ANTHROPIC_DEFAULT_SONNET_MODEL\":\"model\",\"keep\":\"fixture\"}");
      compare(nativeCode,source,method("claude","resolve_anthropic_key")+",\"opaque_log_result\":true,\"env\":{\"ANTHROPIC_BASE_URL\":\"base\",\"ANTHROPIC_API_KEY\":\"key\"}");
      for(var fail:List.of("get","pop","set","log"))for(var error:List.of("KeyError","RuntimeError","KeyboardInterrupt","ShadowValueError")) {
        boolean resolving=fail.equals("get")||fail.equals("log");
        compare(nativeCode,source,method("claude",resolving?"resolve_anthropic_key":"agent_credential_env")+(resolving?"":",\"arguments\":{\"api_key\":\"fixture\"}")+",\"env\":{\"ANTHROPIC_BASE_URL\":\"base\",\"ANTHROPIC_API_KEY\":\"key\"},\"fail\":"+q(fail)+",\"error\":"+q(error));
      }
      for(int mask=0;mask<8;mask++)for(var name:List.of("auth_env","byok_active","provider_base","provider_env","provider_secret_names","anthropic_optimizer_endpoint")) {
        var env="{\"COPILOT_PROVIDER_BASE_URL\":"+q((mask&1)==0?"":"http://provider")+",\"COPILOT_PROVIDER_TYPE\":\"AnThRoPiC\",\"COPILOT_PROVIDER_API_KEY\":"+q((mask&2)==0?"":"key")+",\"COPILOT_PROVIDER_BEARER_TOKEN\":"+q((mask&4)==0?"":"bearer")+",\"GH_TOKEN\":\"gh\",\"GITHUB_TOKEN\":\"github\",\"COPILOT_PROVIDER_MODEL_ID\":\"model\"}";
        compare(nativeCode,source,method("copilot",name)+",\"env\":"+env);
      }
      for(var kind:List.of("", "openai","ANTHROPIC"," anthropic ","é😀"))compare(nativeCode,source,method("copilot","anthropic_optimizer_endpoint")+",\"explicit\":true,\"env\":{\"COPILOT_PROVIDER_BASE_URL\":\"base\",\"COPILOT_PROVIDER_TYPE\":"+q(kind)+",\"COPILOT_PROVIDER_API_KEY\":\"key\"}");
      for(var name:List.of("provider_env","provider_secret_names","auth_env","anthropic_optimizer_endpoint"))for(var error:List.of("KeyError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,method("copilot",name)+",\"env\":{\"COPILOT_PROVIDER_BASE_URL\":\"base\"},\"fail\":\"get\",\"error\":"+q(error));
      for(var model:List.of("null","false","0","1","[]","{}","[1]","\" auto \"","\"AUTO\"","\" model-é😀 \"",q("\u001cAUTO\u001f")))for(var name:List.of("normalize_model","effort_applies"))for(var applies:List.of("0","1"))
        compare(nativeCode,source,method("copilot",name)+",\"arguments\":{\"model\":"+model+"},\"env\":{\"TE_EFFORT_APPLIES\":"+q(applies)+"}");
      for(var name:List.of("normalize_model","effort_applies"))compare(nativeCode,source,method("copilot",name)+",\"model_bytes\":[32,65,85,84,79,32]");
      for(var agent:List.of("copilot","codex"))for(var error:List.of("","ValueError","RuntimeError","KeyboardInterrupt"))
        compare(nativeCode,source,method(agent,"validate_priceable")+",\"arguments\":{\"model\":\"fixture\",\"agent\":"+q(agent)+"}"+(error.isEmpty()?"":",\"fail\":\"pricing\",\"error\":"+q(error)));
      var fetch=method("codex","fetch_catalog")+",\"arguments\":{\"base_url\":\"http://fixture/v1///\",\"api_key\":\"fixture\"}";
      for(var body:List.of("", "bad","null","[]","{}","{\"data\":{}}","{\"data\":[{},null,3,{\"id\":null},{\"id\":3},{\"id\":\"\"},{\"id\":\"é😀\"},{\"id\":\"é😀\"}]}","{\"metadata\":NaN,\"data\":[{\"id\":\"a\"}]}","{\"data\":[{\"id\":\"\\ud800\"}]}"))
        compare(nativeCode,source,fetch+",\"body\":"+q(body));
      for(int depth:List.of(150,1000,5000))compare(nativeCode,source,fetch+",\"body\":"+q("{\"data\":[{\"id\":\"a\"}],\"unused\":"+"[".repeat(depth)+"0"+"]".repeat(depth)+"}"));
      for(int limit:List.of(0,640))for(int digits:List.of(640,641,650))compare(nativeCode,source,fetch+",\"int_max_digits\":"+limit+",\"body\":"+q("{\"data\":[{\"id\":\"a\"}],\"unused\":"+"7".repeat(digits)+"}"));
      for(var fail:List.of("cache.contains","cache.set","request","urlopen","enter","read","exit"))for(var error:List.of("URLError","OSError","ValueError","TypeError","RuntimeError","KeyboardInterrupt","ShadowValueError"))
        compare(nativeCode,source,fetch+",\"fail\":"+q(fail)+",\"error\":"+q(error));
      compare(nativeCode,source,fetch+",\"bytes\":[255]");
      compare(nativeCode,source,fetch+",\"body\":\"bad\",\"suppress\":true");
      compare(nativeCode,source,fetch);
      compare(nativeCode,source,fetch+",\"keep_cache\":true,\"assert_cached_identity\":true,\"body\":\"bad\"");
      for(var model:List.of("\"a\"","\"missing\"","null","[]"))for(var body:List.of("bad","{\"data\":[{\"id\":\"z\"},{\"id\":\"a\"}]}"))
        compare(nativeCode,source,method("codex","verify_model")+",\"arguments\":{\"model\":"+model+",\"routing\":{\"base_url\":\"http://fixture\",\"api_key\":\"fixture\"}},\"body\":"+q(body));
      var many="{\"data\":["+String.join(",",java.util.stream.IntStream.range(0,25).mapToObj(i->"{\"id\":"+q("model-"+(25-i))+"}").toList())+"]}";
      compare(nativeCode,source,method("codex","verify_model")+",\"arguments\":{\"model\":\"missing\",\"routing\":{\"base_url\":\"http://fixture\",\"api_key\":\"fixture\"}},\"body\":"+q(many));
      for(var model:List.of("\"missing\"","\"\\ud800\""))
        compare(nativeCode,source,method("codex","verify_model")+",\"arguments\":{\"model\":"+model+",\"routing\":{\"base_url\":\"http://fixture\",\"api_key\":\"fixture\"}},\"body\":"+q("{\"data\":[{\"id\":\"\\ud800\"},{\"id\":\"é😀\"}]}"));
      http(nativeCode,source);
      for(var agent:List.of("copilot","codex"))for(var model:List.of("gpt-5.6-sol","claude-sonnet-5","unknown-fixture-model","unknown'fixture","unknown\"fixture","é😀 unknown\nfixture"))
        compare(nativeCode,source,method(agent,"validate_priceable")+",\"real_pricing\":true,\"arguments\":{\"model\":"+q(model)+",\"agent\":"+q(agent)+"}");
    }
    System.out.println((standalone?"Standalone SDK (reference imports forbidden): ":"")+"Provider policy matches the source in "+cases+" comparisons, including real local HTTP, cache identity, mapping mutations, credentials and catalogue failures");
  }
}
