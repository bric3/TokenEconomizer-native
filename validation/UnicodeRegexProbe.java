//JAVA 25
//SOURCES ../scripts/UnicodeRegexTables.java RegexProbe.java AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.List;
import java.util.Locale;
import java.util.TreeSet;

/** Checks every scalar's class membership and the pinned Unicode case/identifier tables. */
class UnicodeRegexProbe {
  static final int LIMIT=UnicodeRegexTables.LIMIT;
  static List<String> chunks(BitSet values) {
    var result=new ArrayList<String>();var text=new StringBuilder();int count=0;
    for(int code=values.nextSetBit(0);code>=0;code=values.nextSetBit(code+1)) {
      text.appendCodePoint(code);
      if(++count==1024){result.add(text.toString());text.setLength(0);count=0;}
    }
    if(!text.isEmpty())result.add(text.toString());return result;
  }
  static void uniform(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String pattern,List<String> texts)throws Exception {
    var request="{\"pattern\":"+ParserProbe.q(pattern)+",\"texts\":"+ParserProbe.array(texts)+"}";
    var expected=source.call(request);
    if(!expected.contains("\"matches\": [")||expected.contains("false"))throw new AssertionError("Unicode table disagrees with Python: "+pattern+" "+expected);
    var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Native class differs: "+pattern+"\n"+actual+"\n"+expected);
    RegexProbe.cases++;
  }
  static BitSet identifiers(String property)throws Exception {
    var values=new BitSet(LIMIT);
    for(var row:UnicodeRegexTables.input(17,"DerivedCoreProperties.txt")) {
      var line=row.split("#",2)[0].strip();if(line.isEmpty())continue;
      var fields=line.split(";",-1);if(!fields[1].strip().equals(property))continue;
      var bounds=fields[0].strip().split("\\.\\.");int first=Integer.parseInt(bounds[0],16),last=Integer.parseInt(bounds[bounds.length-1],16);
      values.set(first,last+1);
    }
    if(values.isEmpty())throw new AssertionError("Missing identifier property: "+property);return values;
  }
  static String escape(int code){return String.format(Locale.ROOT,"\\U%08X",code);}
  public static void main(String[] args)throws Exception {
    var tables=UnicodeRegexTables.tables(17);var scalars=new BitSet(LIMIT);scalars.set(0,LIMIT);scalars.clear(0xd800,0xe000);
    long members=0;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"regex_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"regex_host.py")) {
      var classes=List.of(tables.word(),tables.digit(),tables.space());var letters=List.of("w","d","s");
      for(int index=0;index<classes.size();index++) {
        var included=classes.get(index);var excluded=UnicodeRegexTables.difference(scalars,included);
        uniform(nativeCode,source,"\\A\\"+letters.get(index)+"+\\Z",chunks(included));
        uniform(nativeCode,source,"\\A\\"+letters.get(index).toUpperCase(Locale.ROOT)+"+\\Z",chunks(excluded));
        members+=included.cardinality()+excluded.cardinality();
      }
      var added=UnicodeRegexTables.difference(tables.word(),UnicodeRegexTables.tables(16).word());
      uniform(nativeCode,source,"\\A\\b\\w+\\b\\Z",chunks(added));
      uniform(nativeCode,source,"(?a)\\A\\W+\\Z",chunks(added));
      var folds=UnicodeRegexTables.folds(17);
      for(int key:new TreeSet<>(folds.values().stream().map(TreeSet::first).toList())) {
        var group=folds.get(key);var texts=new ArrayList<String>();
        for(int code:group)texts.add(new String(Character.toChars(code)));
        texts.add("");texts.add("😀");
        for(int code:group)RegexProbe.compare(nativeCode,source,"(?i)\\A"+escape(code)+"\\Z",texts);
      }
      for(var group:UnicodeRegexTables.changedFolds()) {
        var texts=group.stream().map(code->new String(Character.toChars(code))).toList();
        for(var mode:List.of("(?i)","(?ai)"))for(boolean negate:List.of(false,true))
          RegexProbe.compare(nativeCode,source,mode+"\\A["+(negate?"^":"")+escape(group.first())+"-"+escape(group.last())+"]\\Z",texts);
      }
      var starts=identifiers("XID_Start");starts.set('_');var continues=identifiers("XID_Continue");
      for(var name:chunks(starts))uniform(nativeCode,source,"(?P<"+name+">a)",List.of("a"));
      for(var name:chunks(continues))uniform(nativeCode,source,"(?P<_"+name+">a)",List.of("a"));
      var boundaries=new BitSet(LIMIT);
      for(int start=starts.nextSetBit(0);start>=0;) {
        int end=starts.nextClearBit(start);if(start>0)boundaries.set(start-1);if(end<LIMIT)boundaries.set(end);start=starts.nextSetBit(end);
      }
      boundaries.clear(0xd800,0xe000);
      for(int code=boundaries.nextSetBit(0);code>=0;code=boundaries.nextSetBit(code+1))
        RegexProbe.compare(nativeCode,source,"(?P<"+new String(Character.toChars(code))+">a)",List.of("","a"));
    }
    System.out.println("Unicode regex parity passed "+RegexProbe.cases+" source comparisons; "+members+" scalar class memberships, all simple-fold groups and all valid identifier code points checked");
  }
}
