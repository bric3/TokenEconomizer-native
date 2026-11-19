//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Random;

/** Compares complete ledger summaries, preserving integer types and source float results. */
class AccountingSumProbe {
  static final List<String> CASES = new ArrayList<>();
  static String record(String kind, String effective, String overhead, String extra) {
    return "{\"kind\":"+ParserProbe.q(kind)+",\"effective_tokens\":"+effective+",\"overhead\":"+overhead
        +",\"requests\":1,\"nudges\":2,\"aborts\":3,\"turns\":4"+extra+"}";
  }
  static void records(List<String> records) {
    CASES.add("{\"op\":\"accounting.summarize\",\"args\":{\"records\":["+String.join(",",records)+"]}}");
  }
  static void sequence(List<String> values) {
    var execution = new ArrayList<String>();
    var overhead = new ArrayList<String>();
    var combined = new ArrayList<String>();
    for (int i=0;i<values.size();i++) {
      var value=values.get(i);
      var cost="{\"optimizer_tokens\":"+value+"}";
      execution.add(record("session",value,"{}",""));
      overhead.add(record(i%2==0?"session":"overhead","0",cost,""));
      combined.add(record("session",value,cost,",\"injected_chars\":1"));
    }
    records(execution); records(overhead); records(combined);
  }
  public static void main(String[] args) throws Exception {
    for (var values : List.of(
        List.<String>of(), List.of("0"), List.of("1"), List.of("true", "false"),
        List.of("18446744073709551615", "18446744073709551615"),
        List.of("-9223372036854775808", "-9223372036854775808"),
        Collections.nCopies(6, "18446744073709551615"),
        List.of("-1"), List.of("-7"), List.of("-9223372036854775808"), List.of("1", "-2"),
        List.of("0.0"), List.of("-0.0"), List.of("0.0", "1e100", "1.0", "-1e100"),
        List.of("9007199254740993", "-9007199254740992", "0.0", "3"),
        List.of("18446744073709551615", "-9223372036854775808", "-9223372036854775808", "0.0", "3"),
        List.of("9223372036854775807", "1", "1e20", "3.14", "-1e20"),
        List.of("9007199254740993", "1", "-9007199254740992"),
        List.of("9007199254740993", "0.25", "-9007199254740992.0"),
        List.of("1.0", "9007199254740993", "-9007199254740992"))) sequence(values);
    for (var execution : List.of("1", "7", "9007199254740991", "9007199254740993", "9223372036854775807", "-1", "-7", "-9007199254740993"))
      for (var overhead : List.of("0", "1", "3", "9007199254740993", "9223372036854775807", "-1", "-7", "-9007199254740993"))
        records(List.of(record("session",execution,"{\"optimizer_tokens\":"+overhead+"}","")));
    var random=new Random(537829);
    for(int trial=0;trial<400;trial++) {
      var values=new ArrayList<String>();
      for(int i=0;i<8;i++) {
        long count=random.nextLong(1L<<52,1L<<56);
        if((trial+i)%3==0) values.add(Double.toString(Math.scalb(random.nextDouble(-1,1),random.nextInt(110))));
        else values.add(Long.toString((i%2==0?1:-1)*count));
      }
      sequence(values);
    }
    for(int trial=0;trial<400;trial++) {
      long execution=random.nextLong(1,Long.MAX_VALUE)*(trial%2==0?1:-1);
      long overhead=random.nextLong(1,Long.MAX_VALUE)*(trial%3==0?1:-1);
      records(List.of(record("session",Long.toString(execution),"{\"optimizer_tokens\":"+overhead+"}","")));
    }
    for(var imported:List.of("true","1","\"yes\"","[1]","{}","false","0","null"))
      records(List.of("null", "1", "[]", record("session","17","{\"optimizer_tokens\":31}",",\"imported\":"+imported),
          record("overhead","23","{\"optimizer_tokens\":47}",",\"imported\":"+imported),
          record("other","29","{\"optimizer_tokens\":53}",""),record("session","true","{}","")));
    records(List.of(record("session","1","{\"optimizer_tokens\":9007199254740993}",""),
        record("overhead","0","{\"optimizer_tokens\":-9007199254740992}",""),
        record("overhead","0","{\"optimizer_tokens\":9007199254740993,\"untool_classify_tokens\":-9007199254740992,\"untool_judge_tokens\":0.0}","")));
    int failures=0;
    var checkout=Path.of("").toAbsolutePath();
    try(var nativeCode=new AdapterProbe.Client(checkout,true,"ledger_sum_host.py");
        var source=new AdapterProbe.Client(checkout,false,"ledger_sum_host.py")) {
      for(int index=0;index<CASES.size();index+=30) {
        var batch=CASES.subList(index,Math.min(index+30,CASES.size()));
        var request="["+String.join(",",batch)+"]";
        var actual=nativeCode.call(request);var expected=source.call(request);
        if(!actual.equals(expected))for(var item:batch) {
          actual=nativeCode.call("["+item+"]");expected=source.call("["+item+"]");
          if(!actual.equals(expected)) {
            if(failures<12)System.out.println(item+"\nNative: "+actual+"\nSource: "+expected);
            failures++;
          }
        }
      }
    }
    if(failures!=0)throw new AssertionError(failures+" mismatches in "+CASES.size()+" session accounting cases");
    System.out.println("Session accounting matches the source in "+CASES.size()+" complete summaries; integer types, compensated sums, overhead fractions, imported records and record filtering passed");
  }
}
