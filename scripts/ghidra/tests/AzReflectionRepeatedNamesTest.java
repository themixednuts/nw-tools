import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
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
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Repeated C++ qualified names are valid for distinct functions and data addresses. */
public class AzReflectionRepeatedNamesTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-repeated-names-test-");
        try {
            Address base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
            int tx = program.startTransaction("repeated qualified names");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x400, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("data", base.add(0x2000), 0x3000, (byte) 0,
                    TaskMonitor.DUMMY, false);
                for (int i = 0; i < 2; i++) {
                    Address table = base.add(0x2000 + i * 0x200), text = table.add(0x50);
                    long code = 0x100 + i * 0x80;
                    program.getMemory().setBytes(text, "MB::Same<Javelin::Key>\0".getBytes(StandardCharsets.US_ASCII));
                    program.getListing().createData(text, TerminatedStringDataType.dataType);
                    // AZ_RTTI block: GetType, GetTypeName, IsTypeOf, EnumTypes and both folded AddressOf overloads.
                    program.getMemory().setBytes(table, ByteBuffer.allocate(48).order(ByteOrder.LITTLE_ENDIAN)
                        .putLong(base.add(code).getOffset()).putLong(base.add(code + 0x10).getOffset())
                        .putLong(base.add(code + 0x20).getOffset()).putLong(base.add(code + 0x30).getOffset())
                        .putLong(base.add(code + 0x40).getOffset()).putLong(base.add(code + 0x40).getOffset()).array());
                    program.getReferenceManager().addMemoryReference(base.add(code + 0x50), table, RefType.DATA,
                        SourceType.USER_DEFINED, 0);
                    program.getMemory().setBytes(base.add(code + 0x10), ByteBuffer.allocate(11).order(ByteOrder.LITTLE_ENDIAN)
                        .put((byte) 0x48).put((byte) 0xb8).putLong(text.getOffset()).put((byte) 0xc3).array());
                    for (long offset : new long[]{code, code + 0x10, code + 0x20, code + 0x30, code + 0x40}) {
                        Address entry = base.add(offset);
                        program.getFunctionManager().createFunction("FUN_" + offset, entry,
                            new AddressSet(entry, entry.add(offset == code + 0x10 ? 10 : 0)), SourceType.DEFAULT);
                    }
                }
                Address signature = base.add(0x4000);
                program.getMemory().setBytes(signature, "const char *__cdecl MB::GetTypeName<class Javelin::Key>(void)\0"
                    .getBytes(StandardCharsets.US_ASCII));
                program.getListing().createData(signature, TerminatedStringDataType.dataType);
                Namespace mb = program.getSymbolTable().createNameSpace(program.getGlobalNamespace(), "MB", SourceType.USER_DEFINED);
                Namespace repeated = program.getSymbolTable().createNameSpace(mb, "MB", SourceType.USER_DEFINED);
                Namespace duplicate = program.getSymbolTable().createClass(repeated, "Same", SourceType.USER_DEFINED);
                for (long offset : new long[]{0x300, 0x310, 0x320, 0x330}) {
                    Address entry = base.add(offset);
                    if (offset >= 0x320) program.getFunctionManager().createFunction("Run", duplicate, entry,
                        new AddressSet(entry), SourceType.USER_DEFINED);
                    else {
                        program.getFunctionManager().createFunction("FUN_" + offset, entry, new AddressSet(entry), SourceType.DEFAULT);
                        program.getReferenceManager().addMemoryReference(entry, signature, RefType.DATA, SourceType.USER_DEFINED, 0);
                    }
                }
            }
            finally { program.endTransaction(tx, true); }
            Files.writeString(resources.resolve("serialize.json"), "{\"uuidMap\":{},\"classData\":[]}");
            long symbols = program.getSymbolTable().getNumSymbols();
            long[] offsets = {0x100, 0x110, 0x130, 0x140, 0x180, 0x190, 0x1b0, 0x1c0, 0x300, 0x310, 0x320, 0x330};
            long[] ids = Arrays.stream(offsets).map(offset -> program.getFunctionManager().getFunctionAt(base.add(offset)).getID()).toArray();
            run(program, resources, true);
            check(program.getSymbolTable().getNumSymbols() == symbols, "preview changed symbols");
            run(program, resources, false);
            for (long offset : new long[]{0x100, 0x180})
                check(program.getFunctionManager().getFunctionAt(base.add(offset)).getName(true)
                    .equals("MB::Same<Javelin::Key>::GetType"), "duplicate RTTI function was skipped");
            for (long offset : new long[]{0x2000, 0x2200})
                check(Arrays.stream(program.getSymbolTable().getSymbols(base.add(offset))).anyMatch(symbol ->
                    symbol.getName(true).equals("MB::Same<Javelin::Key>::vftable")), "duplicate vtable was skipped");
            for (long offset : new long[]{0x300, 0x310})
                check(program.getFunctionManager().getFunctionAt(base.add(offset)).getName(true).equals("MB::GetTypeName<Javelin::Key>"),
                    "duplicate MB generic function was skipped");
            for (long offset : new long[]{0x320, 0x330})
                check(program.getFunctionManager().getFunctionAt(base.add(offset)).getName(true).equals("MB::Same::Run"),
                    "duplicate namespace cleanup was skipped");
            long after = program.getSymbolTable().getNumSymbols();
            JsonObject report = JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
            check(!report.getAsJsonObject("actionReasonCounts").has("duplicate-target-name"), "valid names still recorded as unresolved skips");
            run(program, resources, false);
            check(program.getSymbolTable().getNumSymbols() == after, "retry duplicated symbols");
            for (int i = 0; i < offsets.length; i++)
                check(program.getFunctionManager().getFunctionAt(base.add(offsets[i])).getID() == ids[i], "function identity changed");
            System.out.println("PASS: same qualified names at distinct native RTTI functions/vtables, MB generic functions and cleaned namespaces, " +
                "no synthetic suffixes, preserved function IDs, preview and retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }
    static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} :
            new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); } finally { script.end(true); }
    }
    static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
