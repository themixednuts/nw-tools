import java.io.File;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;
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

/** Applies the real capture to RET stubs in a disposable database under a fixed heap. */
public class AzReflectionCaptureMemoryTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        Path resources = Files.createTempDirectory("az-reflection-capture-memory-");
        try {
            Files.copy(Path.of(args[1], "behavior-context.json.zip"), resources.resolve("behavior-context.json.zip"));
            TreeSet<Long> implementations = new TreeSet<>();
            TreeSet<Long> evidenceAddresses = new TreeSet<>();
            int uses = indexAddresses(program, resources, implementations, evidenceAddresses);
            int setup = program.startTransaction("real capture address stubs");
            try {
                program.setImageBase(program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
                TreeSet<Long> pages = new TreeSet<>();
                for (long offset : implementations) pages.add(offset & ~0xfffL);
                for (long offset : evidenceAddresses) pages.add(offset & ~0xfffL);
                for (long offset : pages) {
                    program.getMemory().createInitializedBlock("page_" + Long.toHexString(offset),
                        program.getImageBase().add(offset), 4096, (byte) 0xc3, TaskMonitor.DUMMY, false).setExecute(true);
                }
                for (long offset : implementations) {
                    var address = program.getImageBase().add(offset);
                    program.getFunctionManager().createFunction("FUN_" + Long.toHexString(offset),
                        address, new AddressSet(address), SourceType.DEFAULT);
                }
            }
            finally {
                program.endTransaction(setup, true);
            }
            run(program, resources);
            checkReport(resources, uses);
            for (long offset : implementations) assertSingleComment(program, offset);
            for (long offset : evidenceAddresses) assertSingleComment(program, offset);
            System.out.println("PASS: full BehaviorContext application and report, " + uses + " captured function uses, " +
                implementations.size() + " implementation addresses, " + evidenceAddresses.size() +
                " adapter/reflector addresses under " + Runtime.getRuntime().maxMemory() / (1024 * 1024) + " MiB heap");
        }
        finally {
            program.release(consumer);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static int indexAddresses(ProgramDB program, Path resources, Set<Long> implementations,
            Set<Long> evidenceAddresses) throws Exception {
        AzReflectionRenamer script = script(program);
        set(script, "cli", GhidraCli.parse(new String[0], Set.of(), Set.of(), 0));
        set(script, "gson", new GsonBuilder().create());
        try {
            Method load = AzReflectionRenamer.class.getDeclaredMethod("loadBehaviorContextEvidence", File.class);
            load.setAccessible(true);
            Object index = load.invoke(script, resources.resolve("serialize.json").toFile());
            if ((int) field(index, "skippedInputs") != 0) throw new AssertionError("Capture failed to load");
            List<?> candidates = (List<?>) field(index, "functionCandidates");
            for (Object candidate : candidates) {
                addOffset(implementations, (String) field(candidate, "address"));
                JsonObject metadata = (JsonObject) field(candidate, "metadata");
                if (metadata == null) continue;
                if (metadata.has("vtable")) addOffset(evidenceAddresses, metadata.get("vtable").getAsString());
                if (metadata.has("m_reflectFunction") && metadata.get("m_reflectFunction").isJsonArray()) {
                    for (JsonElement value : metadata.getAsJsonArray("m_reflectFunction")) {
                        if (value.isJsonPrimitive()) addOffset(evidenceAddresses, value.getAsString());
                    }
                }
            }
            return candidates.size();
        }
        finally {
            Method close = AzReflectionRenamer.class.getDeclaredMethod("closeTemporarySpools");
            close.setAccessible(true);
            close.invoke(script);
        }
    }

    private static void addOffset(Set<Long> addresses, String text) {
        if (text != null && text.startsWith("NewWorld+0x")) {
            addresses.add(Long.parseUnsignedLong(text.substring("NewWorld+0x".length()), 16));
        }
    }

    private static void run(ProgramDB program, Path resources) throws Exception {
        AzReflectionRenamer script = script(program);
        script.setScriptArgs(new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
        if (!((List<?>) field(script, "temporarySpools")).isEmpty()) throw new AssertionError("Report spools leaked");
    }

    private static AzReflectionRenamer script(ProgramDB program) {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(System.out, true));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        return script;
    }

    private static void assertSingleComment(ProgramDB program, long offset) {
        int histories = program.getListing().getCommentHistory(program.getImageBase().add(offset), CommentType.PLATE).length;
        if (histories > 1) throw new AssertionError("Repeated comment writes at RVA " + Long.toHexString(offset));
    }

    private static void checkReport(Path resources, int uses) throws Exception {
        Set<String> checked = new java.util.HashSet<>();
        try (JsonReader reader = new JsonReader(Files.newBufferedReader(resources.resolve("reflection-report.json")))) {
            reader.beginObject();
            while (reader.hasNext()) {
                switch (reader.nextName()) {
                    case "behaviorContextEvidence" -> {
                        reader.beginObject();
                        while (reader.hasNext()) {
                            if (reader.nextName().equals("inventory")) {
                                reader.beginObject();
                                while (reader.hasNext()) {
                                    String name = reader.nextName();
                                    int expected = switch (name) {
                                        case "classes", "typeToClassMap" -> 611;
                                        case "globalProperties" -> 2647;
                                        case "ebuses" -> 626;
                                        case "onDemandReflection" -> 168;
                                        case "functionUses" -> uses;
                                        default -> -1;
                                    };
                                    if (expected < 0) reader.skipValue();
                                    else {
                                        int actual = countArray(reader);
                                        if (actual != expected) throw new AssertionError(name + ": " + actual + " != " + expected);
                                        checked.add(name);
                                    }
                                }
                                reader.endObject();
                            }
                            else reader.skipValue();
                        }
                        reader.endObject();
                    }
                    case "actions" -> {
                        if (countArray(reader) < uses) throw new AssertionError("Full action report missing");
                        checked.add("actions");
                    }
                    default -> reader.skipValue();
                }
            }
            reader.endObject();
            if (reader.peek() != JsonToken.END_DOCUMENT) throw new AssertionError("Trailing report content");
        }
        if (checked.size() != 7) throw new AssertionError("Missing report inventories: " + checked);
    }

    private static int countArray(JsonReader reader) throws Exception {
        int count = 0;
        reader.beginArray();
        while (reader.hasNext()) { reader.skipValue(); count++; }
        reader.endArray();
        return count;
    }

    private static Object field(Object object, String name) throws Exception {
        Field field = object.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(object);
    }

    private static void set(Object object, String name, Object value) throws Exception {
        Field field = AzReflectionRenamer.class.getDeclaredField(name);
        field.setAccessible(true);
        field.set(object, value);
    }
}
