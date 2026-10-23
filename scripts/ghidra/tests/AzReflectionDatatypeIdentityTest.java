import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
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
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.Pointer;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Distinct reflected UUIDs retain their own layouts and typed consumers even when C++ names match. */
public class AzReflectionDatatypeIdentityTest {
    static final String FIRST = "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA";
    static final String SECOND = "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB";
    static final String NAME = "Payload<Javelin::Key>";
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-datatype-identity-test-");
        var manager = program.getDataTypeManager();
        Structure legacy, manual, oldConsumer;
        Function firstFunction, secondFunction;
        try {
            int tx = program.startTransaction("duplicate reflected datatype names");
            try {
                var base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                StructureDataType source = new StructureDataType(CategoryPath.ROOT, "MB::" + NAME, 4);
                source.setDescription("AZ SerializeContext reflected type: MB::" + NAME + " (" + FIRST + ")");
                legacy = (Structure) manager.addDataType(source, DataTypeConflictHandler.DEFAULT_HANDLER);
                StructureDataType manualSource = new StructureDataType(new CategoryPath("/MB"), NAME, 16);
                manualSource.replaceAtOffset(0, IntegerDataType.dataType, 4, "manual_field", null);
                manualSource.setDescription("Hand-written canonical layout; no reflected UUID identity");
                manual = (Structure) manager.addDataType(manualSource, DataTypeConflictHandler.DEFAULT_HANDLER);
                StructureDataType oldHolder = new StructureDataType(CategoryPath.ROOT, "ExistingConsumer", 0);
                oldHolder.add(new PointerDataType(legacy, 8, manager), "value", null);
                oldConsumer = (Structure) manager.addDataType(oldHolder, DataTypeConflictHandler.DEFAULT_HANDLER);
                // Another UUID's generated type already owns the canonical path of a bodyless field type.
                StructureDataType borrowed = new StructureDataType(new CategoryPath("/Javelin"), "Borrowed", 12);
                borrowed.setDescription("AZ SerializeContext reflected type: Javelin::Borrowed (EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE)");
                manager.addDataType(borrowed, DataTypeConflictHandler.DEFAULT_HANDLER);
                var mb = program.getSymbolTable().createNameSpace(program.getGlobalNamespace(), "MB", SourceType.USER_DEFINED);
                var cls = program.getSymbolTable().createClass(mb, NAME, SourceType.USER_DEFINED);
                firstFunction = program.getFunctionManager().createFunction("Validate", cls, base.add(0x100),
                    new AddressSet(base.add(0x100)), SourceType.USER_DEFINED);
                secondFunction = program.getFunctionManager().createFunction("Validate", cls, base.add(0x110),
                    new AddressSet(base.add(0x110)), SourceType.USER_DEFINED);
                firstFunction.setCallingConvention("__thiscall");
                secondFunction.setCallingConvention("__thiscall");
            }
            finally { program.endTransaction(tx, true); }
            Files.writeString(resources.resolve("serialize.json"), """
                {"uuidMap":{
                  "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA":{"name":"MB::Payload<Javelin::Key>","typeId":"AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA","version":"1","factory":null,"attributes":[],
                    "elements":[{"name":"first_value","typeId":"72039442-EB38-4D42-A1AD-CB68F7E0EEF6","offset":"0","dataSize":"4",
                      "attributes":[[{"attributeName":"Validate","value":{"memberFunction":"NewWorld+0x100"}}]]}]},
                  "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB":{"name":"MB::Payload<Javelin::Key>","typeId":"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB","version":"1","factory":null,"attributes":[],
                    "elements":[{"name":"second_value","typeId":"110C4B14-11A8-4E9D-8638-5051013A56AC","offset":"0","dataSize":"8",
                      "attributes":[[{"attributeName":"Validate","value":{"memberFunction":"NewWorld+0x110"}}]]}]},
                  "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC":{"name":"Javelin::Holder","typeId":"CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC","version":"1","factory":null,"attributes":[],
                    "elements":[
                      {"name":"first","typeId":"AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA","offset":"0","dataSize":"4","attributes":[]},
                      {"name":"second","typeId":"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB","offset":"8","dataSize":"8","attributes":[]},
                      {"name":"first_ptr","typeId":"AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA","offset":"16","dataSize":"8","is_pointer":true,"attributes":[]},
                      {"name":"second_ptr","typeId":"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB","offset":"24","dataSize":"8","is_pointer":true,"attributes":[]},
                      {"name":"bodyless_ptr","typeId":"FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF","offset":"32","dataSize":"8","is_pointer":true,
                       "azRtti":{"typeName":"Javelin::Borrowed","typeId":"FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF"},"attributes":[]}]},
                  "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD":{"name":"Javelin::Unknown","typeId":"DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD","version":"1","factory":null,"attributes":[],"elements":[]}
                }}
                """);
            long legacyId = manager.getID(legacy), functionId = firstFunction.getID();
            int beforeCount = manager.getDataTypeCount(true);
            String beforePrototype = firstFunction.getPrototypeString(true, true);
            run(program, resources, true);
            check(manager.getDataTypeCount(true) == beforeCount && legacy.getPathName().equals("/MB::" + NAME), "preview mutated datatypes");
            check(firstFunction.getPrototypeString(true, true).equals(beforePrototype), "preview mutated this signature");
            run(program, resources, false);
            Structure first = structure(program, "/MB/__typeid__/" + FIRST, NAME);
            Structure second = structure(program, "/MB/__typeid__/" + SECOND, NAME);
            check(first == legacy && manager.getID(first) == legacyId, "migration changed datatype identity");
            check(first != second && first.getLength() == 4 && second.getLength() == 8, "distinct captured layouts merged");
            check("first_value".equals(first.getComponentAt(0).getFieldName()) &&
                "second_value".equals(second.getComponentAt(0).getFieldName()), "named reflected fields lost");
            check(((Pointer) oldConsumer.getComponentAt(0).getDataType()).getDataType() == first, "existing consumer pointer detached");
            Structure holder = structure(program, "/Javelin", "Holder");
            check(holder.getComponentAt(0).getDataType() == first && holder.getComponentAt(8).getDataType() == second,
                "by-value fields selected datatype by name instead of UUID");
            check(((Pointer) holder.getComponentAt(16).getDataType()).getDataType() == first &&
                ((Pointer) holder.getComponentAt(24).getDataType()).getDataType() == second, "pointer fields selected wrong UUID: " +
                    holder.getComponentAt(16).getDataType().getPathName() + ", " + holder.getComponentAt(24).getDataType().getPathName());
            check(((Pointer) firstFunction.getParameter(0).getDataType()).getDataType() == first &&
                ((Pointer) secondFunction.getParameter(0).getDataType()).getDataType() == second, "this uses wrong reflected datatype");
            check(firstFunction.getParameter(0).getName().equals("this") && firstFunction.getID() == functionId, "this/function identity lost");
            DataType bodyless = ((Pointer) holder.getComponentAt(32).getDataType()).getDataType();
            check(bodyless.getCategoryPath().getPath().equals("/Javelin/__typeid__/FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF") &&
                manager.getDataType(new CategoryPath("/Javelin"), "Borrowed").getLength() == 12,
                "bodyless field borrowed another UUID's datatype: " + bodyless.getPathName());
            check(manager.getDataType(new CategoryPath("/MB"), NAME) == manual && manual.getLength() == 16 &&
                manual.getDescription().startsWith("Hand-written"), "unidentified manual canonical layout modified");
            check(manager.getDataType(new CategoryPath("/Javelin"), "Unknown") == null, "unknown native size was invented");
            JsonObject report = JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
            check(!report.getAsJsonObject("actionReasonCounts").has("ambiguous-datatype-path; distinct-UUIDs-preserved"), "resolved UUID layout still skipped");
            int resolved = 0;
            for (var row : report.getAsJsonArray("reflectedTypes")) {
                JsonObject type = row.getAsJsonObject();
                if (!type.has("identityResolution")) continue;
                check(type.get("identityResolution").getAsString().equals("uuid-specific-datatype") &&
                    type.get("layoutInProgram").getAsBoolean(), "report hides resolved layout");
                resolved++;
            }
            check(resolved == 2, "report lost distinct UUID identities");
            int afterCount = manager.getDataTypeCount(true);
            run(program, resources, false);
            check(manager.getDataTypeCount(true) == afterCount && manager.getID(first) == legacyId,
                "retry duplicated/migrated UUID datatypes");
            System.out.println("PASS: distinct UUID layouts for identical nested generic C++ names, fields, pointers, this signatures, " +
                "migration preserves IDs/references, manual canonical layout, unknown-size skip, truthful report, preview and retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }
    static Structure structure(ProgramDB program, String category, String name) {
        DataType type = program.getDataTypeManager().getDataType(new CategoryPath(category), name);
        check(type instanceof Structure, "missing structure " + category + "/" + name);
        return (Structure) type;
    }
    static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} : new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); } finally { script.end(true); }
    }
    static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
