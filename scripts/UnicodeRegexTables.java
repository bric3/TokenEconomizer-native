//JAVA 25

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.BitSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;

/** Reproduces the Unicode 16 -> 17 corrections for regex-syntax's pinned tables. */
class UnicodeRegexTables {
  record Input(int version,String file,String sha256) {}
  static final List<Input> INPUTS=List.of(
      new Input(16,"UnicodeData.txt","ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f"),
      new Input(17,"UnicodeData.txt","2e1efc1dcb59c575eedf5ccae60f95229f706ee6d031835247d843c11d96470c"),
      new Input(16,"CaseFolding.txt","6f1f9c588eb4a5c718d9e8f93b782685e5c7fec872cf05e8e6878053599e09bb"),
      new Input(17,"CaseFolding.txt","ff8d8fefbf123574205085d6714c36149eb946d717a0c585c27f0f4ef58c4183"),
      new Input(17,"DerivedCoreProperties.txt","24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08"));
  static final int LIMIT=0x110000;
  record Tables(BitSet word,BitSet digit,BitSet space) {}
  static List<String> input(int version,String file)throws Exception {
    var spec=INPUTS.stream().filter(i->i.version==version&&i.file.equals(file)).findFirst().orElseThrow();
    var path=Path.of("validation/results/unicode-ucd",version+"-"+file);
    if(!Files.exists(path)) {
      var uri=URI.create("https://www.unicode.org/Public/"+version+".0.0/ucd/"+file);
      var response=HttpClient.newHttpClient().send(HttpRequest.newBuilder(uri).GET().build(),HttpResponse.BodyHandlers.ofByteArray());
      if(response.statusCode()!=200)throw new IllegalStateException(uri+": HTTP "+response.statusCode());
      if(!hash(response.body()).equals(spec.sha256))throw new IllegalStateException("Unexpected Unicode source: "+uri);
      Files.createDirectories(path.getParent());Files.write(path,response.body());
    }
    if(!hash(Files.readAllBytes(path)).equals(spec.sha256))throw new IllegalStateException("Unicode source changed: "+path);
    return Files.readAllLines(path);
  }
  static String hash(byte[] bytes)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));}
  static Tables tables(int version)throws Exception {
    var result=new Tables(new BitSet(LIMIT),new BitSet(LIMIT),new BitSet(LIMIT));
    int first=-1;
    for(var line:input(version,"UnicodeData.txt")) {
      var fields=line.split(";",-1);int code=Integer.parseInt(fields[0],16);
      if(fields[1].endsWith(", First>")){first=code;continue;}
      int start=fields[1].endsWith(", Last>")?first:code;
      if(start<0)throw new IllegalStateException("Unpaired Unicode range");
      if(fields[2].startsWith("L")||fields[2].startsWith("N"))result.word.set(start,code+1);
      if(fields[2].equals("Nd"))result.digit.set(start,code+1);
      if(fields[2].equals("Zs")||List.of("WS","B","S").contains(fields[4]))result.space.set(start,code+1);
      first=-1;
    }
    result.word.set('_');return result;
  }
  static int root(int[] parent,int value) {
    while(parent[value]!=value){parent[value]=parent[parent[value]];value=parent[value];}return value;
  }
  static Map<Integer,TreeSet<Integer>> folds(int version)throws Exception {
    var parent=new int[LIMIT];Arrays.setAll(parent,i->i);var seen=new TreeSet<Integer>();
    for(var line:input(version,"CaseFolding.txt")) {
      if(line.isBlank()||line.startsWith("#"))continue;
      var fields=line.split(";");var status=fields[1].strip();
      if(!status.equals("C")&&!status.equals("S"))continue;
      int from=Integer.parseInt(fields[0].strip(),16),to=Integer.parseInt(fields[2].strip(),16);
      parent[root(parent,from)]=root(parent,to);seen.add(from);seen.add(to);
    }
    var groups=new TreeMap<Integer,TreeSet<Integer>>();
    for(int code:seen)groups.computeIfAbsent(root(parent,code),k->new TreeSet<>()).add(code);
    var byCharacter=new TreeMap<Integer,TreeSet<Integer>>();
    for(var group:groups.values())for(int code:group)byCharacter.put(code,group);
    return byCharacter;
  }
  static List<TreeSet<Integer>> changedFolds()throws Exception {
    var before=folds(16);var after=folds(17);
    for(var entry:before.entrySet())if(!after.getOrDefault(entry.getKey(),new TreeSet<>()).containsAll(entry.getValue()))throw new IllegalStateException("Unicode 17 removes a case-fold edge; additive correction is insufficient");
    var result=new ArrayList<TreeSet<Integer>>();
    for(var group:new TreeSet<>(after.values().stream().map(TreeSet::first).toList())) {
      var values=after.get(group);if(!values.equals(before.get(group)))result.add(values);
    }
    return result;
  }
  static BitSet difference(BitSet left,BitSet right){var result=(BitSet)left.clone();result.andNot(right);return result;}
  static Map<Integer,Integer> lowercase()throws Exception {
    var result=new TreeMap<Integer,Integer>();
    for(var line:input(17,"UnicodeData.txt")) {
      var fields=line.split(";",-1);
      if(!fields[13].isEmpty())result.put(Integer.parseInt(fields[0],16),Integer.parseInt(fields[13],16));
    }
    return result;
  }
  static String character(int code){return "'\\u{"+Integer.toHexString(code)+"}'";}
  static void ranges(StringBuilder text,String name,BitSet values) {
    var entries=new ArrayList<String>();
    for(int start=values.nextSetBit(0);start>=0;){int end=values.nextClearBit(start);entries.add("("+character(start)+", "+character(end-1)+")");start=values.nextSetBit(end);}
    text.append("pub(super) const ").append(name).append(": &[(char, char)] = &[");
    if(entries.size()<2)text.append(String.join(", ",entries)).append("];\n");
    else text.append("\n    ").append(String.join(",\n    ",entries)).append(",\n];\n");
  }
  public static void main(String[] args)throws Exception {
    if(args.length>1||args.length==1&&!args[0].equals("--check"))throw new IllegalArgumentException("usage: UnicodeRegexTables.java [--check]");
    for(var spec:INPUTS)input(spec.version,spec.file);
    var before=tables(16);var after=tables(17);var folds=changedFolds();
    if(!before.space.equals(after.space))throw new IllegalStateException("Whitespace changed; add an explicit correction");
    if(!difference(before.word,after.word).isEmpty()||!difference(before.digit,after.digit).isEmpty())throw new IllegalStateException("Unicode 17 removes class members; additive correction is insufficient");
    var text=new StringBuilder("// Generated by scripts/UnicodeRegexTables.java from Unicode 16 and 17.\n// Copyright © 2025 Unicode, Inc.; data licence: data/LICENSE-UNICODE-V3.\n// Corrections for regex-syntax 0.8.11.\n");
    for(var input:INPUTS)text.append("// ").append(input.version).append(".0.0/").append(input.file).append(" SHA-256: ").append(input.sha256).append('\n');
    ranges(text,"WORD_ADDED",difference(after.word,before.word));
    ranges(text,"DIGIT_ADDED",difference(after.digit,before.digit));
    text.append("pub(super) const CASE_FOLD_ADDED: &[&[char]] = &[\n");
    for(var group:folds)text.append("    &[").append(String.join(", ",group.stream().map(UnicodeRegexTables::character).toList())).append("],\n");
    text.append("];\n");
    text.append("pub(super) const LOWER: &[(char, char)] = &[\n");
    for(var entry:lowercase().entrySet())text.append("    (").append(character(entry.getKey())).append(", ").append(character(entry.getValue())).append("),\n");
    text.append("];\n");
    var output=Path.of("src/microskills/distiller/unicode17.rs");var rendered=text.toString();
    if(args.length==1){if(!Files.readString(output).equals(rendered))throw new IllegalStateException("Unicode regex tables are stale");}
    else Files.writeString(output,rendered);
    System.out.println("Verified Unicode 16 -> 17 regex corrections: "+difference(after.word,before.word).cardinality()+" added word characters, "+difference(after.digit,before.digit).cardinality()+" added digits, "+folds.size()+" changed case-fold groups");
  }
}
