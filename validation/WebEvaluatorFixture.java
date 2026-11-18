//JAVA 25

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.regex.Pattern;

/** Offline evaluator stand-in for configuration, HTTP rendezvous and stdout checks. */
class WebEvaluatorFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=5||!args[1].equals("--auto-run")||!args[2].equals("--config")||!args[4].equals("--without-local-config"))throw new AssertionError("wrong evaluator arguments");
    if(!Path.of("").toRealPath().equals(Path.of(args[0]).getParent().getParent().toRealPath()))throw new AssertionError("wrong evaluator directory");
    if(!"1".equals(System.getenv("NO_COLOR"))||!"0".equals(System.getenv("FORCE_COLOR")))throw new AssertionError("colour environment missing");
    var config=Files.readString(Path.of(args[3]));
    var endpoint=Pattern.compile("http://127[.]0[.]0[.]1:[0-9]+/message").matcher(config);
    if(!endpoint.find())throw new AssertionError("missing endpoint");
    var range=Pattern.compile("\"startTask\"\\s*:\\s*\"([^\"]+)\"").matcher(config);
    var task=range.find()?range.group(1):"init";
    System.out.println("[project] "+task+" 1st Attempt Started");
    System.out.flush();
    var body="{\"type\":\"task\",\"task\":\""+task+"\",\"files\":{}}";
    var call=HttpRequest.newBuilder(URI.create(endpoint.group())).timeout(Duration.ofSeconds(5)).POST(HttpRequest.BodyPublishers.ofString(body)).build();
    var reply=HttpClient.newHttpClient().send(call,HttpResponse.BodyHandlers.ofString());
    if(reply.statusCode()!=200||!reply.body().contains("fixture answer"))throw new AssertionError("wrong agent reply: "+reply.body());
    System.out.println("[project] "+task+" 1st Attempt ✅");
    System.exit(0);
  }
}
