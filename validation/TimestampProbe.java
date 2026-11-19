//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

/** Exercises the original ISO timestamp contracts, including local-time transitions. */
class TimestampProbe {
  static int checks,failures;
  static String q(Object value){return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String host,String input)throws Exception {
    var request="{\"host\":"+q(host)+",\"args\":{\"ts\":"+input+"}}";
    var expected=source.call(request);var actual=nativeCode.call(request);checks++;
    if(!actual.equals(expected)) {
      if(failures<40)System.out.println("Mismatch "+request+"\nNative: "+actual+"\nSource: "+expected);
      failures++;
    }
  }
  static List<String> syntax() {
    var cases=new ArrayList<>(List.of("", "bad", "2025", "2025-01", "2025-001", "0000-01-01", "10000-01-01", "2025-02-29", "2024-02-29", "2025-13-01", "2025-00-01", "2025-01-32", "2025-W00-1", "2025-W54-1", "2025-W01-0", "2025-W01-8", "2025-W01", "2025W01", "2025-W01-1", "2025W011", "2025W017", "20250101", "2025-01-01", "0001-01-01", "9999-12-31", " 2025-01-01", "2025-01-01 ", "2025-01-01T", "2025-01-01Z", "2025-01-01+00:00", "2025-01-01T1:00", "2025-01-01T01:2", "2025-01-01T01:02:3", "2025-01-01T23:59:60", "2025-01-01T24:00", "2025-01-01T24:00:01", "2025-01-01T24:00:00.000001", "9999-12-31T24:00:00", "2025-01-01T12.5", "2025-01-01T12:34.5", "2025-01-01T12:34:56.", "2025-01-01T12:34:56.123456789", "2025-01-01T12:34:56,25", "2025-01-01T12:34:56z", "2025-01-01T12:34:56+24:00", "2025-01-01T12:34:56+00:60", "2025-01-01T12:34:56+00:00:60", "2025-01-01T12:34:56+00:00:00.5", "2025-01-01T12:34:56-00:00:00.5", "2025-01-01T12:34:56+01:02:03.456789", "2025-01-01T12:34:56+01.5", "2025-01-01T12:34:56+01:02.5", "2025-01-01T12:34:56\u0000", "2025-01-01T12:34:56+01:00junk"));
    for(var date:List.of("2025-01-01","20250101","2025-W01-1","2025W011","2025-W01","2025W01"))
      for(var separator:List.of("T"," ","t","_","é","😀","\n","\u0000","0","+","-","Z"))
        for(var time:List.of("12","1234","12:34","123456","12:34:56","12:34:56.123456"))
          cases.add(date+separator+time+"+01:00");
    for(var offset:List.of("Z","+00","-00","+0000","+00:00","+00:00:00.000001","-00:00:00.999999","+01","-01","+01:23","-0123","+012345","+01:23:45.123456789","-01:23:45,25","+23:59:59.999999","+1","+001","+01:","+01:2","+01:23:","+01:23:4","+00:61:00","+00:00:61","+24:00:00","Z+01","+01Z","-00.5"))
      cases.add("2025-01-01T12:34:56.654321"+offset);
    for(var date:List.of("0001-01-01","0001-01-02","1900-01-01","1969-12-31","1970-01-01","2255-06-01","9999-12-31"))
      for(var fraction:List.of("000001","000009","000010","123456","500001","999999"))
        for(var offset:List.of("Z","+01:23:45.123456","-23:59:59.999999",""))cases.add(date+"T00:00:00."+fraction+offset);
    return cases;
  }
  static List<String> mutations() {
    var cases=new ArrayList<String>();
    for(var value:List.of("2025-01-00T24:00:00Z","0000-12-31T24:00:00+00:00","9999-W52-7T00:00Z","2025-01-01T12:34:56.123456\u0000junk","2025-01-01T12:34:56Z\u0000junk","2025-W01-1200","2025W011200","2025W0111200","2025-01-01T12:34:56."+"9".repeat(10_000)+"+00:00"))cases.add(value);
    var seeds=List.of("2025-01-01T12:34:56.123456+01:00","20250101T123456Z","2025-W01-1 12:34","2025W01😀12:34:56Z","2025-01-01","2024-02-29T24:00Z","0001-01-01T00:00:00+00:00","9999-12-31T23:59:59.999999-23:59:59.999999");
    var fragments=List.of("0","1","9","-","+","W","w","T","Z","z",":",".",","," ","é","😀","\u0000","\n");
    var random=new Random(0x71ae57L);
    for(int i=0;i<5000;i++) {
      var value=new StringBuilder(seeds.get(random.nextInt(seeds.size())));var boundaries=value.toString().codePoints().mapToObj(code->new String(Character.toChars(code))).toList();
      var parts=new ArrayList<>(boundaries);int position=random.nextInt(parts.size()+1);
      switch(random.nextInt(4)) {
        case 0 -> parts.add(position,fragments.get(random.nextInt(fragments.size())));
        case 1 -> {if(position<parts.size())parts.remove(position);}
        case 2 -> {if(position<parts.size())parts.set(position,fragments.get(random.nextInt(fragments.size())));}
        default -> parts.subList(position,parts.size()).clear();
      }
      cases.add(String.join("",parts));
    }
    return cases;
  }
  static void batch(AdapterProbe.Client nativeCode,AdapterProbe.Client source,List<String> inputs)throws Exception {
    for(int start=0;start<inputs.size();start+=100) {
      var values=inputs.subList(start,Math.min(inputs.size(),start+100));
      var request="{\"batch\":"+ParserProbe.array(values)+"}";
      var expected=source.call(request);var actual=nativeCode.call(request);checks+=values.size();
      if(!actual.equals(expected)) {
        var left=actual.substring(1,actual.length()-1).split(",");var right=expected.substring(1,expected.length()-1).split(",");
        for(int i=0;i<values.size();i++)if(!left[i].equals(right[i])) {
          if(failures<40)System.out.println("Mutation "+q(values.get(i))+"\nNative: "+left[i]+"\nSource: "+right[i]);failures++;
        }
      }
    }
  }
  static void traces(AdapterProbe.Client nativeCode,AdapterProbe.Client source,Path root)throws Exception {
    var path=root.resolve("session.jsonl");
    for(var timestamp:List.of("2025-W01-1é12:34:56.123456+00:00:00.5","2025-01-01T24:00Z","1969-12-31T23:59:59.999999Z","2025-03-09T02:30","2025-01-01T12:34:56.123456\u0000junk")) {
      for(var host:List.of("claude","droid")) {
        if(host.equals("claude"))Files.writeString(path,"{\"sessionId\":\"owned\",\"cwd\":\"/repo\",\"entrypoint\":\"cli\",\"timestamp\":\"invalid\"}\n{\"timestamp\":"+q(timestamp)+"}\n");
        else Files.writeString(path,"{\"type\":\"session_start\",\"id\":\"owned\",\"cwd\":\"/repo\"}\n{\"type\":\"message\",\"timestamp\":"+q(timestamp)+"}\n");
        var request="{\"host\":"+q(host)+",\"method\":"+q(host+"_log_head")+",\"args\":{\"path\":"+q(path)+"}}";
        var expected=source.call(request);var actual=nativeCode.call(request);checks++;
        if(!expected.equals(actual)) {System.out.println("Trace head "+request+"\nNative: "+actual+"\nSource: "+expected);failures++;}
        if(host.equals("droid")) {
          Files.setLastModifiedTime(path,FileTime.fromMillis(1735689600123L));
          request="{\"host\":\"droid\",\"method\":\"droid_sessions\",\"args\":{\"sessions_root\":"+q(root)+"}}";
          expected=source.call(request);actual=nativeCode.call(request);checks++;
          if(!expected.equals(actual)) {System.out.println("Session discovery "+request+"\nNative: "+actual+"\nSource: "+expected);failures++;}
        }
      }
    }
  }
  public static void main(String[] args)throws Exception {
    var checkout=Path.of("").toAbsolutePath();
    var root=Files.createTempDirectory(checkout.resolve("validation/results"),"timestamps-");
    for(var zone:List.of("UTC","America/New_York","Europe/Paris","Australia/Lord_Howe","Pacific/Apia")) {
      int before=failures;
      try(var source=new AdapterProbe.Client(checkout,false,"time_host.py","--tz="+zone);var nativeCode=new AdapterProbe.Client(checkout,true,"time_host.py","--tz="+zone)) {
        for(var input:List.of("null","false","true","0","42","[]","{}"))for(var host:List.of("claude","droid"))compare(nativeCode,source,host,input);
        for(var value:syntax())for(var host:List.of("claude","droid"))compare(nativeCode,source,host,q(value));
        for(var date:List.of("2025-03-09","2025-11-02","2025-03-30","2025-10-26","2025-04-06","2025-10-05","2011-12-30","1945-08-14","1916-06-14"))
          for(var time:List.of("00:00","01:30","02:00","02:15","02:30","03:00","12:00","23:59"))compare(nativeCode,source,"claude",q(date+"T"+time));
        batch(nativeCode,source,mutations());traces(nativeCode,source,root);
      }
      System.out.println(zone+": "+(failures-before)+" mismatches");
    }
    if(failures>0)throw new AssertionError(failures+" timestamp mismatches in "+checks+" source comparisons");
    ContainerProbe.clear(root);
    System.out.println("Verified "+checks+" timestamp source comparisons across five local time zones");
  }
}
