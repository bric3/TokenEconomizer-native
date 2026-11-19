//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

/** Compares number and reserved-key handling through actual configuration JSON reads. */
class NumberCodecProbe {
  static String q(Object value){return ParserProbe.q(value);}
  static final List<String> DOCUMENTS=new ArrayList<>();
  public static void main(String[] args)throws Exception {
    for(var token:List.of("0","-0","1","-1","9007199254740993","18446744073709551615",
        "18446744073709551616","-9223372036854775809","170141183460469231731687303715884105727",
        "170141183460469231731687303715884105728","1"+"0".repeat(100),"9".repeat(1000),
        "0.0","-0.0","1.0","1e0","1e2","1.00e+2","0.00001","1e-5","1e+20","1.2345678901234567",
        "5e-324","1e309","-1e9999","0e9999","NaN","Infinity","-Infinity"))
      DOCUMENTS.add("{\"value\":"+token+",\"nested\":["+token+",{\"number\":"+token+"}],\"text\":"+q(token)+"}");
    for(var key:List.of("$serde_json::private::Number","$serde_json::private::RawValue"))
      for(var value:List.of("\"17\"","\"1e2\"","\"{\\\"keep\\\":1}\"","17","null","true","[]","{}")) {
        DOCUMENTS.add("{"+q(key)+":"+value+"}");
        DOCUMENTS.add("{\"nested\": {"+q(key)+":"+value+",\"keep\":1}}");
        DOCUMENTS.add("{"+q(key)+":"+value+",\"keep\":1}");
      }
    var random=new Random(0x5e4de151L);
    for(int i=0;i<512;i++) {
      var token=Double.toString(Double.longBitsToDouble(random.nextLong()));
      DOCUMENTS.add("{\"value\":"+token+",\"paid_model\":"+token+"}");
    }
    for(int digits:List.of(19,20,21,38,39,40,100,1000))for(var sign:List.of("","-")) {
      var token=sign+"9".repeat(digits);
      DOCUMENTS.add("{\"value\":"+token+",\"paid_model\":"+token+"}");
    }
    var checkout=Path.of("").toAbsolutePath();
    var root=Files.createTempDirectory(checkout.resolve("validation/results"),"number-codec-");
    int checks=0,failures=0;boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(checkout,true,"number_codec_host.py");
        var source=new AdapterProbe.Client(checkout,false,"number_codec_host.py")) {
      int index=0;
      for(var document:DOCUMENTS) {
        var directory=Files.createDirectory(root.resolve("case-"+index++));
        var path=directory.resolve("config.json");Files.writeString(path,document);
        var request="{\"method\":\"read\",\"path\":"+q(path)+"}";
        var expected=source.call(request);var actual=nativeCode.call(request);checks++;
        if(!actual.equals(expected)){if(failures<12)System.out.println(document+"\nNative: "+actual+"\nSource: "+expected);failures++;}
      }
      for(var token:List.of("1e0","1e2","1.00e+2","0.00001","1e-5","1e+20","-0","-0.0","NaN","Infinity","-Infinity")) {
        var directory=Files.createDirectory(root.resolve("case-"+index++));
        Files.writeString(directory.resolve("config.json"),"{\"paid_model\":"+token+"}");
        var request="{\"method\":\"load\",\"root\":"+q(directory)+"}";
        var expected=source.call(request);var actual=nativeCode.call(request);checks++;
        if(!actual.equals(expected)){if(failures<12)System.out.println("paid_model="+token+"\nNative: "+actual+"\nSource: "+expected);failures++;}
      }
      if(failures!=0)throw new AssertionError(failures+" mismatches in "+checks+" JSON number/key cases");
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("JSON number and key codecs match source in "+checks+" file-read and setting-conversion cases");
  }
}
