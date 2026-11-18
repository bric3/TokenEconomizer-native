//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares evaluator HTTP parsing, rendezvous, response bytes and failure boundaries. */
class WebHttpProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static String compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;return actual;
  }
  public static void main(String[] args) throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_http_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_http_host.py")) {
      compare(nativeCode,source,"");
      for(var length:List.of("null","false","true","0","-1","1","1.5","NaN","Infinity","\"\"","\"0\"","\"-1\"","\"1\"","\"bad\"","\"١٢\"","[]","{}","[1]"))compare(nativeCode,source,"\"length\":"+length);
      for(var doc:List.of(""," ","{}","null","true","false","0","1.5","NaN","Infinity","[]","[1]","\"x\"","bad","{","{\"a\":}","{\"é\":\"😀\"}","{\"raw\":\"\\ud800\\udc00\"}"))compare(nativeCode,source,"\"document\":"+q(doc));
      for(var bytes:List.of("[255]","[239,187,191,123,125]","[240,159,152]"))compare(nativeCode,source,"\"raw\":"+bytes);
      for(var payload:List.of("null","false","0","1.5","NaN","Infinity","\"é😀\"","[]","{}","{\"files\":{\"a\":\"é😀\"}}"))compare(nativeCode,source,"\"payload\":"+payload);
      for(var options:List.of("\"opaque_length\":true","\"opaque_length\":true,\"mutate_integer\":true","\"opaque_raw\":true","\"opaque_raw\":true,\"opaque_document\":true","\"opaque_request\":true","\"opaque_encoded\":true","\"opaque_encoded\":true,\"opaque_body\":true,\"mutate_length\":true","\"mutate_loads\":true","\"opaque_payload\":true","\"opaque_payload\":true,\"opaque_encoded\":true","\"empty\":true","\"empty\":true,\"opaque_timeout\":true,\"mutate_log\":true","\"empty\":true,\"timeouts\":[10,20]"))compare(nativeCode,source,options);
      for(var length:List.of(0,-1,1))compare(nativeCode,source,"\"opaque_encoded\":true,\"opaque_body\":true,\"body_length\":"+length);
      for(int limit:List.of(0,640))compare(nativeCode,source,"\"int_max_digits\":"+limit+",\"document\":"+q("{\"n\":"+"7".repeat(650)+"}") );
      for(int limit:List.of(0,640))compare(nativeCode,source,"\"int_max_digits\":"+limit+",\"length\":"+q("7".repeat(650)));
      compare(nativeCode,source,"\"empty\":true,\"fail\":\"dumps\",\"exception_state\":true");
      for(var failure:List.of("member:headers","headers.get","member:rfile","read","loads"))for(var error:List.of("RuntimeError","OSError","ValueError","KeyboardInterrupt","DetailError","ShadowException"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error)+",\"exception_state\":true");
      for(var failure:List.of("queue","member:session","session.requests","requests.put","reply.get_function","session.reply_timeout","reply.get","dumps","member:send_response","send_response","member:send_header","send_header","member:end_headers","end_headers","member:wfile","write"))for(var error:List.of("Empty","SilentEmpty","ShadowEmpty","BrokenPipeError","ConnectionResetError","ShadowBrokenPipeError","RuntimeError","KeyboardInterrupt","DetailError"))
        compare(nativeCode,source,"\"fail\":"+q(failure)+",\"error\":"+q(error)+",\"exception_state\":true");
      for(var failure:List.of("member:send_error","send_error"))compare(nativeCode,source,"\"document\":\"bad\",\"fail\":"+q(failure)+",\"error\":\"RuntimeError\",\"exception_state\":true");
      compare(nativeCode,source,"\"fail\":\"read\",\"error\":\"DetailError\",\"mutate_error_formatter\":true,\"exception_state\":true");
      compare(nativeCode,source,"\"fail\":\"read\",\"error\":\"DetailError\",\"fail_format\":true,\"exception_state\":true");
      compare(nativeCode,source,"\"fail\":\"read\",\"error\":\"DetailError\",\"string_points\":true,\"error_text\":"+q("é😀\ud800\udc00"));
      for(var failure:List.of("member:session","session.reply_timeout","format:timeout","log"))compare(nativeCode,source,"\"empty\":true,\"opaque_timeout\":true,\"fail\":"+q(failure)+",\"fail_at\":"+(failure.equals("member:session")?3:failure.equals("session.reply_timeout")?2:1)+",\"exception_state\":true");
      compare(nativeCode,source,"\"fail\":\"write\",\"error\":\"BrokenPipeError\",\"exception_state\":true,\"mutate_log\":true");
      for(var error:List.of("RuntimeError","KeyboardInterrupt")) {
        compare(nativeCode,source,"\"fail\":\"write\",\"error\":\"BrokenPipeError\",\"exception_state\":true,\"log_error\":"+q(error));
        compare(nativeCode,source,"\"empty\":true,\"exception_state\":true,\"log_error\":"+q(error));
      }
      for(var input:List.of("[]","[1,null,{}]","[\"%s\",\"message\"]"))compare(nativeCode,source,"\"quiet\":true,\"arguments\":"+input);
      for(var fields:List.of("","\"empty\":true","\"document\":\"bad\"","\"raw\":[255]","\"length\":\"bad\"","\"payload\":null","\"payload\":{\"files\":{\"é\":\"😀\"}}")) {
        var result=compare(nativeCode,source,"\"socket\":true"+(fields.isEmpty()?"":","+fields));
        int expected=fields.contains("document")||fields.contains("raw")||fields.contains("length")?400:200;
        if(!result.contains("\"status\":"+expected))throw new AssertionError("real HTTP response failed: "+result);
      }
    }
    System.out.println("Web-Bench HTTP handling matches the source in "+cases+" comparisons; request parsing, raw queue identities, timeouts, exact responses, exception state and local HTTP requests passed");
  }
}
