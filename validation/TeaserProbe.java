//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java FiguresProbe.java
//SOURCES TeaserEncoder.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Source/poster and animation output checks for the native teaser. */
class TeaserProbe {
  static int checks;
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static void comparePoster(Path checkout,Path work,Map<String,String> env,Path tool,Path fonts)throws Exception {
    var source=Files.createDirectories(work.resolve("source"));var nativeOut=work.resolve("native");
    var expected=NativeCliProbe.process(work,env,List.of(checkout.resolve(".venv/bin/python").toString(),"-I",checkout.resolve("validation/teaser_source.py").toString(),fonts.toString(),source.toString(),"poster"),120);
    require(expected.code()==0,"Source teaser poster failed: "+expected);
    var actual=NativeCliProbe.process(work,env,List.of(tool.toString(),"teaser","--plex",fonts.toString(),"--out",nativeOut.toString(),"--still"),120);
    require(actual.code()==0&&actual.err().equals(expected.err())&&actual.out().replace(nativeOut.toString(),"OUT").equals(expected.out().replace(source.toString(),"OUT")),"Teaser poster differs\nNative: "+actual+"\nSource: "+expected);
    FiguresProbe.png(nativeOut.resolve("teaser.png"),source.resolve("teaser.png"));checks+=4;
  }
  static String shell(String value){return "'"+value.replace("'","'\\''")+"'";}
  static String console(String text,Path out){return text.replace(out.toString(),"OUT").replaceAll("\\([0-9]+\\.[0-9]{2} MB,","(SIZE MB,");}
  static void animation(Path checkout,Path work,Map<String,String> environment,Path tool,Path fonts,boolean page)throws Exception {
    var source=Files.createDirectory(work.resolve("source"));var nativeOut=work.resolve("native");
    var sourceCapture=work.resolve("source-capture");var nativeCapture=work.resolve("native-capture");
    if(!page)for(var logo:List.of("logo_berkeley.png","logo_microsoft.png"))Files.copy(checkout.resolve("reference/upstream/docs/assets").resolve(logo),source.resolve(logo));
    var env=new HashMap<>(environment);env.put("TE_FFMPEG_MODE","fixture");env.put("TE_FFMPEG_CAPTURE",sourceCapture.toString());
    System.out.println("Rendering source "+(page?"page":"GIF")+" frames");
    var expected=NativeCliProbe.process(work,env,List.of(checkout.resolve(".venv/bin/python").toString(),"-I",checkout.resolve("validation/teaser_source.py").toString(),fonts.toString(),source.toString(),page?"page":"gif"),1200);
    require(expected.code()==0,"Source animation failed: "+expected);
    env.put("TE_FFMPEG_MODE","real");env.put("TE_FFMPEG_CAPTURE",nativeCapture.toString());
    System.out.println("Rendering and encoding native "+(page?"page":"GIF")+" frames");
    var actual=NativeCliProbe.process(work,env,List.of(tool.toString(),"teaser","--plex",fonts.toString(),page?"--page":"--out",nativeOut.toString()),1800);
    require(actual.code()==0&&actual.err().equals(expected.err())&&console(actual.out(),nativeOut).equals(console(expected.out(),source)),"Animation command differs\nNative: "+actual+"\nSource: "+expected);
    var sourceManifest=Files.readAllLines(sourceCapture.resolve("frames.sha256"));var nativeManifest=Files.readAllLines(nativeCapture.resolve("frames.sha256"));
    require(sourceManifest.size()==581&&nativeManifest.size()==581,"Animation frame count differs");
    for(int i=0;i<sourceManifest.size();i++) {
      if(!sourceManifest.get(i).equals(nativeManifest.get(i)))FiguresProbe.png(nativeCapture.resolve("frames").resolve(String.format("%04d.png",i)),sourceCapture.resolve("frames").resolve(String.format("%04d.png",i)));
      require(sourceManifest.get(i).equals(nativeManifest.get(i)),"Animation frame bytes differ at "+i+" in "+work);
    }
    for(var name:page?List.of("teaser.webm","teaser.mp4"):List.of("teaser.gif")) {
      require(Files.mismatch(sourceCapture.resolve(name+".args"),nativeCapture.resolve(name+".args"))==-1,"Encoder arguments differ: "+name);
      var file=nativeOut.resolve(name);require(Files.size(file)>1000,"Encoded output is missing: "+name);
      var probed=NativeCliProbe.process(work,env,List.of("/opt/homebrew/bin/ffprobe","-v","error","-count_frames","-select_streams","v:0","-show_entries","stream=width,height,nb_read_frames,r_frame_rate,codec_name","-of","default=noprint_wrappers=1",file.toString()),120);
      require(probed.code()==0&&probed.out().contains("nb_read_frames=581")&&probed.out().contains("width="+(page?2240:1400))&&probed.out().contains("height="+(page?1200:750)),"Encoded frame metadata differs: "+probed);
    }
    FiguresProbe.png(nativeOut.resolve("teaser.png"),source.resolve("teaser.png"));checks+=4;
    System.out.println("Verified all 581 "+(page?"page":"GIF")+" frame hashes and real encoder outputs");
  }
  public static void main(String[] args)throws Exception {
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory(checkout.resolve("validation/results"),"teaser-");boolean passed=false;
    try {
      var tool=root.resolve("relocated teaser tool");Files.copy(checkout.resolve("target/debug/te-figures"),tool,StandardCopyOption.COPY_ATTRIBUTES);
      var fonts=checkout.resolve("validation/results/teaser-fonts");
      var env=new HashMap<>(Map.of("HOME",root.toString(),"PATH","","TE_PYTHON",checkout.resolve(".venv/bin/python").toString(),"MPLCONFIGDIR",root.resolve("mpl").toString(),"LANG","C.UTF-8"));
      comparePoster(checkout,Files.createDirectory(root.resolve("poster")),env,tool,fonts);
      if(args.length>0) {
        var bin=Files.createDirectory(root.resolve("bin"));
        var java=Path.of(System.getProperty("java.home"),"bin/java").toString();var classes=Path.of(TeaserEncoder.class.getProtectionDomain().getCodeSource().getLocation().toURI()).toString();
        var encoder=bin.resolve("ffmpeg");Files.writeString(encoder,"#!/bin/sh\nexec "+shell(java)+" -cp "+shell(classes)+" TeaserEncoder \"$@\"\n");encoder.toFile().setExecutable(true);
        env.put("PATH",bin.toString());env.put("TE_FFMPEG_REAL","/opt/homebrew/bin/ffmpeg");
        animation(checkout,Files.createDirectory(root.resolve("gif")),env,tool,fonts,false);
        animation(checkout,Files.createDirectory(root.resolve("page")),env,tool,fonts,true);
      }
      System.out.println("Verified "+checks+" teaser checks; source pixels/bytes, frames, encoder inputs, console output, embedded logos and relocation");passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Teaser fixtures: "+root);}
  }
}
