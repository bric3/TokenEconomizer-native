//JAVA 25
//SOURCES TeaserProbe.java TeaserEncoder.java FiguresProbe.java NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Exercises font selection, preflight order and encoder-failure cleanup. */
class TeaserFailureProbe {
  static int checks;
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static String error(String text){return text.replace("te-figures: ","").replace("teaser.py needs ffmpeg","teaser needs ffmpeg");}
  static void refusal(Path checkout,Path root,Map<String,String> env,Path tool,Path fonts,boolean still)throws Exception {
    var work=Files.createTempDirectory(root,"refused-");var source=work.resolve("source");var nativeOut=work.resolve("native");
    var expected=NativeCliProbe.process(work,env,List.of(checkout.resolve(".venv/bin/python").toString(),"-I",checkout.resolve("validation/teaser_source.py").toString(),fonts.toString(),source.toString(),still?"poster":"gif"),90);
    var command=new ArrayList<>(List.of(tool.toString(),"teaser","--plex",fonts.toString(),"--out",nativeOut.toString()));if(still)command.add("--still");
    var actual=NativeCliProbe.process(work,env,command,90);
    require(expected.code()!=0&&actual.code()!=0&&error(actual.err()).equals(error(expected.err())),"Refusal differs\nNative: "+actual+"\nSource: "+expected);
    require(!Files.exists(source)&&!Files.exists(nativeOut),"Preflight refusal created output directories");
  }
  public static void main(String[] args)throws Exception {
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory(checkout.resolve("validation/results"),"teaser-failures-");boolean passed=false;
    try {
      var tool=root.resolve("relocated tool's directory/te-figures");Files.createDirectories(tool.getParent());Files.copy(checkout.resolve("target/debug/te-figures"),tool,StandardCopyOption.COPY_ATTRIBUTES);
      var fonts=checkout.resolve("validation/results/teaser-fonts");
      var env=new HashMap<>(Map.of("HOME",root.toString(),"PATH","","TE_PYTHON",checkout.resolve(".venv/bin/python").toString(),"MPLCONFIGDIR",root.resolve("mpl").toString(),"LANG","C.UTF-8"));
      for(var logo:List.of("logo_berkeley.png","logo_microsoft.png"))require(Files.mismatch(checkout.resolve("src/figures/assets").resolve(logo),checkout.resolve("reference/upstream/docs/assets").resolve(logo))==-1,"Embedded logo differs from pinned asset");
      refusal(checkout,root,env,tool,root.resolve("absent-fonts"),true);
      refusal(checkout,root,env,tool,root.resolve("absent-fonts"),false);
      var selected=Files.createDirectory(root.resolve("fonts"));
      for(var name:List.of("IBMPlexSans-Regular","IBMPlexSans-SemiBold","IBMPlexSans-Italic","IBMPlexMono-Regular","IBMPlexMono-SemiBold")) {
        var folder=name.startsWith("IBMPlexSans")?"sans":"mono";
        Files.createSymbolicLink(selected.resolve(name+".ttf"),fonts.resolve(folder+"/package/fonts/complete/woff/"+name+".woff"));
        Files.writeString(selected.resolve(name+".woff"),"a .ttf must take precedence");
      }
      TeaserProbe.comparePoster(checkout,Files.createDirectory(root.resolve("font-precedence")),env,tool,selected);checks+=6;
      for(var arguments:List.of(List.of("teaser"),List.of("teaser","--plex"),List.of("teaser","--plex","--still"),List.of("teaser","--plex",fonts.toString(),"--page"),List.of("teaser","--plex",fonts.toString(),"--out","a","--page","b"),List.of("results","--still"))) {
        var command=new ArrayList<>(List.of(tool.toString()));command.addAll(arguments);
        var result=NativeCliProbe.process(root,env,command);
        require(result.code()!=0&&result.err().startsWith("te-figures:")&&result.out().isEmpty(),"Invalid teaser arguments reached rendering: "+result);
      }
      if(args.length>0) {
        var bin=Files.createDirectory(root.resolve("bin"));var java=Path.of(System.getProperty("java.home"),"bin/java").toString();var classes=Path.of(TeaserEncoder.class.getProtectionDomain().getCodeSource().getLocation().toURI()).toString();
        var encoder=bin.resolve("ffmpeg");Files.writeString(encoder,"#!/bin/sh\nexec "+TeaserProbe.shell(java)+" -cp "+TeaserProbe.shell(classes)+" TeaserEncoder \"$@\"\n");encoder.toFile().setExecutable(true);
        env.put("PATH",bin.toString());env.put("TE_FFMPEG_MODE","fail");
        var source=Files.createDirectory(root.resolve("source"));var nativeOut=root.resolve("native");
        for(var logo:List.of("logo_berkeley.png","logo_microsoft.png"))Files.copy(checkout.resolve("reference/upstream/docs/assets").resolve(logo),source.resolve(logo));
        env.put("TE_FFMPEG_CAPTURE",root.resolve("source-capture").toString());
        System.out.println("Checking source encoder refusal");
        var expected=NativeCliProbe.process(root,env,List.of(checkout.resolve(".venv/bin/python").toString(),"-I",checkout.resolve("validation/teaser_source.py").toString(),fonts.toString(),source.toString(),"gif"),1200);
        require(expected.code()!=0&&expected.err().contains("owned encoder refusal")&&expected.out().isEmpty(),"Source encoder refusal did not reproduce: "+expected);
        env.put("TE_FFMPEG_CAPTURE",root.resolve("native-capture").toString());
        System.out.println("Checking native encoder refusal");
        var actual=NativeCliProbe.process(root,env,List.of(tool.toString(),"teaser","--plex",fonts.toString(),"--out",nativeOut.toString()),1200);
        require(actual.code()!=0&&actual.err().contains("owned encoder refusal")&&actual.err().contains("ffmpeg failed")&&actual.out().isEmpty(),"Native encoder failure reported success: "+actual);
        require(!Files.exists(source.resolve("teaser.gif"))&&!Files.exists(source.resolve("teaser.png"))&&!Files.exists(nativeOut.resolve("teaser.gif"))&&!Files.exists(nativeOut.resolve("teaser.png")),"Encoder failure wrote a poster or completed movie");
        var sourceArgs=Files.readAllLines(root.resolve("source-capture/teaser.gif.args"));var nativeArgs=Files.readAllLines(root.resolve("native-capture/teaser.gif.args"));
        require(sourceArgs.equals(nativeArgs),"Failed encoder arguments differ");
        require(!Files.exists(Path.of(Files.readString(root.resolve("source-capture/scratch.txt"))))&&!Files.exists(Path.of(Files.readString(root.resolve("native-capture/scratch.txt")))),"Encoder refusal leaked temporary frame directories");
      }
      System.out.println("Verified "+checks+" teaser preflight, font precedence, asset, argument and encoder refusal checks");passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Teaser failure fixtures: "+root);}
  }
}
