import java.io.File;
import java.io.FileReader;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.lang.reflect.InvocationTargetException;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import ghidra.GhidraApplicationLayout;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.flatapi.FlatProgramAPI;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.Array;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.Pointer;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Program;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Tests real captured layouts in a disposable database; never opens the game project. */
public class AzReflectionDatatypesTest {
    private static final Class<?> SCRIPT = AzReflectionRenamer.class;
    private static final String UUID = "E152C105-A133-4D03-BBF8-3D4B2FBA3E2A";
    private static final String PAIR_BYTES = "48895c240848896c2410488974241848897c24204154415641574883ec30488bd94c8d6110488b49484c8d7a0833ff498bf0488bea4885c9740b488b01ff50084c8bc8eb034c8bcf488b064c8d4320488bd54c89642420488d4e084c8b500841ffd284c07438488b8bb8000000488dab800000004885c97409488b01ff5008488bf8488b064c8d83900000004c8bcf48896c2420498bd7488d4e08ff5008488b5c2450488b6c2458488b742460488b7c24684883c430415f415e415cc3";

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("Renamed NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        AzReflectionRenamer script = new AzReflectionRenamer();
        int transaction = program.startTransaction("datatype regressions");
        try {
            program.setImageBase(program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
            block(program, 0xd5ce20, "488b4218c3");
            block(program, 0xdb0880, "488b4a0848b88988888888888888482b0a48");
            Address callback = block(program, 0x13f6710, PAIR_BYTES);
            check(new DisassembleCommand(callback, null, true).applyTo(program), "pair callback disassembled");
            program.getFunctionManager().createFunction("PairEnum", callback,
                new AddressSet(callback, callback.add(PAIR_BYTES.length() / 2 - 1)), SourceType.USER_DEFINED);
            Address table = block(program, 0x804cd70, "00".repeat(40));
            program.getMemory().setLong(table.add(24), callback.getOffset());
            var manager = program.getDataTypeManager();
            StructureDataType placeholder = new StructureDataType(new CategoryPath("/AZ"), "Uuid", 8);
            placeholder.setDescription("AZ SerializeContext reflected field type: AZ::Uuid (" + UUID + ")");
            Structure uuid = (Structure) manager.addDataType(placeholder, DataTypeConflictHandler.DEFAULT_HANDLER);
            Object uuidId = uuid.getUniversalID();
            StructureDataType oldVector3Source = new StructureDataType(CategoryPath.ROOT, "Vector3", 16);
            oldVector3Source.setDescription("AZ SerializeContext reflected field type: Vector3 (8379EB7D-01FA-4538-B64B-A6543B4BE73D)");
            DataType oldVector3 = manager.addDataType(oldVector3Source, DataTypeConflictHandler.DEFAULT_HANDLER);
            Object vector3Id = oldVector3.getUniversalID();
            StructureDataType holderSource = new StructureDataType(CategoryPath.ROOT, "ExistingHolder", 0);
            holderSource.add(new PointerDataType(uuid, 8, manager), "value", null);
            Structure holder = (Structure) manager.addDataType(holderSource, DataTypeConflictHandler.DEFAULT_HANDLER);

            set(FlatProgramAPI.class, script, "currentProgram", program);
            set(FlatProgramAPI.class, script, "monitor", TaskMonitor.DUMMY);
            set(SCRIPT, script, "dataTypeManager", manager);
            set(SCRIPT, script, "symbols", program.getSymbolTable());
            for (String name : List.of("scopeCache", "pointerReadCache", "executableAddressCache", "targetNameOwners")) {
                set(SCRIPT, script, name, new HashMap<>());
            }
            for (String name : List.of("moduleEvidence", "classRegistrationEvidence", "fieldRegistrationEvidence")) {
                Field field = SCRIPT.getDeclaredField(name);
                Constructor<?> constructor = field.getType().getDeclaredConstructor();
                constructor.setAccessible(true);
                set(SCRIPT, script, name, constructor.newInstance());
            }
            if (args.length > 2) {
                set(SCRIPT, script, "cli", GhidraCli.parse(new String[0], java.util.Set.of(), java.util.Set.of(), 0));
                set(SCRIPT, script, "gson", new com.google.gson.GsonBuilder().create());
                File input = new File(args[1]);
                set(SCRIPT, script, "moduleEvidence", call(script, "loadModuleEvidence", new Class<?>[]{File.class}, input));
                set(SCRIPT, script, "behaviorContextEvidence", call(script, "loadBehaviorContextEvidence", new Class<?>[]{File.class}, input));
            }
            JsonElement root;
            try (FileReader reader = new FileReader(args[1])) { root = JsonParser.parseReader(reader); }
            Object scan = call(script, "scanSerializeContext", new Class<?>[]{JsonElement.class}, root);
            List<?> classes = (List<?>) field(scan, "classData");
            check((Boolean) field(script, "nativeLayoutProfile"), "NewWorld profile accepted after live byte checks");
            check(List.of(0, 8).equals(call(script, "pairCallbackOffsets", new Class<?>[]{Address.class}, callback)),
                "pair second member at native +8, not metadata +4");
            int beforePreview = manager.getDataTypeCount(true);
            Object preview = sink(script);
            JsonObject validation = new JsonObject();
            validation.add("errors", new com.google.gson.JsonArray());
            set(SCRIPT, script, "validating", true);
            set(SCRIPT, script, "validationReport", validation);
            set(SCRIPT, script, "currentPhase", "datatypes-and-layouts");
            call(script, "processReflectedDatatypes", new Class<?>[]{List.class, preview.getClass()}, classes, preview);
            check(manager.getDataTypeCount(true) == beforePreview && uuid.getLength() == 8,
                "preview leaves datatypes and placeholder sizes unchanged");
            check((int) field(script, "validationErrors") == 0,
                "missing native callback/stride evidence incorrectly blocks validation: " + validation);
            check((int) ((Map<?, ?>) field(field(preview, "stats"), "reasonCounts")).get("native-layout-unresolved") > 0,
                "fixture no longer tests incomplete native evidence");
            Listing actualListing = program.getListing();
            Listing faultyListing = (Listing) Proxy.newProxyInstance(SCRIPT.getClassLoader(), new Class<?>[]{Listing.class},
                (proxy, method, values) -> {
                    if (method.getName().equals("getInstructions")) throw new IllegalStateException("injected pair layout decoder error");
                    try { return method.invoke(actualListing, values); }
                    catch (InvocationTargetException error) { throw error.getCause(); }
                });
            Program faultyProgram = (Program) Proxy.newProxyInstance(SCRIPT.getClassLoader(), new Class<?>[]{Program.class},
                (proxy, method, values) -> {
                    if (method.getName().equals("getListing")) return faultyListing;
                    try { return method.invoke(program, values); }
                    catch (InvocationTargetException error) { throw error.getCause(); }
                });
            set(FlatProgramAPI.class, script, "currentProgram", faultyProgram);
            Object failedPreview = sink(script);
            List<?> pairs = classes.stream().filter(data -> {
                try {
                    String name = (String) field(data, "targetTypeName");
                    return name != null && name.startsWith("AZStd::pair<");
                }
                catch (Exception error) { throw new RuntimeException(error); }
            }).toList();
            call(script, "processReflectedDatatypes", new Class<?>[]{List.class, failedPreview.getClass()}, pairs, failedPreview);
            check((int) field(script, "validationErrors") > 0 &&
                validation.toString().contains("injected pair layout decoder error"),
                "caught native decoder bug was silently treated as missing evidence");
            check(manager.getDataTypeCount(true) == beforePreview, "failed layout preview changed datatypes");
            set(FlatProgramAPI.class, script, "currentProgram", program);
            set(SCRIPT, script, "validating", false);
            set(SCRIPT, script, "validationReport", null);
            set(SCRIPT, script, "validationErrors", 0);
            set(SCRIPT, script, "applyRenames", true);
            Object applied = sink(script);
            call(script, "processReflectedDatatypes", new Class<?>[]{List.class, applied.getClass()}, classes, applied);
            check(uuid == type(manager, "/AZ", "Uuid") && uuidId.equals(uuid.getUniversalID()), "placeholder upgraded in place");
            check(uuid.getLength() == 16 && "data".equals(uuid.getComponentAt(0).getFieldName()), "UUID native data[16]");
            check(uuid.getAlignment() == 16, "UUID native alignment preserved");
            check(((Pointer) holder.getComponent(0).getDataType()).getDataType() == uuid, "existing pointer references retained");
            Structure component = structure(manager, "/AZ", "Component");
            check(component.getLength() == 24 && "m_entity".equals(component.getComponentAt(8).getFieldName()), "Component native entity +8");
            check("Id".equals(component.getComponentAt(16).getFieldName()), "reflected Component Id +16");
            Structure vector = structure(manager, "/AZStd", "vector<EncounterConditionEvent>");
            check(vector.getLength() == 32 && "m_end".equals(vector.getComponentAt(16).getFieldName()), "vector native pointer fields");
            DataType vectorElement = ((Pointer) vector.getComponentAt(0).getDataType()).getDataType();
            check(vectorElement.getLength() == 120, "generic element stride proves full EncounterConditionEvent sizeof");
            Structure pointerVector = structure(manager, "/AZStd", "vector<StateFlexCondition*>");
            check(((Pointer) pointerVector.getComponentAt(0).getDataType()).getDataType() instanceof Pointer,
                "vector<T*> stores T** bounds");
            Structure map = structure(manager, "/AZStd", "unordered_map<unsigned_int,AZStd::string>");
            check(map.getLength() == 144 && "m_buckets".equals(map.getComponentAt(0x60).getFieldName()), "map native bucket pointer +60");
            Structure set = structure(manager, "/AZStd", "unordered_set<AZ::EntityId>");
            check(set.getLength() == 144 && "m_list".equals(set.getComponentAt(8).getFieldName()), "set native linked list +8");
            boolean smallNodeChecked = false;
            var nodeTypes = manager.getAllDataTypes();
            while (nodeTypes.hasNext()) {
                DataType candidate = nodeTypes.next();
                if (candidate instanceof Structure && candidate.getName().equals("node")) {
                    var payload = ((Structure) candidate).getComponentAt(16);
                    if (payload != null && payload.getLength() == 4) {
                        check(candidate.getLength() == 24, "list node includes pointer-aligned tail padding");
                        smallNodeChecked = true;
                    }
                }
            }
            check(smallNodeChecked, "capture exercised four-byte hash payload nodes");
            Structure pair = structure(manager, "/AZStd", "pair<unsigned_int,AZStd::string>");
            check("second".equals(pair.getComponentAt(8).getFieldName()) && pair.getLength() == 48, "pair aligned second and native tail padding");
            Structure array = structure(manager, "/AZStd", "array<AZ::Vector3,2>");
            check(array.getLength() == 32 && array.getComponentAt(0).getDataType() instanceof Array, "array of typed 16-byte Vector3");
            Structure vector3 = structure(manager, "/AZ", "Vector3");
            check("m_value".equals(vector3.getComponentAt(0).getFieldName()), "Vector3 native SIMD storage typed");
            check(oldVector3 == vector3 && vector3Id.equals(vector3.getUniversalID()), "UUID qualifies older unqualified datatype in place");
            Structure cutscene = structure(manager, "/Javelin", "CutsceneEntityEvent");
            check("m_cutsceneName".equals(cutscene.getComponentAt(0).getFieldName()) &&
                "m_cutsceneEvent".equals(cutscene.getComponentAt(40).getFieldName()), "Javelin reflected field names and offsets");
            Structure contract = structure(manager, "/", "ContractItemSimpleData");
            check("m_rarityLevel".equals(contract.getComponentAt(264).getFieldName()) && contract.getNumDefinedComponents() == 13,
                "identical duplicate reflection metadata collapses to one native field");
            pointerAndEnumCases(script, uuid);
            component.setDescription("Hand-written native layout");
            Object repeated = sink(script);
            call(script, "processReflectedDatatypes", new Class<?>[]{List.class, repeated.getClass()}, classes, repeated);
            check(component.getDescription().equals("Hand-written native layout"), "manual matching-size datatype preserved");
            if (((Number) field(field(repeated, "stats"), "appliedCount")).intValue() != 0) {
                Method fullActions = repeated.getClass().getDeclaredMethod("fullActions");
                fullActions.setAccessible(true);
                var rows = (com.google.gson.JsonArray) fullActions.invoke(repeated);
                for (JsonElement row : rows) {
                    JsonObject action = row.getAsJsonObject();
                    if (action.has("applied") && action.get("applied").getAsBoolean()) System.out.println(action);
                }
            }
            check(((Number) field(field(repeated, "stats"), "appliedCount")).intValue() == 0, "repeated native/reflected layouts are idempotent");
            program.getMemory().setByte(program.getImageBase().add(0xd5ce20), (byte) 0);
            check(!(Boolean) call(script, "matchesNativeLayoutProfile", new Class<?>[]{}), "mismatched ABI fingerprints rejected");
            System.out.println("Datatype action counts: " + field(field(applied, "stats"), "kindCounts"));
            System.out.println("Datatype report reasons in fixture database: " + field(field(applied, "stats"), "reasonCounts"));
            Method reportActions = applied.getClass().getDeclaredMethod("fullActions");
            reportActions.setAccessible(true);
            for (JsonElement row : (com.google.gson.JsonArray) reportActions.invoke(applied)) {
                JsonObject action = row.getAsJsonObject();
                if (action.has("reason") && action.get("reason").getAsString().equals("datatype-field-failed")) System.out.println(action);
            }
            System.out.println("PASS: full capture, placeholder IDs/references, pointers, enums, Component, vectors, maps/sets, arrays, " +
                "pair alignment, generic strides, Javelin fields, manual layouts, preview, idempotence and ABI rejection");
        }
        finally {
            call(script, "closeTemporarySpools", new Class<?>[0]);
            program.endTransaction(transaction, false);
            program.release(consumer);
        }
    }

    private static void pointerAndEnumCases(AzReflectionRenamer script, Structure uuid) throws Exception {
        Class<?> ownerType = Class.forName("AzReflectionRenamer$ClassData");
        Class<?> elementType = Class.forName("AzReflectionRenamer$ElementData");
        Object owner = construct(ownerType), element = construct(elementType);
        set(ownerType, owner, "typeId", "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        set(elementType, element, "typeId", UUID);
        set(elementType, element, "typeName", "AZ::Uuid");
        set(elementType, element, "isPointer", true);
        DataType pointer = (DataType) call(script, "fieldDataType", new Class<?>[]{ownerType, elementType, int.class}, owner, element, 8);
        check(pointer instanceof Pointer && ((Pointer) pointer).getDataType() == uuid, "pointer flag builds pointer to 16-byte UUID");
        Map<String, String> enums = (Map<String, String>) field(script, "enumUnderlyingTypes");
        enums.put("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB", "72B9409A-7D1A-4831-9CFE-FCB3FADD3426");
        set(elementType, element, "isPointer", false);
        set(elementType, element, "typeId", "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        set(elementType, element, "typeName", "MB::FixtureEnum<Javelin::Payload>");
        DataType enumType = (DataType) call(script, "fieldDataType", new Class<?>[]{ownerType, elementType, int.class}, owner, element, 1);
        check(enumType instanceof ghidra.program.model.data.TypeDef && enumType.getLength() == 1 &&
            enumType.getCategoryPath().getPath().equals("/MB"), "enum keeps qualified generic identity and actual underlying type");
    }

    private static Address block(ProgramDB program, long rva, String hex) throws Exception {
        Address address = program.getImageBase().add(rva);
        byte[] bytes = java.util.HexFormat.of().parseHex(hex);
        program.getMemory().createInitializedBlock("fixture_" + Long.toHexString(rva), address, bytes.length, (byte) 0, TaskMonitor.DUMMY, false).setExecute(true);
        program.getMemory().setBytes(address, bytes);
        return address;
    }
    private static Object sink(AzReflectionRenamer script) throws Exception {
        Class<?> type = Class.forName("AzReflectionRenamer$ActionSink");
        Constructor<?> constructor = type.getDeclaredConstructor(SCRIPT, boolean.class);
        constructor.setAccessible(true);
        return constructor.newInstance(script, true);
    }
    private static Object construct(Class<?> type) throws Exception {
        Constructor<?> constructor = type.getDeclaredConstructor(); constructor.setAccessible(true); return constructor.newInstance();
    }
    private static DataType type(ghidra.program.model.data.DataTypeManager manager, String path, String name) {
        DataType result = manager.getDataType(new CategoryPath(path), name);
        check(result != null, "datatype exists: " + path + "/" + name);
        return result;
    }
    private static Structure structure(ghidra.program.model.data.DataTypeManager manager, String path, String name) {
        return (Structure) type(manager, path, name);
    }
    private static void set(Class<?> type, Object instance, String name, Object value) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); field.set(instance, value);
    }
    private static Object field(Object instance, String name) throws Exception {
        Field field = instance.getClass().getDeclaredField(name); field.setAccessible(true); return field.get(instance);
    }
    private static Object call(Object instance, String name, Class<?>[] types, Object... values) throws Exception {
        Method method = SCRIPT.getDeclaredMethod(name, types); method.setAccessible(true); return method.invoke(instance, values);
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
