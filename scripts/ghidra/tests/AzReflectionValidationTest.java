import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.TerminatedStringDataType;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Program;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;
import ghidra.util.task.TaskMonitorAdapter;
import ghidra.util.exception.CancelledException;

/** Runs the inferred RTTI scan against real native memory, including missing vtable slots. */
public class AzReflectionValidationTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-validation-test-");
        try {
            Address base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
            Address table = base.add(0x2000);
            Address string = table.add(0x50);
            int setup = program.startTransaction("native RTTI fixture");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("rtti", table, 0x100, (byte) 0, TaskMonitor.DUMMY, false);
                byte[] getter = ByteBuffer.allocate(11).order(ByteOrder.LITTLE_ENDIAN)
                    .put((byte) 0x48).put((byte) 0xb8).putLong(string.getOffset()).put((byte) 0xc3).array();
                program.getMemory().setBytes(base.add(0x110), getter);
                program.getMemory().setBytes(base.add(0x160), ByteBuffer.allocate(9).order(ByteOrder.LITTLE_ENDIAN)
                    .put(new byte[]{0x48, (byte) 0x83, (byte) 0xe9, 8, (byte) 0xe9}).putInt(0x100 - 0x169).array());
                program.getMemory().setBytes(base.add(0x170), ByteBuffer.allocate(5).order(ByteOrder.LITTLE_ENDIAN)
                    .put((byte) 0xe9).putInt(0x100 - 0x175).array());
                check(new DisassembleCommand(base.add(0x160), null, true).applyTo(program, TaskMonitor.DUMMY),
                    "could not decode this-adjust fixture");
                check(new DisassembleCommand(base.add(0x170), null, true).applyTo(program, TaskMonitor.DUMMY),
                    "could not decode direct-jump fixture");
                // AZ_RTTI block (AddressOf overloads folded), then unmapped and null words the scan must reject.
                byte[] pointers = ByteBuffer.allocate(64).order(ByteOrder.LITTLE_ENDIAN)
                    .putLong(base.add(0x100).getOffset()).putLong(base.add(0x110).getOffset())
                    .putLong(base.add(0x120).getOffset()).putLong(base.add(0x130).getOffset())
                    .putLong(base.add(0x140).getOffset()).putLong(base.add(0x140).getOffset())
                    .putLong(0xdeadbeefL).putLong(0).array();
                program.getMemory().setBytes(table, pointers);
                program.getReferenceManager().addMemoryReference(base.add(0x180), table, RefType.DATA,
                    SourceType.IMPORTED, 0);
                String name = "MB::Block<Javelin::Key>";
                program.getMemory().setBytes(string, (name + "\0").getBytes(StandardCharsets.US_ASCII));
                program.getListing().createData(string, TerminatedStringDataType.dataType);
                for (long offset : new long[]{0x100, 0x110, 0x120, 0x130, 0x140}) {
                    Address entry = base.add(offset);
                    program.getFunctionManager().createFunction("FUN_" + Long.toHexString(offset), entry,
                        new AddressSet(entry, entry.add(offset == 0x110 ? 10 : 0)), SourceType.DEFAULT);
                }
            }
            finally { program.endTransaction(setup, true); }
            Files.writeString(resources.resolve("serialize.json"), "{\"uuidMap\":{},\"classData\":[]}");
            int types = program.getDataTypeManager().getDataTypeCount(true);
            long symbols = program.getSymbolTable().getNumSymbols();
            AzReflectionRenamer preview = script(program);
            run(preview, resources, "--dry-run");
            check(program.getDataTypeManager().getDataTypeCount(true) == types &&
                program.getSymbolTable().getNumSymbols() == symbols, "native dry run changed the program");
            JsonObject diagnostics = diagnostics(resources);
            check(diagnostics.get("status").getAsString().equals("passed") &&
                diagnostics.get("appliedCount").getAsInt() == 0 &&
                diagnostics.getAsJsonObject("rejectedNativeCodePointers").get("null").getAsInt() > 0 &&
                diagnostics.getAsJsonObject("rejectedNativeCodePointers").get("unmapped").getAsInt() > 0,
                "saved dry run lacks native pointer diagnostics");
            check(!Files.exists(resources.resolve("reflection-report.json")), "dry run overwrote the applied report");
            check((boolean) call(preview, "isInferredStaticRttiHelperVtable", table, string),
                "valid AZ_RTTI block was discarded");
            for (Address invalid : new Address[]{null, base.add(0x9000), table}) {
                check(call(preview, "thisAdjustThunk", invalid) == null, "invalid thunk was decoded");
                check(call(preview, "directJumpTarget", invalid) == null, "invalid jump was decoded");
            }
            check(call(preview, "resolvedThunkTarget", (Address) null) == null, "null target was retained");
            check(call(preview, "resolvedThunkTarget", base.add(0x9000)) == null, "unmapped target was retained");
            check(call(preview, "resolvedThunkTarget", base.add(0x160)).equals(base.add(0x100)) &&
                call(preview, "resolvedThunkTarget", base.add(0x170)).equals(base.add(0x100)),
                "valid decoded thunks no longer resolve");
            checkFailure(program, resources, false);
            checkFailure(program, resources, true);
            checkCancellation(program, resources);
            run(script(program), resources);
            check(program.getSymbolTable().getSymbols(table).length > 0, "native inferred RTTI table was not labeled");
            check(diagnostics(resources).get("status").getAsString().equals("passed"), "normal retry did not recover");
            System.out.println("PASS: actual inferred RTTI scan, zero/unmapped/non-code slots, AZ_RTTI blocks, decoded thunks, " +
                "saved preview, pre-edit validation failure, apply rollback, cancellation, prior-report preservation and retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void checkFailure(ProgramDB program, Path resources, boolean applyOnly) throws Exception {
        String previous = "{\"previous\":true}";
        Files.writeString(resources.resolve("reflection-report.json"), previous);
        int types = program.getDataTypeManager().getDataTypeCount(true);
        long symbols = program.getSymbolTable().getNumSymbols();
        int functions = program.getFunctionManager().getFunctionCount();
        Address getter = program.getImageBase().add(0x110);
        Listing actual = program.getListing();
        Listing listing = (Listing) Proxy.newProxyInstance(Listing.class.getClassLoader(), new Class<?>[]{Listing.class},
            (proxy, method, values) -> {
                if (method.getName().equals("getInstructionAt") && getter.equals(values[0]) &&
                    (!applyOnly || program.getDataTypeManager().getDataTypeCount(true) > types)) {
                    throw new IllegalStateException("injected native RTTI inspection failure");
                }
                return forward(actual, method, values);
            });
        Program wrapped = (Program) Proxy.newProxyInstance(Program.class.getClassLoader(), new Class<?>[]{Program.class},
            (proxy, method, values) -> method.getName().equals("getListing") ? listing : forward(program, method, values));
        try {
            run(script(wrapped), resources);
            check(!applyOnly, "apply-only native error was not exercised");
        }
        catch (IllegalStateException expected) { check(applyOnly, "validation exception escaped the dry run"); }
        check(program.getDataTypeManager().getDataTypeCount(true) == types &&
            program.getSymbolTable().getNumSymbols() == symbols &&
            program.getFunctionManager().getFunctionCount() == functions, "failed run left partial program edits");
        check(Files.readString(resources.resolve("reflection-report.json")).equals(previous), "previous applied report replaced");
        JsonObject report = diagnostics(resources);
        check(report.get("status").getAsString().equals("failed") && report.get("errorCount").getAsInt() == 1,
            "failure not saved in diagnostics");
        JsonObject error = report.getAsJsonArray("errors").get(0).getAsJsonObject();
        check(error.get("phase").getAsString().equals("inferred-static-rtti") &&
            error.get("message").getAsString().contains("injected native RTTI") &&
            error.getAsJsonArray("stackTrace").toString().contains("thisAdjustThunk"),
            "diagnostic lost phase, message or the native call stack");
        check(report.getAsJsonArray("phases").size() == 11, "validation did not cover every reflection phase");
        if (applyOnly) check(report.get("failurePass").getAsString().equals("apply"), "apply failure mislabeled as validation");
        checkNoScratch(resources);
    }

    private static void checkCancellation(ProgramDB program, Path resources) throws Exception {
        int types = program.getDataTypeManager().getDataTypeCount(true);
        long symbols = program.getSymbolTable().getNumSymbols();
        String previous = Files.readString(resources.resolve("reflection-report.json"));
        TaskMonitorAdapter monitor = new TaskMonitorAdapter(true) {
            @Override public void setMessage(String message) {
                if (message.equals("Validating: inferred-static-rtti")) cancel();
            }
        };
        AzReflectionRenamer script = script(program);
        script.set(script.getState(), monitor, new PrintWriter(new StringWriter()));
        try { run(script, resources); throw new AssertionError("cancelled validation continued to apply"); }
        catch (CancelledException expected) { }
        check(program.getDataTypeManager().getDataTypeCount(true) == types &&
            program.getSymbolTable().getNumSymbols() == symbols &&
            Files.readString(resources.resolve("reflection-report.json")).equals(previous), "cancellation left program edits");
        check(diagnostics(resources).get("status").getAsString().equals("cancelled"), "cancellation not saved");
        checkNoScratch(resources);
    }

    private static Object forward(Object target, Method method, Object[] arguments) throws Throwable {
        try { return method.invoke(target, arguments); }
        catch (InvocationTargetException error) { throw error.getCause(); }
    }

    private static JsonObject diagnostics(Path resources) throws Exception {
        return JsonParser.parseString(Files.readString(resources.resolve("reflection-dry-run.json"))).getAsJsonObject();
    }

    private static void checkNoScratch(Path resources) throws Exception {
        try (var paths = Files.list(resources)) {
            check(paths.noneMatch(path -> path.getFileName().toString().startsWith(".az-reflection-") ||
                path.getFileName().toString().startsWith(".reflection-validation-")), "temporary report files leaked");
        }
    }

    private static AzReflectionRenamer script(Program program) {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY,
            new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        return script;
    }

    private static void run(AzReflectionRenamer script, Path resources, String... extra) throws Exception {
        var arguments = new java.util.ArrayList<String>(java.util.List.of("--resources", resources.toString()));
        arguments.addAll(java.util.List.of(extra));
        script.setScriptArgs(arguments.toArray(String[]::new));
        script.start();
        try { script.run(); }
        finally { script.end(true); }
    }

    private static Object call(Object target, String name, Address... values) throws Exception {
        Class<?>[] parameters = new Class<?>[values.length];
        java.util.Arrays.fill(parameters, Address.class);
        Method method = AzReflectionRenamer.class.getDeclaredMethod(name, parameters);
        method.setAccessible(true);
        try { return method.invoke(target, (Object[]) values); }
        catch (InvocationTargetException error) {
            if (error.getCause() instanceof Exception cause) throw cause;
            throw error;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
