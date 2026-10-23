import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.lang.reflect.Field;
import java.util.List;

import com.google.gson.stream.JsonWriter;
import com.google.gson.stream.JsonReader;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.CommentType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Run with -Xmx256m to catch accumulation at real shared-function call sites. */
public class AzReflectionMemoryTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        Path resources = Files.createTempDirectory("az-reflection-memory-");
        int count = args.length > 1 ? Integer.parseInt(args[1]) : 30;
        try {
            int setup = program.startTransaction("fixture");
            try {
                program.setImageBase(program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
                for (int offset : List.of(0x100, 0x110)) {
                    var address = program.getImageBase().add(offset);
                    program.getMemory().createInitializedBlock("fixture" + offset, address, 16,
                        (byte) 0xc3, TaskMonitor.DUMMY, false).setExecute(true);
                    program.getFunctionManager().createFunction("FUN_" + offset, address,
                        new AddressSet(address), SourceType.DEFAULT);
                }
                program.getMemory().createInitializedBlock("adapter", program.getImageBase().add(0x1000), 16,
                    (byte) 0, TaskMonitor.DUMMY, false);
            }
            finally {
                program.endTransaction(setup, true);
            }
            try (JsonWriter writer = new JsonWriter(Files.newBufferedWriter(resources.resolve("behavior-context.json")))) {
                writer.beginObject().name("ebuses").beginArray().beginObject().name("name").value("MB::MemoryBus<AZ::Uuid>");
                writer.name("m_events").beginArray();
                for (int i = 0; i < count; i++) {
                    writer.beginObject().name("name").value("Event" + i).name("m_broadcast").beginObject();
                    writer.name("m_functionPtr").value("NewWorld+0x100");
                    writer.name("virtualDispatchSlotOffset").value("0x8");
                    writer.name("vtable").value("NewWorld+0x1000");
                    writer.name("m_reflectFunction").beginArray().value("NewWorld+0x110").endArray();
                    writer.name("m_attributes").beginArray().beginObject().name("attributeName").value("ToolTip");
                    writer.name("value").beginObject().name("kind").value("string").name("value")
                        .value("Capture metadata " + i + ": " + "x".repeat(4096)).endObject().endObject().endArray();
                    writer.endObject().endObject();
                }
                writer.endArray().endObject().endArray().endObject();
            }
            run(program, resources);
            if (!Files.isRegularFile(resources.resolve("reflection-report.json"))) {
                throw new AssertionError("Complete import report missing");
            }
            for (int offset : List.of(0x100, 0x110, 0x1000)) {
                int histories = program.getListing().getCommentHistory(program.getImageBase().add(offset), CommentType.PLATE).length;
                if (histories > 1) throw new AssertionError("Shared-address comment rewritten " +
                    histories + " times; importer must write one merged comment per address");
            }
            run(program, resources);
            try (JsonReader reader = new JsonReader(Files.newBufferedReader(resources.resolve("reflection-report.json")))) {
                boolean checked = false;
                reader.beginObject();
                while (reader.hasNext()) {
                    if (reader.nextName().equals("appliedCount")) {
                        if (reader.nextInt() != 0) throw new AssertionError("Shared-function rerun is not idempotent");
                        checked = true;
                    }
                    else reader.skipValue();
                }
                reader.endObject();
                if (!checked) throw new AssertionError("Report appliedCount missing");
            }
            for (int offset : List.of(0x100, 0x110, 0x1000)) {
                if (program.getListing().getCommentHistory(program.getImageBase().add(offset), CommentType.PLATE).length > 1) {
                    throw new AssertionError("Rerun rewrote an unchanged shared-address comment");
                }
            }
            System.out.println("PASS: " + count + " shared implementations applied under " +
                Runtime.getRuntime().maxMemory() / (1024 * 1024) + " MiB heap");
        }
        finally {
            program.release(consumer);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void run(ProgramDB program, Path resources) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
        Field spools = AzReflectionRenamer.class.getDeclaredField("temporarySpools");
        spools.setAccessible(true);
        if (!((List<?>) spools.get(script)).isEmpty()) throw new AssertionError("Report spools leaked");
    }
}
