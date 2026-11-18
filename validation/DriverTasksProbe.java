//JAVA 25
//SOURCES DriverSelectionProbe.java AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Compares complete task/workload selection with the pinned driver and real files. */
class DriverTasksProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"driver-tasks-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"driver_selection_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"driver_selection_host.py")) {
      var tasks=root.resolve("tasks é😀.json");var workload=root.resolve("workload.json");
      var compare=new Object(){String run(String fields)throws Exception{return DriverSelectionProbe.compare(nativeCode,source,fields);}};
      for(var task:List.of("null","false","17","[]","{}","{\"repo\":null}","{\"repo\":42}","{\"repo\":\"org/pkg-name\"}"))
        for(var repo:List.of("null","0","\"\"","\"other/pkg-name\"","\"pkg-name\"","\"different\""))compare.run("\"operation\":\"group\",\"task\":"+task+",\"repo\":"+repo);
      for(var groups:List.of("{}","{\"a\":[\"init\",\"task-1\"],\"b\":[\"init\",\"task-2\"]}","{\"only\":[\"init\",\"task-1\",\"init\"]}","{\"only\":[1,true,null,\"1\"]}","{\"only\":[[]]}","{\"a\":{},\"b\":\"ids\"}","[]","null")) {
        Files.writeString(workload,"{\"groups\":"+groups+"}");
        for(var repo:List.of("","a","b","only","unknown","é😀"))compare.run("\"operation\":\"workload\",\"workload\":"+q(workload)+",\"repo\":"+q(repo));
      }
      var samples=List.of("[]","[{\"instance_id\":\"init\",\"repo\":\"a\"},{\"instance_id\":\"init\",\"repo\":\"b\"},{\"instance_id\":\"task-1\",\"repo\":\"a\"},{\"instance_id\":\"task-2\",\"repo\":\"b\"}]","[{\"instance_id\":\"pkg_name__1\"},{\"instance_id\":\"pkg-name-2\",\"repo\":null},{\"instance_id\":\"other-3\"}]","[{}, {\"instance_id\":null}, {\"instance_id\":true}, {\"instance_id\":42}]","[null]","[[]]","{}","null");
      Files.writeString(workload,"{\"groups\":{\"a\":[\"init\",\"task-1\"],\"b\":[\"init\",\"task-2\"]}}");
      for(var sample:samples) {
        Files.writeString(tasks,sample);
        for(var repo:List.of("","a","b","pkg-name","org/pkg-name","unknown"))for(var limit:List.of("0","1","-1","99","-99","true","null","1.5","\"1\"","999999999999999999999999999"))
          for(boolean manifest:List.of(false,true))compare.run("\"operation\":\"tasks\",\"tasks\":"+q(tasks)+",\"repo\":"+q(repo)+",\"limit\":"+limit+",\"workload\":"+(manifest?q(workload):"null"));
      }
      for(var document:List.of("[","{\"groups\":}","{}","[]","null","{\"groups\":{}}")) {
        Files.writeString(workload,document);Files.writeString(tasks,document);
        compare.run("\"operation\":\"workload\",\"workload\":"+q(workload));
        compare.run("\"operation\":\"tasks\",\"tasks\":"+q(tasks));
      }
      Files.write(tasks,new byte[]{(byte)255});compare.run("\"operation\":\"tasks\",\"tasks\":"+q(tasks));
      Files.delete(tasks);compare.run("\"operation\":\"tasks\",\"tasks\":"+q(tasks));
      Files.createDirectory(tasks);compare.run("\"operation\":\"tasks\",\"tasks\":"+q(tasks));
      for(var failure:List.of("open","enter","read","exit"))for(var error:List.of("OSError","RuntimeError","KeyboardInterrupt"))
        compare.run("\"operation\":\"tasks\",\"tasks\":\"tasks\",\"documents\":{\"tasks\":\"[]\"},\"fail\":"+q(failure)+",\"error\":"+q(error));
      for(boolean suppress:List.of(false,true))for(var operation:List.of("tasks","workload"))for(var document:List.of("[","{}","null","[]"))
        compare.run("\"operation\":"+q(operation)+",\"tasks\":\"input\",\"workload\":\"input\",\"documents\":{\"input\":"+q(document)+"},\"suppress\":"+suppress);
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Experiment task selection matches the source in "+DriverSelectionProbe.cases+" comparisons; workload groups, duplicate ids, legacy prefixes, limits, real JSON/UTF-8/filesystem errors and context cleanup passed offline");
  }
}
