//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;
import javax.xml.parsers.DocumentBuilderFactory;

/** Compares generated documentation assets with the unchanged authoring scripts. */
class FiguresProbe {
  static int checks;
  static Path checkout, root, tool;
  static void require(boolean condition,String message) {if(!condition)throw new AssertionError(message);checks++;}
  static void png(Path nativeFile,Path sourceFile) throws Exception {
    var actual=ImageIO.read(nativeFile.toFile());var expected=ImageIO.read(sourceFile.toFile());
    require(actual!=null&&expected!=null,"PNG could not be decoded");
    require(actual.getWidth()==expected.getWidth()&&actual.getHeight()==expected.getHeight(),"PNG dimensions differ");
    int width=actual.getWidth(),height=actual.getHeight();
    var actualPixels=actual.getRGB(0,0,width,height,null,0,width);var expectedPixels=expected.getRGB(0,0,width,height,null,0,width);
    require(java.util.Arrays.equals(actualPixels,expectedPixels),"PNG pixels differ: "+nativeFile);
    require(Files.mismatch(nativeFile,sourceFile)==-1,"PNG export bytes differ: "+nativeFile);
  }
  static void svg(Path nativeFile,Path sourceFile) throws Exception {
    var factory=DocumentBuilderFactory.newInstance();factory.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd",false);
    factory.setFeature("http://xml.org/sax/features/external-general-entities",false);factory.setFeature("http://xml.org/sax/features/external-parameter-entities",false);
    var document=factory.newDocumentBuilder().parse(nativeFile.toFile());
    require(document.getDocumentElement().getNodeName().equals("svg"),"SVG root is invalid");
    var svg=Files.readString(nativeFile);
    var labels = nativeFile.getFileName().toString().startsWith("results") ? List.of("23.6%","30.9%","24.7%","26.1%","18.4%","SWE-Bench: 277 tasks","TokenEconomizer","Sonnet 5","Opus 4.8","GPT-5.6 Terra","GPT-5.6 Sol") : List.of("PLUGIN","YOUR CODING AGENT","Micro-skills","Tool pruning","Failure-loop intervention","REDIRECT","NOTICE","the finished session's log");
    for(var label:labels)
      require(svg.contains(label),"SVG omitted paper data: "+label);
    require(!svg.contains("<dc:date>"),"SVG contains a creation timestamp");
    require(Files.mismatch(nativeFile,sourceFile)==-1,"SVG export bytes differ: "+nativeFile);
  }
  static void compare(Map<String,String> env,String figure) throws Exception {
    var work=Files.createTempDirectory(root,"render-");var source=Files.createDirectory(work.resolve("source"));var nativeDir=work.resolve("native");
    var expected=NativeCliProbe.process(work,env,List.of(checkout.resolve(".venv/bin/python").toString(),"-I",checkout.resolve("validation/figures_source.py").toString(),figure,source.toString()),90);
    require(expected.code()==0,"Source renderer failed: "+expected);
    var command=figure.equals("results_overview")?"results":"framework";
    var actual=NativeCliProbe.process(work,env,List.of(tool.toString(),command,"--out",nativeDir.toString()),90);
    require(actual.code()==expected.code()&&actual.err().equals(expected.err())&&actual.out().replace(nativeDir.toString(),"OUT").equals(expected.out().replace(source.toString(),"OUT")),"Figure command differs\nNative: "+actual+"\nSource: "+expected);
    png(nativeDir.resolve(figure+".png"),source.resolve(figure+".png"));svg(nativeDir.resolve(figure+".svg"),source.resolve(figure+".svg"));
    var repeated=Files.createDirectory(work.resolve("repeated"));
    var overridden=new HashMap<>(env);overridden.put("TE_PYTHON","/missing-python");
    var again=NativeCliProbe.process(work,overridden,List.of(tool.toString(),command,"--out",repeated.toString(),"--python",checkout.resolve(".venv/bin/python").toString()),90);
    require(again.code()==0,"Explicit interpreter failed: "+again);
    require(Files.mismatch(nativeDir.resolve(figure+".png"),repeated.resolve(figure+".png"))==-1,"PNG changed between native renders");
    require(Files.mismatch(nativeDir.resolve(figure+".svg"),repeated.resolve(figure+".svg"))==-1,"SVG changed between native renders");
  }
  static void refusal(Map<String,String> env) throws Exception {
    var work=Files.createTempDirectory(root,"overflow-");var source=Files.createDirectory(work.resolve("source"));var nativeDir=work.resolve("native");
    var expected=NativeCliProbe.process(work,env,List.of(checkout.resolve(".venv/bin/python").toString(),"-I",checkout.resolve("validation/figures_source.py").toString(),"framework_overview",source.toString()),90);
    var actual=NativeCliProbe.process(work,env,List.of(tool.toString(),"framework","--out",nativeDir.toString()),90);
    var reason="card 1 body is 13.01 units, room 13.00";
    require(expected.code()!=0&&expected.err().contains(reason),"Source overflow guard did not trigger: "+expected);
    require(actual.code()!=0&&actual.err().contains(reason)&&actual.out().isEmpty(),"Native text overflow guard differs: "+actual);
    try(var outputs=Files.list(source)) { require(outputs.findAny().isEmpty()&&!Files.exists(nativeDir),"Overflow guard wrote an incomplete figure"); }
  }
  public static void main(String[] args)throws Exception {
    checkout=Path.of("").toAbsolutePath();root=Files.createTempDirectory(checkout.resolve("validation/results"),"figures-");boolean passed=false;
    try {
      tool=root.resolve("relocated figure tool");Files.copy(checkout.resolve("target/debug/te-figures"),tool,StandardCopyOption.COPY_ATTRIBUTES);
      var env=new HashMap<>(Map.of("HOME",root.toString(),"PATH","","TE_PYTHON",checkout.resolve(".venv/bin/python").toString(),"MPLCONFIGDIR",root.resolve("mpl").toString(),"LANG","C.UTF-8"));
      Files.createDirectory(root.resolve("mpl"));
      Files.writeString(root.resolve("mpl/matplotlibrc"),"svg.hashsalt: native-figure-validation\n");
      compare(env,"results_overview");
      compare(env,"framework_overview");
      Files.writeString(root.resolve("mpl/matplotlibrc"),"svg.hashsalt: native-figure-validation\nfigure.dpi: 137\n");
      compare(env,"results_overview");
      refusal(env);
      Files.writeString(root.resolve("mpl/matplotlibrc"),"svg.hashsalt: native-figure-validation\nfigure.dpi: 200\n");
      compare(env,"framework_overview");
      var noPython=new HashMap<>(env);noPython.put("TE_PYTHON","/missing-python");
      var help=NativeCliProbe.process(root,noPython,List.of(tool.toString(),"--help"));
      require(help.code()==0&&help.out().contains("Requires Matplotlib"),"Help required Python: "+help);
      for(var test:List.of(Map.entry(List.<String>of(),"expected figure name"),Map.entry(List.of("missing"),"unknown figure"),Map.entry(List.of("results","--out"),"--out requires a directory"),Map.entry(List.of("results","--python"),"--python requires an interpreter"),Map.entry(List.of("results","--bad"),"unrecognized argument"),Map.entry(List.of("results","--out","--python"),"--out requires a directory"),Map.entry(List.of("framework","--python","--out"),"--python requires an interpreter"),Map.entry(List.of("results","--out",""),"--out requires a directory"))) {
        var command=new ArrayList<>(List.of(tool.toString()));command.addAll(test.getKey());
        var invalid=NativeCliProbe.process(root,noPython,command);
        require(invalid.code()!=0&&invalid.err().startsWith("te-figures:")&&invalid.err().contains(test.getValue()),"Argument validation started Python: "+invalid);
      }
      var unavailable=NativeCliProbe.process(root,noPython,List.of(tool.toString(),"results","--out",root.resolve("must-not-exist").toString()));
      require(unavailable.code()!=0&&!Files.exists(root.resolve("must-not-exist")),"Missing renderer created partial output: "+unavailable);
      var defaultOut=Files.createDirectory(root.resolve("default-output"));
      var defaults=NativeCliProbe.process(defaultOut,env,List.of(tool.toString(),"results"),90);
      require(defaults.code()==0&&Files.isRegularFile(defaultOut.resolve("docs/assets/results_overview.png"))&&Files.isRegularFile(defaultOut.resolve("docs/assets/results_overview.svg")),"Default output did not use the working directory: "+defaults);
      System.out.println("Verified "+checks+" documentation figure checks; source PNG pixels/bytes, SVG bytes, renderer DPI, deterministic exports, relocation and CLI validation");passed=true;
    } finally {if(passed)ContainerProbe.clear(root);else System.err.println("Figure fixtures: "+root);}
  }
}
