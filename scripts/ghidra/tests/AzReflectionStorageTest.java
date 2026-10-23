import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.io.IOException;
import java.io.Writer;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Uses a disposable database and never fills a real disk. */
public class AzReflectionStorageTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-storage-test-");
        try {
            int setup = program.startTransaction("storage fixture");
            Address address = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000100L);
            try {
                program.setImageBase(address.subtract(0x100), false);
                program.getMemory().createInitializedBlock("fixture", address, 16, (byte) 0xc3, TaskMonitor.DUMMY, false).setExecute(true);
                program.getFunctionManager().createFunction("FUN_fixture", address, new AddressSet(address), SourceType.DEFAULT);
            }
            finally { program.endTransaction(setup, true); }
            checkSharedAliasStorage(program, resources, args.length > 2 ? Integer.parseInt(args[2]) : 1000);
            Files.writeString(resources.resolve("behavior-context.json"), """
                {"ebuses":[{"name":"MB::StorageBus","m_events":[{"name":"Run",
                 "m_broadcast":{"m_functionPtr":"NewWorld+0x100","virtualDispatchSlotOffset":"0x8"}}]}]}
                """);
            for (Fault fault : Fault.values()) checkFailure(program, resources, fault);
            AzReflectionRenamer success = script(program);
            run(success, resources);
            var report = JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
            if (report.get("appliedCount").getAsInt() == 0 || report.get("reportFormatVersion").getAsInt() != 2) {
                throw new AssertionError("A normal rerun did not recover after storage failures");
            }
            checkNoScratch(resources);
            System.out.println("PASS: linear shared-alias report, report-local scratch, low-space preflight, " +
                "mid-spool and mid-report failures, program rollback, previous-report preservation, scratch cleanup and recovery");
        }
        finally {
            program.release(consumer);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void checkSharedAliasStorage(ProgramDB program, Path resources, int count) throws Exception {
        String originalName = program.getFunctionManager().getFunctionAt(program.getImageBase().add(0x100)).getName(true);
        AzReflectionRenamer script = script(program);
        set(script, "symbols", program.getSymbolTable());
        set(script, "scopeCache", new HashMap<>());
        set(script, "targetNameOwners", new HashMap<>());
        set(script, "gson", new GsonBuilder().create());
        set(script, "applyRenames", true);
        set(script, "outputFile", resources.resolve("reflection-report.json").toFile());
        Class<?> groupClass = nested("SlotGroup");
        Constructor<?> groupConstructor = groupClass.getDeclaredConstructor(String.class);
        groupConstructor.setAccessible(true);
        Object group = groupConstructor.newInstance("NewWorld+0x100");
        set(group, "useCount", count);
        Class<?> sinkClass = nested("ActionSink");
        Constructor<?> sinkConstructor = sinkClass.getDeclaredConstructor(AzReflectionRenamer.class, boolean.class);
        sinkConstructor.setAccessible(true);
        Object sink = sinkConstructor.newInstance(script, true);
        Method add = AzReflectionRenamer.class.getDeclaredMethod("addFunctionAlias", String.class, List.class,
            String.class, groupClass, java.util.Set.class, sinkClass);
        add.setAccessible(true);
        script.start();
        try {
            HashSet<String> seen = new HashSet<>();
            for (int i = 0; i < count; i++) {
                add.invoke(script, "NewWorld+0x100", List.of("MB", "SharedOwner<T" + i + ">"),
                    "Destroy", group, seen, sink);
            }
            Object spool = field(sink, "fullActionSpool");
            Method finish = spool.getClass().getDeclaredMethod("finish");
            finish.setAccessible(true);
            finish.invoke(spool);
            Path path = (Path) field(spool, "path");
            long bytes = Files.size(path);
            boolean local = path.toAbsolutePath().normalize().startsWith(resources.toAbsolutePath().normalize());
            if (bytes > count * 1500L || !local) {
                throw new AssertionError(count + " aliases wrote " + bytes + " bytes in " + path +
                    "; expected under " + count * 1500L + " bytes beside the report");
            }
            Object snapshots = field(sink, "symbolNameSnapshots");
            Method read = snapshots.getClass().getDeclaredMethod("readArray");
            read.setAccessible(true);
            JsonArray names = (JsonArray) read.invoke(snapshots);
            if (names.size() != 1 || names.get(0).getAsJsonObject().getAsJsonArray("names").size() != 1 ||
                !names.get(0).getAsJsonObject().getAsJsonArray("names").get(0).getAsString().equals(originalName)) {
                throw new AssertionError("Original symbol names were not retained exactly once");
            }
            JsonArray actions = (JsonArray) read.invoke(spool);
            for (var action : actions) {
                if (!action.getAsJsonObject().get("oldNamesRef").getAsString()
                    .equals(names.get(0).getAsJsonObject().get("address").getAsString())) {
                    throw new AssertionError("Alias action cannot resolve its original names");
                }
            }
            System.out.println(count + " alias actions: " + bytes + " bytes, one original-name snapshot");
        }
        finally {
            script.end(false);
            Method close = AzReflectionRenamer.class.getDeclaredMethod("closeTemporarySpools");
            close.setAccessible(true);
            close.invoke(script);
        }
    }

    private enum Fault { PREFLIGHT, SPOOL, REPORT, SPACE_DURING_WRITE }

    private static class FaultScript extends AzReflectionRenamer {
        final Fault fault;
        final ProgramDB program;
        final int originalTypes;
        boolean failed;
        boolean sawMutation;
        boolean writerOpened;

        FaultScript(Fault fault, ProgramDB program) {
            this.fault = fault;
            this.program = program;
            this.originalTypes = program.getDataTypeManager().getDataTypeCount(true);
        }

        @Override protected long availableSpace(Path directory) throws IOException {
            if (fault == Fault.PREFLIGHT || (fault == Fault.SPACE_DURING_WRITE && writerOpened)) {
                failed = true;
                sawMutation |= program.getDataTypeManager().getDataTypeCount(true) > originalTypes;
                return 0;
            }
            return super.availableSpace(directory);
        }

        @Override protected Writer openReportWriter(Path path) throws IOException {
            writerOpened = true;
            Writer actual = super.openReportWriter(path);
            boolean inject = fault == Fault.SPOOL && path.getFileName().toString().startsWith("actions-") ||
                fault == Fault.REPORT && path.getFileName().toString().startsWith(".reflection-report-");
            if (!inject) return actual;
            return new java.io.FilterWriter(actual) {
                private long characters;

                private void check(int length) throws IOException {
                    characters += length;
                    if (characters > 200) {
                        failed = true;
                        sawMutation |= program.getDataTypeManager().getDataTypeCount(true) > originalTypes;
                        throw new IOException("There is not enough space on the disk (injected)");
                    }
                }

                @Override public void write(int value) throws IOException { check(1); out.write(value); }
                @Override public void write(String value, int offset, int length) throws IOException {
                    check(length); out.write(value, offset, length);
                }
                @Override public void write(char[] value, int offset, int length) throws IOException {
                    check(length); out.write(value, offset, length);
                }
            };
        }
    }

    private static void checkFailure(ProgramDB program, Path resources, Fault fault) throws Exception {
        String previous = "{\"previous\":true}";
        Files.writeString(resources.resolve("reflection-report.json"), previous);
        int originalTypes = program.getDataTypeManager().getDataTypeCount(true);
        String originalName = program.getFunctionManager().getFunctionAt(program.getImageBase().add(0x100)).getName(true);
        int originalSymbols = program.getSymbolTable().getSymbols(program.getImageBase().add(0x100)).length;
        FaultScript script = new FaultScript(fault, program);
        StringWriter console = new StringWriter();
        initialize(script, program, console);
        run(script, resources);
        if (!script.failed) throw new AssertionError("Storage fault was not exercised: " + fault);
        if (fault != Fault.PREFLIGHT && fault != Fault.SPACE_DURING_WRITE && !script.sawMutation) {
            throw new AssertionError("Fault must exercise rollback after real program mutations: " + fault);
        }
        if (program.getDataTypeManager().getDataTypeCount(true) != originalTypes ||
            !program.getFunctionManager().getFunctionAt(program.getImageBase().add(0x100)).getName(true).equals(originalName) ||
            program.getSymbolTable().getSymbols(program.getImageBase().add(0x100)).length != originalSymbols) {
            throw new AssertionError("Storage fault left partial edits: " + fault);
        }
        if (!Files.readString(resources.resolve("reflection-report.json")).equals(previous)) {
            throw new AssertionError("Previous report replaced on failure: " + fault);
        }
        if (!console.toString().contains("rolled back") || console.toString().contains("Non-conflicting actions were applied")) {
            throw new AssertionError("Storage fault produced a misleading completion message: " + fault);
        }
        checkNoScratch(resources);
    }

    private static void run(AzReflectionRenamer script, Path resources) throws Exception {
        script.setScriptArgs(new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
        if (!((List<?>) field(script, "temporarySpools")).isEmpty()) throw new AssertionError("Spool handles leaked");
    }

    private static void checkNoScratch(Path resources) throws Exception {
        try (var files = Files.list(resources)) {
            if (files.anyMatch(path -> path.getFileName().toString().startsWith(".az-reflection-") ||
                path.getFileName().toString().startsWith(".reflection-report-") ||
                path.getFileName().toString().startsWith(".reflection-validation-") ||
                path.getFileName().toString().startsWith(".reflection-write-check-"))) {
                throw new AssertionError("Temporary report files leaked");
            }
        }
    }

    private static AzReflectionRenamer script(ProgramDB program) {
        AzReflectionRenamer script = new AzReflectionRenamer();
        initialize(script, program, new StringWriter());
        return script;
    }

    private static void initialize(AzReflectionRenamer script, ProgramDB program, StringWriter console) {
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY,
            new PrintWriter(console));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
    }

    private static Class<?> nested(String name) {
        for (Class<?> type : AzReflectionRenamer.class.getDeclaredClasses()) {
            if (type.getSimpleName().equals(name)) return type;
        }
        throw new AssertionError(name);
    }

    private static Object field(Object object, String name) throws Exception {
        Class<?> type = object.getClass();
        while (type != null) {
            try { return value(object, type.getDeclaredField(name)); }
            catch (NoSuchFieldException ignored) { type = type.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    private static Object value(Object object, Field field) throws Exception {
        field.setAccessible(true);
        return field.get(object);
    }

    private static void set(Object object, String name, Object value) throws Exception {
        Field field = object.getClass().getDeclaredField(name);
        field.setAccessible(true);
        field.set(object, value);
    }
}
