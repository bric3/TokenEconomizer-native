//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Checks source summation order for mixed integer/float mechanism overheads. */
class LedgerSumProbe {
  static final List<String> KEYS = List.of("optimizer_tokens", "untool_classify_tokens", "untool_judge_tokens",
      "codex_untool_adapt_tokens", "spiral_judge_tokens", "spiral_nudge_writer_tokens");
  static final List<String> CASES = new ArrayList<>();
  static void row(List<String> values) {
    var fields = new ArrayList<String>();
    for (int i=0;i<values.size();i++) fields.add(ParserProbe.q(KEYS.get(i))+":"+values.get(i));
    var row = "{"+String.join(",",fields)+"}";
    CASES.add("{\"op\":\"ledger.overhead\",\"args\":{\"row\":"+row+"}}");
    CASES.add("{\"op\":\"accounting.overhead_total\",\"args\":{\"overhead\":"+row+"}}");
  }
  public static void main(String[] args) throws Exception {
    var large = List.of("9007199254740993", "9007199254740995", "-9007199254740993", "-9007199254740995",
        "9223372036854775807", "-9223372036854775808", "18446744073709551615");
    for (var integer : large) for (var small : List.of("1", "-1", "true", "false", "null", "\"ignored\"", "[]", "{}")) {
      for (var floating : List.of("0.0", "1.0", "-1.0", "0.25", "-0.25", "1e20", "-1e20")) {
        row(List.of(integer, small, floating, "-9007199254740992.0", "1.0", "0"));
        row(List.of(integer, small, "-9007199254740992", floating, "1", "0"));
        row(List.of(floating, integer, small, "-9007199254740992", "1", "0"));
        row(List.of(integer, floating, small, "-9007199254740992.0", "1", "0"));
      }
    }
    for (var values : List.of(
        List.of("9007199254740993", "-9007199254740992", "0.0"),
        List.of("9007199254740992", "1.0", "-9007199254740992.0"),
        List.of("9223372036854775807", "1", "1e20", "3.14", "-1e20", "0"),
        List.of("9223372036854775807", "1e20", "3.14", "-1e20", "0", "0"),
        List.of("18446744073709551615", "-9223372036854775808", "-9223372036854775808", "0.0", "0", "0"),
        List.of("2147483647", "1", "1e20", "3.14", "-1e20", "0"),
        List.of("2147483648", "1e20", "3.14", "-1e20", "0", "0"),
        List.of("0.0", "1e100", "1.0", "-1e100", "0", "0"))) row(values);
    int failures=0;
    var checkout=Path.of("").toAbsolutePath();
    try (var nativeCode=new AdapterProbe.Client(checkout,true,"ledger_sum_host.py");
         var source=new AdapterProbe.Client(checkout,false,"ledger_sum_host.py")) {
      for(int index=0;index<CASES.size();index+=50) {
        var batch=CASES.subList(index,Math.min(index+50,CASES.size()));
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
    if(failures!=0)throw new AssertionError(failures+" mismatches in "+CASES.size()+" mixed ledger sum cases");
    System.out.println("Ledger and accounting overheads match source floats in "+CASES.size()+" mixed numeric cases; integer prefixes, cancellation and machine-integer overflow transitions passed");
  }
}
