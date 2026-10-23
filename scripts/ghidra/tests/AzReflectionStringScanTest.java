import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Map;

import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.*;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.exception.CancelledException;
import ghidra.util.task.TaskMonitor;
import ghidra.util.task.TaskMonitorAdapter;

/** Exercises string evidence through the importer; it does not simulate Swing dispatch. */
public class AzReflectionStringScanTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-string-scan-test-");
        String mode = args.length > 2 ? args[2] : "all";
        try {
            Address base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
            ArrayList<String> typeNames = new ArrayList<>(java.util.List.of(
                "MB::ClassType", "Javelin::StructType", "MB::EnumType", "Javelin::UnqualifiedType"));
            ArrayList<String> arguments = new ArrayList<>(java.util.List.of(
                "class " + typeNames.get(0), "struct " + typeNames.get(1),
                "enum " + typeNames.get(2), typeNames.get(3)));
            String longType = "AZStd::tuple<" + java.util.stream.IntStream.range(0, 48)
                .mapToObj(i -> "Javelin::Value" + i).collect(java.util.stream.Collectors.joining(",")) + ">";
            typeNames.add(longType);
            arguments.add("class " + longType);
            int tx = program.startTransaction("string fixture");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("data", base.add(0x2000), 0xc000, (byte) 0,
                    TaskMonitor.DUMMY, false);
                program.getListing().createData(base.add(0x2000), IntegerDataType.dataType);
                program.getListing().createData(base.add(0x2010), new PointerDataType(IntegerDataType.dataType, 8));
                StructureDataType structure = new StructureDataType("NonString", 0);
                structure.add(IntegerDataType.dataType, "value", null);
                program.getListing().createData(base.add(0x2020), structure);
                for (int i = 0; i < arguments.size(); i++) {
                    Address text = base.add(0x3000 + i * 0x800);
                    boolean wide = i == 2;
                    putString(program, text, "const char *__cdecl MB::GetTypeName<" + arguments.get(i) + ">(void)",
                        wide ? TerminatedUnicodeDataType.dataType : TerminatedStringDataType.dataType, wide);
                    Address function = base.add(0x100 + i * 0x10);
                    program.getFunctionManager().createFunction("FUN_" + i, function, new AddressSet(function), SourceType.DEFAULT);
                    program.getReferenceManager().addMemoryReference(function, text, RefType.DATA, SourceType.USER_DEFINED, 0);
                }
                putString(program, base.add(0x7000), "duplicate", TerminatedStringDataType.dataType, false);
                putString(program, base.add(0x7100), "duplicate", new TypedefDataType("StringAlias", new ArrayDataType(CharDataType.dataType, 10, 1)), false);
                putString(program, base.add(0x7200), "array_text", new ArrayDataType(CharDataType.dataType, 11, 1), false);
                putString(program, base.add(0x7300), "Unicode \u03a9", TerminatedUnicodeDataType.dataType, true);
                putString(program, base.add(0x8000), "irrelevant".repeat(700), TerminatedStringDataType.dataType, false);
                putString(program, base.add(0xa000), "b18d903b-7fad-4a53-918a-3967b3198224", TerminatedStringDataType.dataType, false);
                Address longUuid = base.add(0xa100);
                putString(program, longUuid, "prefix".repeat(100) + "{B18D903B-7FAD-4A53-918A-3967B3198224}",
                    TerminatedStringDataType.dataType, false);
                Address cast = base.add(0x160);
                program.getMemory().setBytes(cast, new byte[]{0x48, (byte) 0x8b, 0x40, 0x20,
                    0x48, (byte) 0xff, (byte) 0xe0, 0x33, (byte) 0xc0, (byte) 0xc3});
                check(new DisassembleCommand(cast, null, true).applyTo(program, TaskMonitor.DUMMY) &&
                    new DisassembleCommand(cast.add(7), null, true).applyTo(program, TaskMonitor.DUMMY),
                    "could not decode core RTTI cast fixture");
                program.getFunctionManager().createFunction("FUN_cast", cast, new AddressSet(cast, cast.add(9)), SourceType.DEFAULT);
                program.getReferenceManager().addMemoryReference(cast, longUuid, RefType.DATA, SourceType.USER_DEFINED, 0);
            }
            finally { program.endTransaction(tx, true); }
            Files.writeString(resources.resolve("serialize.json"), "{\"uuidMap\":{},\"classData\":[]}");
            Stats stats = new Stats();
            TaskMonitorAdapter monitor = new TaskMonitorAdapter(true);
            Program wrapped = wrapped(program, stats, monitor, mode.equals("all") || mode.equals("guard"));
            if (mode.equals("cancellation")) {
                cancellation(wrapped, stats, monitor);
                System.out.println("PASS: cancelled string indexing propagates and a retry rebuilds the complete cache");
                return;
            }
            AzReflectionRenamer script = script(wrapped, monitor);
            long symbols = program.getSymbolTable().getNumSymbols();
            int types = program.getDataTypeManager().getDataTypeCount(true);
            run(script, resources, true);
            check(program.getSymbolTable().getNumSymbols() == symbols && program.getDataTypeManager().getDataTypeCount(true) == types,
                "string scan preview changed the database");
            check(JsonParser.parseString(Files.readString(resources.resolve("reflection-dry-run.json")))
                .getAsJsonObject().get("status").getAsString().equals("passed"), "string scanning failed validation");
            stats.scans = stats.stringReads = 0;
            run(script, resources, false);
            if (mode.equals("all") || mode.equals("variants")) {
                for (int i = 0; i < typeNames.size(); i++) {
                    Function function = program.getFunctionManager().getFunctionAt(base.add(0x100 + i * 0x10));
                    check(function.getName(true).equals("MB::GetTypeName<" + typeNames.get(i) + ">"),
                        "MB string spelling or long generic was missed: " + arguments.get(i));
                }
                check(program.getFunctionManager().getFunctionAt(base.add(0x160)).getName(true).equals("AZ::RttiCast<ReflectContext>"),
                    "core RTTI UUID evidence in a long string was lost");
            }
            if (mode.equals("all") || mode.equals("scan-count")) {
                check(stats.scans == 1, "validation and application repeated the full data scan: " + stats.scans);
                check(stats.stringReads == 12, "string values were decoded more than once: " + stats.stringReads);
                @SuppressWarnings("unchecked")
                Map<String, ArrayList<Address>> index = (Map<String, ArrayList<Address>>) call(script, "definedStringAddressesByValue");
                check(index.get("duplicate").size() == 2 && index.containsKey("array_text") && index.containsKey("Unicode \u03a9"),
                    "string array, typedef, Unicode or duplicate addresses were lost");
                check(index.keySet().stream().noneMatch(value -> value.length() > 512), "unbounded strings retained in the value index");
                check(stats.scans == 1, "cached string lookup scanned data again");
            }
            System.out.println("PASS: string evidence skips non-string values, scans once across preview/apply, preserves ASCII/Unicode/" +
                "arrays/typedefs/duplicates, and imports class/struct/enum/unqualified MB generics over 512 characters");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void cancellation(Program program, Stats stats, TaskMonitorAdapter monitor) throws Exception {
        AzReflectionRenamer script = script(program, monitor);
        stats.cancelAfter = 5;
        try { call(script, "definedStringAddressesByValue"); throw new AssertionError("string indexing swallowed cancellation"); }
        catch (CancelledException expected) { }
        stats.cancelAfter = 0;
        monitor.clearCancelled();
        @SuppressWarnings("unchecked")
        Map<String, ArrayList<Address>> index = (Map<String, ArrayList<Address>>) call(script, "definedStringAddressesByValue");
        check(index.get("duplicate").size() == 2 && index.containsKey("Unicode \u03a9") && stats.scans == 2,
            "retry reused a partially populated string cache");
        monitor.cancel();
        try { call(script, "definedStringAddressesByValue"); throw new AssertionError("cache hit swallowed cancellation"); }
        catch (CancelledException expected) { }
    }

    private static void putString(Program program, Address address, String text, DataType type, boolean wide) throws Exception {
        byte[] bytes = (text + "\0").getBytes(wide ? StandardCharsets.UTF_16LE : StandardCharsets.US_ASCII);
        program.getMemory().setBytes(address, bytes);
        if (type == TerminatedStringDataType.dataType || type == TerminatedUnicodeDataType.dataType)
            program.getListing().createData(address, type);
        else program.getListing().createData(address, type, bytes.length);
        check(program.getListing().getDataAt(address).hasStringValue(), "fixture is not string-valued: " + type);
    }

    private static Program wrapped(Program program, Stats stats, TaskMonitorAdapter monitor, boolean guard) {
        Listing listing = (Listing) Proxy.newProxyInstance(Listing.class.getClassLoader(), new Class<?>[]{Listing.class},
            (proxy, method, args) -> {
                Object result = forward(program.getListing(), method, args);
                if (!method.getName().equals("getDefinedData")) return result;
                stats.scans++;
                DataIterator actual = (DataIterator) result;
                return new DataIterator() {
                    int count;
                    public boolean hasNext() { return actual.hasNext(); }
                    public Data next() {
                        if (stats.cancelAfter > 0 && ++count == stats.cancelAfter) monitor.cancel();
                        Data data = actual.next();
                        return (Data) Proxy.newProxyInstance(Data.class.getClassLoader(), new Class<?>[]{Data.class},
                            (view, operation, arguments) -> {
                                if (operation.getName().equals("getValue")) {
                                    if (!data.hasStringValue() && guard)
                                        throw new AssertionError("non-string value materialized during reflection string search at " + data.getAddress());
                                    if (data.hasStringValue()) stats.stringReads++;
                                }
                                return forward(data, operation, arguments);
                            });
                    }
                };
            });
        return (Program) Proxy.newProxyInstance(Program.class.getClassLoader(), new Class<?>[]{Program.class},
            (proxy, method, args) -> method.getName().equals("getListing") ? listing : forward(program, method, args));
    }

    private static AzReflectionRenamer script(Program program, TaskMonitor monitor) {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), monitor, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        return script;
    }

    private static void run(AzReflectionRenamer script, Path resources, boolean preview) throws Exception {
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} :
            new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
    }

    private static Object call(Object target, String name) throws Exception {
        Method method = target.getClass().getDeclaredMethod(name);
        method.setAccessible(true);
        try { return method.invoke(target); }
        catch (InvocationTargetException error) {
            if (error.getCause() instanceof Exception exception) throw exception;
            if (error.getCause() instanceof Error failure) throw failure;
            throw error;
        }
    }

    private static Object forward(Object target, Method method, Object[] args) throws Throwable {
        try { return method.invoke(target, args); }
        catch (InvocationTargetException error) { throw error.getCause(); }
    }

    private static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
    private static class Stats { int scans, stringReads, cancelAfter; }
}
