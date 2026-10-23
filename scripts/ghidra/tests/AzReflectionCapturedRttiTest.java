import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

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
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/**
 * Names RTTI functions from captured hierarchies, rttiOnly and sharedTypeIds over the 3-26 layouts:
 * a nine-slot RttiHelper vftable whose GetTypeId folds with the class's RTTI_GetType, AZ_RTTI blocks,
 * AzTypeInfo getters, names an earlier six-slot import left, colliding names, shared UUIDs and a capture
 * the binary contradicts.
 */
public class AzReflectionCapturedRttiTest {
    private static final String WIDGET = "11111111-1111-4111-8111-111111111111";
    private static final String IFACE = "22222222-2222-4222-8222-222222222222";
    private static final String ACTION_A = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
    private static final String ACTION_B = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB";
    private static final String GHOST = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC";
    private static final String HELPER = "AZ::Internal::RttiHelper<Javelin::Widget>";

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-captured-rtti-test-");
        try {
            Address base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
            int setup = program.startTransaction("captured rtti fixture");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock(".text", base.add(0x1000), 0x1000, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock(".rdata", base.add(0x3000), 0x1000, (byte) 0,
                    TaskMonitor.DUMMY, false);
                program.getMemory().createInitializedBlock(".data", base.add(0x5000), 0x100, (byte) 0,
                    TaskMonitor.DUMMY, false);
                // RTTI_GetType bodies are `jmp AzTypeInfo<T>::Uuid`.
                for (long[] thunk : new long[][]{{0x1010, 0x1000}, {0x1230, 0x1200}, {0x1310, 0x1300},
                    {0x1410, 0x1400}, {0x1510, 0x1500}, {0x1610, 0x1600}}) {
                    jump(program, base, thunk[0], thunk[1]);
                }
                // Javelin::Widget: AZ_RTTI block; GetTypeId (slot 5) of its RttiHelper is the same body.
                table(program, base, 0x3000, 0x1010, 0x1020, 0x1030, 0x1040, 0x1050, 0x1050);
                table(program, base, 0x3040, 0x1100, 0x1110, 0x1120, 0x1130, 0x1140, 0x1010, 0x1150, 0x1160, 0x1170);
                program.getMemory().setLong(base.add(0x5000), base.add(0x3040).getOffset());
                // IWidget has only an RttiHelper; its dtor, Cast overloads and IsAbstract fold with Widget's.
                table(program, base, 0x3100, 0x1100, 0x1210, 0x1220, 0x1130, 0x1140, 0x1230, 0x1240, 0x1160, 0x1250);
                table(program, base, 0x3200, 0x1310, 0x1320, 0x1330, 0x1340, 0x1350, 0x1350);
                table(program, base, 0x3240, 0x1410, 0x1420, 0x1430, 0x1440, 0x1450, 0x1450);
                table(program, base, 0x3280, 0x1510, 0x1520, 0x1530, 0x1540, 0x1550, 0x1550);
                table(program, base, 0x32c0, 0x1610, 0x1620, 0x1630, 0x1640, 0x1650, 0x1650);
                program.getReferenceManager().addMemoryReference(base.add(0x1800), base.add(0x3000), RefType.DATA,
                    SourceType.USER_DEFINED, 0);
                for (long offset = 0x1000; offset < 0x1700; offset += 0x10) {
                    Address entry = base.add(offset);
                    program.getFunctionManager().createFunction("FUN_" + Long.toHexString(offset), entry,
                        new AddressSet(entry), SourceType.DEFAULT);
                }
                // What the six-slot import named: GetTypeId at the dtor, GetTypeName at GetActualUuid, Cast at GetTypeId.
                var symbols = program.getSymbolTable();
                Namespace az = symbols.createNameSpace(program.getGlobalNamespace(), "AZ", SourceType.USER_DEFINED);
                Namespace internal = symbols.createNameSpace(az, "Internal", SourceType.USER_DEFINED);
                Namespace helper = symbols.createClass(internal, "RttiHelper<Javelin::Widget>", SourceType.USER_DEFINED);
                symbols.createLabel(base.add(0x1100), "GetTypeId", helper, SourceType.USER_DEFINED);
                function(program, base, 0x1110).getSymbol().setNameAndNamespace("GetTypeName", helper, SourceType.USER_DEFINED);
                function(program, base, 0x1010).getSymbol().setNameAndNamespace("Cast", helper, SourceType.USER_DEFINED);
            }
            finally { program.endTransaction(setup, true); }

            Files.writeString(resources.resolve("serialize.json"), """
                {"uuidMap":{"%1$s":{"name":"Javelin::Widget","typeId":"%1$s","version":"1","attributes":[],"elements":[],
                  "azRtti":{"source":"AZ::IRttiHelper","address":"NewWorld+0x5000","typeId":"%1$s","typeName":"Javelin::Widget",
                    "hierarchy":[
                      {"typeId":"%1$s","typeName":"Javelin::Widget","getType":"NewWorld+0x1000","helper":"NewWorld+0x3000","getTypeName":"NewWorld+0x1020"},
                      {"typeId":"%2$s","typeName":"IWidget","getType":"NewWorld+0x1200","helper":"NewWorld+0x3100","getTypeName":"NewWorld+0x1220"},
                      {"typeId":"%5$s","typeName":null,"getType":"NewWorld+0x1700","helper":null,"getTypeName":null}]}}},
                 "rttiOnly":[
                   {"typeId":"%3$s","typeName":"Action","getType":"NewWorld+0x1310","getTypeName":"NewWorld+0x1320",
                    "enumHierarchy":"NewWorld+0x1340","vtable":"NewWorld+0x3200","hierarchy":[
                      {"typeId":"%3$s","typeName":"Action","getType":"NewWorld+0x1300","helper":"NewWorld+0x3200","getTypeName":"NewWorld+0x1320"}]},
                   {"typeId":"%4$s","typeName":"Action","getType":"NewWorld+0x1410","getTypeName":"NewWorld+0x1420",
                    "enumHierarchy":"NewWorld+0x1440","vtable":"NewWorld+0x3240","hierarchy":[
                      {"typeId":"%4$s","typeName":"Action","getType":"NewWorld+0x1400","helper":"NewWorld+0x3240","getTypeName":"NewWorld+0x1420"}]},
                   {"typeId":"%5$s","typeName":"Ghost","getType":"NewWorld+0x1610","getTypeName":"NewWorld+0x1699",
                    "enumHierarchy":"NewWorld+0x1640","vtable":"NewWorld+0x32c0","hierarchy":[]}],
                 "sharedTypeIds":[{"typeId":"%1$s","types":[
                   {"typeName":"Javelin::Widget","getType":"NewWorld+0x1000","staticTypeId":null,"helper":"NewWorld+0x3000","identities":1},
                   {"typeName":"SiegeWarfareHelper","getType":"NewWorld+0x1500","staticTypeId":null,"helper":"NewWorld+0x3280","identities":1}]}]}
                """.formatted(WIDGET, IFACE, ACTION_A, ACTION_B, GHOST));

            long symbolCount = program.getSymbolTable().getNumSymbols();
            run(program, resources, true);
            check(program.getSymbolTable().getNumSymbols() == symbolCount &&
                name(program, base, 0x1010).equals(HELPER + "::Cast"), "dry run edited symbols");
            run(program, resources, false);
            check(json(resources.resolve("reflection-dry-run.json")).get("status").getAsString().equals("passed"),
                "validation failed");

            check(name(program, base, 0x1010).equals("Javelin::Widget::GetType") &&
                has(program, base, 0x1010, HELPER + "::GetTypeId") && !hasLocal(program, base, 0x1010, "Cast"),
                "folded RTTI_GetType/GetTypeId body: " + names(program, base, 0x1010));
            check(name(program, base, 0x1110).equals(HELPER + "::GetActualUuid"), "six-slot name kept: " + names(program, base, 0x1110));
            check(name(program, base, 0x1120).equals(HELPER + "::GetActualTypeName") &&
                name(program, base, 0x1170).equals(HELPER + "::EnumHierarchy"), "nine-slot RttiHelper layout");
            check(has(program, base, 0x1100, HELPER + "::~RttiHelper") &&
                has(program, base, 0x1100, "AZ::Internal::RttiHelper<IWidget>::~RttiHelper") &&
                !has(program, base, 0x1100, HELPER + "::GetTypeId"), "shared dtor: " + names(program, base, 0x1100));
            check(name(program, base, 0x1050).equals("Javelin::Widget::AddressOfConst") &&
                has(program, base, 0x1050, "Javelin::Widget::AddressOf"), "folded AddressOf overloads");
            check(has(program, base, 0x3000, "Javelin::Widget::vftable"), "referenced AZ_RTTI block vftable");
            check(name(program, base, 0x1000).equals("AZ::AzTypeInfo<Javelin::Widget>::Uuid") &&
                name(program, base, 0x1200).equals("AZ::AzTypeInfo<IWidget>::Uuid"), "type-id getters");
            check(has(program, base, 0x3100, "AZ::Internal::RttiHelper<IWidget>::vftable") &&
                name(program, base, 0x1230).equals("AZ::Internal::RttiHelper<IWidget>::GetTypeId"), "hierarchy-only RttiHelper");
            check(name(program, base, 0x1310).equals("Action::GetType") && name(program, base, 0x1410).equals("Action::GetType") &&
                name(program, base, 0x1300).equals("AZ::AzTypeInfo<Action>::Uuid") &&
                name(program, base, 0x1400).equals("AZ::AzTypeInfo<Action>::Uuid") &&
                Arrays.stream(program.getSymbolTable().getSymbols(base.add(0x3200))).noneMatch(s -> s.getName().equals("vftable")),
                "distinct types named Action: " + names(program, base, 0x1310));
            check(name(program, base, 0x1510).equals("SiegeWarfareHelper::GetType") &&
                name(program, base, 0x1500).equals("AZ::AzTypeInfo<SiegeWarfareHelper>::Uuid"), "type sharing Widget's UUID");
            check(name(program, base, 0x1610).startsWith("FUN_") && name(program, base, 0x1600).startsWith("FUN_"),
                "capture the binary contradicts was applied");

            JsonObject report = json(resources.resolve("reflection-report.json"));
            check(report.get("capturedRttiHelperCount").getAsInt() == 6 &&
                report.get("capturedTypeIdGetterCount").getAsInt() == 5, "captured counts: " + report.get("capturedRttiHelperCount"));
            check(report.getAsJsonObject("actionReasonCounts").get("capture-does-not-match-binary").getAsInt() == 1,
                "contradicted capture not reported");
            long after = program.getSymbolTable().getNumSymbols();
            run(program, resources, false);
            check(json(resources.resolve("reflection-report.json")).get("appliedCount").getAsInt() == 0 &&
                program.getSymbolTable().getNumSymbols() == after, "retry changed imported RTTI names");
            System.out.println("PASS: captured RttiHelper vftables (nine slots), AZ_RTTI blocks, AzTypeInfo getters, folded and " +
                "shared bodies, stale six-slot names, colliding names, shared UUIDs and contradicted captures");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void jump(ProgramDB program, Address base, long from, long to) throws Exception {
        int displacement = (int) (to - (from + 5));
        program.getMemory().setBytes(base.add(from), new byte[]{(byte) 0xe9, (byte) displacement,
            (byte) (displacement >> 8), (byte) (displacement >> 16), (byte) (displacement >> 24)});
    }

    private static void table(ProgramDB program, Address base, long at, long... slots) throws Exception {
        for (int i = 0; i < slots.length; i++) {
            program.getMemory().setLong(base.add(at + i * 8L), base.add(slots[i]).getOffset());
        }
    }

    private static ghidra.program.model.listing.Function function(ProgramDB program, Address base, long offset) {
        return program.getFunctionManager().getFunctionAt(base.add(offset));
    }

    private static String name(ProgramDB program, Address base, long offset) {
        return function(program, base, offset).getName(true);
    }

    private static boolean has(ProgramDB program, Address base, long offset, String name) {
        return Arrays.stream(program.getSymbolTable().getSymbols(base.add(offset))).anyMatch(s -> s.getName(true).equals(name));
    }

    private static boolean hasLocal(ProgramDB program, Address base, long offset, String name) {
        return Arrays.stream(program.getSymbolTable().getSymbols(base.add(offset))).anyMatch(s -> s.getName().equals(name));
    }

    private static String names(ProgramDB program, Address base, long offset) {
        return Arrays.toString(Arrays.stream(program.getSymbolTable().getSymbols(base.add(offset))).map(s -> s.getName(true)).toArray());
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
