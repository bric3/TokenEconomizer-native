//JAVA 25
//SOURCES ../scripts/UnicodeRegexTables.java RegexProbe.java AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;
import java.util.ArrayList;
import java.util.TreeMap;
import java.util.TreeSet;

/** Compares scoped backreferences, lookbehinds and backtracking with Python. */
class BackreferenceProbe {
  static long searches;
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String pattern,List<String> texts)throws Exception {
    searches+=texts.size();RegexProbe.compare(nativeCode,source,pattern,texts);
  }
  public static void main(String[] args)throws Exception {
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"regex_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"regex_host.py")) {
      var texts=List.of("","aa","aA","SS","Sſ","ſS","Σς","Σσ","ςσ","Ii","İi","ıi","KK","KK","éÉ","éé","aéAé","😀😀",new String(Character.toChars(0x16ea0))+new String(Character.toChars(0x16ebb)),"aAéÉ","aAéé","éÉaA","ééaA","aA aA","\naa","xxaa","aaab","aab","abAB","abab","aaAA","aaaa","bB","aa\naa");
      for(var pattern:List.of("(?i)(.)\\1","(?ai)(.)\\1","(?i)(.+)\\1","(?ai)(.+)\\1","(?ai)(.)\\1(?ui:(.)\\2)","(?i)(.)\\1(?ai:(.)\\2)","(?ai)(?P<a>.+)(?P=a)(?ui:(?P<b>.+)(?P=b))","(?i)(.)(?-i:\\1)","(?i)(.)?(?(1)\\1|b)","(?i)((.)\\2)+","(?i)(.)\\1+","(?i)(.*)\\1b","(?is)(.*?)\\1b","(?i)(a|ab)\\1","(?i)(?>a|ab)(.)\\1","(?i)(.)\\1(?<=\\1\\1)","(?i)(..)\\1(?<=\\1)","(?i)(a{2})\\1(?<=\\1)","(?i)(a{2}?)\\1(?<=\\1)","(?i)((?:a|b){2})\\1(?<=\\1)","(?i)(?:((.)\\2)|b)+\\1","(?i)(?=(.))\\1","(?i)(?P<g>.)\\1(?P=g)","(?i)(.)\\1.*\\1","(?i)(.)\\1$"))
        compare(nativeCode,source,pattern,texts);
      var lower=new TreeMap<Integer,TreeSet<Integer>>();
      for(var entry:UnicodeRegexTables.lowercase().entrySet()) {
        var group=lower.computeIfAbsent(entry.getValue(),k->new TreeSet<>());group.add(entry.getKey());group.add(entry.getValue());
      }
      var pairs=new ArrayList<String>();
      for(var group:lower.values())for(int a:group)for(int b:group)pairs.add(new String(Character.toChars(a))+new String(Character.toChars(b)));
      var folds=UnicodeRegexTables.folds(17);
      for(int key:new TreeSet<>(folds.values().stream().map(TreeSet::first).toList()))for(int a:folds.get(key))for(int b:folds.get(key))pairs.add(new String(Character.toChars(a))+new String(Character.toChars(b)));
      for(var pattern:List.of("(?i)\\A(.)\\1\\Z","(?ai)\\A(.)\\1\\Z","(?i)\\A(?P<x>.)(?P=x)\\Z","(?i)\\A(.)\\1(?<=\\1\\1)\\Z"))compare(nativeCode,source,pattern,pairs);
      var triples=new ArrayList<String>();var alphabet=List.of("a","A","é","É","S","ſ","Σ","σ","ς","İ","ı","K","K",new String(Character.toChars(0x16ea0)),new String(Character.toChars(0x16ebb)),"😀");
      for(var a:alphabet)for(var b:alphabet)for(var c:alphabet)triples.add(a+b+c);
      for(var pattern:List.of("(?ai)\\A(.)\\1(?ui:\\1)\\Z","(?i)\\A(.)\\1(?ai:\\1)\\Z","(?i)\\A(.)\\1(?-i:\\1)\\Z"))compare(nativeCode,source,pattern,triples);
      for(var pattern:List.of("(?i)((?=é)é){2}\\1(?<=\\1\\1)","(?i)(?=(..))..\\1(?<=\\1)","(?i)(ab|cd)\\1(?<=\\1)","(?i)((?:é|ß){2})\\1(?<=\\1)","(?i)((?:é){2}+)\\1(?<=\\1)","(?i)((?:é){2}?)\\1(?<=\\1)","(?i)(é(?#comment){2})\\1(?<=\\1)","(?i)(é{1,2})\\1","(?i)(é|éé)\\1(?<=\\1)","(?i)(é*)\\1(?<=\\1)"))
        compare(nativeCode,source,pattern,List.of("", "éé", "éÉ", "ééé", "éÉé", "éééé", "éÉÉé", "éßÉẞ", "abab", "abAB", "cdCD", "Σς", "Sſ"));
      for(int size:List.of(0,1,32,4096,32768)) {
        var body="a".repeat(size)+"é";var doubled=body+body;
        compare(nativeCode,source,"(?i)^(.+)\\1$",List.of(doubled,doubled+"X"));
      }
      for(var pattern:List.of("(?i)(.)\\1","(?ai)(.)\\1","(?ai)(.)\\1(?ui:(.)\\2)")) {
        for(var blob:List.of("İi","Σς","Sſ","aAéÉ","éÉaA",new String(Character.toChars(0x16ea0))+new String(Character.toChars(0x16ebb))))for(boolean features:List.of(false,true)) {
          var spec="{\"scope\":"+ParserProbe.q(pattern)+",\"confirm\":"+ParserProbe.q(pattern)+",\"contradict\":\"failure\",\"fired\":"+ParserProbe.q(pattern)+"}";
          var digest="{\"_cmd_blob\":"+ParserProbe.q(blob)+",\"_out_blob\":"+ParserProbe.q(blob)+"}";
          RegexProbe.request(nativeCode,source,"{\"mode\":\"detector\",\"spec\":"+spec+",\"digest\":"+digest+(features?",\"features\":{}":"")+"}");
        }
        for(var kind:List.of("cmd_regex","out_regex","cmd_count")) {
          var spec="{\"name\":\"feature\",\"kind\":"+ParserProbe.q(kind)+",\"pattern\":"+ParserProbe.q(pattern)+"}";
          RegexProbe.request(nativeCode,source,"{\"mode\":\"extractor\",\"spec\":"+spec+",\"commands\":[[\"aAéÉ\",\"İi\"],[\"Σς\",\"Sſ\"],[\"İi\",\"none\"]]}");
        }
      }
    }
    System.out.println("Backreference parity passed "+RegexProbe.cases+" source comparisons covering "+searches+" searches");
  }
}
