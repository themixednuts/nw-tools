import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

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
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Replays native RTTI imports over databases with previously created helper aliases. */
public class AzReflectionSymbolRenameTest {
    private enum Case { SOURCE_ALIAS, TARGET_ALIAS, BOTH_ALIASES, SAME_NAMESPACE, NAMESPACE_ONLY, ALREADY_CURRENT, TEMPLATES, LATE_ALIAS }

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        Case[] scenarios = args.length > 2 ? new Case[]{Case.valueOf(args[2])} : Case.values();
        for (Case scenario : scenarios) checkScenario(scenario, Path.of(args[1]));
        System.out.println("PASS: actual RTTI import with source/target/both aliases, same-namespace and namespace-only moves, " +
            "aliases created by an earlier pass, nested generics, preserved function/source-alias IDs, read-only preview and idempotent retry");
    }

    private static void checkScenario(Case scenario, Path work) throws Exception {
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(work, "az-symbol-rename-test-");
        try {
            Address base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x7ff600000000L);
            Address entry = base.add(0x100);
            Address table = base.add(0x2000);
            Address string = table.add(0x50);
            String type = scenario == Case.TEMPLATES ? "MB::Box<AZStd::vector<Javelin::Key>>" : "CurrentCardInfo";
            String targetClass = "RttiHelper<" + type + ">";
            String sourceClass = scenario == Case.SAME_NAMESPACE || scenario == Case.ALREADY_CURRENT ?
                targetClass : "RttiHelper<CurrentCardData>";
            Function function;
            Symbol sourceAlias = null;
            long sourceAliasId = -1;
            int setup = program.startTransaction("existing reflection aliases");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", entry, 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("rtti", table, 0x100, (byte) 0, TaskMonitor.DUMMY, false);
                program.getMemory().setBytes(base.add(0x110), ByteBuffer.allocate(11).order(ByteOrder.LITTLE_ENDIAN)
                    .put((byte) 0x48).put((byte) 0xb8).putLong(string.getOffset()).put((byte) 0xc3).array());
                // AZ_RTTI block; the string scan finds it through RTTI_GetTypeName.
                program.getMemory().setBytes(table, ByteBuffer.allocate(48).order(ByteOrder.LITTLE_ENDIAN)
                    .putLong(entry.getOffset()).putLong(base.add(0x110).getOffset()).putLong(base.add(0x120).getOffset())
                    .putLong(base.add(0x130).getOffset()).putLong(base.add(0x140).getOffset())
                    .putLong(base.add(0x140).getOffset()).array());
                program.getMemory().setBytes(string, (type + "\0").getBytes(StandardCharsets.US_ASCII));
                program.getListing().createData(string, TerminatedStringDataType.dataType);
                if (scenario == Case.LATE_ALIAS) {
                    // Two RttiHelper instances over one nine-slot vftable; its GetTypeId folds with RTTI_GetType.
                    program.getMemory().setLong(table.add(0x60), table.add(0x80).getOffset());
                    program.getMemory().setLong(table.add(0x68), table.add(0x80).getOffset());
                    long[] helperSlots = {0x150, 0x160, 0x170, 0x180, 0x190, 0x100, 0x1a0, 0x1b0, 0x1c0};
                    for (int slot = 0; slot < helperSlots.length; slot++) {
                        program.getMemory().setLong(table.add(0x80 + slot * 8L), base.add(helperSlots[slot]).getOffset());
                    }
                }
                var symbols = program.getSymbolTable();
                Namespace az = symbols.createNameSpace(program.getGlobalNamespace(), "AZ", SourceType.USER_DEFINED);
                Namespace internal = symbols.createNameSpace(az, "Internal", SourceType.USER_DEFINED);
                Namespace source = symbols.createClass(internal, sourceClass, SourceType.USER_DEFINED);
                Namespace target = sourceClass.equals(targetClass) ? source :
                    symbols.createClass(internal, targetClass, SourceType.USER_DEFINED);
                String oldName = scenario == Case.NAMESPACE_ONLY || scenario == Case.ALREADY_CURRENT ? "GetTypeId" : "GetActualUuid";
                function = program.getFunctionManager().createFunction(oldName, source, entry,
                    new AddressSet(entry), SourceType.USER_DEFINED);
                if (scenario == Case.SOURCE_ALIAS || scenario == Case.BOTH_ALIASES || scenario == Case.TEMPLATES) {
                    sourceAlias = symbols.createLabel(entry, "GetTypeId", source, SourceType.USER_DEFINED);
                    sourceAliasId = sourceAlias.getID();
                }
                if (scenario == Case.TARGET_ALIAS || scenario == Case.BOTH_ALIASES ||
                    scenario == Case.SAME_NAMESPACE || scenario == Case.NAMESPACE_ONLY) {
                    symbols.createLabel(entry, "GetTypeId", target, SourceType.USER_DEFINED);
                }
                for (long offset : new long[]{0x110, 0x120, 0x130, 0x140}) {
                    Address body = base.add(offset);
                    program.getFunctionManager().createFunction("FUN_" + Long.toHexString(offset), body,
                        new AddressSet(body, body.add(offset == 0x110 ? 10 : 0)), SourceType.DEFAULT);
                }
            }
            finally { program.endTransaction(setup, true); }
            Files.writeString(resources.resolve("serialize.json"), scenario == Case.LATE_ALIAS ? """
                {"uuidMap":{},"classData":[],"rttiEvidence":[
                  {"address":"NewWorld+0x2060","typeName":"CurrentCardData","typeId":"AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"},
                  {"address":"NewWorld+0x2068","typeName":"OtherCardData","typeId":"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"}]}
                """ : "{\"uuidMap\":{},\"classData\":[]}");
            long functionId = function.getSymbol().getID();
            long symbolCount = program.getSymbolTable().getNumSymbols();
            String oldName = function.getName(true);
            run(program, resources, true);
            check(function.getName(true).equals(oldName) && program.getSymbolTable().getNumSymbols() == symbolCount,
                "dry run edited existing symbols: " + scenario);
            run(program, resources, false);
            // The block's RTTI_GetType is the class's own function; with the helper it folds, the helper names it.
            String expected = scenario == Case.LATE_ALIAS ? "AZ::Internal::RttiHelper<CurrentCardData>::GetTypeId" :
                type + "::GetType";
            check(function.getName(true).equals(expected),
                "RTTI function kept its previous owner/name: " + scenario + " " + function.getName(true));
            check(java.util.Arrays.stream(program.getSymbolTable().getSymbols(entry)).noneMatch(symbol ->
                symbol.getName(true).startsWith("AZ::Internal::" + targetClass + "::")),
                "six-slot RttiHelper name left on an AZ_RTTI block: " + scenario);
            check(function.getSymbol().getID() == functionId, "function symbol identity replaced: " + scenario);
            if (sourceAlias != null) {
                check(program.getSymbolTable().getSymbol(sourceAliasId) != null &&
                    sourceAlias.getName(true).equals("AZ::Internal::RttiHelper<CurrentCardData>::GetTypeId"),
                    "unrelated source alias deleted: " + scenario);
            }
            if (scenario == Case.LATE_ALIAS) {
                var aliases = program.getSymbolTable().getSymbols(entry);
                check(java.util.Arrays.stream(aliases).anyMatch(symbol -> symbol.getName(true)
                    .equals("AZ::Internal::RttiHelper<CurrentCardData>::GetTypeId")) &&
                    java.util.Arrays.stream(aliases).anyMatch(symbol -> symbol.getName(true)
                    .equals("AZ::Internal::RttiHelper<OtherCardData>::GetTypeId")) &&
                    java.util.Arrays.stream(aliases).anyMatch(symbol -> symbol.getName(true).equals(type + "::GetType")),
                    "aliases created earlier in the same import were not retained");
            }
            JsonObject validation = json(resources.resolve("reflection-dry-run.json"));
            check(validation.get("status").getAsString().equals("passed"), "import failed after validation: " + scenario);
            run(program, resources, false);
            check(json(resources.resolve("reflection-report.json")).get("appliedCount").getAsInt() == 0,
                "retry changed already imported symbols: " + scenario);
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY,
            new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} :
            new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
    }

    private static JsonObject json(Path path) throws Exception {
        return JsonParser.parseString(Files.readString(path)).getAsJsonObject();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
