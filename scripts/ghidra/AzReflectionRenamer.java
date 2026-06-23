// Rename AZ reflection evidence from resources/serialize.json and optional sibling modules.
//@category NewWorld

import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.prefs.Preferences;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.stream.JsonReader;

import org.apache.commons.compress.archivers.sevenz.SevenZArchiveEntry;
import org.apache.commons.compress.archivers.sevenz.SevenZFile;

import docking.widgets.filechooser.GhidraFileChooser;
import docking.widgets.filechooser.GhidraFileChooserMode;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.ArrayDataType;
import ghidra.program.model.data.BooleanDataType;
import ghidra.program.model.data.ByteDataType;
import ghidra.program.model.data.Category;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.CharDataType;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeComponent;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.DoubleDataType;
import ghidra.program.model.data.FloatDataType;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.LongLongDataType;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.ShortDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.data.Undefined1DataType;
import ghidra.program.model.data.UnsignedIntegerDataType;
import ghidra.program.model.data.UnsignedLongLongDataType;
import ghidra.program.model.data.UnsignedShortDataType;
import ghidra.program.model.data.VoidDataType;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.CommentType;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Function.FunctionUpdateType;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.listing.GhidraClass;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Parameter;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.program.model.symbol.SymbolType;
import ghidra.util.exception.CancelledException;

public class AzReflectionRenamer extends GhidraScript {
    private static final Pattern MODULE_ADDR_RE =
        Pattern.compile("^(?<module>[^+]+)\\+0x(?<offset>[0-9a-fA-F]+)$");
    private static final Pattern HEX_ADDR_RE =
        Pattern.compile("^0x(?<addr>[0-9a-fA-F]+)$");
    private static final Pattern BAD_NAME_CHARS_RE =
        Pattern.compile("[^A-Za-z0-9_:<>,~]");
    private static final Pattern UNDERSCORES_RE = Pattern.compile("_+");
    private static final Pattern UUID_RE = Pattern.compile(
        "(?i)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}");
    private static final Pattern MB_GET_TYPE_NAME_SIGNATURE_RE = Pattern.compile(
        "const char \\*__cdecl MB::GetTypeName<class (?<type>[^>]+)>\\(void\\)");
    private static final int MAX_REF_DEPTH = 64;
    private static final int MAX_TEMPLATE_VALUE_DEPTH = 16;
    private static final int MAX_SUMMARY_ACTION_EXAMPLES = 8;
    private static final int MAX_CREATE_COMPONENT_CALLS = 32;
    private static final int MAX_CONSTRUCTOR_VTABLE_CANDIDATES = 8;
    private static final int MAX_VPTR_STORE_LOOKAHEAD = 8;
    private static final String PREFS_NODE = "newworld/az_serialize_context_renamer";
    private static final String PREF_LAST_INPUT_DIR = "lastInputDirectory";
    private static final String CORE_REFLECTION_DATATYPE_DESCRIPTION_PREFIX =
        "AZ source-backed reflection context type";
    private static final CoreRttiCastTarget[] CORE_RTTI_CAST_TARGETS = {
        new CoreRttiCastTarget(
            "ReflectContext",
            "B18D903B-7FAD-4A53-918A-3967B3198224"),
        new CoreRttiCastTarget(
            "SerializeContext",
            "83482F97-84DA-4FD4-BF9E-7FE34C8E091F"),
        new CoreRttiCastTarget(
            "BehaviorContext",
            "ED75FE05-9196-4F69-A3E5-1BDF5FF034CF"),
    };

    private static final String[] RTTI_SLOT_NAMES = {
        "~RttiHelper",
        "GetActualUuid",
        "GetActualTypeName",
        "CastConst",
        "Cast",
        "GetTypeId",
        "IsTypeOf",
        "IsAbstract",
        "EnumHierarchy",
    };

    private static final String[] SERIALIZE_RTTI_SLOT_NAMES = {
        "GetTypeId",
        "GetTypeName",
        "IsTypeOf",
        "EnumHierarchy",
        "CastConst",
        "Cast",
    };

    private static final String[] OBJECT_FIELDS = {
        "factory",
        "serializer",
        "eventHandler",
        "container",
        "dataConverter",
    };

    private static final Map<String, String> FUNCTION_FIELDS = functionFieldNames();

    private SymbolTable symbols;
    private DataTypeManager dataTypeManager;
    private Map<String, Structure> declaredStructuresByTypeId = Collections.emptyMap();
    private Gson gson;
    private boolean applyRenames;
    private File inputFile;
    private File outputFile;
    private ModuleEvidenceIndex moduleEvidence;
    private ClassRegistrationEvidenceIndex classRegistrationEvidence;
    private FieldRegistrationEvidenceIndex fieldRegistrationEvidence;
    private BehaviorContextEvidence behaviorContextEvidence;
    private TypeEvidenceIndex typeEvidence;
    private Map<String, JsonObject> jsonObjectsById;
    private Map<String, ArrayList<Address>> definedStringAddressesByValue;
    private Set<String> labelSeen;
    private Map<String, String> targetNameOwners;
    private String currentModuleSource;
    private String currentModuleName;
    private Set<String> ambiguousModuleComponentNames;
    private static final Map<String, String> MODULE_NAMES_BY_VTABLE = moduleNamesByVtable();

    @Override
    protected void run() throws Exception {
        if (currentProgram == null) {
            popup("No current program is open.");
            return;
        }

        symbols = currentProgram.getSymbolTable();
        dataTypeManager = currentProgram.getDataTypeManager();
        gson = new GsonBuilder().setPrettyPrinting().create();
        labelSeen = new HashSet<>();
        targetNameOwners = new HashMap<>();
        definedStringAddressesByValue = null;
        ambiguousModuleComponentNames = new TreeSet<>();

        inputFile = chooseInputFile();
        moduleEvidence = loadModuleEvidence(inputFile);
        classRegistrationEvidence = loadClassRegistrationEvidence(inputFile);
        fieldRegistrationEvidence = loadFieldRegistrationEvidence(inputFile);
        applyRenames = chooseApplyRenames();
        outputFile = chooseOutputFile(inputFile);
        behaviorContextEvidence = loadBehaviorContextEvidence(inputFile);

        JsonElement root;
        try (FileReader reader = new FileReader(inputFile)) {
            root = JsonParser.parseReader(reader);
        }
        if (root == null || !root.isJsonObject()) {
            popup("No SerializeContext object found in " + inputFile);
            return;
        }

        ScanResult scan = scanSerializeContext(root);
        enrichFieldRegistrationEvidence();
        Map<String, SlotGroup> rttiSlotGroups =
            rttiSlotGroups(scan.rttiTypes.values(), SERIALIZE_RTTI_SLOT_NAMES);
        Map<String, SlotGroup> callbackGroups = callbackGroups(scan.classData);
        Map<String, SlotGroup> classRegistrationGroups = classRegistrationFunctionGroups();
        ClassRegistrationFunctionIndex classFunctionIndex =
            classRegistrationFunctionIndex(classRegistrationEvidence);
        boolean includeFullActions = includeFullActionReport();
        Set<String> functionSeen = new HashSet<>();
        Set<String> aliasSeen = new HashSet<>();
        ActionSink actions = new ActionSink(includeFullActions);
        cleanupRepeatedFunctionNamespaces(actions);
        ensureCoreReflectionDatatypes(actions);
        processCoreRttiCastHelpers(functionSeen, actions);

        for (RttiType type : scan.rttiTypes.values()) {
            processRttiType(
                type,
                SERIALIZE_RTTI_SLOT_NAMES,
                rttiSlotGroups,
                functionSeen,
                aliasSeen,
                actions);
            if (monitor.isCancelled()) {
                break;
            }
        }

        for (ClassData classData : scan.classData) {
            processClassData(classData, callbackGroups, functionSeen, aliasSeen, actions);
            if (monitor.isCancelled()) {
                break;
            }
        }
        processReflectedDatatypes(scan.classData, actions);
        processClassRegistrationTraces(
            classRegistrationGroups,
            classFunctionIndex,
            functionSeen,
            aliasSeen,
            actions);
        processFieldRegistrationTraces(functionSeen, actions);
        processModuleDescriptorRenames(actions);
        processBehaviorContextEvidence(
            behaviorContextEvidence,
            functionSeen,
            aliasSeen,
            actions);
        processMbGetTypeNameFunctions(functionSeen, actions);
        processStaticReflectFunctionParameters(scan.classData, actions);

        ActionStats stats = actions.stats;

        JsonObject report = new JsonObject();
        report.addProperty("input", inputFile.getAbsolutePath());
        report.addProperty("mode", "ghidra-java");
        report.addProperty("requestedRenames", applyRenames);
        report.addProperty("applyRenames", applyRenames);
        report.addProperty("rttiTypeCount", scan.rttiTypes.size());
        report.addProperty("classDataCount", scan.classData.size());
        report.addProperty("elementCallbackCount", elementCallbackCount(scan.classData));
        report.addProperty("moduleDescriptorCount", moduleEvidence.descriptorCount);
        report.addProperty("typeNameCollisionCount", scan.collidingTypeNames.size());
        report.addProperty("unresolvedTypeNameCollisionCount",
            scan.unresolvedCollidingTypeNames.size());
        report.addProperty("actionCount", actions.size());
        report.addProperty("wouldApplyCount", stats.wouldApplyCount);
        report.addProperty("appliedCount", stats.appliedCount);
        report.addProperty("actionsIncluded", includeFullActions);
        report.add("moduleEvidence", moduleEvidenceReport(moduleEvidence));
        report.add("classRegistrationEvidence",
            classRegistrationEvidenceReport(classRegistrationEvidence));
        report.add("fieldRegistrationEvidence",
            fieldRegistrationEvidenceReport(fieldRegistrationEvidence));
        report.add(
            "duplicateModuleComponentShortNames",
            stringArray(ambiguousModuleComponentNames));
        report.add("behaviorContextEvidence",
            behaviorContextEvidenceReport(behaviorContextEvidence));
        report.add("typeEvidence", typeEvidenceReport(typeEvidence));
        report.add("actionKindCounts", mapToJson(stats.kindCounts));
        report.add("actionReasonCounts", mapToJson(stats.reasonCounts));
        report.add("actionSamples", actions.actionSamples());
        JsonArray collisions = new JsonArray();
        for (String name : scan.collidingTypeNames) {
            collisions.add(name);
        }
        report.add("typeNameCollisions", collisions);
        report.add("typeNameCollisionTypeIds", stringSetMapToJson(scan.collidingTypeIdsByName));
        JsonArray unresolvedCollisions = new JsonArray();
        for (String name : scan.unresolvedCollidingTypeNames) {
            unresolvedCollisions.add(name);
        }
        report.add("unresolvedTypeNameCollisions", unresolvedCollisions);
        report.add(
            "unresolvedTypeNameCollisionTypeIds",
            stringSetMapToJson(scan.unresolvedCollidingTypeIdsByName));
        if (includeFullActions) {
            report.add("actions", actions.fullActions());
        }

        try (FileWriter writer = new FileWriter(outputFile)) {
            gson.toJson(report, writer);
        }

        String summary = buildSummary(scan, actions, stats);
        println(summary);
    }

    private File chooseInputFile() throws Exception {
        String explicit = envValue("AZ_SERIALIZE_JSON");
        if (explicit != null) {
            return new File(explicit);
        }
        return chooseInputFileViaPicker();
    }

    private boolean includeFullActionReport() {
        String explicit = envValue("AZ_SERIALIZE_RENAME_FULL_ACTIONS");
        return explicit != null && envBool(explicit);
    }

    private File chooseInputFileViaPicker() throws Exception {
        GhidraFileChooser chooser = new GhidraFileChooser(null);
        chooser.setTitle("Select SerializeContext JSON capture");
        chooser.setApproveButtonText("Open");
        chooser.setFileSelectionMode(GhidraFileChooserMode.FILES_AND_DIRECTORIES);
        File lastDirectory = lastInputDirectory();
        if (lastDirectory != null) {
            chooser.setCurrentDirectory(lastDirectory);
            File defaultInput = defaultSerializeInput(lastDirectory);
            if (defaultInput != null) {
                chooser.setSelectedFile(defaultInput);
            }
        }
        File selected = chooser.getSelectedFile();
        chooser.dispose();
        if (selected == null) {
            throw new CancelledException();
        }
        File input = selectedSerializeInput(selected);
        if (input == null) {
            throw new IllegalArgumentException(
                "Selected directory does not contain serialize.json: "
                    + selected.getAbsolutePath());
        }
        rememberInputDirectory(input);
        return input;
    }

    private File selectedSerializeInput(File selected) {
        if (selected == null) {
            return null;
        }
        if (selected.isDirectory()) {
            return defaultSerializeInput(selected);
        }
        return selected;
    }

    private File defaultSerializeInput(File directory) {
        if (directory == null || !directory.isDirectory()) {
            return null;
        }
        File serialize = new File(directory, "serialize.json");
        if (serialize.isFile()) {
            return serialize;
        }
        return null;
    }

    private File lastInputDirectory() {
        String path;
        try {
            path = Preferences.userRoot()
                .node(PREFS_NODE)
                .get(PREF_LAST_INPUT_DIR, null);
        }
        catch (SecurityException e) {
            println("Unable to read last SerializeContext input directory: " +
                e.getMessage());
            return null;
        }
        if (path == null || path.trim().isEmpty()) {
            return null;
        }
        File directory = new File(path);
        return directory.isDirectory() ? directory : null;
    }

    private void rememberInputDirectory(File file) {
        if (file == null) {
            return;
        }
        File directory = file.isDirectory() ? file : file.getParentFile();
        if (directory == null) {
            return;
        }
        try {
            Preferences.userRoot()
                .node(PREFS_NODE)
                .put(PREF_LAST_INPUT_DIR, directory.getAbsolutePath());
        }
        catch (SecurityException e) {
            println("Unable to persist last SerializeContext input directory: " +
                e.getMessage());
        }
    }

    private boolean chooseApplyRenames() throws Exception {
        String explicit = envValue("AZ_SERIALIZE_RENAME");
        if (explicit != null) {
            return envBool(explicit);
        }
        return askYesNo(
            "AZ SerializeContext Renamer",
            "Apply renames, datatypes, and signatures to the current Ghidra program?\n\n" +
                "Yes: mutate symbols/namespaces/datatype manager/function signatures.\n" +
                "No: dry-run only and write the report.");
    }

    private File chooseOutputFile(File input) throws Exception {
        String explicit = envValue("AZ_SERIALIZE_RENAME_OUT");
        if (explicit != null) {
            return new File(explicit);
        }
        String selected = askString(
            "AZ SerializeContext Rename Report",
            "Write rename report JSON to:",
            defaultReportPath(input));
        if (selected == null || selected.trim().isEmpty()) {
            return new File(defaultReportPath(input));
        }
        return new File(selected.trim());
    }

    private String defaultReportPath(File input) {
        String path = input.getAbsolutePath();
        int dot = path.lastIndexOf('.');
        String base = dot > 0 ? path.substring(0, dot) : path;
        return base + ".renames.json";
    }

    private ModuleEvidenceIndex loadModuleEvidence(File serializeInput) {
        ModuleEvidenceIndex index = new ModuleEvidenceIndex();
        File input = moduleEvidenceInput(serializeInput);
        if (input == null || !input.exists()) {
            return index;
        }
        index.input = input.getAbsolutePath();

        List<File> files = moduleJsonFiles(input);
        index.inputCount = files.size();
        for (File file : files) {
            ModuleCapture module;
            try (FileReader reader = new FileReader(file)) {
                module = gson.fromJson(reader, ModuleCapture.class);
            }
            catch (Exception e) {
                index.skippedInputs++;
                index.skippedReasons.add(file.getName() + ":read-failed");
                continue;
            }
            if (module == null || module.descriptors == null) {
                index.skippedInputs++;
                index.skippedReasons.add(file.getName() + ":missing-descriptors");
                continue;
            }

            String moduleName = moduleName(module);
            if (moduleName == null) {
                index.skippedInputs++;
                index.skippedReasons.add(file.getName() + ":missing-module-name");
                continue;
            }

            index.moduleNames.add(moduleName);
            ModuleCaptureInput captureInput = new ModuleCaptureInput(file, module, moduleName);
            index.inputs.add(captureInput);
            for (Descriptor descriptor : module.descriptors) {
                index.descriptorCount++;
                index.descriptors.add(descriptor);
                String typeId = descriptorTypeId(descriptor);
                if (typeId == null) {
                    index.descriptorsWithoutTypeId++;
                    continue;
                }

                ModuleOwner owner = new ModuleOwner();
                owner.typeId = typeId;
                owner.moduleName = moduleName;
                owner.componentName = descriptorComponentName(descriptor);
                owner.source = file.getName();
                rememberModuleOwner(index, owner);
            }
        }
        return index;
    }

    private File moduleEvidenceInput(File serializeInput) {
        String explicit = envValue("AZ_SERIALIZE_MODULE_PATH");
        if (explicit == null) {
            explicit = envValue("AZ_SERIALIZE_MODULE_DIR");
        }
        if (explicit == null) {
            explicit = envValue("AZ_MODULE_PATH");
        }
        if (explicit == null) {
            explicit = envValue("AZ_MODULE_DIR");
        }
        if (explicit != null) {
            return new File(explicit);
        }

        File parent = serializeInput == null ? null : serializeInput.getParentFile();
        if (parent == null) {
            return null;
        }
        File modules = new File(parent, "modules");
        return modules.exists() ? modules : null;
    }

    private List<File> moduleJsonFiles(File input) {
        ArrayList<File> files = new ArrayList<>();
        if (input.isFile()) {
            if (isModuleJsonFile(input)) {
                files.add(input);
            }
            return files;
        }
        File[] children = input.listFiles();
        if (children == null) {
            return files;
        }
        for (File child : children) {
            if (child.isFile() && isModuleJsonFile(child)) {
                files.add(child);
            }
        }
        Collections.sort(files, (left, right) ->
            left.getAbsolutePath().compareToIgnoreCase(right.getAbsolutePath()));
        return files;
    }

    private boolean isModuleJsonFile(File file) {
        String name = file.getName().toLowerCase(Locale.ROOT);
        return name.endsWith(".json") &&
            !name.endsWith(".debug.json") &&
            !name.endsWith(".renames.json");
    }

    private void rememberModuleOwner(ModuleEvidenceIndex index, ModuleOwner owner) {
        String normalizedTypeId = normalizeTypeId(owner.typeId);
        if (normalizedTypeId == null) {
            return;
        }
        owner.typeId = normalizedTypeId;
        ModuleOwner existing = index.ownersByTypeId.get(normalizedTypeId);
        if (existing == null) {
            index.ownersByTypeId.put(normalizedTypeId, owner);
            return;
        }
        if (existing.moduleName.equals(owner.moduleName)) {
            return;
        }
        index.duplicateTypeIds++;
        LinkedHashSet<String> modules = index.duplicateModulesByTypeId.get(normalizedTypeId);
        if (modules == null) {
            modules = new LinkedHashSet<>();
            index.duplicateModulesByTypeId.put(normalizedTypeId, modules);
        }
        modules.add(existing.moduleName);
        modules.add(owner.moduleName);
    }

    private String moduleName(ModuleCapture module) {
        if (module == null || module.vftable == null) {
            return null;
        }
        String offset = captureOffset(module.vftable);
        return offset == null ? null : MODULE_NAMES_BY_VTABLE.get(offset);
    }

    private String descriptorTypeId(Descriptor descriptor) {
        if (descriptor == null) {
            return null;
        }
        String typeId = descriptor.componentUuid;
        if (typeId == null && descriptor.azRtti != null) {
            typeId = descriptor.azRtti.typeId;
        }
        return normalizeTypeId(typeId);
    }

    private String descriptorComponentName(Descriptor descriptor) {
        if (descriptor == null) {
            return null;
        }
        String name = safeTypeName(descriptor.componentName);
        if (name != null) {
            return name;
        }
        if (descriptor.azRtti != null) {
            name = safeTypeName(descriptor.azRtti.typeName);
            if (name != null) {
                return name;
            }
        }
        return safeTypeName(componentNameFromGetNameSlot(descriptor));
    }

    private String componentNameFromGetNameSlot(Descriptor descriptor) {
        if (descriptor.vtableSlots == null) {
            return null;
        }
        for (VTableSlot slot : descriptor.vtableSlots) {
            if ("GetName".equals(slot.expected) && slot.address != null) {
                return stringReturnedBySimpleFunction(slot.address);
            }
        }
        return null;
    }

    private String stringReturnedBySimpleFunction(String jsonAddress) {
        Address address = parseCaptureAddress(jsonAddress);
        if (!isProgramAddress(address)) {
            return null;
        }
        try {
            int b0 = unsignedByte(address, 0);
            int b1 = unsignedByte(address, 1);
            int b2 = unsignedByte(address, 2);
            if (b0 == 0x48 && b1 == 0x8d && b2 == 0x05) {
                int displacement = int32(address, 3);
                return readCString(absoluteAddress(address.getOffset() + 7L + displacement));
            }
            if (b0 == 0x48 && b1 == 0xb8) {
                return readCString(absoluteAddress(int64(address, 2)));
            }
            if (b0 == 0xb8) {
                return readCString(absoluteAddress(uint32(address, 1)));
            }
        }
        catch (Exception ignored) {
            return null;
        }
        return null;
    }

    private JsonObject moduleEvidenceReport(ModuleEvidenceIndex index) {
        JsonObject report = new JsonObject();
        report.addProperty("input", index.input);
        report.addProperty("inputCount", index.inputCount);
        report.addProperty("skippedInputs", index.skippedInputs);
        report.addProperty("descriptorCount", index.descriptorCount);
        report.addProperty("ownerTypeIdCount", index.ownersByTypeId.size());
        report.addProperty("descriptorsWithoutTypeId", index.descriptorsWithoutTypeId);
        report.addProperty("duplicateTypeIdCount", index.duplicateTypeIds);

        JsonArray modules = new JsonArray();
        for (String moduleName : index.moduleNames) {
            modules.add(moduleName);
        }
        report.add("modules", modules);

        JsonArray skipped = new JsonArray();
        for (String reason : index.skippedReasons) {
            skipped.add(reason);
        }
        report.add("skippedReasons", skipped);
        report.add("duplicateTypeIds", stringSetMapToJson(index.duplicateModulesByTypeId));
        return report;
    }

    private ClassRegistrationEvidenceIndex loadClassRegistrationEvidence(File serializeInput) {
        ClassRegistrationEvidenceIndex index = new ClassRegistrationEvidenceIndex();
        File input = classRegistrationEvidenceInput(serializeInput);
        if (input == null || !input.exists() || !input.isFile()) {
            return index;
        }
        index.input = input.getAbsolutePath();

        try (BufferedReader reader = new BufferedReader(new FileReader(input))) {
            String line;
            int lineNumber = 0;
            while ((line = reader.readLine()) != null) {
                lineNumber++;
                String trimmed = line.trim();
                if (trimmed.isEmpty()) {
                    continue;
                }

                JsonElement value;
                try {
                    value = JsonParser.parseString(trimmed);
                }
                catch (Exception e) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":parse-failed");
                    continue;
                }
                if (value == null || !value.isJsonObject()) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":not-object");
                    continue;
                }

                ClassRegistrationRecord record =
                    classRegistrationRecord(value.getAsJsonObject(), lineNumber);
                if (record == null) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":missing-type-id");
                    continue;
                }
                rememberClassRegistrationRecord(index, record);
            }
        }
        catch (Exception e) {
            index.skippedInputs++;
            index.skippedReasons.add(input.getName() + ":read-failed");
            return index;
        }
        return index;
    }

    private File classRegistrationEvidenceInput(File serializeInput) {
        String explicit = envValue("AZ_SERIALIZE_CLASS_REGISTRATION_PATH");
        if (explicit == null) {
            explicit = envValue("AZ_CLASS_REGISTRATION_TRACE");
        }
        if (explicit != null) {
            return new File(explicit);
        }

        File parent = serializeInput == null ? null : serializeInput.getParentFile();
        return parent == null ? null : new File(parent, "serialize-class-registration.jsonl");
    }

    private ClassRegistrationRecord classRegistrationRecord(JsonObject object, int lineNumber) {
        String typeId = normalizeTypeId(stringMemberDirect(object, "typeId"));
        if (typeId == null) {
            return null;
        }

        ClassRegistrationRecord record = new ClassRegistrationRecord();
        record.lineNumber = lineNumber;
        record.sequence = longMemberDirect(object, "sequence");
        record.typeId = typeId;
        record.typeName = stringMemberDirect(object, "typeName");
        record.returnAddress = stringMemberDirect(object, "returnAddress");
        record.classDataFactory = stringMemberDirect(object, "classDataFactory");
        record.classDataAzRtti = stringMemberDirect(object, "classDataAzRtti");
        record.anyCreator = stringMemberDirect(object, "anyCreator");
        return record;
    }

    private void rememberClassRegistrationRecord(
        ClassRegistrationEvidenceIndex index,
        ClassRegistrationRecord record) {

        index.recordCount++;
        index.records.add(record);
        if (record.returnAddress == null) {
            index.recordsWithoutReturnAddress++;
        }
        if (record.classDataAzRtti == null) {
            index.recordsWithoutClassDataAzRtti++;
        }
        else {
            index.rttiBackedRecordCount++;
        }

        ClassRegistrationRecord existing = index.recordsByTypeId.get(record.typeId);
        if (existing == null) {
            index.recordsByTypeId.put(record.typeId, record);
            return;
        }
        if (sameAddress(existing.returnAddress, record.returnAddress)) {
            return;
        }

        LinkedHashSet<String> returnAddresses =
            index.duplicateReturnAddressesByTypeId.get(record.typeId);
        if (returnAddresses == null) {
            returnAddresses = new LinkedHashSet<>();
            index.duplicateReturnAddressesByTypeId.put(record.typeId, returnAddresses);
            index.duplicateTypeIds++;
        }
        returnAddresses.add(displayAddress(existing.returnAddress));
        returnAddresses.add(displayAddress(record.returnAddress));
    }

    private String displayAddress(String address) {
        return address == null ? "<missing>" : address;
    }

    private boolean sameAddress(String left, String right) {
        if (left == null || right == null) {
            return left == right;
        }
        return addressKey(left).equals(addressKey(right));
    }

    private boolean sameTypeId(String left, String right) {
        String normalizedLeft = normalizeTypeId(left);
        String normalizedRight = normalizeTypeId(right);
        if (normalizedLeft == null || normalizedRight == null) {
            return normalizedLeft == normalizedRight;
        }
        return normalizedLeft.equals(normalizedRight);
    }

    private JsonObject classRegistrationEvidenceReport(ClassRegistrationEvidenceIndex index) {
        JsonObject report = new JsonObject();
        if (index == null) {
            report.addProperty("recordCount", 0);
            report.addProperty("rttiBackedRecordCount", 0);
            return report;
        }

        report.addProperty("input", index.input);
        report.addProperty("recordCount", index.recordCount);
        report.addProperty("uniqueTypeIdCount", index.recordsByTypeId.size());
        report.addProperty("rttiBackedRecordCount", index.rttiBackedRecordCount);
        report.addProperty("duplicateTypeIdCount", index.duplicateTypeIds);
        report.addProperty("recordsWithoutReturnAddress", index.recordsWithoutReturnAddress);
        report.addProperty("recordsWithoutClassDataAzRtti", index.recordsWithoutClassDataAzRtti);
        report.addProperty("skippedInputs", index.skippedInputs);
        report.addProperty("skippedRecords", index.skippedRecords);
        report.add("duplicateReturnAddressesByTypeId",
            stringSetMapToJson(index.duplicateReturnAddressesByTypeId));

        JsonArray skipped = new JsonArray();
        for (String reason : index.skippedReasons) {
            skipped.add(reason);
        }
        report.add("skippedReasons", skipped);
        return report;
    }

    private FieldRegistrationEvidenceIndex loadFieldRegistrationEvidence(File serializeInput) {
        FieldRegistrationEvidenceIndex index = new FieldRegistrationEvidenceIndex();
        File input = fieldRegistrationEvidenceInput(serializeInput);
        if (input == null || !input.exists() || !input.isFile()) {
            return index;
        }
        index.input = input.getAbsolutePath();

        try (BufferedReader reader = new BufferedReader(new FileReader(input))) {
            String line;
            int lineNumber = 0;
            while ((line = reader.readLine()) != null) {
                lineNumber++;
                String trimmed = line.trim();
                if (trimmed.isEmpty()) {
                    continue;
                }

                JsonElement value;
                try {
                    value = JsonParser.parseString(trimmed);
                }
                catch (Exception e) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":parse-failed");
                    continue;
                }
                if (value == null || !value.isJsonObject()) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":not-object");
                    continue;
                }

                FieldRegistrationRecord record;
                try {
                    record = fieldRegistrationRecord(value.getAsJsonObject(), lineNumber);
                }
                catch (Exception e) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":record-failed:" +
                        e.getClass().getSimpleName() + ":" + e.getMessage());
                    continue;
                }
                if (record == null) {
                    index.skippedRecords++;
                    index.skippedReasons.add(lineNumber + ":missing-field-evidence");
                    continue;
                }
                rememberFieldRegistrationRecord(index, record);
            }
        }
        catch (Exception e) {
            index.skippedInputs++;
            index.skippedReasons.add(input.getName() + ":read-failed");
            return index;
        }
        return index;
    }

    private void enrichFieldRegistrationEvidence() {
        if (fieldRegistrationEvidence == null) {
            return;
        }
        ClassRegistrationFunctionIndex functionIndex =
            classRegistrationFunctionIndex(classRegistrationEvidence);

        fieldRegistrationEvidence.recordsWithoutOwner = 0;
        fieldRegistrationEvidence.recordsWithoutFieldTypeName = 0;
        fieldRegistrationEvidence.recordsWithLiveOwner = 0;
        fieldRegistrationEvidence.recordsWithStaticOwner = 0;
        fieldRegistrationEvidence.recordsWithClassRegistrationFieldTypeName = 0;
        fieldRegistrationEvidence.recordsWithoutFieldCallReturnAddress = 0;
        fieldRegistrationEvidence.recordsWithoutFieldCallsite = 0;
        fieldRegistrationEvidence.recordsWithoutFieldFunction = 0;
        fieldRegistrationEvidence.recordsWithoutClassRegistrationOwner = 0;
        fieldRegistrationEvidence.recordsWithAmbiguousStaticOwner = 0;
        fieldRegistrationEvidence.ownerResolutionReasonCounts.clear();
        fieldRegistrationEvidence.recordsWithGraphOwner = 0;
        fieldRegistrationEvidence.recordsWithGraphFieldName = 0;
        fieldRegistrationEvidence.recordsWithGraphFieldTypeName = 0;
        fieldRegistrationEvidence.recordsWithAmbiguousGraphOwner = 0;
        fieldRegistrationEvidence.recordsWithoutGraphOwner = 0;

        for (FieldRegistrationRecord record : fieldRegistrationEvidence.records) {
            if (record.fieldCallReturnAddress == null) {
                fieldRegistrationEvidence.recordsWithoutFieldCallReturnAddress++;
            }
            if (record.ownerTypeId != null) {
                fieldRegistrationEvidence.recordsWithLiveOwner++;
            }
            else {
                OwnerResolution owner = classRegistrationOwnerForField(record, functionIndex);
                record.ownerResolution = owner.reason;
                record.ownerFunctionAddress = owner.functionAddress;
                if (owner.record != null) {
                    record.ownerTypeId = owner.record.typeId;
                    record.ownerTypeName = owner.record.typeName;
                    record.ownerSource = "class-registration-function";
                    fieldRegistrationEvidence.recordsWithStaticOwner++;
                }
                else if ("field-callsite-not-found".equals(owner.reason)) {
                    fieldRegistrationEvidence.recordsWithoutFieldCallsite++;
                }
                else if ("field-function-not-found".equals(owner.reason)) {
                    fieldRegistrationEvidence.recordsWithoutFieldFunction++;
                }
                else if ("ambiguous-owner-function".equals(owner.reason)) {
                    fieldRegistrationEvidence.recordsWithAmbiguousStaticOwner++;
                }
                else if ("no-class-registration-in-function".equals(owner.reason) ||
                    "no-preceding-class-registration-in-function".equals(owner.reason)) {
                    fieldRegistrationEvidence.recordsWithoutClassRegistrationOwner++;
                }
            }

            FieldGraphResolution graph = fieldGraphResolution(record);
            if (graph.owner != null && graph.element != null) {
                if (record.ownerTypeId == null) {
                    record.ownerTypeId = graph.owner.typeId;
                    record.ownerTypeName = graph.owner.typeName;
                    record.ownerSource = "serialize-field-graph";
                    record.ownerResolution = graph.reason;
                    fieldRegistrationEvidence.recordsWithGraphOwner++;
                }
                if (record.fieldName == null && graph.element.name != null) {
                    record.fieldName = graph.element.name;
                    record.fieldNameSource = graph.reason;
                    fieldRegistrationEvidence.recordsWithGraphFieldName++;
                }
                if (record.fieldTypeName == null && graph.element.typeName != null) {
                    record.fieldTypeName = graph.element.typeName;
                    record.fieldTypeNameSource = graph.reason;
                    fieldRegistrationEvidence.recordsWithGraphFieldTypeName++;
                }
            }
            else if (graph.ambiguous) {
                fieldRegistrationEvidence.recordsWithAmbiguousGraphOwner++;
            }
            else {
                fieldRegistrationEvidence.recordsWithoutGraphOwner++;
            }

            if (record.fieldTypeName == null && record.fieldTypeId != null &&
                classRegistrationEvidence != null) {
                ClassRegistrationRecord fieldType =
                    classRegistrationEvidence.recordsByTypeId.get(record.fieldTypeId);
                if (fieldType != null) {
                    record.fieldTypeName = fieldType.typeName;
                    record.fieldTypeNameSource = "class-registration-type-id";
                    fieldRegistrationEvidence.recordsWithClassRegistrationFieldTypeName++;
                }
            }

            if (record.ownerTypeId == null) {
                fieldRegistrationEvidence.recordsWithoutOwner++;
            }
            increment(
                fieldRegistrationEvidence.ownerResolutionReasonCounts,
                record.ownerResolution);
            if (record.fieldTypeName == null) {
                fieldRegistrationEvidence.recordsWithoutFieldTypeName++;
            }
        }

        fieldRegistrationEvidence.ownerFunctionCount =
            functionIndex.recordsByFunction.size();
        fieldRegistrationEvidence.ambiguousOwnerFunctionCount =
            functionIndex.ambiguousFunctions.size();
        fieldRegistrationEvidence.ownerClassRegistrationRecordsWithoutFunction =
            functionIndex.recordsWithoutFunction;
    }

    private ClassRegistrationFunctionIndex classRegistrationFunctionIndex(
        ClassRegistrationEvidenceIndex index) {

        ClassRegistrationFunctionIndex result = new ClassRegistrationFunctionIndex();
        if (index == null) {
            return result;
        }

        for (ClassRegistrationRecord record : index.records) {
            Address callsite = classRegistrationCallsite(record.returnAddress);
            Function function = callsite == null
                ? null
                : currentProgram.getFunctionManager().getFunctionContaining(callsite);
            if (function == null) {
                result.recordsWithoutFunction++;
                continue;
            }

            String key = formatAddress(function.getEntryPoint());
            ArrayList<ClassRegistrationCallsite> records =
                result.recordsByFunction.get(key);
            if (records == null) {
                records = new ArrayList<>();
                result.recordsByFunction.put(key, records);
            }
            ClassRegistrationCallsite callsiteRecord = new ClassRegistrationCallsite();
            callsiteRecord.record = record;
            callsiteRecord.callsite = callsite;
            records.add(callsiteRecord);
        }
        return result;
    }

    private OwnerResolution classRegistrationOwnerForField(
        FieldRegistrationRecord record,
        ClassRegistrationFunctionIndex functionIndex) {

        if (record == null || functionIndex == null) {
            return OwnerResolution.missing("invalid-record");
        }
        if (!isAddressLike(record.fieldCallReturnAddress)) {
            return OwnerResolution.missing("missing-field-call-return");
        }
        Address callsite = callsiteBeforeReturn(record.fieldCallReturnAddress);
        Function function = callsite == null
            ? null
            : currentProgram.getFunctionManager().getFunctionContaining(callsite);
        if (callsite == null) {
            return OwnerResolution.missing("field-callsite-not-found");
        }
        if (function == null) {
            return OwnerResolution.missing("field-function-not-found");
        }
        String key = formatAddress(function.getEntryPoint());
        ArrayList<ClassRegistrationCallsite> owners =
            functionIndex.recordsByFunction.get(key);
        if (owners == null || owners.isEmpty()) {
            return OwnerResolution.missing("no-class-registration-in-function", key);
        }

        ClassRegistrationCallsite selected = null;
        boolean ambiguous = false;
        for (ClassRegistrationCallsite owner : owners) {
            if (owner.callsite == null ||
                owner.callsite.compareTo(callsite) > 0) {
                continue;
            }
            if (selected == null ||
                owner.callsite.compareTo(selected.callsite) > 0) {
                selected = owner;
                ambiguous = false;
                continue;
            }
            if (owner.callsite.equals(selected.callsite) &&
                !sameTypeId(owner.record.typeId, selected.record.typeId)) {
                ambiguous = true;
            }
        }

        if (selected == null) {
            return OwnerResolution.missing(
                "no-preceding-class-registration-in-function",
                key);
        }
        if (ambiguous) {
            functionIndex.ambiguousFunctions.add(key);
            return OwnerResolution.missing("ambiguous-owner-function", key);
        }
        return OwnerResolution.found(selected.record, key);
    }

    private FieldGraphResolution fieldGraphResolution(FieldRegistrationRecord record) {
        if (record == null || typeEvidence == null) {
            return FieldGraphResolution.missing("serialize-field-graph-unavailable");
        }

        String ownerTypeId = normalizeTypeId(record.ownerTypeId);
        if (ownerTypeId != null) {
            ClassData owner = typeEvidence.classBodiesByTypeId.get(ownerTypeId);
            FieldOwner ownerField = uniqueOwnerFieldMatch(owner, record);
            if (ownerField != null) {
                return FieldGraphResolution.found(
                    ownerField.owner,
                    ownerField.element,
                    "serialize-field-graph-owner-body");
            }
        }

        FieldGraphResolution exact = fieldGraphResolutionByKey(
            typeEvidence.fieldOwnersByExactKey,
            exactFieldOwnerKey(record.fieldName, record.fieldTypeId, record.fieldOffset),
            "serialize-field-graph-exact");
        if (exact.resolvedOrAmbiguous()) {
            return exact;
        }

        FieldGraphResolution nameType = fieldGraphResolutionByKey(
            typeEvidence.fieldOwnersByNameTypeKey,
            nameTypeFieldOwnerKey(record.fieldName, record.fieldTypeId),
            "serialize-field-graph-name-type");
        if (nameType.resolvedOrAmbiguous()) {
            return nameType;
        }

        FieldGraphResolution typeOffset = fieldGraphResolutionByKey(
            typeEvidence.fieldOwnersByTypeOffsetKey,
            typeOffsetFieldOwnerKey(record.fieldTypeId, record.fieldOffset),
            "serialize-field-graph-type-offset");
        if (typeOffset.resolvedOrAmbiguous()) {
            return typeOffset;
        }

        return FieldGraphResolution.missing("serialize-field-graph-miss");
    }

    private FieldOwner uniqueOwnerFieldMatch(ClassData owner, FieldRegistrationRecord record) {
        if (owner == null || owner.elements == null) {
            return null;
        }

        ArrayList<FieldOwner> matches = new ArrayList<>();
        for (ElementData element : owner.elements) {
            if (!fieldElementMatches(record, element)) {
                continue;
            }
            FieldOwner fieldOwner = new FieldOwner();
            fieldOwner.owner = owner;
            fieldOwner.element = element;
            matches.add(fieldOwner);
        }
        return uniqueFieldOwner(matches);
    }

    private boolean fieldElementMatches(
        FieldRegistrationRecord record,
        ElementData element) {

        String fieldTypeId = normalizeTypeId(record.fieldTypeId);
        if (fieldTypeId != null && !sameTypeId(fieldTypeId, element.typeId)) {
            return false;
        }
        String offset = normalizeFieldOffset(record.fieldOffset);
        if (offset != null && !offset.equals(normalizeFieldOffset(element.offset))) {
            return false;
        }
        String name = normalizeFieldName(record.fieldName);
        if (name != null && !name.equals(normalizeFieldName(element.name))) {
            return false;
        }
        return fieldTypeId != null || offset != null || name != null;
    }

    private FieldGraphResolution fieldGraphResolutionByKey(
        LinkedHashMap<String, ArrayList<FieldOwner>> index,
        String key,
        String reason) {

        if (key == null) {
            return FieldGraphResolution.missing(reason + "-missing-key");
        }
        ArrayList<FieldOwner> owners = index.get(key);
        if (owners == null || owners.isEmpty()) {
            return FieldGraphResolution.missing(reason + "-miss");
        }
        FieldOwner owner = uniqueFieldOwner(owners);
        if (owner == null) {
            return FieldGraphResolution.ambiguous(reason + "-ambiguous");
        }
        return FieldGraphResolution.found(owner.owner, owner.element, reason);
    }

    private File fieldRegistrationEvidenceInput(File serializeInput) {
        String explicit = envValue("AZ_SERIALIZE_FIELD_REGISTRATION_PATH");
        if (explicit == null) {
            explicit = envValue("AZ_FIELD_REGISTRATION_TRACE");
        }
        if (explicit != null) {
            return new File(explicit);
        }

        File parent = serializeInput == null ? null : serializeInput.getParentFile();
        return parent == null ? null : new File(parent, "serialize-field-registration.jsonl");
    }

    private FieldRegistrationRecord fieldRegistrationRecord(JsonObject object, int lineNumber) {
        String fieldCallReturnAddress = stringMemberDirect(object, "fieldCallReturnAddress");
        JsonObject owner = objectMemberDirect(object, "ownerClassData");
        JsonObject element = objectMemberDirect(object, "classElement");
        String fieldName = stringMemberDirect(element, "name");
        String fieldTypeId = normalizeTypeId(stringMemberDirect(element, "typeId"));
        if (!isAddressLike(fieldCallReturnAddress) && fieldName == null && fieldTypeId == null) {
            return null;
        }

        FieldRegistrationRecord record = new FieldRegistrationRecord();
        record.lineNumber = lineNumber;
        record.sequence = longMemberDirect(object, "sequence");
        record.fieldCallReturnAddress =
            isAddressLike(fieldCallReturnAddress) ? fieldCallReturnAddress : null;
        record.helperReturnAddress = stringMemberDirect(object, "helperReturnAddress");
        record.ownerTypeName = stringMemberDirect(owner, "name");
        record.ownerTypeId = normalizeTypeId(stringMemberDirect(owner, "typeId"));
        if (record.ownerTypeId != null) {
            record.ownerSource = "live-frame";
            record.ownerResolution = "live-frame";
        }
        record.fieldName = fieldName;
        record.fieldTypeId = fieldTypeId;
        record.fieldOffset = stringMemberDirect(element, "offset");
        record.fieldTypeName = fieldTypeName(element);
        return record;
    }

    private JsonObject objectMemberDirect(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull() || !value.isJsonObject()) {
            return null;
        }
        return value.getAsJsonObject();
    }

    private String fieldTypeName(JsonObject classElement) {
        JsonObject azRtti = objectMemberDirect(classElement, "azRtti");
        String typeName = stringMemberDirect(azRtti, "typeName");
        if (typeName != null) {
            return typeName;
        }
        return stringMemberDirect(classElement, "typeName");
    }

    private void rememberFieldRegistrationRecord(
        FieldRegistrationEvidenceIndex index,
        FieldRegistrationRecord record) {

        index.recordCount++;
        if (record.fieldCallReturnAddress == null) {
            index.recordsWithoutFieldCallReturnAddress++;
        }
        if (record.ownerTypeId == null) {
            index.recordsWithoutOwner++;
        }
        if (record.fieldTypeName == null) {
            index.recordsWithoutFieldTypeName++;
        }
        index.records.add(record);
    }

    private JsonObject fieldRegistrationEvidenceReport(FieldRegistrationEvidenceIndex index) {
        JsonObject report = new JsonObject();
        if (index == null) {
            report.addProperty("recordCount", 0);
            return report;
        }

        report.addProperty("input", index.input);
        report.addProperty("recordCount", index.recordCount);
        report.addProperty("skippedInputs", index.skippedInputs);
        report.addProperty("skippedRecords", index.skippedRecords);
        report.addProperty("recordsWithLiveOwner", index.recordsWithLiveOwner);
        report.addProperty("recordsWithStaticOwner", index.recordsWithStaticOwner);
        report.addProperty("recordsWithoutOwner", index.recordsWithoutOwner);
        report.addProperty(
            "recordsWithoutFieldCallReturnAddress",
            index.recordsWithoutFieldCallReturnAddress);
        report.addProperty(
            "recordsWithoutFieldCallsite",
            index.recordsWithoutFieldCallsite);
        report.addProperty(
            "recordsWithoutFieldFunction",
            index.recordsWithoutFieldFunction);
        report.addProperty(
            "recordsWithoutClassRegistrationOwner",
            index.recordsWithoutClassRegistrationOwner);
        report.addProperty(
            "recordsWithAmbiguousStaticOwner",
            index.recordsWithAmbiguousStaticOwner);
        report.addProperty(
            "recordsWithClassRegistrationFieldTypeName",
            index.recordsWithClassRegistrationFieldTypeName);
        report.addProperty("recordsWithGraphOwner", index.recordsWithGraphOwner);
        report.addProperty(
            "recordsWithAmbiguousGraphOwner",
            index.recordsWithAmbiguousGraphOwner);
        report.addProperty("recordsWithoutGraphOwner", index.recordsWithoutGraphOwner);
        report.addProperty("recordsWithGraphFieldName", index.recordsWithGraphFieldName);
        report.addProperty(
            "recordsWithGraphFieldTypeName",
            index.recordsWithGraphFieldTypeName);
        report.addProperty("recordsWithoutFieldTypeName", index.recordsWithoutFieldTypeName);
        report.addProperty("ownerFunctionCount", index.ownerFunctionCount);
        report.addProperty(
            "ambiguousOwnerFunctionCount",
            index.ambiguousOwnerFunctionCount);
        report.addProperty(
            "ownerClassRegistrationRecordsWithoutFunction",
            index.ownerClassRegistrationRecordsWithoutFunction);
        report.add("ownerResolutionReasons", mapToJson(index.ownerResolutionReasonCounts));
        JsonArray skipped = new JsonArray();
        for (String reason : index.skippedReasons) {
            skipped.add(reason);
        }
        report.add("skippedReasons", skipped);
        return report;
    }

    private BehaviorContextEvidence loadBehaviorContextEvidence(File serializeInput) {
        BehaviorContextEvidence index = new BehaviorContextEvidence();
        BehaviorContextInput input = findBehaviorContextInput(serializeInput);
        if (input == null) {
            return index;
        }
        index.input = input.description();
        index.inputFormat = input.format();
        index.archiveEntry = input.archiveEntryName;

        boolean readClasses = false;
        boolean readMethods = false;
        boolean readProperties = false;
        boolean readEbuses = false;
        boolean readTypeToClassMap = false;
        try (
            Reader source = input.openReader();
            JsonReader reader = new JsonReader(new BufferedReader(source))
        ) {
            reader.beginObject();
            while (reader.hasNext()) {
                String name = reader.nextName();
                if ((name.equals("classes") || name.equals("m_classes")) && !readClasses) {
                    readClasses = true;
                    scanBehaviorClasses(reader, index);
                }
                else if ((name.equals("methods") || name.equals("m_methods")) && !readMethods) {
                    readMethods = true;
                    scanBehaviorMethods(reader, index);
                }
                else if ((name.equals("properties") || name.equals("m_properties")) &&
                    !readProperties) {
                    readProperties = true;
                    scanBehaviorProperties(reader, index);
                }
                else if ((name.equals("ebuses") || name.equals("m_ebuses")) && !readEbuses) {
                    readEbuses = true;
                    scanBehaviorEbuses(reader, index);
                }
                else if ((name.equals("typeToClassMap") || name.equals("m_typeToClassMap")) &&
                    !readTypeToClassMap) {
                    readTypeToClassMap = true;
                    index.typeToClassMapCount = countArrayValues(reader);
                }
                else {
                    reader.skipValue();
                }
            }
            reader.endObject();
        }
        catch (Exception error) {
            index.skippedInputs++;
            index.skippedReasons.add(input.description() + ":" + error.getMessage());
        }

        finalizeBehaviorContextEvidence(index);
        return index;
    }

    private BehaviorContextInput findBehaviorContextInput(File serializeInput) {
        String explicit = envValue("AZ_BEHAVIOR_CONTEXT_JSON");
        if (explicit != null) {
            File file = new File(explicit);
            return behaviorContextFileInput(file);
        }
        if (serializeInput == null) {
            return null;
        }
        File parent = serializeInput.getParentFile();
        if (parent == null) {
            return null;
        }
        File sibling = new File(parent, "behavior-context.json");
        if (sibling.exists()) {
            return behaviorContextFileInput(sibling);
        }
        File compressedSibling = new File(parent, "behavior-context.7z");
        return behaviorContextFileInput(compressedSibling);
    }

    private BehaviorContextInput behaviorContextFileInput(File file) {
        if (file == null || !file.exists() || !file.isFile()) {
            return null;
        }
        if (file.getName().toLowerCase(Locale.ROOT).endsWith(".7z")) {
            String entryName = behaviorContextArchiveEntry(file);
            if (entryName == null) {
                return null;
            }
            return new BehaviorContextInput(file, entryName);
        }
        return new BehaviorContextInput(file);
    }

    private String behaviorContextArchiveEntry(File archiveFile) {
        String firstJson = null;
        String firstFile = null;
        try (SevenZFile archive = new SevenZFile(archiveFile)) {
            SevenZArchiveEntry entry;
            while ((entry = archive.getNextEntry()) != null) {
                if (entry.isDirectory()) {
                    continue;
                }
                String entryName = entry.getName();
                if (entryName == null || entryName.isEmpty()) {
                    continue;
                }
                String normalized = entryName.replace('\\', '/').toLowerCase(Locale.ROOT);
                if (normalized.equals("behavior-context.json") ||
                    normalized.endsWith("/behavior-context.json")) {
                    return entryName;
                }
                if (firstJson == null && normalized.endsWith(".json")) {
                    firstJson = entryName;
                }
                if (firstFile == null) {
                    firstFile = entryName;
                }
            }
        }
        catch (Exception ignored) {
            return null;
        }
        return firstJson != null ? firstJson : firstFile;
    }

    private int countArrayValues(JsonReader reader) throws Exception {
        int count = 0;
        reader.beginArray();
        while (reader.hasNext()) {
            reader.skipValue();
            count++;
        }
        reader.endArray();
        return count;
    }

    private void scanBehaviorClasses(JsonReader reader, BehaviorContextEvidence index)
        throws Exception {

        reader.beginArray();
        while (reader.hasNext()) {
            JsonObject record = JsonParser.parseReader(reader).getAsJsonObject();
            scanBehaviorClass(record, index);
        }
        reader.endArray();
    }

    private void scanBehaviorMethods(JsonReader reader, BehaviorContextEvidence index)
        throws Exception {

        reader.beginArray();
        while (reader.hasNext()) {
            JsonObject method = behaviorMethod(JsonParser.parseReader(reader).getAsJsonObject());
            if (method == null) {
                index.skippedRecords++;
                continue;
            }
            index.globalMethodCount++;
            addBehaviorMethodCandidate(
                index,
                behaviorScope("AZ", "BehaviorContext"),
                "global.method",
                behaviorLocalName(method, "Method"),
                method);
        }
        reader.endArray();
    }

    private void scanBehaviorProperties(JsonReader reader, BehaviorContextEvidence index)
        throws Exception {

        reader.beginArray();
        while (reader.hasNext()) {
            JsonObject property =
                behaviorProperty(JsonParser.parseReader(reader).getAsJsonObject());
            if (property == null) {
                index.skippedRecords++;
                continue;
            }
            index.globalPropertyCount++;
            String propertyName = safeTypeName(behaviorName(property, "Property"));
            addBehaviorMethodCandidate(
                index,
                behaviorScope("AZ", "BehaviorContext"),
                "global.property.getter",
                behaviorAccessorName("Get", propertyName),
                objectMemberDirect(property, "m_getter"));
            addBehaviorMethodCandidate(
                index,
                behaviorScope("AZ", "BehaviorContext"),
                "global.property.setter",
                behaviorAccessorName("Set", propertyName),
                objectMemberDirect(property, "m_setter"));
        }
        reader.endArray();
    }

    private void scanBehaviorEbuses(JsonReader reader, BehaviorContextEvidence index)
        throws Exception {

        reader.beginArray();
        while (reader.hasNext()) {
            JsonObject bus = JsonParser.parseReader(reader).getAsJsonObject();
            scanBehaviorEbus(bus, index);
        }
        reader.endArray();
    }

    private void scanBehaviorClass(JsonObject klass, BehaviorContextEvidence index) {
        index.classCount++;
        String className = behaviorName(klass, null);
        String typeId = behaviorTypeId(klass);
        if (typeId != null) {
            String normalizedTypeId = normalizeTypeId(typeId);
            if (normalizedTypeId != null) {
                index.classTypeIds.add(normalizedTypeId);
            }
        }
        addBehaviorRttiType(index, klass, className, typeId);
        ArrayList<String> scope = behaviorQualifiedScope(className, "BehaviorClass");
        if (!scope.isEmpty()) {
            String localTypeName = scope.get(scope.size() - 1);
            increment(index.classNameCounts, fullName(scope.subList(0, scope.size() - 1),
                localTypeName));
        }

        addBehaviorClassFunction(index, scope, "class.allocate", "Allocate", klass, "m_allocate");
        addBehaviorClassFunction(
            index,
            scope,
            "class.deallocate",
            "Deallocate",
            klass,
            "m_deallocate");
        addBehaviorClassFunction(
            index,
            scope,
            "class.defaultConstructor",
            "DefaultConstructor",
            klass,
            "m_defaultConstructor");
        addBehaviorClassFunction(
            index,
            scope,
            "class.destructor",
            "Destructor",
            klass,
            "m_destructor");
        addBehaviorClassFunction(index, scope, "class.cloner", "Clone", klass, "m_cloner");
        addBehaviorClassFunction(index, scope, "class.mover", "Move", klass, "m_mover");
        addBehaviorClassFunction(
            index,
            scope,
            "class.equalityComparer",
            "Equals",
            klass,
            "m_equalityComparer");
        addBehaviorClassFunction(index, scope, "class.unwrapper", "Unwrap", klass, "m_unwrapper");
        addBehaviorClassFunction(
            index,
            scope,
            "class.valueHasher",
            "HashValue",
            klass,
            "m_valueHasher");

        JsonArray constructors = arrayMember(klass, "m_constructors");
        if (constructors != null) {
            for (JsonElement value : constructors) {
                JsonObject method = behaviorMethod(value);
                if (method == null) {
                    continue;
                }
                index.constructorCount++;
                addBehaviorMethodCandidate(index, scope, "class.constructor", "Constructor", method);
            }
        }

        JsonArray methods = arrayMember(klass, "m_methods");
        if (methods != null) {
            for (JsonElement value : methods) {
                JsonObject method = behaviorMethod(value);
                if (method == null) {
                    continue;
                }
                index.classMethodCount++;
                addBehaviorMethodCandidate(
                    index,
                    scope,
                    "class.method",
                    behaviorLocalName(method, "Method"),
                    method);
            }
        }

        JsonArray properties = arrayMember(klass, "m_properties");
        if (properties != null) {
            for (JsonElement value : properties) {
                JsonObject property = behaviorProperty(value);
                if (property == null) {
                    continue;
                }
                index.classPropertyCount++;
                String propertyName = safeTypeName(behaviorName(property, "Property"));
                addBehaviorMethodCandidate(
                    index,
                    scope,
                    "class.property.getter",
                    behaviorAccessorName("Get", propertyName),
                    objectMemberDirect(property, "m_getter"));
                addBehaviorMethodCandidate(
                    index,
                    scope,
                    "class.property.setter",
                    behaviorAccessorName("Set", propertyName),
                    objectMemberDirect(property, "m_setter"));
            }
        }

        JsonArray baseClasses = arrayMember(klass, "m_baseClasses");
        if (baseClasses != null) {
            index.classBaseEdgeCount += baseClasses.size();
        }
        JsonArray requestBuses = arrayMember(klass, "m_requestBuses");
        if (requestBuses != null) {
            index.classRequestBusEdgeCount += requestBuses.size();
        }
        JsonArray notificationBuses = arrayMember(klass, "m_notificationBuses");
        if (notificationBuses != null) {
            index.classNotificationBusEdgeCount += notificationBuses.size();
        }
    }

    private void addBehaviorRttiType(
        BehaviorContextEvidence index,
        JsonObject object,
        String typeName,
        String typeId) {

        String address = behaviorRttiAddress(object);
        String targetTypeName = serializedTypeName(typeName);
        if (!isAddressLike(address) || targetTypeName == null) {
            return;
        }

        String key = addressKey(address);
        if (index.rttiTypes.containsKey(key)) {
            return;
        }

        RttiType type = new RttiType();
        type.address = address;
        type.typeName = typeName;
        type.typeId = typeId;
        type.targetTypeName = targetTypeName;
        attachModuleOwner(type);
        index.rttiTypes.put(key, type);
    }

    private void scanBehaviorEbus(JsonObject bus, BehaviorContextEvidence index) {
        index.ebusCount++;
        String busName = behaviorName(bus, null);
        if (busName != null) {
            increment(index.busNameCounts, busName);
        }
        ArrayList<String> scope = behaviorQualifiedScope(busName, "BehaviorBus");
        addBehaviorClassFunction(
            index,
            scope,
            "ebus.createHandler",
            "CreateHandler",
            bus,
            "m_createHandler");
        addBehaviorClassFunction(
            index,
            scope,
            "ebus.destroyHandler",
            "DestroyHandler",
            bus,
            "m_destroyHandler");
        addBehaviorClassFunction(
            index,
            scope,
            "ebus.getCurrentId",
            "GetCurrentId",
            bus,
            "m_getCurrentId");
        addBehaviorClassFunction(
            index,
            scope,
            "ebus.queueFunction",
            "QueueFunction",
            bus,
            "m_queueFunction");
        addBehaviorClassFunction(
            index,
            scope,
            "ebus.handlerOnDemandReflector",
            "OnDemandReflect",
            bus,
            "m_ebusHandlerOnDemandReflector");

        JsonArray events = arrayMember(bus, "m_events");
        Map<String, String> eventNamesByAddress = new HashMap<>();
        if (events != null) {
            for (JsonElement value : events) {
                JsonObject event = behaviorEvent(value);
                if (event == null) {
                    continue;
                }
                index.ebusEventCount++;
                String eventName = safeTypeName(behaviorName(event, "Event"));
                rememberBehaviorEventName(eventNamesByAddress, event, eventName);
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.broadcast",
                    "Broadcast",
                    behaviorEventFunctionName(eventName),
                    objectMemberDirect(event, "m_broadcast"));
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.event",
                    "Event",
                    behaviorEventFunctionName(eventName),
                    objectMemberDirect(event, "m_event"));
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.queueBroadcast",
                    "QueueBroadcast",
                    behaviorEventFunctionName(eventName),
                    objectMemberDirect(event, "m_queueBroadcast"));
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.queueEvent",
                    "QueueEvent",
                    behaviorEventFunctionName(eventName),
                    objectMemberDirect(event, "m_queueEvent"));
            }
        }

        JsonArray virtualProperties = arrayMember(bus, "m_virtualProperties");
        if (virtualProperties != null) {
            for (JsonElement value : virtualProperties) {
                JsonObject property = behaviorEvent(value);
                if (property == null) {
                    continue;
                }
                index.ebusVirtualPropertyCount++;
                String propertyName = safeTypeName(behaviorName(property, "Property"));
                scanBehaviorVirtualProperty(
                    index,
                    scope,
                    busName,
                    propertyName,
                    eventNamesByAddress,
                    objectMemberDirect(property, "m_getter"),
                    "getter");
                scanBehaviorVirtualProperty(
                    index,
                    scope,
                    busName,
                    propertyName,
                    eventNamesByAddress,
                    objectMemberDirect(property, "m_setter"),
                    "setter");
            }
        }
    }

    private void scanBehaviorVirtualProperty(
        BehaviorContextEvidence index,
        ArrayList<String> scope,
        String busName,
        String propertyName,
        Map<String, String> eventNamesByAddress,
        JsonObject property,
        String side) {

        if (property == null) {
            return;
        }
        String localName = behaviorEventNameFor(property, eventNamesByAddress);
        if (localName == null) {
            index.virtualPropertyEventNameMissingCount++;
            index.skippedRecords++;
            index.skippedReasons.add(
                "BehaviorContext EBus virtual property event name is not present in bus event table: "
                    + reasonName(busName)
                    + "::"
                    + reasonName(propertyName)
                    + "."
                    + side);
            return;
        }
        index.virtualPropertyEventNameResolvedCount++;
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".broadcast",
            "Broadcast",
            localName,
            objectMemberDirect(property, "m_broadcast"));
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".event",
            "Event",
            localName,
            objectMemberDirect(property, "m_event"));
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".queueBroadcast",
            "QueueBroadcast",
            localName,
            objectMemberDirect(property, "m_queueBroadcast"));
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".queueEvent",
            "QueueEvent",
            localName,
            objectMemberDirect(property, "m_queueEvent"));
    }

    private void rememberBehaviorEventName(
        Map<String, String> eventNamesByAddress,
        JsonObject event,
        String eventName) {

        if (eventName == null) {
            return;
        }
        rememberBehaviorEventAddress(eventNamesByAddress, stringMemberDirect(event, "address"),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_broadcast")),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_event")),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_queueBroadcast")),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_queueEvent")),
            eventName);
    }

    private void rememberBehaviorEventAddress(
        Map<String, String> eventNamesByAddress,
        String address,
        String eventName) {

        if (address == null) {
            return;
        }
        eventNamesByAddress.put(addressKey(address), eventName);
    }

    private String behaviorEventNameFor(
        JsonObject event,
        Map<String, String> eventNamesByAddress) {

        String directName = safeTypeName(behaviorName(event, null));
        if (directName != null) {
            return directName;
        }

        String name = behaviorEventNameByAddress(
            eventNamesByAddress,
            stringMemberDirect(event, "address"));
        if (name != null) {
            return name;
        }
        name = behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_broadcast")));
        if (name != null) {
            return name;
        }
        name = behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_event")));
        if (name != null) {
            return name;
        }
        name = behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_queueBroadcast")));
        if (name != null) {
            return name;
        }
        return behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(objectMemberDirect(event, "m_queueEvent")));
    }

    private String behaviorEventNameByAddress(
        Map<String, String> eventNamesByAddress,
        String address) {

        if (address == null) {
            return null;
        }
        return eventNamesByAddress.get(addressKey(address));
    }

    private String behaviorMethodAddress(JsonObject method) {
        return method == null ? null : stringMemberDirect(method, "address");
    }

    private void addBehaviorClassFunction(
        BehaviorContextEvidence index,
        ArrayList<String> scope,
        String kind,
        String localName,
        JsonObject owner,
        String field) {

        JsonObject method = objectMemberDirect(owner, field);
        addBehaviorMethodCandidate(index, scope, kind, localName, method);
    }

    private void addBehaviorEventCandidate(
        BehaviorContextEvidence index,
        ArrayList<String> scope,
        String kind,
        String localName,
        JsonObject method) {

        addBehaviorMethodCandidate(index, scope, kind, localName, method);
        if (method != null && behaviorVirtualSlot(method) == null &&
            behaviorFunctionAddress(method) != null) {
            index.directEventFunctionCount++;
        }
        else if (method != null && behaviorVirtualSlot(method) != null) {
            index.virtualEventFunctionCount++;
        }
    }

    private void addBehaviorScopedEventCandidate(
        BehaviorContextEvidence index,
        ArrayList<String> scope,
        String kind,
        String eventScope,
        String localName,
        JsonObject method) {

        ArrayList<String> scoped = new ArrayList<>(scope);
        String safeScope = safeTypeName(eventScope);
        if (safeScope != null) {
            scoped.add(safeScope);
        }
        addBehaviorEventCandidate(index, scoped, kind, localName, method);
    }

    private void addBehaviorMethodCandidate(
        BehaviorContextEvidence index,
        ArrayList<String> scope,
        String kind,
        String localName,
        JsonObject method) {

        if (method == null) {
            return;
        }
        String address = behaviorFunctionAddress(method);
        if (!isAddressLike(address)) {
            return;
        }
        String safeLocalName = safeTypeName(localName);
        if (safeLocalName == null) {
            safeLocalName = "Function";
        }
        BehaviorFunctionCandidate candidate = new BehaviorFunctionCandidate();
        candidate.address = address;
        candidate.scope = new ArrayList<>(scope);
        candidate.localName = safeLocalName;
        candidate.kind = kind;
        candidate.slot = behaviorVirtualSlot(method);
        candidate.methodName = behaviorName(method, null);
        index.functionCandidates.add(candidate);
        increment(index.kindCounts, kind);

        String key = addressKey(address);
        BehaviorFunctionGroup group = index.groupsByAddress.get(key);
        if (group == null) {
            group = new BehaviorFunctionGroup();
            group.address = address;
            index.groupsByAddress.put(key, group);
        }
        group.candidates.add(candidate);
        if (candidate.slot != null) {
            group.slots.add(candidate.slot);
        }
        group.targetNames.add(fullName(candidate.scope, candidate.localName));
    }

    private void finalizeBehaviorContextEvidence(BehaviorContextEvidence index) {
        index.functionAddressCount = index.groupsByAddress.size();
        for (BehaviorFunctionGroup group : index.groupsByAddress.values()) {
            if (group.candidates.size() > 1) {
                index.duplicateFunctionAddressGroupCount++;
            }
            if (!group.slots.isEmpty()) {
                index.virtualDispatchGroupCount++;
            }
            else if (group.candidates.size() == 1 || group.targetNames.size() == 1) {
                index.safeDirectFunctionCount++;
            }
            else {
                index.sharedFunctionGroupCount++;
            }
        }
        for (Map.Entry<String, Integer> entry : index.busNameCounts.entrySet()) {
            if (entry.getValue() > 1) {
                index.duplicateBusNames.add(entry.getKey());
            }
        }
    }

    private void processBehaviorContextEvidence(
        BehaviorContextEvidence index,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (index == null || index.input == null) {
            return;
        }
        Map<String, SlotGroup> behaviorRttiSlotGroups =
            rttiSlotGroups(index.rttiTypes.values(), RTTI_SLOT_NAMES);
        for (RttiType type : index.rttiTypes.values()) {
            processRttiType(
                type,
                RTTI_SLOT_NAMES,
                behaviorRttiSlotGroups,
                functionSeen,
                aliasSeen,
                actions);
            if (monitor.isCancelled()) {
                return;
            }
        }

        for (BehaviorFunctionGroup group : index.groupsByAddress.values()) {
            if (!group.slots.isEmpty()) {
                processBehaviorVirtualDispatchGroup(group, functionSeen, aliasSeen, actions);
            }
            else if (group.candidates.size() == 1 || group.targetNames.size() == 1) {
                BehaviorFunctionCandidate candidate = group.candidates.get(0);
                renameBehaviorFunction(group, candidate, functionSeen, actions);
            }
            else {
                addBehaviorSharedFunction(group, actions);
            }
            if (monitor.isCancelled()) {
                return;
            }
        }
    }

    private void processBehaviorVirtualDispatchGroup(
        BehaviorFunctionGroup group,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (group.slots.size() != 1) {
            addBehaviorSharedFunction(group, actions);
            return;
        }
        String slot = group.slots.first();
        BehaviorFunctionCandidate candidate = new BehaviorFunctionCandidate();
        candidate.address = group.address;
        candidate.scope = behaviorScope("AZ", "BehaviorContext", "VirtualDispatchThunk");
        candidate.localName = "Slot" + safeSlotName(slot);
        candidate.kind = "behavior_virtual_dispatch_thunk";
        candidate.slot = slot;
        renameBehaviorFunction(group, candidate, functionSeen, actions);
        addBehaviorVirtualDispatchAliases(group, slot, aliasSeen, actions);
        addBehaviorVirtualDispatchUseComments(group, slot, aliasSeen, actions);
        addBehaviorComment(
            group.address,
            "BehaviorContext virtual dispatch " + slot + ": " +
                behaviorGroupExamples(group),
            "behavior-virtual-dispatch",
            actions);
    }

    private void addBehaviorVirtualDispatchAliases(
        BehaviorFunctionGroup group,
        String slot,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        for (BehaviorFunctionCandidate candidate : group.candidates) {
            if (candidate.scope == null || candidate.localName == null) {
                continue;
            }

            String targetName = fullName(candidate.scope, candidate.localName);
            String key = addressKey(candidate.address) + "|" + targetName;
            if (!aliasSeen.add(key)) {
                continue;
            }

            Address address = parseCaptureAddress(candidate.address);
            JsonObject action = new JsonObject();
            action.addProperty("kind", "behavior_virtual_dispatch_alias");
            action.addProperty("address", candidate.address);
            action.addProperty("name", targetName);
            action.addProperty("behaviorKind", candidate.kind);
            action.addProperty("virtualDispatchSlotOffset", slot);
            if (candidate.methodName != null) {
                action.addProperty("methodName", candidate.methodName);
            }
            if (!isProgramAddress(address)) {
                action.addProperty("applied", false);
                action.addProperty("reason", "address-not-in-program");
                actions.add(action);
                continue;
            }
            if (!reserveTargetName(targetName, candidate.address, action, actions)) {
                continue;
            }

            boolean applied = false;
            if (applyRenames) {
                applied = ensureLabel(address, candidate.scope, candidate.localName, action);
            }
            action.addProperty("applied", applied);
            if (!applyRenames) {
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
            actions.add(action);
        }
    }

    private void addBehaviorVirtualDispatchUseComments(
        BehaviorFunctionGroup group,
        String slot,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        Address dispatchAddress = parseCaptureAddress(group.address);
        if (!isProgramAddress(dispatchAddress)) {
            return;
        }

        LinkedHashSet<String> commentsSeen = new LinkedHashSet<>();
        for (BehaviorFunctionCandidate candidate : group.candidates) {
            if (candidate.localName == null || candidate.scope == null) {
                continue;
            }

            String targetName = fullName(candidate.scope, candidate.localName);
            String comment = "BehaviorContext use: " + targetName +
                " -> VirtualDispatchThunk::Slot" + safeSlotName(slot);
            ArrayList<Address> strings = behaviorNameStringAddresses(candidate);
            for (Address stringAddress : strings) {
                ReferenceIterator references =
                    currentProgram.getReferenceManager().getReferencesTo(stringAddress);
                while (references.hasNext()) {
                    monitor.checkCancelled();
                    Reference reference = references.next();
                    Address useAddress = reference.getFromAddress();
                    Function owner = currentProgram.getFunctionManager()
                        .getFunctionContaining(useAddress);
                    if (owner == null || !functionReferencesAddressNear(
                        owner,
                        useAddress,
                        dispatchAddress)) {
                        continue;
                    }

                    Address callsite = nextCallInFunction(owner, useAddress, 12);
                    if (callsite == null) {
                        continue;
                    }
                    String key = addressKey(formatAddress(callsite)) + "|" + comment;
                    if (!commentsSeen.add(key)) {
                        continue;
                    }
                    addBehaviorUseComment(
                        callsite,
                        group.address,
                        targetName,
                        slot,
                        comment,
                        actions);
                    Address callTarget = callTargetFromCallsite(callsite);
                    if (callTarget != null) {
                        addBehaviorEventBuilderUse(
                            callsite,
                            callTarget,
                            candidate,
                            targetName,
                            slot,
                            aliasSeen,
                            actions);
                    }
                }
            }
        }
    }

    private ArrayList<Address> behaviorNameStringAddresses(BehaviorFunctionCandidate candidate) {
        ArrayList<Address> result = new ArrayList<>();
        addStringAddresses(result, candidate.localName);
        addStringAddresses(result, candidate.methodName);
        return result;
    }

    private void addStringAddresses(ArrayList<Address> result, String text) {
        if (text == null || text.isEmpty()) {
            return;
        }
        ArrayList<Address> addresses = definedStringAddressesByValue().get(text);
        if (addresses == null) {
            return;
        }
        for (Address address : addresses) {
            if (!result.contains(address)) {
                result.add(address);
            }
        }
    }

    private boolean functionReferencesAddressNear(
        Function owner,
        Address center,
        Address target) {

        Instruction instruction =
            currentProgram.getListing().getInstructionContaining(center);
        if (instruction == null) {
            instruction = currentProgram.getListing().getInstructionAt(center);
        }
        if (instruction == null) {
            return false;
        }

        Instruction cursor = instruction;
        for (int i = 0; i < 48 && cursor != null &&
            owner.getBody().contains(cursor.getMinAddress()); i++) {
            if (instructionReferencesAddress(cursor, target)) {
                return true;
            }
            cursor = cursor.getPrevious();
        }

        cursor = instruction.getNext();
        for (int i = 0; i < 16 && cursor != null &&
            owner.getBody().contains(cursor.getMinAddress()); i++) {
            if (instructionReferencesAddress(cursor, target)) {
                return true;
            }
            cursor = cursor.getNext();
        }
        return false;
    }

    private boolean instructionReferencesAddress(Instruction instruction, Address target) {
        if (instruction == null || target == null) {
            return false;
        }
        for (Reference reference : instruction.getReferencesFrom()) {
            if (target.equals(reference.getToAddress())) {
                return true;
            }
        }
        Address[] flows = instruction.getFlows();
        if (flows != null) {
            for (Address flow : flows) {
                if (target.equals(flow)) {
                    return true;
                }
            }
        }
        return false;
    }

    private Address nextCallInFunction(Function owner, Address center, int maxInstructions) {
        Instruction cursor =
            currentProgram.getListing().getInstructionContaining(center);
        if (cursor == null) {
            cursor = currentProgram.getListing().getInstructionAt(center);
        }
        for (int i = 0; i < maxInstructions && cursor != null &&
            owner.getBody().contains(cursor.getMinAddress()); i++) {
            if (cursor.getFlowType().isCall()) {
                return cursor.getMinAddress();
            }
            cursor = cursor.getNext();
        }
        return null;
    }

    private Address callTargetFromCallsite(Address callsite) {
        Instruction instruction = currentProgram.getListing().getInstructionAt(callsite);
        if (instruction == null || !instruction.getFlowType().isCall()) {
            return null;
        }

        Address[] flows = instruction.getFlows();
        if (flows != null) {
            for (Address flow : flows) {
                if (isProgramAddress(flow)) {
                    return flow;
                }
            }
        }
        for (Reference reference : instruction.getReferencesFrom()) {
            if (reference.getReferenceType().isCall() &&
                isProgramAddress(reference.getToAddress())) {
                return reference.getToAddress();
            }
        }
        return null;
    }

    private void addBehaviorEventBuilderUse(
        Address callsite,
        Address callTarget,
        BehaviorFunctionCandidate candidate,
        String eventTargetName,
        String slot,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        ArrayList<String> builderScope = behaviorEventBuilderScope(candidate);
        String localName = behaviorEventBuilderLocalName(candidate);
        if (builderScope == null || localName == null) {
            return;
        }

        String jsonAddress = formatAddress(callTarget);
        String targetName = fullName(builderScope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (!aliasSeen.add(key)) {
            return;
        }

        JsonObject action = new JsonObject();
        action.addProperty("kind", "behavior_event_builder_use");
        action.addProperty("address", jsonAddress);
        action.addProperty("callsite", formatAddress(callsite));
        action.addProperty("name", targetName);
        action.addProperty("eventName", eventTargetName);
        action.addProperty("virtualDispatchSlotOffset", slot);

        Function function = currentProgram.getFunctionManager().getFunctionAt(callTarget);
        if (function == null && applyRenames) {
            function = createMissingFunction(callTarget, localName, action);
        }
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", applyRenames ? "function-create-failed" : "function-missing");
            actions.add(action);
            return;
        }

        int callReferenceCount = callReferenceCount(callTarget);
        action.addProperty("callReferenceCount", callReferenceCount);
        action.addProperty("oldName", function.getName(true));

        boolean applied = false;
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        String comment = "BehaviorContext builder use: " + eventTargetName +
            " via EBusBuilder::Event";
        CodeUnit codeUnit = currentProgram.getListing().getCodeUnitAt(function.getEntryPoint());
        if (applyRenames && codeUnit != null) {
            applied |= ensureCommentLine(codeUnit, CommentType.PLATE, comment, action);
        }

        if (applyRenames) {
            if (callReferenceCount == 1) {
                applied |= applyFunctionRename(function, builderScope, localName, action);
            }
            else {
                applied |= ensureLabel(callTarget, builderScope, localName, action);
                action.addProperty("reason", "shared-builder-call-target");
            }
        }

        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", callReferenceCount == 1
                ? "dry-run"
                : "shared-builder-call-target");
        }
        actions.add(action);
    }

    private ArrayList<String> behaviorEventBuilderScope(BehaviorFunctionCandidate candidate) {
        if (candidate.scope == null || candidate.scope.size() < 2) {
            return null;
        }
        String eventScope = candidate.scope.get(candidate.scope.size() - 1);
        if (!isBehaviorEventScope(eventScope)) {
            return null;
        }

        List<String> busScope = candidate.scope.subList(0, candidate.scope.size() - 1);
        if (busScope.isEmpty()) {
            return null;
        }
        String busName = busScope.get(busScope.size() - 1);
        ArrayList<String> scope =
            behaviorScope("AZ", "BehaviorContext", "EBusBuilder<" + busName + ">");
        scope.add(eventScope);
        return scope;
    }

    private String behaviorEventBuilderLocalName(BehaviorFunctionCandidate candidate) {
        if (candidate.localName == null) {
            return null;
        }
        return safeTypeName("Event<" + candidate.localName + ">");
    }

    private int callReferenceCount(Address target) {
        int count = 0;
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(target);
        while (references.hasNext()) {
            Reference reference = references.next();
            Instruction instruction =
                currentProgram.getListing().getInstructionAt(reference.getFromAddress());
            if (instruction != null && instruction.getFlowType().isCall()) {
                count++;
            }
        }
        return count;
    }

    private void addBehaviorUseComment(
        Address callsite,
        String dispatchAddress,
        String targetName,
        String slot,
        String comment,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "behavior_virtual_dispatch_use");
        action.addProperty("address", formatAddress(callsite));
        action.addProperty("dispatchAddress", dispatchAddress);
        action.addProperty("name", targetName);
        action.addProperty("virtualDispatchSlotOffset", slot);
        action.addProperty("comment", comment);

        CodeUnit codeUnit = currentProgram.getListing().getCodeUnitAt(callsite);
        if (codeUnit == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "codeunit-missing");
            actions.add(action);
            return;
        }

        boolean applied = false;
        if (applyRenames) {
            applied = ensureCommentLine(codeUnit, CommentType.PRE, comment, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private void renameBehaviorFunction(
        BehaviorFunctionGroup group,
        BehaviorFunctionCandidate candidate,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        String targetName = fullName(candidate.scope, candidate.localName);
        String functionKey = addressKey(candidate.address);
        if (functionSeen.contains(functionKey)) {
            return;
        }
        functionSeen.add(functionKey);

        Address address = parseCaptureAddress(candidate.address);
        JsonObject action = new JsonObject();
        action.addProperty("kind", candidate.kind);
        action.addProperty("address", candidate.address);
        action.addProperty("name", targetName);
        action.addProperty("behaviorUseCount", group.candidates.size());
        if (candidate.slot != null) {
            action.addProperty("virtualDispatchSlotOffset", candidate.slot);
        }
        if (actions.keepsFullActions()) {
            action.add("behaviorExamples", behaviorGroupExamplesJson(group));
        }
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, candidate.address, action, actions)) {
            return;
        }

        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        boolean created = false;
        if (function == null && applyRenames) {
            function = createMissingFunction(address, candidate.localName, action);
            created = function != null;
        }
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", applyRenames ? "function-create-failed" : "function-missing");
            actions.add(action);
            return;
        }

        action.addProperty("oldName", function.getName(true));
        action.addProperty("created", created);
        boolean applied = false;
        if (applyRenames && !function.getName(true).equals(targetName)) {
            applied = applyFunctionRename(function, candidate.scope, candidate.localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !function.getName(true).equals(targetName)) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private void addBehaviorSharedFunction(BehaviorFunctionGroup group, ActionSink actions) {
        JsonObject action = new JsonObject();
        action.addProperty("kind", "behavior_shared_function");
        action.addProperty("address", group.address);
        action.addProperty("applied", false);
        action.addProperty("reason", "shared-behavior-function");
        action.addProperty("behaviorUseCount", group.candidates.size());
        action.add("behaviorExamples", behaviorGroupExamplesJson(group));
        actions.add(action);
    }

    private void addBehaviorComment(
        String jsonAddress,
        String comment,
        String reason,
        ActionSink actions) throws Exception {

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = new JsonObject();
        action.addProperty("kind", "behavior_comment");
        action.addProperty("address", jsonAddress);
        action.addProperty("comment", comment);
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }

        boolean applied = false;
        if (applyRenames) {
            CodeUnit codeUnit = currentProgram.getListing().getCodeUnitAt(address);
            if (codeUnit == null) {
                Function function = currentProgram.getFunctionManager().getFunctionAt(address);
                if (function != null) {
                    codeUnit = currentProgram.getListing().getCodeUnitAt(function.getEntryPoint());
                }
            }
            if (codeUnit != null) {
                applied = ensureCommentLine(codeUnit, CommentType.PLATE, comment, action);
            }
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", reason);
        }
        actions.add(action);
    }

    private boolean ensureCommentLine(
        CodeUnit codeUnit,
        CommentType commentType,
        String comment,
        JsonObject action) {

        String existing = codeUnit.getComment(commentType);
        if (existing != null) {
            for (String line : existing.split("\\R")) {
                if (line.equals(comment)) {
                    action.addProperty("reason", "already-current");
                    return false;
                }
            }
            codeUnit.setComment(commentType, existing + "\n" + comment);
            return true;
        }

        codeUnit.setComment(commentType, comment);
        return true;
    }

    private JsonObject behaviorMethod(JsonElement value) {
        JsonObject object = resolveBehaviorObject(value);
        if (object == null) {
            return null;
        }
        JsonObject method = objectMemberDirect(object, "method");
        if (method != null) {
            return method;
        }
        JsonObject second = objectMemberDirect(object, "second");
        if (second != null && behaviorFunctionAddress(second) != null) {
            return second;
        }
        return object;
    }

    private JsonObject behaviorProperty(JsonElement value) {
        JsonObject object = resolveBehaviorObject(value);
        if (object == null) {
            return null;
        }
        JsonObject property = objectMemberDirect(object, "property");
        if (property != null) {
            return property;
        }
        JsonObject second = objectMemberDirect(object, "second");
        if (second != null && (
            objectMemberDirect(second, "m_getter") != null ||
                objectMemberDirect(second, "m_setter") != null)) {
            return second;
        }
        return object;
    }

    private JsonObject behaviorEvent(JsonElement value) {
        JsonObject object = resolveBehaviorObject(value);
        if (object == null) {
            return null;
        }
        JsonObject event = objectMemberDirect(object, "event");
        return event == null ? object : event;
    }

    private JsonObject resolveBehaviorObject(JsonElement value) {
        if (value == null || value.isJsonNull() || !value.isJsonObject()) {
            return null;
        }
        return value.getAsJsonObject();
    }

    private String behaviorName(JsonObject object, String fallback) {
        String name = stringMemberDirect(object, "m_name");
        if (name == null) {
            name = stringMemberDirect(object, "name");
        }
        if (name == null) {
            name = stringMemberDirect(object, "first");
        }
        if (name == null) {
            name = stringMemberDirect(object, "mapKey");
        }
        return name == null ? fallback : name;
    }

    private String behaviorTypeId(JsonObject object) {
        String typeId = stringMemberDirect(object, "m_typeId");
        if (typeId != null) {
            return typeId;
        }
        typeId = stringMemberDirect(object, "typeId");
        if (typeId != null) {
            return typeId;
        }
        JsonObject azRtti = objectMemberDirect(object, "m_azRtti");
        if (azRtti == null) {
            azRtti = objectMemberDirect(object, "azRtti");
        }
        return azRtti == null ? null : stringMemberDirect(azRtti, "typeId");
    }

    private String behaviorRttiAddress(JsonObject object) {
        String address = stringMemberDirect(object, "m_azRtti");
        if (address == null) {
            address = stringMemberDirect(object, "azRtti");
        }
        if (address != null) {
            return address;
        }

        JsonObject azRtti = objectMemberDirect(object, "m_azRtti");
        if (azRtti == null) {
            azRtti = objectMemberDirect(object, "azRtti");
        }
        return azRtti == null ? null : stringMemberDirect(azRtti, "address");
    }

    private String behaviorFunctionAddress(JsonObject method) {
        if (method == null) {
            return null;
        }
        String address = stringMemberDirect(method, "m_functionPtr");
        if (address == null) {
            address = stringMemberDirect(method, "implementationFunction");
        }
        return address;
    }

    private String behaviorVirtualSlot(JsonObject method) {
        if (method == null) {
            return null;
        }
        return stringMemberDirect(method, "virtualDispatchSlotOffset");
    }

    private ArrayList<String> behaviorQualifiedScope(String qualifiedName, String fallback) {
        ArrayList<String> scope = new ArrayList<>();
        String value = qualifiedName == null || qualifiedName.trim().isEmpty()
            ? fallback
            : qualifiedName;
        if (value == null || value.trim().isEmpty()) {
            scope.add("BehaviorContext");
            return scope;
        }
        for (String part : value.split("::")) {
            String safe = safeTypeName(part);
            if (safe != null) {
                scope.add(safe);
            }
        }
        if (scope.isEmpty()) {
            scope.add("BehaviorContext");
        }
        return scope;
    }

    private ArrayList<String> behaviorScope(String... parts) {
        ArrayList<String> scope = new ArrayList<>();
        for (String part : parts) {
            String safe = safeTypeName(part);
            if (safe != null) {
                scope.add(safe);
            }
        }
        return scope;
    }

    private String behaviorLocalName(JsonObject method, String fallback) {
        String name = behaviorName(method, fallback);
        if (name != null) {
            int index = name.lastIndexOf("::");
            if (index >= 0 && index + 2 < name.length()) {
                name = name.substring(index + 2);
            }
        }
        String safe = safeTypeName(name);
        return safe == null ? fallback : safe;
    }

    private String behaviorAccessorName(String prefix, String propertyName) {
        String safeProperty = safeTypeName(propertyName);
        if (safeProperty == null) {
            safeProperty = "Property";
        }
        return prefix + "_" + safeProperty;
    }

    private String behaviorEventFunctionName(String eventName) {
        String safeEvent = safeTypeName(eventName);
        if (safeEvent == null) {
            safeEvent = "Event";
        }
        return safeEvent;
    }

    private String safeSlotName(String slot) {
        if (slot == null) {
            return "Unknown";
        }
        String safe = safeTypeName(slot);
        return safe == null ? "Unknown" : safe;
    }

    private String behaviorGroupExamples(BehaviorFunctionGroup group) {
        StringBuilder builder = new StringBuilder();
        int count = 0;
        for (BehaviorFunctionCandidate candidate : group.candidates) {
            if (count >= MAX_SUMMARY_ACTION_EXAMPLES) {
                builder.append("; ...");
                break;
            }
            if (builder.length() > 0) {
                builder.append("; ");
            }
            builder.append(fullName(candidate.scope, candidate.localName));
            if (candidate.slot != null) {
                builder.append(" slot=").append(candidate.slot);
            }
            count++;
        }
        return builder.toString();
    }

    private JsonArray behaviorGroupExamplesJson(BehaviorFunctionGroup group) {
        JsonArray examples = new JsonArray();
        int count = 0;
        for (BehaviorFunctionCandidate candidate : group.candidates) {
            if (count >= MAX_SUMMARY_ACTION_EXAMPLES) {
                break;
            }
            JsonObject example = new JsonObject();
            example.addProperty("name", fullName(candidate.scope, candidate.localName));
            example.addProperty("kind", candidate.kind);
            if (candidate.slot != null) {
                example.addProperty("virtualDispatchSlotOffset", candidate.slot);
            }
            if (candidate.methodName != null) {
                example.addProperty("methodName", candidate.methodName);
            }
            examples.add(example);
            count++;
        }
        return examples;
    }

    private JsonObject behaviorContextEvidenceReport(BehaviorContextEvidence index) {
        JsonObject report = new JsonObject();
        if (index == null) {
            report.addProperty("input", (String)null);
            return report;
        }
        report.addProperty("input", index.input);
        report.addProperty("inputFormat", index.inputFormat);
        report.addProperty("archiveEntry", index.archiveEntry);
        report.addProperty("skippedInputs", index.skippedInputs);
        report.addProperty("skippedRecords", index.skippedRecords);
        report.addProperty("classes", index.classCount);
        report.addProperty("rttiHelpers", index.rttiTypes.size());
        report.addProperty("globalMethods", index.globalMethodCount);
        report.addProperty("globalProperties", index.globalPropertyCount);
        report.addProperty("ebuses", index.ebusCount);
        report.addProperty("typeToClassMap", index.typeToClassMapCount);
        report.addProperty("classMethods", index.classMethodCount);
        report.addProperty("classProperties", index.classPropertyCount);
        report.addProperty("constructors", index.constructorCount);
        report.addProperty("classBaseEdges", index.classBaseEdgeCount);
        report.addProperty("classRequestBusEdges", index.classRequestBusEdgeCount);
        report.addProperty(
            "classNotificationBusEdges",
            index.classNotificationBusEdgeCount);
        report.addProperty("ebusEvents", index.ebusEventCount);
        report.addProperty("ebusVirtualProperties", index.ebusVirtualPropertyCount);
        report.addProperty(
            "ebusVirtualPropertyEventNamesResolved",
            index.virtualPropertyEventNameResolvedCount);
        report.addProperty(
            "ebusVirtualPropertyEventNamesMissing",
            index.virtualPropertyEventNameMissingCount);
        report.addProperty("functionCandidates", index.functionCandidates.size());
        report.addProperty("functionAddresses", index.functionAddressCount);
        report.addProperty(
            "duplicateFunctionAddressGroups",
            index.duplicateFunctionAddressGroupCount);
        report.addProperty("safeDirectFunctionGroups", index.safeDirectFunctionCount);
        report.addProperty("sharedFunctionGroups", index.sharedFunctionGroupCount);
        report.addProperty("virtualDispatchGroups", index.virtualDispatchGroupCount);
        report.addProperty("directEventFunctions", index.directEventFunctionCount);
        report.addProperty("virtualEventFunctions", index.virtualEventFunctionCount);
        report.addProperty("behaviorClassTypeIds", index.classTypeIds.size());
        report.add("kindCounts", mapToJson(index.kindCounts));
        JsonArray duplicateBusNames = new JsonArray();
        for (String name : index.duplicateBusNames) {
            duplicateBusNames.add(name);
        }
        report.add("duplicateBusNames", duplicateBusNames);
        JsonArray skipped = new JsonArray();
        for (String reason : index.skippedReasons) {
            skipped.add(reason);
        }
        report.add("skippedReasons", skipped);
        return report;
    }

    private JsonObject typeEvidenceReport(TypeEvidenceIndex index) {
        JsonObject report = new JsonObject();
        if (index == null) {
            report.addProperty("classBodyCount", 0);
            report.addProperty("usageCount", 0);
            report.addProperty("staticOwnerCount", 0);
            report.addProperty("fieldOwnerExactKeyCount", 0);
            report.addProperty("fieldOwnerNameTypeKeyCount", 0);
            report.addProperty("fieldOwnerTypeOffsetKeyCount", 0);
            report.addProperty("ambiguousFieldOwnerExactKeyCount", 0);
            report.addProperty("ambiguousFieldOwnerNameTypeKeyCount", 0);
            report.addProperty("ambiguousFieldOwnerTypeOffsetKeyCount", 0);
            report.add("usageKinds", new JsonObject());
            report.add("staticOwnerScopes", new JsonObject());
            return report;
        }
        report.addProperty("classBodyCount", index.classBodyCount);
        report.addProperty("usageCount", index.usageCount);
        report.addProperty("staticOwnerCount", index.staticOwnerCount);
        report.addProperty("fieldOwnerExactKeyCount", index.fieldOwnerExactKeyCount);
        report.addProperty("fieldOwnerNameTypeKeyCount", index.fieldOwnerNameTypeKeyCount);
        report.addProperty("fieldOwnerTypeOffsetKeyCount", index.fieldOwnerTypeOffsetKeyCount);
        report.addProperty(
            "ambiguousFieldOwnerExactKeyCount",
            index.ambiguousFieldOwnerExactKeyCount);
        report.addProperty(
            "ambiguousFieldOwnerNameTypeKeyCount",
            index.ambiguousFieldOwnerNameTypeKeyCount);
        report.addProperty(
            "ambiguousFieldOwnerTypeOffsetKeyCount",
            index.ambiguousFieldOwnerTypeOffsetKeyCount);
        report.add("usageKinds", mapToJson(index.usageKindCounts));
        JsonObject staticOwnerScopes = new JsonObject();
        for (Map.Entry<String, Integer> entry : index.staticOwnerScopeCounts.entrySet()) {
            staticOwnerScopes.addProperty(entry.getKey(), entry.getValue());
        }
        report.add("staticOwnerScopes", staticOwnerScopes);
        return report;
    }

    private String buildSummary(ScanResult scan, ActionSink actions, ActionStats stats) {
        int duplicateCount = countReason(stats, "duplicate-target-name");
        int addressOutsideProgramCount = countReason(stats, "address-not-in-program");
        int functionMissingCount = countReason(stats, "function-missing");
        int functionCreateFailedCount = countReason(stats, "function-create-failed");
        int dryRunCount = countReason(stats, "dry-run");

        StringBuilder summary = new StringBuilder();
        summary.append("AZ SerializeContext rename ");
        summary.append(applyRenames ? "applied" : "dry run");
        summary.append("\n\nInput:\n  ");
        summary.append(inputFile.getAbsolutePath());
        summary.append("\n\nOutput:\n  ");
        summary.append(outputFile.getAbsolutePath());

        summary.append("\n\nScanned:");
        summary.append("\n  RTTI helpers: ").append(scan.rttiTypes.size());
        summary.append("\n  Class records: ").append(scan.classData.size());
        summary.append("\n  Element callbacks: ").append(elementCallbackCount(scan.classData));
        summary.append("\n  Ambiguous source type names: ").append(scan.collidingTypeNames.size());
        summary.append("\n  Unresolved type-name collisions: ")
            .append(scan.unresolvedCollidingTypeNames.size());
        appendCollisionExamples(summary, scan.collidingTypeIdsByName);

        summary.append("\n\nModule evidence:");
        if (moduleEvidence.input == null) {
            summary.append("\n  No module descriptor captures found.");
        }
        else {
            summary.append("\n  Input: ").append(moduleEvidence.input);
            summary.append("\n  Files: ").append(moduleEvidence.inputCount);
            summary.append("\n  Descriptors: ").append(moduleEvidence.descriptorCount);
            summary.append("\n  Owner type IDs: ").append(moduleEvidence.ownersByTypeId.size());
            summary.append("\n  Skipped inputs: ").append(moduleEvidence.skippedInputs);
            summary.append("\n  Duplicate type IDs: ").append(moduleEvidence.duplicateTypeIds);
        }

        summary.append("\n\nClass/field registration traces:");
        if (classRegistrationEvidence == null || classRegistrationEvidence.input == null) {
            summary.append("\n  No Class<T> trace found.");
        }
        else {
            summary.append("\n  Class<T> records: ")
                .append(classRegistrationEvidence.recordCount);
        }
        if (fieldRegistrationEvidence == null || fieldRegistrationEvidence.input == null) {
            summary.append("\n  No Field<T> trace found.");
        }
        else {
            summary.append("\n  Field<T> records: ")
                .append(fieldRegistrationEvidence.recordCount);
            summary.append("\n  Field<T> owners resolved from SerializeContext graph: ")
                .append(fieldRegistrationEvidence.recordsWithGraphOwner);
        }

        summary.append("\n\nType evidence:");
        if (typeEvidence == null) {
            summary.append("\n  No SerializeContext graph evidence built.");
        }
        else {
            summary.append("\n  Class bodies: ").append(typeEvidence.classBodyCount);
            summary.append("\n  Graph usages: ").append(typeEvidence.usageCount);
            summary.append("\n  Static owners: ").append(typeEvidence.staticOwnerCount);
            summary.append("\n  Field owner exact keys: ")
                .append(typeEvidence.fieldOwnerExactKeyCount);
            summary.append("\n  Field owner name/type keys: ")
                .append(typeEvidence.fieldOwnerNameTypeKeyCount);
            summary.append("\n  Field owner type/offset keys: ")
                .append(typeEvidence.fieldOwnerTypeOffsetKeyCount);
        }

        summary.append("\n\nBehaviorContext evidence:");
        if (behaviorContextEvidence == null || behaviorContextEvidence.input == null) {
            summary.append("\n  No BehaviorContext capture found.");
        }
        else {
            summary.append("\n  Input: ").append(behaviorContextEvidence.input);
            summary.append("\n  Input format: ").append(behaviorContextEvidence.inputFormat);
            if (behaviorContextEvidence.archiveEntry != null) {
                summary.append("\n  Archive entry: ").append(behaviorContextEvidence.archiveEntry);
            }
            summary.append("\n  Classes: ").append(behaviorContextEvidence.classCount);
            summary.append("\n  RTTI helpers: ").append(behaviorContextEvidence.rttiTypes.size());
            summary.append("\n  Global properties: ")
                .append(behaviorContextEvidence.globalPropertyCount);
            summary.append("\n  EBus records: ").append(behaviorContextEvidence.ebusCount);
            summary.append("\n  EBus events: ").append(behaviorContextEvidence.ebusEventCount);
            summary.append("\n  EBus virtual property names resolved/missing: ")
                .append(behaviorContextEvidence.virtualPropertyEventNameResolvedCount)
                .append("/")
                .append(behaviorContextEvidence.virtualPropertyEventNameMissingCount);
            summary.append("\n  Function candidates: ")
                .append(behaviorContextEvidence.functionCandidates.size());
            summary.append("\n  Unique function addresses: ")
                .append(behaviorContextEvidence.functionAddressCount);
            summary.append("\n  Safe direct groups: ")
                .append(behaviorContextEvidence.safeDirectFunctionCount);
            summary.append("\n  Shared groups skipped: ")
                .append(behaviorContextEvidence.sharedFunctionGroupCount);
            summary.append("\n  Virtual dispatch groups: ")
                .append(behaviorContextEvidence.virtualDispatchGroupCount);
            summary.append("\n  Duplicate bus names: ")
                .append(behaviorContextEvidence.duplicateBusNames.size());
        }

        summary.append("\n\nActions:");
        summary.append("\n  Total: ").append(actions.size());
        summary.append("\n  Would apply: ").append(stats.wouldApplyCount);
        summary.append("\n  Applied: ").append(stats.appliedCount);
        summary.append("\n  Dry-run ready: ").append(dryRunCount);
        summary.append("\n  Function missing in dry run: ").append(functionMissingCount);
        summary.append("\n  Function create failed in apply: ").append(functionCreateFailedCount);
        summary.append("\n  Duplicate target names skipped: ").append(duplicateCount);
        summary.append("\n  Outside loaded program skipped: ").append(addressOutsideProgramCount);

        summary.append("\n\nBy kind:");
        appendCount(summary, stats.kindCounts, "function");
        appendCount(summary, stats.kindCounts, "function_alias");
        appendCount(summary, stats.kindCounts, "classdata_object");
        appendCount(summary, stats.kindCounts, "classdata_object_vftable");
        appendCount(summary, stats.kindCounts, "rtti_helper");
        appendCount(summary, stats.kindCounts, "rtti_vftable");
        appendCount(summary, stats.kindCounts, "core_reflection_datatype");
        appendCount(summary, stats.kindCounts, "datatype_structure");
        appendCount(summary, stats.kindCounts, "this_parameter");
        appendCount(summary, stats.kindCounts, "reflect_context_parameter");
        appendCount(summary, stats.kindCounts, "descriptor_reflect_parameter");
        appendCount(summary, stats.kindCounts, "module_descriptor_reflect_thunk_target");
        appendCount(summary, stats.kindCounts, "behavior_virtual_dispatch_thunk");
        appendCount(summary, stats.kindCounts, "behavior_virtual_dispatch_alias");
        appendCount(summary, stats.kindCounts, "behavior_virtual_dispatch_use");
        appendCount(summary, stats.kindCounts, "behavior_event_builder_use");
        appendCount(summary, stats.kindCounts, "behavior_shared_function");
        appendCount(summary, stats.kindCounts, "behavior_comment");

        appendActionExamples(summary, actions, "dry-run", "Ready rename examples");
        appendActionExamples(summary, actions, "function-missing", "Functions that apply mode can create");
        appendActionExamples(summary, actions, "function-create-failed", "Functions apply mode could not create");
        appendActionExamples(summary, actions, "duplicate-target-name", "Duplicate target names skipped");
        appendActionExamples(summary, actions, "address-not-in-program", "Outside-program addresses skipped");
        appendActionExamples(
            summary,
            actions,
            "shared-behavior-function",
            "Shared BehaviorContext helpers skipped");
        appendActionExamples(
            summary,
            actions,
            "behavior-virtual-dispatch",
            "BehaviorContext virtual dispatch comments");

        summary.append("\n\nRead:");
        if (!applyRenames) {
            summary.append("\n  This did not mutate the program.");
            summary.append("\n  Apply is OK for non-conflicting actions.");
            summary.append("\n  duplicate-target-name entries will be skipped, not renamed.");
            summary.append("\n  ambiguous source type names are reported with their UUID sets.");
            summary.append("\n  function-missing entries are expected in dry run; apply mode tries to create them first.");
        }
        else {
            summary.append("\n  Non-conflicting actions were applied.");
            summary.append("\n  duplicate-target-name and address-not-in-program entries were skipped.");
            summary.append("\n  function-create-failed entries include clear/disassemble diagnostics.");
            summary.append("\n  ambiguous source type names were not given synthetic suffixes.");
        }
        return summary.toString();
    }

    private void appendCollisionExamples(
        StringBuilder summary,
        LinkedHashMap<String, LinkedHashSet<String>> collisions) {

        if (collisions.isEmpty()) {
            return;
        }
        summary.append("\n  First ambiguous names:");
        int count = 0;
        for (Map.Entry<String, LinkedHashSet<String>> entry : collisions.entrySet()) {
            if (count >= 8) {
                int remaining = collisions.size() - count;
                if (remaining > 0) {
                    summary.append("\n    ... ").append(remaining).append(" more");
                }
                break;
            }
            summary.append("\n    ");
            summary.append(entry.getKey());
            summary.append(" -> ");
            boolean first = true;
            for (String typeId : entry.getValue()) {
                if (!first) {
                    summary.append(", ");
                }
                summary.append(typeId);
                first = false;
            }
            count++;
        }
    }

    private void appendCount(StringBuilder summary, Map<String, Integer> counts, String name) {
        Integer count = counts.get(name);
        if (count == null) {
            count = 0;
        }
        summary.append("\n  ").append(name).append(": ").append(count);
    }

    private int countReason(ActionStats stats, String reason) {
        Integer count = stats.reasonCounts.get(reason);
        return count == null ? 0 : count;
    }

    private ActionStats actionStats(JsonArray actions) {
        ActionStats result = new ActionStats();
        for (JsonElement value : actions) {
            if (value == null || value.isJsonNull() || !value.isJsonObject()) {
                continue;
            }
            JsonObject action = value.getAsJsonObject();
            increment(result.kindCounts, stringMember(action, "kind"));
            increment(result.reasonCounts, stringMember(action, "reason"));
            if (boolMember(action, "wouldApply") == Boolean.TRUE) {
                result.wouldApplyCount++;
            }
            if (boolMember(action, "applied") == Boolean.TRUE) {
                result.appliedCount++;
            }
        }
        return result;
    }

    private void increment(Map<String, Integer> counts, String name) {
        if (name == null) {
            name = "none";
        }
        Integer count = counts.get(name);
        counts.put(name, count == null ? 1 : count + 1);
    }

    private JsonObject mapToJson(Map<String, Integer> counts) {
        JsonObject result = new JsonObject();
        for (Map.Entry<String, Integer> entry : counts.entrySet()) {
            result.addProperty(entry.getKey(), entry.getValue());
        }
        return result;
    }

    private JsonArray stringArray(Set<String> values) {
        JsonArray result = new JsonArray();
        if (values == null) {
            return result;
        }
        for (String value : values) {
            result.add(value);
        }
        return result;
    }

    private JsonArray listStringArray(List<String> values) {
        JsonArray result = new JsonArray();
        if (values == null) {
            return result;
        }
        for (String value : values) {
            result.add(value);
        }
        return result;
    }

    private JsonObject actionSamples(JsonArray actions) {
        LinkedHashMap<String, JsonArray> byReason = new LinkedHashMap<>();
        for (JsonElement value : actions) {
            if (value == null || value.isJsonNull() || !value.isJsonObject()) {
                continue;
            }
            JsonObject action = value.getAsJsonObject();
            String reason = stringMember(action, "reason");
            if (reason == null) {
                reason = "none";
            }
            JsonArray samples = byReason.get(reason);
            if (samples == null) {
                samples = new JsonArray();
                byReason.put(reason, samples);
            }
            if (samples.size() < MAX_SUMMARY_ACTION_EXAMPLES) {
                samples.add(actionSample(action));
            }
        }

        JsonObject result = new JsonObject();
        for (Map.Entry<String, JsonArray> entry : byReason.entrySet()) {
            result.add(entry.getKey(), entry.getValue());
        }
        return result;
    }

    private JsonObject actionSample(JsonObject action) {
        JsonObject sample = new JsonObject();
        copyActionProperty(action, sample, "kind");
        copyActionProperty(action, sample, "address");
        copyActionProperty(action, sample, "name");
        copyActionProperty(action, sample, "reason");
        copyActionProperty(action, sample, "existingAddress");
        copyActionProperty(action, sample, "oldName");
        copyActionProperty(action, sample, "created");
        copyActionProperty(action, sample, "shared");
        copyActionProperty(action, sample, "useCount");
        copyActionProperty(action, sample, "source");
        copyActionProperty(action, sample, "module");
        copyActionProperty(action, sample, "componentName");
        copyActionProperty(action, sample, "componentUuid");
        copyActionProperty(action, sample, "createComponent");
        copyActionProperty(action, sample, "constructor");
        copyActionProperty(action, sample, "sourceInstruction");
        copyActionProperty(action, sample, "vptrOffset");
        copyActionProperty(action, sample, "returnAddress");
        copyActionProperty(action, sample, "helperReturnAddress");
        copyActionProperty(action, sample, "ownerTypeId");
        copyActionProperty(action, sample, "ownerTypeName");
        copyActionProperty(action, sample, "ownerSource");
        copyActionProperty(action, sample, "ownerResolution");
        copyActionProperty(action, sample, "ownerFunctionAddress");
        copyActionProperty(action, sample, "fieldName");
        copyActionProperty(action, sample, "fieldNameSource");
        copyActionProperty(action, sample, "fieldTypeId");
        copyActionProperty(action, sample, "fieldTypeName");
        copyActionProperty(action, sample, "fieldTypeNameSource");
        copyActionProperty(action, sample, "fieldOffset");
        copyActionProperty(action, sample, "dataSize");
        copyActionProperty(action, sample, "datatypePath");
        copyActionProperty(action, sample, "layoutSize");
        copyActionProperty(action, sample, "fieldCount");
        copyActionProperty(action, sample, "fieldsWritten");
        copyActionProperty(action, sample, "fieldFailures");
        copyActionProperty(action, sample, "function");
        copyActionProperty(action, sample, "behaviorUseCount");
        copyActionProperty(action, sample, "virtualDispatchSlotOffset");
        copyActionProperty(action, sample, "behaviorExamples");
        copyActionProperty(action, sample, "comment");
        copyActionProperty(action, sample, "wouldApply");
        copyActionProperty(action, sample, "applied");
        copyActionProperty(action, sample, "slotNames");
        return sample;
    }

    private void copyActionProperty(JsonObject source, JsonObject target, String name) {
        JsonElement value = source.get(name);
        if (value != null && !value.isJsonNull()) {
            target.add(name, value);
        }
    }

    private void appendActionExamples(
        StringBuilder summary,
        ActionSink actions,
        String reason,
        String title) {

        int count = 0;
        JsonArray samples = actions.samplesForReason(reason);
        if (samples == null) {
            return;
        }
        for (JsonElement value : samples) {
            JsonObject action = value.getAsJsonObject();
            if (count == 0) {
                summary.append("\n\n").append(title).append(":");
            }
            if (count >= MAX_SUMMARY_ACTION_EXAMPLES) {
                summary.append("\n  ... more in JSON report");
                return;
            }
            summary.append("\n  ").append(actionSummaryLine(action));
            count++;
        }
    }

    private String actionSummaryLine(JsonObject action) {
        StringBuilder line = new StringBuilder();
        appendActionPart(line, stringMember(action, "kind"));
        appendActionPart(line, stringMember(action, "address"));
        appendActionPart(line, stringMember(action, "name"));

        String existingAddress = stringMember(action, "existingAddress");
        if (existingAddress != null) {
            appendActionPart(line, "existing=" + existingAddress);
        }
        Boolean wouldApply = boolMember(action, "wouldApply");
        if (wouldApply != null) {
            appendActionPart(line, "wouldApply=" + wouldApply);
        }
        Boolean applied = boolMember(action, "applied");
        if (applied != null) {
            appendActionPart(line, "applied=" + applied);
        }
        JsonElement slots = action.get("slotNames");
        if (slots != null && slots.isJsonArray() && slots.getAsJsonArray().size() > 0) {
            appendActionPart(line, "slots=" + compactJson(slots));
        }
        return line.toString();
    }

    private void appendActionPart(StringBuilder line, String value) {
        if (value == null || value.isEmpty()) {
            return;
        }
        if (line.length() > 0) {
            line.append(" | ");
        }
        line.append(value);
    }

    private String compactJson(JsonElement value) {
        return value == null || value.isJsonNull() ? "null" : value.toString();
    }

    private JsonObject stringSetMapToJson(Map<String, LinkedHashSet<String>> values) {
        JsonObject result = new JsonObject();
        for (Map.Entry<String, LinkedHashSet<String>> entry : values.entrySet()) {
            JsonArray typeIds = new JsonArray();
            for (String typeId : entry.getValue()) {
                typeIds.add(typeId);
            }
            result.add(entry.getKey(), typeIds);
        }
        return result;
    }

    private String envValue(String name) {
        String value = System.getenv(name);
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        return value;
    }

    private boolean envBool(String value) {
        String v = value.toLowerCase();
        return v.equals("1") || v.equals("true") || v.equals("yes") ||
            v.equals("y") || v.equals("on");
    }

    private ScanResult scanSerializeContext(JsonElement root) {
        LinkedHashMap<String, RttiType> rttiTypes = new LinkedHashMap<>();
        ArrayList<ClassData> classData = new ArrayList<>();
        LinkedHashMap<String, LinkedHashSet<String>> typeIdsByName = new LinkedHashMap<>();
        ArrayList<JsonObject> objects = collectObjects(root);
        LinkedHashMap<String, JsonObject> objectsById = indexObjectsById(objects);
        jsonObjectsById = objectsById;
        LinkedHashMap<String, JsonObject> genericInfoByTypeId =
            indexGenericInfo(root, objects, objectsById);
        LinkedHashMap<String, String> rawTypeNamesByTypeId =
            indexRawTypeNames(objects);

        for (JsonObject object : objects) {
            ClassData classRecord =
                classDataFrom(object, genericInfoByTypeId, rawTypeNamesByTypeId);
            if (classRecord != null) {
                classData.add(classRecord);
                rememberTypeName(typeIdsByName, classRecord.typeName, classRecord.typeId);
            }
            collectRttiType(
                object,
                classRecord,
                rttiTypes,
                typeIdsByName,
                genericInfoByTypeId,
                rawTypeNamesByTypeId);
        }

        LinkedHashSet<String> collidingTypeNames = new LinkedHashSet<>();
        LinkedHashMap<String, LinkedHashSet<String>> collidingTypeIdsByName = new LinkedHashMap<>();
        for (Map.Entry<String, LinkedHashSet<String>> entry : typeIdsByName.entrySet()) {
            if (entry.getValue().size() > 1) {
                collidingTypeNames.add(entry.getKey());
                collidingTypeIdsByName.put(entry.getKey(), entry.getValue());
            }
        }

        typeEvidence = buildTypeEvidence(classData);
        for (RttiType type : rttiTypes.values()) {
            type.targetTypeName = serializedTypeName(type.typeName);
            attachModuleOwner(type);
        }
        for (ClassData data : classData) {
            data.targetTypeName = serializedTypeName(data.typeName);
            attachModuleOwner(data);
        }
        attachStaticOwners(rttiTypes.values(), classData);

        CollisionReport unresolvedCollisions =
            unresolvedTypeNameCollisions(rttiTypes.values(), classData);

        ScanResult result = new ScanResult();
        result.rttiTypes = rttiTypes;
        result.classData = classData;
        result.collidingTypeNames = collidingTypeNames;
        result.collidingTypeIdsByName = collidingTypeIdsByName;
        result.unresolvedCollidingTypeNames = unresolvedCollisions.names;
        result.unresolvedCollidingTypeIdsByName = unresolvedCollisions.typeIdsByName;
        result.typeEvidence = typeEvidence;
        return result;
    }

    private void attachModuleOwner(RttiType type) {
        ModuleOwner owner = moduleOwner(type.typeId);
        if (owner == null || isAggregateModuleName(owner.moduleName)) {
            return;
        }
        type.ownerScope = ownerScope(owner.moduleName);
        type.ownerReason = "module-descriptor";
        type.ownerComponentName = owner.componentName;
        type.ownerSource = owner.source;
    }

    private void attachModuleOwner(ClassData data) {
        ModuleOwner owner = moduleOwner(data.typeId);
        if (owner == null || isAggregateModuleName(owner.moduleName)) {
            return;
        }
        data.ownerScope = ownerScope(owner.moduleName);
        data.ownerReason = "module-descriptor";
        data.ownerComponentName = owner.componentName;
        data.ownerSource = owner.source;
    }

    private ArrayList<String> ownerScope(String owner) {
        ArrayList<String> scope = new ArrayList<>();
        if (owner != null && !owner.trim().isEmpty()) {
            scope.add(owner);
        }
        return scope;
    }

    private ModuleOwner moduleOwner(String typeId) {
        if (moduleEvidence == null) {
            return null;
        }
        String normalizedTypeId = normalizeTypeId(typeId);
        if (normalizedTypeId == null) {
            return null;
        }
        if (moduleEvidence.duplicateModulesByTypeId.containsKey(normalizedTypeId)) {
            return null;
        }
        return moduleEvidence.ownersByTypeId.get(normalizedTypeId);
    }

    private TypeEvidenceIndex buildTypeEvidence(List<ClassData> classData) {
        TypeEvidenceIndex evidence = new TypeEvidenceIndex();
        for (ClassData data : classData) {
            String parentTypeId = normalizeTypeId(data.typeId);
            if (parentTypeId == null) {
                continue;
            }
            evidence.classBodiesByTypeId.put(
                parentTypeId,
                preferredClassData(evidence.classBodiesByTypeId.get(parentTypeId), data));
            evidence.classBodyCount++;
        }

        for (ClassData data : classData) {
            String parentTypeId = normalizeTypeId(data.typeId);
            if (parentTypeId == null) {
                continue;
            }
            for (ElementData element : data.elements) {
                String targetTypeId = normalizeTypeId(element.typeId);
                if (targetTypeId != null) {
                    TypeUsage usage = new TypeUsage();
                    usage.ownerTypeId = parentTypeId;
                    usage.ownerTypeName = data.typeName;
                    usage.fieldName = element.name;
                    usage.kind = element.isBaseClass ? "base" : "field";
                    rememberTypeUsage(evidence, targetTypeId, usage);
                    if (element.isBaseClass || !element.isPointer) {
                        rememberNativeDataSize(evidence, targetTypeId, element);
                    }
                }
                for (String genericTypeId : element.genericTypeIds) {
                    TypeUsage usage = new TypeUsage();
                    usage.ownerTypeId = parentTypeId;
                    usage.ownerTypeName = data.typeName;
                    usage.fieldName = element.name;
                    usage.kind = "generic";
                    rememberTypeUsage(evidence, genericTypeId, usage);
                }
                rememberFieldOwner(evidence, data, element);
            }
        }
        summarizeFieldOwnerEvidence(evidence);
        return evidence;
    }

    private void rememberNativeDataSize(
        TypeEvidenceIndex evidence,
        String targetTypeId,
        ElementData element) {

        String normalizedTargetTypeId = normalizeTypeId(targetTypeId);
        Integer dataSize = parseLayoutInteger(element.dataSize);
        if (normalizedTargetTypeId == null || dataSize == null || dataSize <= 0) {
            return;
        }
        Integer current = evidence.nativeDataSizesByTypeId.get(normalizedTargetTypeId);
        if (current == null || dataSize > current) {
            evidence.nativeDataSizesByTypeId.put(normalizedTargetTypeId, dataSize);
        }
    }

    private void rememberFieldOwner(
        TypeEvidenceIndex evidence,
        ClassData owner,
        ElementData element) {

        FieldOwner ownerRecord = new FieldOwner();
        ownerRecord.owner = owner;
        ownerRecord.element = element;

        rememberFieldOwnerByKey(
            evidence.fieldOwnersByExactKey,
            exactFieldOwnerKey(element.name, element.typeId, element.offset),
            ownerRecord);
        rememberFieldOwnerByKey(
            evidence.fieldOwnersByNameTypeKey,
            nameTypeFieldOwnerKey(element.name, element.typeId),
            ownerRecord);
        rememberFieldOwnerByKey(
            evidence.fieldOwnersByTypeOffsetKey,
            typeOffsetFieldOwnerKey(element.typeId, element.offset),
            ownerRecord);
    }

    private void rememberFieldOwnerByKey(
        LinkedHashMap<String, ArrayList<FieldOwner>> index,
        String key,
        FieldOwner owner) {

        if (key == null) {
            return;
        }
        ArrayList<FieldOwner> owners = index.get(key);
        if (owners == null) {
            owners = new ArrayList<>();
            index.put(key, owners);
        }
        owners.add(owner);
    }

    private void summarizeFieldOwnerEvidence(TypeEvidenceIndex evidence) {
        evidence.fieldOwnerExactKeyCount = evidence.fieldOwnersByExactKey.size();
        evidence.fieldOwnerNameTypeKeyCount = evidence.fieldOwnersByNameTypeKey.size();
        evidence.fieldOwnerTypeOffsetKeyCount = evidence.fieldOwnersByTypeOffsetKey.size();
        evidence.ambiguousFieldOwnerExactKeyCount =
            ambiguousFieldOwnerKeyCount(evidence.fieldOwnersByExactKey);
        evidence.ambiguousFieldOwnerNameTypeKeyCount =
            ambiguousFieldOwnerKeyCount(evidence.fieldOwnersByNameTypeKey);
        evidence.ambiguousFieldOwnerTypeOffsetKeyCount =
            ambiguousFieldOwnerKeyCount(evidence.fieldOwnersByTypeOffsetKey);
    }

    private int ambiguousFieldOwnerKeyCount(
        LinkedHashMap<String, ArrayList<FieldOwner>> index) {

        int count = 0;
        for (ArrayList<FieldOwner> owners : index.values()) {
            if (uniqueFieldOwner(owners) == null) {
                count++;
            }
        }
        return count;
    }

    private void rememberTypeUsage(
        TypeEvidenceIndex evidence,
        String targetTypeId,
        TypeUsage usage) {

        String normalizedTargetTypeId = normalizeTypeId(targetTypeId);
        if (normalizedTargetTypeId == null || usage.ownerTypeId == null) {
            return;
        }
        ArrayList<TypeUsage> usages = evidence.usagesByTypeId.get(normalizedTargetTypeId);
        if (usages == null) {
            usages = new ArrayList<>();
            evidence.usagesByTypeId.put(normalizedTargetTypeId, usages);
        }
        usages.add(usage);
        evidence.usageCount++;
        increment(evidence.usageKindCounts, usage.kind);
    }

    private void attachStaticOwners(Iterable<RttiType> rttiTypes, List<ClassData> classData) {
        LinkedHashMap<String, ArrayList<String>> ownersByTypeId = new LinkedHashMap<>();
        LinkedHashMap<String, String> reflectFunctionsByTypeId = new LinkedHashMap<>();
        for (ClassData data : classData) {
            String normalizedTypeId = normalizeTypeId(data.typeId);
            StaticOwner owner = staticOwnerForClassData(data);
            if (owner != null) {
                data.staticReflectFunctionAddress = owner.functionAddress;
                if (normalizedTypeId != null) {
                    reflectFunctionsByTypeId.put(normalizedTypeId, owner.functionAddress);
                }
            }
            if (hasOwner(data.ownerScope)) {
                if (normalizedTypeId != null) {
                    ownersByTypeId.put(normalizedTypeId, data.ownerScope);
                }
                continue;
            }
            if (owner == null) {
                continue;
            }
            if (ownerScopeNamesType(owner.scope, data.targetTypeName)) {
                continue;
            }
            data.ownerScope = owner.scope;
            data.ownerReason = owner.reason;
            data.ownerSource = owner.source;
            data.staticReflectFunctionAddress = owner.functionAddress;
            if (normalizedTypeId != null) {
                ownersByTypeId.put(normalizedTypeId, owner.scope);
            }
            rememberStaticOwner(owner);
        }

        for (RttiType type : rttiTypes) {
            if (hasOwner(type.ownerScope)) {
                continue;
            }
            String normalizedTypeId = normalizeTypeId(type.typeId);
            ArrayList<String> ownerScope = ownersByTypeId.get(normalizedTypeId);
            if (ownerScope != null) {
                type.ownerScope = new ArrayList<>(ownerScope);
                type.ownerReason = "classdata-owner";
                type.staticReflectFunctionAddress = reflectFunctionsByTypeId.get(normalizedTypeId);
                continue;
            }

            StaticOwner owner = staticOwnerForRttiType(type);
            if (owner == null) {
                continue;
            }
            if (ownerScopeNamesType(owner.scope, type.targetTypeName)) {
                continue;
            }
            type.ownerScope = owner.scope;
            type.ownerReason = owner.reason;
            type.ownerSource = owner.source;
            type.staticReflectFunctionAddress = owner.functionAddress;
            rememberStaticOwner(owner);
        }
    }

    private boolean hasOwner(List<String> scope) {
        return scope != null && !scope.isEmpty();
    }

    private boolean ownerScopeNamesType(List<String> scope, String targetTypeName) {
        String safeTargetTypeName = safeTypeName(targetTypeName);
        return safeTargetTypeName != null &&
            hasOwner(scope) &&
            safeTargetTypeName.equals(scope.get(scope.size() - 1));
    }

    private void rememberStaticOwner(StaticOwner owner) {
        if (typeEvidence == null || owner == null) {
            return;
        }
        typeEvidence.staticOwnerCount++;
        String key = String.join("::", owner.scope);
        Integer count = typeEvidence.staticOwnerScopeCounts.get(key);
        typeEvidence.staticOwnerScopeCounts.put(key, count == null ? 1 : count + 1);
    }

    private StaticOwner staticOwnerForClassData(ClassData data) {
        ArrayList<String> anchors = new ArrayList<>();
        if (data.rttiAddress != null) {
            anchors.add(data.rttiAddress);
        }
        anchors.addAll(data.callbacks.values());
        anchors.addAll(data.objects.values());
        return staticOwnerFromAnchors(anchors);
    }

    private StaticOwner staticOwnerForRttiType(RttiType type) {
        ArrayList<String> anchors = new ArrayList<>();
        if (type.address != null) {
            anchors.add(type.address);
        }
        return staticOwnerFromAnchors(anchors);
    }

    private StaticOwner staticOwnerFromAnchors(List<String> anchorAddresses) {
        LinkedHashMap<String, StaticOwner> reflectCandidates = new LinkedHashMap<>();
        for (String jsonAddress : anchorAddresses) {
            Address address = parseCaptureAddress(jsonAddress);
            if (!isProgramAddress(address)) {
                continue;
            }
            collectStaticOwnerCandidate(
                currentProgram.getFunctionManager().getFunctionContaining(address),
                "anchor-function",
                jsonAddress,
                reflectCandidates);

            ReferenceIterator references =
                currentProgram.getReferenceManager().getReferencesTo(address);
            int scanned = 0;
            while (references.hasNext() && scanned < 256) {
                Reference reference = references.next();
                scanned++;
                collectStaticOwnerCandidate(
                    currentProgram.getFunctionManager()
                        .getFunctionContaining(reference.getFromAddress()),
                    "xref",
                    jsonAddress,
                    reflectCandidates);
            }
        }
        return uniqueStaticOwner(reflectCandidates);
    }

    private void collectStaticOwnerCandidate(
        Function function,
        String reason,
        String source,
        Map<String, StaticOwner> reflectCandidates) {

        if (function == null || !"Reflect".equals(function.getName())) {
            return;
        }
        ArrayList<String> scope = normalizeStaticOwnerScope(
            namespaceScope(function.getParentNamespace()));
        if (scope.isEmpty() || isGeneratedEvidenceScope(scope)) {
            return;
        }

        StaticOwner owner = new StaticOwner();
        owner.scope = scope;
        owner.reason = "static-" + reason;
        owner.source = source + " -> " + function.getName(true);
        owner.functionAddress = formatAddress(function.getEntryPoint());
        String key = String.join("::", scope);
        reflectCandidates.put(key, owner);
    }

    private StaticOwner uniqueStaticOwner(Map<String, StaticOwner> candidates) {
        if (candidates.size() != 1) {
            return null;
        }
        return candidates.values().iterator().next();
    }

    private ArrayList<String> namespaceScope(Namespace namespace) {
        ArrayList<String> reversed = new ArrayList<>();
        Namespace current = namespace;
        while (current != null && !current.isGlobal()) {
            reversed.add(current.getName());
            current = current.getParentNamespace();
        }
        ArrayList<String> result = new ArrayList<>();
        for (int i = reversed.size() - 1; i >= 0; i--) {
            result.add(reversed.get(i));
        }
        return result;
    }

    private ArrayList<String> normalizeStaticOwnerScope(ArrayList<String> scope) {
        return normalizeRepeatedScope(scope);
    }

    private ArrayList<String> normalizeRepeatedScope(List<String> scope) {
        ArrayList<String> result = new ArrayList<>();
        if (scope == null) {
            return result;
        }
        for (String part : scope) {
            String normalizedPart = safeTypeName(part);
            if (normalizedPart == null) {
                continue;
            }
            if (!result.isEmpty() && result.get(result.size() - 1).equals(normalizedPart)) {
                continue;
            }
            result.add(normalizedPart);
        }
        return result;
    }

    private boolean isGeneratedEvidenceScope(List<String> scope) {
        if (scope.isEmpty()) {
            return true;
        }
        String first = scope.get(0);
        if ("AZ".equals(first) || "AZStd".equals(first) || "std".equals(first)) {
            return true;
        }
        for (String part : scope) {
            if (part.startsWith("RttiHelper<") ||
                part.startsWith("Class<") ||
                part.startsWith("ClassData<") ||
                part.startsWith("ClassElement<") ||
                part.startsWith("Attribute<") ||
                part.startsWith("Field<") ||
                part.startsWith("InstanceFactory<") ||
                part.startsWith("ComponentDescriptorDefault<") ||
                "SerializeContext".equals(part)) {
                return true;
            }
        }
        return false;
    }

    private CollisionReport unresolvedTypeNameCollisions(
        Iterable<RttiType> rttiTypes,
        List<ClassData> classData) {

        LinkedHashMap<String, LinkedHashSet<String>> typeIdsByEffectiveName =
            new LinkedHashMap<>();
        for (RttiType type : rttiTypes) {
            rememberTypeName(
                typeIdsByEffectiveName,
                effectiveTypeName(type.ownerScope, type.targetTypeName),
                type.typeId);
        }
        for (ClassData data : classData) {
            rememberTypeName(
                typeIdsByEffectiveName,
                effectiveTypeName(data.ownerScope, data.targetTypeName),
                data.typeId);
        }

        CollisionReport result = new CollisionReport();
        result.names = new LinkedHashSet<>();
        result.typeIdsByName = new LinkedHashMap<>();
        for (Map.Entry<String, LinkedHashSet<String>> entry : typeIdsByEffectiveName.entrySet()) {
            if (entry.getValue().size() > 1) {
                result.names.add(entry.getKey());
                result.typeIdsByName.put(entry.getKey(), entry.getValue());
            }
        }
        return result;
    }

    private String effectiveTypeName(List<String> ownerScope, String targetTypeName) {
        if (targetTypeName == null) {
            return null;
        }
        if (!hasOwner(ownerScope)) {
            return targetTypeName;
        }
        return String.join("::", ownerScope) + "::" + targetTypeName;
    }

    private ArrayList<JsonObject> collectObjects(JsonElement root) {
        ArrayList<JsonObject> result = new ArrayList<>();
        ArrayDeque<JsonElement> stack = new ArrayDeque<>();
        stack.push(root);
        while (!stack.isEmpty()) {
            JsonElement element = stack.pop();
            if (element == null || element.isJsonNull()) {
                continue;
            }
            if (element.isJsonArray()) {
                JsonArray array = element.getAsJsonArray();
                for (int i = array.size() - 1; i >= 0; i--) {
                    stack.push(array.get(i));
                }
                continue;
            }
            if (!element.isJsonObject()) {
                continue;
            }

            JsonObject object = element.getAsJsonObject();
            result.add(object);
            for (Map.Entry<String, JsonElement> entry : object.entrySet()) {
                stack.push(entry.getValue());
            }
        }
        return result;
    }

    private LinkedHashMap<String, JsonObject> indexObjectsById(List<JsonObject> objects) {
        LinkedHashMap<String, JsonObject> result = new LinkedHashMap<>();
        for (JsonObject object : objects) {
            String id = stringMember(object, "$id");
            if (id != null) {
                result.put(id, object);
            }
        }
        return result;
    }

    private LinkedHashMap<String, JsonObject> indexGenericInfo(
        JsonElement root,
        List<JsonObject> objects,
        Map<String, JsonObject> objectsById) {

        LinkedHashMap<String, JsonObject> result = new LinkedHashMap<>();
        for (JsonObject object : objects) {
            if (!isGenericInfoRecord(object)) {
                continue;
            }
            rememberGenericInfo(result, object);
        }

        JsonObject rootObject = root != null && root.isJsonObject() ? root.getAsJsonObject() : null;
        JsonArray uuidGenericMap = rootObject == null ? null : arrayMember(rootObject, "uuidGenericMap");
        if (uuidGenericMap != null) {
            for (JsonElement entryValue : uuidGenericMap) {
                if (entryValue == null || !entryValue.isJsonArray()) {
                    continue;
                }
                JsonArray entry = entryValue.getAsJsonArray();
                if (entry.size() < 2 || !entry.get(1).isJsonObject()) {
                    continue;
                }
                String typeId = stringElement(entry.get(0));
                JsonObject genericInfo = resolveObject(entry.get(1), objectsById);
                if (typeId != null && genericInfo != null) {
                    result.put(normalizeTypeId(typeId), genericInfo);
                    rememberGenericInfo(result, genericInfo);
                }
            }
        }
        return result;
    }

    private void rememberGenericInfo(Map<String, JsonObject> result, JsonObject genericInfo) {
        String specializedTypeId = stringMember(genericInfo, "specializedTypeId");
        if (specializedTypeId == null) {
            specializedTypeId = stringMember(genericInfo, "typeId");
        }
        if (specializedTypeId != null) {
            result.put(normalizeTypeId(specializedTypeId), genericInfo);
        }
    }

    private boolean isGenericInfoRecord(JsonObject object) {
        return object.has("templatedTypeIds") &&
            (object.has("specializedTypeId") || object.has("genericTypeId"));
    }

    private LinkedHashMap<String, String> indexRawTypeNames(List<JsonObject> objects) {
        LinkedHashMap<String, String> result = new LinkedHashMap<>();
        seedEngineTypeNames(result);
        for (JsonObject object : objects) {
            if (isClassDataRecord(object)) {
                String typeId = classDataTypeId(object);
                String typeName = classDataRawTypeName(object);
                rememberRawTypeName(result, typeId, typeName);
            }

            JsonObject azRtti = objectMember(object, "azRtti");
            if (azRtti != null) {
                rememberRawTypeName(result, stringMember(azRtti, "typeId"), stringMember(azRtti, "typeName"));
            }

            rememberRawTypeName(result, stringMember(object, "typeId"), stringMember(object, "typeName"));
            JsonArray hierarchy = arrayMember(object, "hierarchy");
            if (hierarchy != null) {
                for (JsonElement entryValue : hierarchy) {
                    JsonObject entry = resolveObject(entryValue, jsonObjectsById);
                    if (entry != null) {
                        rememberRawTypeName(
                            result,
                            stringMember(entry, "typeId"),
                            stringMember(entry, "typeName"));
                    }
                }
            }
        }
        return result;
    }

    private void rememberRawTypeName(Map<String, String> result, String typeId, String typeName) {
        String normalizedTypeId = normalizeTypeId(typeId);
        String candidate = knownSourceName(typeName);
        if (normalizedTypeId == null || candidate == null) {
            return;
        }
        String existing = result.get(normalizedTypeId);
        if (shouldKeepExistingRawTypeName(existing, candidate)) {
            return;
        }
        result.put(normalizedTypeId, candidate);
    }

    private boolean shouldKeepExistingRawTypeName(String existing, String candidate) {
        if (existing == null || existing.equals(candidate)) {
            return existing != null;
        }
        if (existing.contains("<") && !candidate.contains("<")) {
            return true;
        }
        return existing.contains("::") && !candidate.contains("::");
    }

    private void seedEngineTypeNames(Map<String, String> result) {
        rememberRawTypeName(result, "3AB0037F-AF8D-48CE-BCA0-A170D18B2C03", "char");
        rememberRawTypeName(result, "CFD606FE-41B8-4744-B79F-8A6BD97713D8", "signed char");
        rememberRawTypeName(result, "58422C0E-1E47-4854-98E6-34098F6FE12D", "s8");
        rememberRawTypeName(result, "B8A56D56-A10D-4DCE-9F63-405EE243DD3C", "s16");
        rememberRawTypeName(result, "72039442-EB38-4D42-A1AD-CB68F7E0EEF6", "s32");
        rememberRawTypeName(result, "8F24B9AD-7C51-46CF-B2F8-277356957325", "s64");
        rememberRawTypeName(result, "70D8A282-A1EA-462D-9D04-51EDE81FAC2F", "s64");
        rememberRawTypeName(result, "72B9409A-7D1A-4831-9CFE-FCB3FADD3426", "u8");
        rememberRawTypeName(result, "ECA0B403-C4F8-4B86-95FC-81688D046E40", "u16");
        rememberRawTypeName(result, "43DA906B-7DEF-4CA8-9790-854106D3F983", "u32");
        rememberRawTypeName(result, "5EC2D6F7-6859-400F-9215-C106F5B10E53", "unsigned long");
        rememberRawTypeName(result, "D6597933-47CD-4FC8-B911-63F3E2B0993A", "u64");
        rememberRawTypeName(result, "EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D", "float");
        rememberRawTypeName(result, "110C4B14-11A8-4E9D-8638-5051013A56AC", "double");
        rememberRawTypeName(result, "A0CA880C-AFE4-43CB-926C-59AC48496112", "bool");
        rememberRawTypeName(result, "E152C105-A133-4D03-BBF8-3D4B2FBA3E2A", "AZ::Uuid");
        rememberRawTypeName(result, "75651658-8663-478D-9090-2432DFCAFA44", "AZ::Entity");
        rememberRawTypeName(result, "EDFCB2CF-F75D-43BE-B26B-F35821B29247", "AZ::Component");
        rememberRawTypeName(result, "0A7929DF-2932-40EA-B2B3-79BC1C3490D0", "AZ::ComponentConfig");
        rememberRawTypeName(result, "C845E5EC-5580-4E12-A9B2-9AE7E5B7826F", "AZ::EntityComponentIdPair");
        rememberRawTypeName(result, "6383F1D3-BB27-4E6B-A49A-6409B2059EAA", "AZ::EntityId");
        rememberRawTypeName(result, "27F37921-4B40-4BE6-B47B-7D3AB8682D58", "AZ::NamedEntityId");
        rememberRawTypeName(result, "C0F1AFAD-5CB3-450E-B0F5-ADB5D46B0E22", "void");
        rememberRawTypeName(result, "9F4E062E-06A0-46D4-85DF-E0DA96467D3A", "AZ::Crc32");
        rememberRawTypeName(result, "0635D08E-DDD2-48DE-A7AE-73CC563C57C3", "AZ::PlatformID");
        rememberRawTypeName(result, "EEA8B695-51EE-4717-B818-4070C6DA849D", "AZ::VectorFloat");
        rememberRawTypeName(result, "3D80F623-C85C-4741-90D0-E4E66164E6BF", "AZ::Vector2");
        rememberRawTypeName(result, "8379EB7D-01FA-4538-B64B-A6543B4BE73D", "AZ::Vector3");
        rememberRawTypeName(result, "0CE9FA36-1E3A-4C06-9254-B7C73A732053", "AZ::Vector4");
        rememberRawTypeName(result, "5D9958E9-9F1E-4985-B532-FFFDE75FEDFD", "AZ::Transform");
        rememberRawTypeName(result, "73103120-3DD3-4873-BAB3-9713FA2804FB", "AZ::Quaternion");
        rememberRawTypeName(result, "7894072A-9050-4F0F-901B-34B1A0D29417", "AZ::Color");
        rememberRawTypeName(result, "63782551-A309-463B-A301-3A360800DF1E", "ColorF");
        rememberRawTypeName(result, "6F0CC2C0-0CC6-4DBF-9297-B043F270E6A4", "ColorB");
        rememberRawTypeName(result, "A54C2B36-D5B8-46A1-A529-4EBDBD2450E7", "AZ::Aabb");
        rememberRawTypeName(result, "004ABD25-CF14-4EB3-BD41-022C247C07FA", "AZ::Obb");
        rememberRawTypeName(result, "847DD984-9DBF-4789-8E25-E0334402E8AD", "AZ::Plane");
        rememberRawTypeName(result, "15A4332F-7C3F-4A58-AC35-50E1CE53FB9C", "AZ::Matrix3x3");
        rememberRawTypeName(result, "157193C7-B673-4A2B-8B43-5681DCC3DEC3", "AZ::Matrix4x4");
        rememberRawTypeName(result, "B1E9136B-D77A-4643-BE8E-2ABDA246AE0E", "AZStd::monostate");
        rememberRawTypeName(result, "E9F5A3BE-2B3D-4C62-9E6B-4E00A13AB452", "AZStd::allocator");
        rememberRawTypeName(result, "42D0AA1E-3C6C-440E-ABF8-435931150470", "AZ::AZStdAlloc");
        rememberRawTypeName(result, "9F835EE3-F23C-454E-B4E3-011E2F3C8118", "AZ::OSAllocator");
        rememberRawTypeName(result, "424C94D8-85CF-4E89-8CD6-AB5EC173E875", "AZ::SystemAllocator");
        rememberRawTypeName(result, "59682E0E-731F-4361-BC0B-039BC5376CA1", "AZ::Module");
        rememberRawTypeName(result, "C5950488-35E0-4B55-B664-29A691A6482F", "AZ::ModuleEntity");
        rememberRawTypeName(result, "1F3B070F-89F7-4C3D-B5A3-8832D5BC81D7", "AZ::ComponentApplication");
        rememberRawTypeName(
            result,
            "70277A3E-2AF5-4309-9BBF-6161AFBDE792",
            "AZ::ComponentApplication::Descriptor");
        rememberRawTypeName(
            result,
            "4C865590-4506-4B76-BF14-6CCB1B83019A",
            "AZ::ComponentApplication::Descriptor::AllocatorRemapping");
        rememberRawTypeName(
            result,
            "E98CF1B5-6B72-46C5-AB87-3DB85FD1B48D",
            "AZ::EntityUtils::SerializableEntityContainer");
        rememberRawTypeName(result, "41B40AFC-68FD-4ED9-9EC7-BA9992802E1B", "AZStd::less");
        rememberRawTypeName(result, "91CC0BDC-FC46-4617-A405-D914EF1C1902", "AZStd::less_equal");
        rememberRawTypeName(result, "907F012A-7A4F-4B57-AC23-48DC08D0782E", "AZStd::greater");
        rememberRawTypeName(result, "EB00488F-E20F-471A-B862-F1E3C39DDA1D", "AZStd::greater_equal");
        rememberRawTypeName(result, "4377BCED-F78C-4016-80BB-6AFACE6E5137", "AZStd::equal_to");
        rememberRawTypeName(result, "EFA74E54-BDFA-47BE-91A7-5A05DA0306D7", "AZStd::hash");
        rememberRawTypeName(result, "9B018C0C-022E-4BA4-AE91-2C1E8592DBB2", "AZStd::char_traits");
        rememberRawTypeName(result, "C26397ED-8F60-4DF6-8320-0D0C592DA3CD", "AZStd::basic_string");
        rememberRawTypeName(result, "03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9", "AZStd::string");
        rememberRawTypeName(result, "EF8FF807-DDEE-4EB0-B678-4CA3A2C490A4", "AZStd::string");
        rememberRawTypeName(result, "D348D661-6BDE-4C0A-9540-FCEA4522D497", "AZStd::basic_string_view");
        rememberRawTypeName(result, "919645C1-E464-482B-A69B-04AA688B6847", "AZStd::pair");
        rememberRawTypeName(result, "A60E3E61-1FF6-4982-B6B8-9E4350C4C679", "AZStd::vector");
        rememberRawTypeName(result, "2BADE35A-6F1B-4698-B2BC-3373D010020C", "AZStd::vector");
        rememberRawTypeName(result, "E1E05843-BB02-4F43-B7DC-3ADB28DF42AC", "AZStd::list");
        rememberRawTypeName(result, "D7E91EA3-326F-4019-87F0-6F45924B909A", "AZStd::forward_list");
        rememberRawTypeName(result, "6C51837F-B0C9-40A3-8D52-2143341EDB07", "AZStd::set");
        rememberRawTypeName(result, "8D60408E-DA65-4670-99A2-8ABB574625AE", "AZStd::unordered_set");
        rememberRawTypeName(result, "B5950921-7F70-4806-9C13-8C7DF841BB90", "AZStd::unordered_multiset");
        rememberRawTypeName(result, "F8ECF58D-D33E-49DC-BF34-8FA499AC3AE1", "AZStd::map");
        rememberRawTypeName(result, "41171F6F-9E5E-4227-8420-289F1DD5D005", "AZStd::unordered_map");
        rememberRawTypeName(result, "9ED846FA-31C1-4133-B4F4-91DF9750BA96", "AZStd::unordered_multimap");
        rememberRawTypeName(result, "FE61C84E-149D-43FD-88BA-1C3DB7E548B4", "AZStd::shared_ptr");
        rememberRawTypeName(result, "530F8502-309E-4EE1-9AEF-5C0456B1F502", "AZStd::intrusive_ptr");
        rememberRawTypeName(result, "B55F90DA-C21E-4EB4-9857-87BE6529BA6D", "AZStd::unique_ptr");
        rememberRawTypeName(result, "AB8C50C0-23A7-4333-81CD-46F648938B1C", "AZStd::optional");
        rememberRawTypeName(result, "74044B6F-E922-4FD7-915D-EFC5D1DC59AE", "AZStd::fixed_vector");
        rememberRawTypeName(result, "508B9687-8410-4A73-AE0C-0BA15CF3F773", "AZStd::fixed_list");
        rememberRawTypeName(result, "0D9D2AB2-F0CC-4E30-A209-A33D78717649", "AZStd::fixed_forward_list");
        rememberRawTypeName(result, "911B2EA8-CCB1-4F0C-A535-540AD00173AE", "AZStd::array");
        rememberRawTypeName(result, "6BAE9836-EC49-466A-85F2-F4B1B70839FB", "AZStd::bitset");
        rememberRawTypeName(result, "F99F9308-DC3E-4384-9341-89CBF1ABD51E", "AZStd::tuple");
        rememberRawTypeName(result, "EAC2A157-5400-499D-81F1-8E8D979E96D8", "AZStd::ranged_int");
        rememberRawTypeName(result, "AA6CB2BA-A6FA-43A3-B08C-4B6E0D751068", "AZStd::unordered_flat_map");
        rememberRawTypeName(result, "1E8BB1E5-410A-4367-8FAA-D43A4DE14D4B", "AZStd::variant");
        rememberRawTypeName(result, "C9F9C644-CCC3-4F77-A792-F5B5DBCA746E", "AZStd::function");
        rememberRawTypeName(result, "C891BF19-B60C-45E2-BFD0-027D15DDC939", "AZ::Data::Asset");
        rememberRawTypeName(result, "77A19D40-8731-4D3C-9041-1B43047366A4", "AZ::Data::Asset");
        rememberRawTypeName(result, "AF3F7D32-1536-422A-89F3-A11E1F5B5A9C", "AZ::Data::AssetData");
        rememberRawTypeName(result, "652ED536-3402-439B-AEBE-4A5DBC554085", "AZ::Data::AssetId");
        rememberRawTypeName(result, "ADFD596B-7177-5519-9752-BC418FE42963", "ByteStream");
        rememberRawTypeName(result, "2590807F-5748-4CD0-A475-83EF5FD216CF", "AZ::Internal::RValueToLValueWrapper");
        rememberRawTypeName(result, "5C059EC7-44B0-4666-9FC9-674192338F39", "MB::ReplicatedField");
        rememberRawTypeName(result, "DFE50973-EA0B-4616-833A-B60B5E2E71DF", "Amazon::Pervasives::UID");
        rememberRawTypeName(result, "DB1EB3E5-F953-53A7-B8F9-9121E6A77F85", "AZStd::unordered_set<AZ::Data::AssetId>");
        rememberRawTypeName(result, "E7781CB0-E712-5E6A-948D-92FD4FE87F0D", "AZStd::vector<AZ::ComponentId>");
        rememberRawTypeName(result, "01DB3319-83C9-55AD-A271-EB299466FE34", "AZ::Data::Asset<AZ::Data::AssetData>");
        rememberRawTypeName(result, "50B333E9-98CD-5FF6-871A-5C6CD54C83A1", "AZStd::vector<DebugValue>");
        rememberRawTypeName(result, "842097CA-15C0-5C8D-A2D8-92EA8995C752", "Internal::RValueToLValueWrapper<s8>");
        rememberRawTypeName(result, "255E32A3-024C-54A0-8D5C-6BA682B43192", "AZStd::ranged_int<u8,0,8>");
        rememberRawTypeName(result, "4852F7A3-95BE-5CBF-B76F-031D6F334DF9", "BitSet<2>");
        rememberRawTypeName(result, "57C9C8DA-F80A-56C2-9ADE-16A19A8F6733", "AZStd::unordered_flat_map<AZ::Crc32,float>");
        rememberRawTypeName(result, "0C58DD7B-90DB-5AD9-A24B-53AD03F6593A", "AZStd::tuple<WarDetails,u64>");
        rememberRawTypeName(result, "DE1CB64D-EBC4-583E-AF31-EB257B8AC677", "AZStd::tuple<s32,AZStd::string,s32>");
        rememberRawTypeName(result, "4BBB5D83-C428-58E2-B045-24D4608383E1", "AZStd::fixed_vector<AZ::Crc32,20>");
        rememberRawTypeName(result, "44BC0C45-DA18-5E2C-9D9D-943F964CB90C", "MB::ReplicatedField<s32>");
        rememberRawTypeName(result, "3485F20A-98C0-5315-876B-21BCD23A7BC0", "Amazon::Pervasives::UID<128>");
    }

    private ClassData classDataFrom(
        JsonObject object,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId) {

        if (!isClassDataRecord(object)) {
            return null;
        }

        String typeId = stringMember(object, "typeId");
        String name = stringMember(object, "name");
        if (typeId == null || name == null) {
            return null;
        }

        boolean hasClassDataFields = false;
        for (String field : OBJECT_FIELDS) {
            hasClassDataFields |= object.has(field);
        }
        for (String field : FUNCTION_FIELDS.keySet()) {
            hasClassDataFields |= object.has(field);
        }
        hasClassDataFields |= object.has("azRtti");
        if (!hasClassDataFields) {
            return null;
        }

        ClassData result = new ClassData();
        String rawTypeName = classDataRawTypeName(object);
        result.typeId = classDataTypeId(object);
        result.typeName = displayTypeName(
            result.typeId,
            rawTypeName,
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            new HashSet<>());
        result.rttiAddress = stringFromAzRtti(object, "address");
        result.callbacks = new LinkedHashMap<>();
        result.objects = new LinkedHashMap<>();
        result.elements = elementDataFrom(object, genericInfoByTypeId, rawTypeNamesByTypeId);
        for (String field : OBJECT_FIELDS) {
            String address = stringMember(object, field);
            if (isAddressLike(address)) {
                result.objects.put(field, address);
            }
        }
        for (String field : FUNCTION_FIELDS.keySet()) {
            String address = stringMember(object, field);
            if (isAddressLike(address)) {
                result.callbacks.put(field, address);
            }
        }
        return result;
    }

    private boolean isClassDataRecord(JsonObject object) {
        if (!object.has("name") || !object.has("typeId")) {
            return false;
        }
        if (!object.has("version") || !object.has("elements") || !object.has("attributes")) {
            return false;
        }
        if (object.has("offset") || object.has("dataSize") || object.has("is_base_class")) {
            return false;
        }
        return object.has("converter") ||
            object.has("factory") ||
            object.has("persistentId") ||
            object.has("doSave") ||
            object.has("serializer") ||
            object.has("eventHandler") ||
            object.has("container") ||
            object.has("azRtti") ||
            object.has("dataConverter");
    }

    private String classDataTypeId(JsonObject object) {
        String typeId = stringFromAzRtti(object, "typeId");
        if (typeId == null) {
            typeId = stringMember(object, "typeId");
        }
        return typeId;
    }

    private String classDataRawTypeName(JsonObject object) {
        String typeName = stringFromAzRtti(object, "typeName");
        if (typeName == null) {
            typeName = stringMember(object, "name");
        }
        return typeName;
    }

    private String displayTypeName(
        String typeId,
        String sourceName,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId,
        Set<String> visitingTypeIds) {

        String normalizedTypeId = normalizeTypeId(typeId);
        if (normalizedTypeId != null && visitingTypeIds.contains(normalizedTypeId)) {
            return null;
        }

        JsonObject genericInfo =
            normalizedTypeId == null ? null : genericInfoByTypeId.get(normalizedTypeId);
        if (genericInfo == null) {
            return knownTypeName(typeId, sourceName, rawTypeNamesByTypeId);
        }

        String exactSpecialization = exactSpecializedTypeName(
            normalizedTypeId,
            rawTypeNamesByTypeId);
        if (exactSpecialization != null) {
            return exactSpecialization;
        }

        if (normalizedTypeId != null) {
            visitingTypeIds.add(normalizedTypeId);
        }

        String baseName = genericBaseName(genericInfo, sourceName, rawTypeNamesByTypeId);
        if (baseName == null) {
            if (normalizedTypeId != null) {
                visitingTypeIds.remove(normalizedTypeId);
            }
            return null;
        }
        if (baseName.equals("AZStd::basic_string") || baseName.equals("AZStd::string")) {
            if (normalizedTypeId != null) {
                visitingTypeIds.remove(normalizedTypeId);
            }
            return "AZStd::string";
        }
        if (baseName.equals("ByteStream")) {
            if (normalizedTypeId != null) {
                visitingTypeIds.remove(normalizedTypeId);
            }
            return "ByteStream";
        }

        ArrayList<String> arguments =
            genericArgumentNames(genericInfo, genericInfoByTypeId, rawTypeNamesByTypeId, visitingTypeIds);
        if (arguments == null) {
            if (normalizedTypeId != null) {
                visitingTypeIds.remove(normalizedTypeId);
            }
            return null;
        }
        if (!addNonTypeTemplateArguments(arguments, genericInfo)) {
            if (normalizedTypeId != null) {
                visitingTypeIds.remove(normalizedTypeId);
            }
            return null;
        }

        if (normalizedTypeId != null) {
            visitingTypeIds.remove(normalizedTypeId);
        }

        if (arguments.isEmpty()) {
            return baseName;
        }
        StringBuilder builder = new StringBuilder(baseName);
        builder.append("<");
        for (int i = 0; i < arguments.size(); i++) {
            if (i > 0) {
                builder.append(",");
            }
            builder.append(arguments.get(i));
        }
        builder.append(">");
        return builder.toString();
    }

    private ArrayList<String> genericArgumentNames(
        JsonObject genericInfo,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId,
        Set<String> visitingTypeIds) {

        JsonArray elements = arrayMember(genericInfo, "elements");
        if (elements != null && elements.size() > 0) {
            ArrayList<String> pairArguments = memberPairArgumentNames(
                elements,
                genericInfoByTypeId,
                rawTypeNamesByTypeId,
                visitingTypeIds);
            if (pairArguments != null) {
                return pairArguments;
            }
            if (genericMember(elements, "value1") != null || genericMember(elements, "value2") != null) {
                return null;
            }

            JsonObject element = genericMember(elements, "element");
            if (element != null) {
                JsonObject elementGenericInfo = objectMember(element, "genericClassInfo");
                if (templateArgumentCount(genericInfo) > 1 && elementGenericInfo != null) {
                    ArrayList<String> elementPairArguments = memberPairArgumentNames(
                        arrayMember(elementGenericInfo, "elements"),
                        genericInfoByTypeId,
                        rawTypeNamesByTypeId,
                        visitingTypeIds);
                    if (elementPairArguments != null) {
                        return elementPairArguments;
                    }
                    JsonArray elementGenericElements = arrayMember(elementGenericInfo, "elements");
                    if (
                        genericMember(elementGenericElements, "value1") != null ||
                        genericMember(elementGenericElements, "value2") != null
                    ) {
                        return null;
                    }
                }
                return singleMemberArgument(
                    element,
                    genericInfoByTypeId,
                    rawTypeNamesByTypeId,
                    visitingTypeIds);
            }

            JsonObject value = genericMember(elements, "value");
            if (value != null) {
                return singleMemberArgument(
                    value,
                    genericInfoByTypeId,
                    rawTypeNamesByTypeId,
                    visitingTypeIds);
            }
        }

        ArrayList<String> result = new ArrayList<>();
        for (String argumentTypeId : genericArgumentTypeIds(genericInfo)) {
            String argumentName = displayTypeName(
                argumentTypeId,
                rawTypeName(argumentTypeId, rawTypeNamesByTypeId),
                genericInfoByTypeId,
                rawTypeNamesByTypeId,
                visitingTypeIds);
            if (argumentName == null) {
                return null;
            }
            result.add(argumentName);
        }
        return result;
    }

    private ArrayList<String> memberPairArgumentNames(
        JsonArray elements,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId,
        Set<String> visitingTypeIds) {

        if (elements == null) {
            return null;
        }
        JsonObject value1 = genericMember(elements, "value1");
        JsonObject value2 = genericMember(elements, "value2");
        if (value1 == null || value2 == null) {
            return null;
        }
        String first = displayMemberTypeName(
            value1,
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            visitingTypeIds);
        String second = displayMemberTypeName(
            value2,
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            visitingTypeIds);
        if (first == null || second == null) {
            return null;
        }
        ArrayList<String> result = new ArrayList<>();
        result.add(first);
        result.add(second);
        return result;
    }

    private ArrayList<String> singleMemberArgument(
        JsonObject member,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId,
        Set<String> visitingTypeIds) {

        String argument = displayMemberTypeName(
            member,
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            visitingTypeIds);
        if (argument == null) {
            return null;
        }
        ArrayList<String> result = new ArrayList<>();
        result.add(argument);
        return result;
    }

    private String displayMemberTypeName(
        JsonObject member,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId,
        Set<String> visitingTypeIds) {

        JsonObject directGenericInfo = objectMember(member, "genericClassInfo");
        if (directGenericInfo != null) {
            String genericTypeId = stringMember(directGenericInfo, "specializedTypeId");
            if (genericTypeId == null) {
                genericTypeId = stringMember(directGenericInfo, "typeId");
            }
            String genericName = displayTypeName(
                genericTypeId,
                rawTypeName(genericTypeId, rawTypeNamesByTypeId),
                genericInfoByTypeId,
                rawTypeNamesByTypeId,
                visitingTypeIds);
            if (genericName != null) {
                return genericName;
            }
        }

        String typeId = stringMember(member, "typeId");
        return displayTypeName(
            typeId,
            rawTypeName(typeId, rawTypeNamesByTypeId),
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            visitingTypeIds);
    }

    private int templateArgumentCount(JsonObject genericInfo) {
        String value = stringMember(genericInfo, "templatedArgumentCount");
        if (value == null) {
            return 0;
        }
        try {
            return Integer.parseInt(value);
        }
        catch (NumberFormatException ignored) {
            return 0;
        }
    }

    private ArrayList<String> genericArgumentTypeIds(JsonObject genericInfo) {
        ArrayList<String> result = new ArrayList<>();
        JsonArray templatedTypeIds = arrayMember(genericInfo, "templatedTypeIds");
        if (templatedTypeIds != null && templatedTypeIds.size() > 0) {
            for (JsonElement argumentTypeIdValue : templatedTypeIds) {
                String argumentTypeId = stringElement(argumentTypeIdValue);
                if (argumentTypeId != null) {
                    result.add(argumentTypeId);
                }
            }
            return result;
        }

        JsonArray elements = arrayMember(genericInfo, "elements");
        if (elements == null || elements.size() == 0) {
            return result;
        }

        String value1 = genericMemberTypeId(elements, "value1");
        String value2 = genericMemberTypeId(elements, "value2");
        if (value1 != null && value2 != null) {
            result.add(value1);
            result.add(value2);
            return result;
        }

        String element = genericMemberTypeId(elements, "element");
        if (element != null) {
            result.add(element);
            return result;
        }

        JsonObject first = resolveObject(elements.get(0), jsonObjectsById);
        String firstTypeId = first == null ? null : stringMember(first, "typeId");
        if (firstTypeId != null) {
            result.add(firstTypeId);
        }
        return result;
    }

    private JsonObject genericMember(JsonArray elements, String memberName) {
        if (elements == null) {
            return null;
        }
        for (JsonElement elementValue : elements) {
            JsonObject element = resolveObject(elementValue, jsonObjectsById);
            if (element == null) {
                continue;
            }
            if (memberName.equals(stringMember(element, "name"))) {
                return element;
            }
        }
        return null;
    }

    private String genericMemberTypeId(JsonArray elements, String memberName) {
        JsonObject member = genericMember(elements, memberName);
        return member == null ? null : stringMember(member, "typeId");
    }

    private String genericBaseName(
        JsonObject genericInfo,
        String sourceName,
        Map<String, String> rawTypeNamesByTypeId) {

        String genericTypeId = stringMember(genericInfo, "genericTypeId");
        String genericName = rawTypeName(genericTypeId, rawTypeNamesByTypeId);
        if (genericName != null) {
            return genericName;
        }

        JsonObject classData = objectMember(genericInfo, "classData");
        if (classData != null) {
            String classDataName = classDataRawTypeName(classData);
            if (knownSourceName(classDataName) != null) {
                return classDataName;
            }
        }
        return knownSourceName(sourceName);
    }

    private String knownTypeName(
        String typeId,
        String sourceName,
        Map<String, String> rawTypeNamesByTypeId) {
        String knownSourceName = knownSourceName(sourceName);
        if (knownSourceName != null) {
            return knownSourceName;
        }
        String rawName = rawTypeName(typeId, rawTypeNamesByTypeId);
        if (knownSourceName(rawName) != null) {
            return rawName;
        }
        return null;
    }

    private String exactSpecializedTypeName(
        String normalizedTypeId,
        Map<String, String> rawTypeNamesByTypeId) {

        if (normalizedTypeId == null) {
            return null;
        }
        String typeName = rawTypeNamesByTypeId.get(normalizedTypeId);
        if (typeName == null || !typeName.contains("<")) {
            return null;
        }
        return knownSourceName(typeName);
    }

    private String knownSourceName(String value) {
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        if (UUID_RE.matcher(value).find()) {
            return null;
        }
        return value;
    }

    private String rawTypeName(String typeId, Map<String, String> rawTypeNamesByTypeId) {
        String normalizedTypeId = normalizeTypeId(typeId);
        return normalizedTypeId == null ? null : rawTypeNamesByTypeId.get(normalizedTypeId);
    }

    private boolean addNonTypeTemplateArguments(ArrayList<String> arguments, JsonObject genericInfo) {
        JsonObject nonTypeTemplateArguments = objectMember(genericInfo, "nonTypeTemplateArguments");
        if (nonTypeTemplateArguments == null) {
            return true;
        }
        for (Map.Entry<String, JsonElement> entry : nonTypeTemplateArguments.entrySet()) {
            if (!appendTemplateArgumentStrings(arguments, entry.getValue(), 0)) {
                return false;
            }
        }
        return true;
    }

    private boolean appendTemplateArgumentStrings(
        ArrayList<String> arguments,
        JsonElement value,
        int depth) {

        if (depth >= MAX_TEMPLATE_VALUE_DEPTH) {
            return false;
        }
        JsonElement resolved = resolveElement(value, jsonObjectsById);
        if (resolved == null || resolved.isJsonNull()) {
            return false;
        }
        if (resolved.isJsonPrimitive()) {
            String text = primitiveString(resolved);
            if (text == null) {
                return false;
            }
            arguments.add(text);
            return true;
        }
        if (!resolved.isJsonArray()) {
            return false;
        }
        for (JsonElement item : resolved.getAsJsonArray()) {
            if (!appendTemplateArgumentStrings(arguments, item, depth + 1)) {
                return false;
            }
        }
        return true;
    }

    private void collectRttiType(
        JsonObject object,
        ClassData classRecord,
        LinkedHashMap<String, RttiType> rttiTypes,
        Map<String, LinkedHashSet<String>> typeIdsByName,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId) {

        JsonObject azRtti = objectMember(object, "azRtti");
        JsonObject source = azRtti != null ? azRtti : object;

        String address = stringMember(source, "address");
        if (!isAddressLike(address)) {
            return;
        }

        String typeName = stringMember(source, "typeName");
        if (typeName == null && classRecord != null) {
            typeName = classRecord.typeName;
        }
        String typeId = stringMember(source, "typeId");
        if (typeId == null && classRecord != null) {
            typeId = classRecord.typeId;
        }
        if (typeName == null || typeName.trim().isEmpty()) {
            return;
        }
        typeName = displayTypeName(
            typeId,
            typeName,
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            new HashSet<>());

        String key = addressKey(address);
        RttiType type = rttiTypes.get(key);
        if (type == null) {
            type = new RttiType();
            type.address = address;
            type.typeName = typeName;
            type.typeId = typeId;
            type.isAbstract = boolMember(source, "isAbstract");
            rttiTypes.put(key, type);
        }
        else {
            if (type.typeName == null && typeName != null) {
                type.typeName = typeName;
            }
            if (type.typeId == null && typeId != null) {
                type.typeId = typeId;
            }
            if (type.isAbstract == null) {
                type.isAbstract = boolMember(source, "isAbstract");
            }
        }
        rememberTypeName(typeIdsByName, typeName, typeId);
    }

    private void rememberTypeName(
        Map<String, LinkedHashSet<String>> typeIdsByName,
        String typeName,
        String typeId) {
        if (typeName == null || typeId == null) {
            return;
        }
        LinkedHashSet<String> typeIds = typeIdsByName.get(typeName);
        if (typeIds == null) {
            typeIds = new LinkedHashSet<>();
            typeIdsByName.put(typeName, typeIds);
        }
        typeIds.add(typeId.toUpperCase());
    }

    private String serializedTypeName(String typeName) {
        return safeTypeName(typeName);
    }

    private void processRttiType(
        RttiType type,
        String[] slotNames,
        Map<String, SlotGroup> rttiSlotGroups,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (type.targetTypeName == null) {
            addSkipped(actions, "rtti_helper", type.address, null, "missing-type-name");
            return;
        }

        List<String> scope = rttiScope(type);
        renameLabel(type.address, scope, "s_instance", "rtti_helper", actions);
        Address helperAddress = parseCaptureAddress(type.address);
        Address vtableAddress = readPointer(helperAddress);
        if (vtableAddress == null) {
            addSkipped(actions, "label", type.address, fullName(scope, "vftable"), "missing-vtable");
            return;
        }

        renameLabel(formatAddress(vtableAddress), scope, "vftable", "rtti_vftable", actions);
        for (int slot = 0; slot < slotNames.length; slot++) {
            Address slotPointer = readPointer(vtableAddress.add(slot * 8L));
            if (slotPointer == null) {
                continue;
            }
            String slotAddress = formatAddress(slotPointer);
            renameFunction(
                slotAddress,
                scope,
                slotNames[slot],
                rttiSlotGroups,
                functionSeen,
                aliasSeen,
                actions);
        }
    }

    private void processClassData(
        ClassData classData,
        Map<String, SlotGroup> callbackGroups,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (classData.targetTypeName == null) {
            return;
        }
        List<String> scope = classDataScope(classData);

        for (Map.Entry<String, String> entry : classData.objects.entrySet()) {
            String field = entry.getKey();
            String address = entry.getValue();
            renameLabel(address, scope, field, "classdata_object", actions);

            Address objectAddress = parseCaptureAddress(address);
            Address vtableAddress = readPointer(objectAddress);
            if (vtableAddress != null) {
                renameLabel(
                    formatAddress(vtableAddress),
                    scope,
                    field + "_vftable",
                    "classdata_object_vftable",
                    actions);
            }
        }

        for (Map.Entry<String, String> entry : classData.callbacks.entrySet()) {
            String field = entry.getKey();
            String address = entry.getValue();
            renameFunction(
                address,
                scope,
                FUNCTION_FIELDS.get(field),
                callbackGroups,
                functionSeen,
                aliasSeen,
                actions);
        }

        for (ElementData element : classData.elements) {
            for (ElementCallback callback : element.callbacks) {
                renameFunction(
                    callback.address,
                    attributeCallbackScope(classData, element, callback),
                    attributeCallbackName(callback),
                    callbackGroups,
                    functionSeen,
                    aliasSeen,
                    actions);
            }
        }
    }

    private int elementCallbackCount(List<ClassData> classData) {
        int count = 0;
        for (ClassData data : classData) {
            for (ElementData element : data.elements) {
                count += element.callbacks.size();
            }
        }
        return count;
    }

    private void cleanupRepeatedFunctionNamespaces(ActionSink actions) throws Exception {
        FunctionIterator functions = currentProgram.getFunctionManager().getFunctions(true);
        while (functions.hasNext()) {
            monitor.checkCancelled();
            Function function = functions.next();
            ArrayList<String> scope = namespaceScope(function.getParentNamespace());
            ArrayList<String> normalizedScope = normalizeRepeatedScope(scope);
            if (scope.equals(normalizedScope)) {
                continue;
            }

            String localName = function.getName();
            String address = formatAddress(function.getEntryPoint());
            String targetName = fullName(normalizedScope, localName);
            JsonObject action = new JsonObject();
            action.addProperty("kind", "function_namespace_cleanup");
            action.addProperty("address", address);
            action.addProperty("oldName", function.getName(true));
            action.addProperty("name", targetName);
            action.add("oldScope", listStringArray(scope));
            action.add("scope", listStringArray(normalizedScope));

            if (normalizedScope.isEmpty()) {
                action.addProperty("applied", false);
                action.addProperty("reason", "empty-normalized-scope");
                actions.add(action);
                continue;
            }
            if (!reserveTargetName(targetName, address, action, actions)) {
                continue;
            }

            boolean applied = false;
            if (applyRenames && !function.getName(true).equals(targetName)) {
                applied = applyFunctionRename(function, normalizedScope, localName, action);
                if (!applied && action.has("reason")) {
                    action.addProperty("applied", false);
                    actions.add(action);
                    continue;
                }
            }
            action.addProperty("applied", applied);
            if (!applyRenames && !function.getName(true).equals(targetName)) {
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
            actions.add(action);
        }
    }

    private void processMbGetTypeNameFunctions(
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        Map<String, SlotGroup> groups = new HashMap<>();
        Set<String> seen = new HashSet<>();
        DataIterator iterator = currentProgram.getListing().getDefinedData(true);
        while (iterator.hasNext()) {
            monitor.checkCancelled();
            Data data = iterator.next();
            String text = dataStringValue(data);
            if (text == null || !text.contains("MB::GetTypeName<class ")) {
                continue;
            }

            Matcher matcher = MB_GET_TYPE_NAME_SIGNATURE_RE.matcher(text);
            while (matcher.find()) {
                String typeName = safeTypeName(matcher.group("type"));
                if (typeName == null) {
                    continue;
                }
                processMbGetTypeNameReferences(
                    data.getAddress(),
                    typeName,
                    groups,
                    seen,
                    functionSeen,
                    actions);
            }
        }
    }

    private void processMbGetTypeNameReferences(
        Address stringAddress,
        String typeName,
        Map<String, SlotGroup> groups,
        Set<String> seen,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(stringAddress);
        while (references.hasNext()) {
            monitor.checkCancelled();
            Reference reference = references.next();
            Function body = currentProgram.getFunctionManager()
                .getFunctionContaining(reference.getFromAddress());
            if (body == null) {
                continue;
            }

            Function target = preferredGetTypeNameFunction(body);
            String targetAddress = formatAddress(target.getEntryPoint());
            String key = targetAddress + "|" + typeName;
            if (!seen.add(key)) {
                continue;
            }

            ArrayList<String> scope = new ArrayList<>();
            scope.add("MB");
            renameFunction(
                "mb_get_type_name",
                targetAddress,
                scope,
                "GetTypeName<" + typeName + ">",
                groups,
                functionSeen,
                new HashSet<>(),
                actions);
        }
    }

    private String dataStringValue(Data data) {
        Object value = data.getValue();
        if (value instanceof String) {
            return (String)value;
        }
        return value == null ? null : value.toString();
    }

    private Map<String, ArrayList<Address>> definedStringAddressesByValue() {
        if (definedStringAddressesByValue != null) {
            return definedStringAddressesByValue;
        }

        definedStringAddressesByValue = new HashMap<>();
        DataIterator iterator = currentProgram.getListing().getDefinedData(true);
        while (iterator.hasNext()) {
            try {
                monitor.checkCancelled();
            }
            catch (CancelledException ignored) {
                break;
            }
            Data data = iterator.next();
            String text = dataStringValue(data);
            if (text == null || text.isEmpty() || text.length() > 512) {
                continue;
            }
            ArrayList<Address> addresses = definedStringAddressesByValue.get(text);
            if (addresses == null) {
                addresses = new ArrayList<>();
                definedStringAddressesByValue.put(text, addresses);
            }
            addresses.add(data.getAddress());
        }
        return definedStringAddressesByValue;
    }

    private void processCoreRttiCastHelpers(
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        Map<String, CoreRttiCastTarget> targets = coreRttiCastTargetsByTypeId();
        Set<String> seen = new HashSet<>();
        DataIterator iterator = currentProgram.getListing().getDefinedData(true);
        while (iterator.hasNext()) {
            monitor.checkCancelled();
            Data data = iterator.next();
            String text = dataStringValue(data);
            if (text == null) {
                continue;
            }

            Matcher matcher = UUID_RE.matcher(text);
            while (matcher.find()) {
                CoreRttiCastTarget target =
                    targets.get(normalizeTypeId(matcher.group()));
                if (target == null) {
                    continue;
                }
                processCoreRttiCastReferences(
                    data.getAddress(),
                    target,
                    seen,
                    functionSeen,
                    actions);
            }
        }
    }

    private Map<String, CoreRttiCastTarget> coreRttiCastTargetsByTypeId() {
        LinkedHashMap<String, CoreRttiCastTarget> result = new LinkedHashMap<>();
        for (CoreRttiCastTarget target : CORE_RTTI_CAST_TARGETS) {
            result.put(normalizeTypeId(target.typeId), target);
        }
        return result;
    }

    private void processCoreRttiCastReferences(
        Address stringAddress,
        CoreRttiCastTarget target,
        Set<String> seen,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(stringAddress);
        while (references.hasNext()) {
            monitor.checkCancelled();
            Reference reference = references.next();
            Function function = currentProgram.getFunctionManager()
                .getFunctionContaining(reference.getFromAddress());
            if (function == null) {
                continue;
            }

            String address = formatAddress(function.getEntryPoint());
            String key = addressKey(address) + "|" + target.typeId;
            if (!seen.add(key)) {
                continue;
            }
            if (!isCoreRttiCastHelper(function)) {
                continue;
            }

            renameCoreRttiCastHelper(function, target, functionSeen, actions);
        }
    }

    private boolean isCoreRttiCastHelper(Function function) throws Exception {
        long bodySize = function.getBody().getNumAddresses();
        if (bodySize <= 0 || bodySize > 0x200) {
            return false;
        }

        boolean hasCastSlotLoad = false;
        boolean hasIndirectTailJump = false;
        boolean hasNullReturn = false;
        Listing listing = currentProgram.getListing();
        Instruction instruction = listing.getInstructionAt(function.getEntryPoint());
        while (instruction != null &&
            function.getBody().contains(instruction.getMinAddress())) {

            byte[] bytes = instructionBytes(instruction);
            if (isRttiCastSlotLoad(bytes)) {
                hasCastSlotLoad = true;
            }
            if (isIndirectRegisterJump(bytes)) {
                hasIndirectTailJump = true;
            }
            if (isZeroReturn(bytes)) {
                hasNullReturn = true;
            }
            instruction = instruction.getNext();
        }

        return hasCastSlotLoad && hasIndirectTailJump && hasNullReturn;
    }

    private boolean isRttiCastSlotLoad(byte[] bytes) {
        return bytes.length == 4 &&
            (unsignedByte(bytes[0]) & 0xf8) == 0x48 &&
            unsignedByte(bytes[1]) == 0x8b &&
            (unsignedByte(bytes[2]) & 0xc0) == 0x40 &&
            unsignedByte(bytes[3]) == 0x20;
    }

    private boolean isIndirectRegisterJump(byte[] bytes) {
        return bytes.length == 3 &&
            (unsignedByte(bytes[0]) & 0xf8) == 0x48 &&
            unsignedByte(bytes[1]) == 0xff &&
            (unsignedByte(bytes[2]) & 0xf8) == 0xe0;
    }

    private boolean isZeroReturn(byte[] bytes) {
        return bytes.length >= 2 &&
            ((unsignedByte(bytes[0]) == 0x33 && unsignedByte(bytes[1]) == 0xc0) ||
                (unsignedByte(bytes[0]) == 0x31 && unsignedByte(bytes[1]) == 0xc0));
    }

    private void renameCoreRttiCastHelper(
        Function function,
        CoreRttiCastTarget target,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        String address = formatAddress(function.getEntryPoint());
        String functionKey = addressKey(address);
        if (functionSeen.contains(functionKey)) {
            return;
        }
        functionSeen.add(functionKey);

        ArrayList<String> scope = new ArrayList<>();
        scope.add("AZ");
        String localName = "RttiCast<" + target.typeName + ">";
        String targetName = fullName(scope, localName);
        JsonObject action = new JsonObject();
        action.addProperty("kind", "core_rtti_cast_helper");
        action.addProperty("address", address);
        action.addProperty("name", targetName);
        action.addProperty("targetType", "AZ::" + target.typeName);
        action.addProperty("targetTypeId", target.typeId);
        action.addProperty("oldName", function.getName(true));

        if (!reserveTargetName(targetName, address, action, actions)) {
            return;
        }

        boolean renamed = false;
        boolean prototypeApplied = false;
        if (applyRenames) {
            if (!function.getName(true).equals(targetName)) {
                renamed = applyFunctionRename(function, scope, localName, action);
                if (!renamed && action.has("reason")) {
                    action.addProperty("applied", false);
                    actions.add(action);
                    return;
                }
            }
            prototypeApplied = applyCoreRttiCastPrototype(function, target, action);
            if (!prototypeApplied && action.has("reason")) {
                action.addProperty("applied", renamed);
                actions.add(action);
                return;
            }
        }

        action.addProperty("applied", renamed || prototypeApplied);
        action.addProperty("prototypeApplied", prototypeApplied);
        if (!applyRenames && !function.getName(true).equals(targetName)) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private boolean applyCoreRttiCastPrototype(
        Function function,
        CoreRttiCastTarget target,
        JsonObject action) {

        DataType reflectContextPointer = coreReflectionPointerType("ReflectContext");
        DataType returnPointer = coreReflectionPointerType(target.typeName);
        if (reflectContextPointer == null || returnPointer == null) {
            action.addProperty("reason", "core-reflection-datatype-missing");
            return false;
        }

        try {
            boolean changed = false;
            if (!sameDataType(function.getReturnType(), returnPointer)) {
                function.setReturnType(returnPointer, SourceType.USER_DEFINED);
                changed = true;
            }
            changed |= replaceLeadingParameters(
                function,
                new String[] { "context" },
                new DataType[] { reflectContextPointer });
            if (!changed) {
                action.addProperty("reason", "already-current");
            }
            return changed;
        }
        catch (Exception error) {
            action.addProperty("reason", "core-rtti-cast-prototype-failed");
            action.addProperty("error", error.getMessage());
            return false;
        }
    }

    private Function preferredGetTypeNameFunction(Function body) {
        Function thunk = singleThunkTo(body);
        return thunk == null ? body : thunk;
    }

    private Function singleThunkTo(Function target) {
        Function result = null;
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(target.getEntryPoint());
        while (references.hasNext()) {
            Reference reference = references.next();
            Function candidate = currentProgram.getFunctionManager()
                .getFunctionContaining(reference.getFromAddress());
            if (candidate == null || candidate.getEntryPoint().equals(target.getEntryPoint())) {
                continue;
            }
            if (!isDirectThunkTo(candidate, target.getEntryPoint())) {
                continue;
            }
            if (result != null && !result.getEntryPoint().equals(candidate.getEntryPoint())) {
                return null;
            }
            result = candidate;
        }
        return result;
    }

    private boolean isDirectThunkTo(Function candidate, Address target) {
        Instruction instruction = currentProgram.getListing()
            .getInstructionAt(candidate.getEntryPoint());
        if (instruction == null) {
            return false;
        }
        Address[] flows = instruction.getFlows();
        return flows != null && flows.length == 1 && flows[0].equals(target);
    }

    private void processModuleDescriptorRenames(ActionSink actions) throws Exception {
        if (moduleEvidence == null || moduleEvidence.inputs.isEmpty()) {
            return;
        }

        Map<String, SlotGroup> slotGroups =
            moduleDescriptorSlotGroups(moduleEvidence.descriptors);
        Set<String> functionSeen = new HashSet<>();
        Set<String> aliasSeen = new HashSet<>();
        ambiguousModuleComponentNames =
            moduleDescriptorAmbiguousComponentNames(moduleEvidence.descriptors);

        monitor.initialize(moduleEvidence.descriptorCount);
        for (ModuleCaptureInput input : moduleEvidence.inputs) {
            currentModuleSource = input.file.getName();
            currentModuleName = input.moduleName;
            if (input.module.descriptors == null) {
                continue;
            }
            for (Descriptor descriptor : input.module.descriptors) {
                monitor.setMessage("Renaming module descriptors: " + currentModuleSource);
                processModuleDescriptor(
                    descriptor,
                    slotGroups,
                    functionSeen,
                    aliasSeen,
                    actions);
                monitor.incrementProgress(1);
                if (monitor.isCancelled()) {
                    break;
                }
            }
            if (monitor.isCancelled()) {
                break;
            }
        }
        currentModuleSource = null;
        currentModuleName = null;
    }

    private void processModuleDescriptor(
        Descriptor descriptor,
        Map<String, SlotGroup> slotGroups,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (currentModuleName == null) {
            JsonObject action = new JsonObject();
            action.addProperty("kind", "module_descriptor");
            action.addProperty("address", descriptor.addr);
            addModuleDescriptorEvidence(action);
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-module-name");
            actions.add(action);
            return;
        }

        DescriptorNames names = moduleDescriptorNames(descriptor);
        if (names == null) {
            JsonObject action = new JsonObject();
            action.addProperty("kind", "module_descriptor");
            action.addProperty("address", descriptor.addr);
            addModuleDescriptorEvidence(action);
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-component-name");
            actions.add(action);
            return;
        }

        renameModuleDescriptorLabel(descriptor.addr, names.scope, "descriptor", actions);
        renameModuleDescriptorLabel(descriptor.vftable, names.scope, "vftable", actions);

        if (descriptor.vtableSlots != null) {
            for (VTableSlot slot : descriptor.vtableSlots) {
                if (slot.expected == null || slot.address == null) {
                    continue;
                }
                renameModuleDescriptorFunction(
                    descriptor,
                    names,
                    slot.address,
                    names.scope,
                    slot.expected,
                    slotGroups,
                    functionSeen,
                    aliasSeen,
                    actions);
            }
        }

        processModuleDescriptorInstanceVtables(descriptor, names, actions);
    }

    private void processModuleDescriptorInstanceVtables(
        Descriptor descriptor,
        DescriptorNames names,
        ActionSink actions) throws Exception {

        VTableSlot createSlot = moduleDescriptorSlot(descriptor, "CreateComponent");
        if (createSlot == null || createSlot.address == null) {
            return;
        }

        Address createAddress = parseCaptureAddress(createSlot.address);
        if (!isProgramAddress(createAddress)) {
            addModuleDescriptorInstanceVtableSkipped(
                descriptor,
                createSlot.address,
                null,
                null,
                "create-component-address-not-in-program",
                actions);
            return;
        }

        Function createFunction = currentProgram.getFunctionManager()
            .getFunctionAt(createAddress);
        if (createFunction == null) {
            addModuleDescriptorInstanceVtableSkipped(
                descriptor,
                createSlot.address,
                null,
                null,
                "create-component-function-missing",
                actions);
            return;
        }

        Set<String> seenVtables = new HashSet<>();
        int appliedCandidates = 0;

        List<InstanceVtableCandidate> directCandidates =
            instanceVtableCandidates(createFunction, MAX_CONSTRUCTOR_VTABLE_CANDIDATES);
        for (InstanceVtableCandidate candidate : directCandidates) {
            String key = candidate.vtableAddress.toString() + "|" + candidate.vptrOffset;
            if (!seenVtables.add(key)) {
                continue;
            }
            renameModuleDescriptorInstanceVtable(
                descriptor,
                names,
                createSlot.address,
                createFunction,
                candidate,
                actions);
            appliedCandidates++;
        }

        List<Function> constructors = Collections.emptyList();
        if (appliedCandidates == 0) {
            constructors = calledFunctions(createFunction, MAX_CREATE_COMPONENT_CALLS);
        }
        for (Function constructor : constructors) {
            List<InstanceVtableCandidate> candidates =
                instanceVtableCandidates(constructor, MAX_CONSTRUCTOR_VTABLE_CANDIDATES);
            for (InstanceVtableCandidate candidate : candidates) {
                String key = candidate.vtableAddress.toString() + "|" + candidate.vptrOffset;
                if (!seenVtables.add(key)) {
                    continue;
                }
                renameModuleDescriptorInstanceVtable(
                    descriptor,
                    names,
                    createSlot.address,
                    constructor,
                    candidate,
                    actions);
                appliedCandidates++;
            }
        }

        if (appliedCandidates == 0) {
            addModuleDescriptorInstanceVtableSkipped(
                descriptor,
                createSlot.address,
                null,
                null,
                directCandidates.isEmpty() && constructors.isEmpty()
                    ? "create-component-constructor-call-missing"
                    : "instance-vtable-candidate-missing",
                actions);
        }
    }

    private VTableSlot moduleDescriptorSlot(Descriptor descriptor, String expected) {
        if (descriptor.vtableSlots == null) {
            return null;
        }
        for (VTableSlot slot : descriptor.vtableSlots) {
            if (expected.equals(slot.expected)) {
                return slot;
            }
        }
        return null;
    }

    private List<Function> calledFunctions(Function function, int limit) {
        ArrayList<Function> result = new ArrayList<>();
        Set<String> seen = new HashSet<>();
        if (function == null || function.getBody() == null) {
            return result;
        }
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(function.getBody(), true)) {
            for (Reference reference : instruction.getReferencesFrom()) {
                if (!reference.getReferenceType().isCall()) {
                    continue;
                }
                Address target = reference.getToAddress();
                if (!isProgramAddress(target)) {
                    continue;
                }
                Function called = currentProgram.getFunctionManager().getFunctionAt(target);
                if (called == null || !seen.add(called.getEntryPoint().toString())) {
                    continue;
                }
                result.add(called);
                if (result.size() >= limit) {
                    return result;
                }
            }
        }
        return result;
    }

    private List<InstanceVtableCandidate> instanceVtableCandidates(
        Function constructor,
        int limit) {

        ArrayList<InstanceVtableCandidate> result = new ArrayList<>();
        Set<String> seen = new HashSet<>();
        if (constructor == null || constructor.getBody() == null) {
            return result;
        }
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(constructor.getBody(), true)) {
            for (Reference reference : instruction.getReferencesFrom()) {
                if (reference.getReferenceType().isCall()) {
                    continue;
                }
                Address target = reference.getToAddress();
                if (!isProgramAddress(target) || isExecutableAddress(target) ||
                    !isVtableLike(target)) {
                    continue;
                }
                Integer vptrOffset = vptrStoreOffsetAfter(constructor, instruction);
                if (vptrOffset == null) {
                    continue;
                }
                String key = target.toString() + "|" + vptrOffset;
                if (!seen.add(key)) {
                    continue;
                }
                InstanceVtableCandidate candidate = new InstanceVtableCandidate();
                candidate.vtableAddress = target;
                candidate.sourceInstruction = instruction.getAddress();
                candidate.vptrOffset = vptrOffset;
                result.add(candidate);
                if (result.size() >= limit) {
                    return result;
                }
            }
        }
        return result;
    }

    private Integer vptrStoreOffsetAfter(Function function, Instruction loadInstruction) {
        Instruction instruction = loadInstruction;
        for (int i = 0; i < MAX_VPTR_STORE_LOOKAHEAD; i++) {
            instruction = instruction.getNext();
            if (instruction == null) {
                return null;
            }
            if (!function.getBody().contains(instruction.getAddress())) {
                return null;
            }
            Integer offset = vptrStoreOffset(instruction);
            if (offset != null) {
                return offset;
            }
            if (isRipRelativeLea(instruction)) {
                return null;
            }
        }
        return null;
    }

    private Integer vptrStoreOffset(Instruction instruction) {
        byte[] bytes = instructionBytes(instruction);
        if (bytes.length == 3 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x89 &&
            (unsignedByte(bytes[2]) == 0x03 || unsignedByte(bytes[2]) == 0x01)) {
            return 0;
        }
        if (bytes.length == 4 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x89 &&
            (unsignedByte(bytes[2]) == 0x43 || unsignedByte(bytes[2]) == 0x41)) {
            return signedByte(bytes[3]);
        }
        if (bytes.length == 7 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x89 &&
            (unsignedByte(bytes[2]) == 0x83 || unsignedByte(bytes[2]) == 0x81)) {
            return int32(bytes, 3);
        }
        return null;
    }

    private boolean isRipRelativeLea(Instruction instruction) {
        byte[] bytes = instructionBytes(instruction);
        return bytes.length >= 7 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x8d &&
            unsignedByte(bytes[2]) == 0x05;
    }

    private byte[] instructionBytes(Instruction instruction) {
        try {
            return instruction.getBytes();
        }
        catch (Exception ignored) {
            return new byte[0];
        }
    }

    private boolean isVtableLike(Address address) {
        int executableSlots = 0;
        for (int slot = 0; slot < 4; slot++) {
            Address target = readPointer(address.add(slot * 8L));
            if (target != null && isExecutableAddress(target)) {
                executableSlots++;
            }
        }
        return executableSlots > 0;
    }

    private void renameModuleDescriptorInstanceVtable(
        Descriptor descriptor,
        DescriptorNames names,
        String createComponentAddress,
        Function constructor,
        InstanceVtableCandidate candidate,
        ActionSink actions) throws Exception {

        String localName = candidate.vptrOffset == 0
            ? "vftable"
            : "vftable_at_0x" + Integer.toHexString(candidate.vptrOffset);
        List<String> scope = moduleDescriptorComponentScope(descriptor, names);
        String targetName = fullName(scope, localName);
        String jsonAddress = formatAddress(candidate.vtableAddress);

        JsonObject action = new JsonObject();
        action.addProperty("kind", "module_descriptor_instance_vftable");
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);
        action.addProperty("componentName", descriptorComponentName(descriptor));
        action.addProperty("componentUuid", descriptor.componentUuid);
        action.addProperty("createComponent", createComponentAddress);
        action.addProperty("constructor", formatAddress(constructor.getEntryPoint()));
        action.addProperty("sourceInstruction", formatAddress(candidate.sourceInstruction));
        action.addProperty("vptrOffset", "0x" + Integer.toHexString(candidate.vptrOffset));
        addModuleDescriptorEvidence(action);
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(candidate.vtableAddress));
        }
        boolean applied = false;
        if (applyRenames) {
            applied = ensurePrimaryLabel(candidate.vtableAddress, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private void addModuleDescriptorInstanceVtableSkipped(
        Descriptor descriptor,
        String createComponentAddress,
        Function constructor,
        InstanceVtableCandidate candidate,
        String reason,
        ActionSink actions) {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "module_descriptor_instance_vftable");
        action.addProperty("componentName", descriptorComponentName(descriptor));
        action.addProperty("componentUuid", descriptor.componentUuid);
        action.addProperty("createComponent", createComponentAddress);
        if (constructor != null) {
            action.addProperty("constructor", formatAddress(constructor.getEntryPoint()));
        }
        if (candidate != null) {
            action.addProperty("address", formatAddress(candidate.vtableAddress));
            action.addProperty("sourceInstruction", formatAddress(candidate.sourceInstruction));
        }
        addModuleDescriptorEvidence(action);
        action.addProperty("applied", false);
        action.addProperty("reason", reason);
        actions.add(action);
    }

    private List<String> moduleDescriptorComponentScope(
        Descriptor descriptor,
        DescriptorNames names) {

        ArrayList<String> scope = new ArrayList<>();
        if (!isAggregateModuleName(currentModuleName)) {
            scope.add(currentModuleName);
        }
        String componentType = descriptorComponentName(descriptor);
        if (componentType != null) {
            scope.add(componentType);
        }
        else {
            scope.addAll(names.scope);
        }
        return scope;
    }

    private DescriptorNames moduleDescriptorNames(Descriptor descriptor) {
        String componentType = descriptorComponentName(descriptor);
        if (componentType == null || componentType.isEmpty()) {
            return null;
        }
        DescriptorNames result = new DescriptorNames();
        result.scope = new ArrayList<>();
        if (!isAggregateModuleName(currentModuleName)) {
            result.scope.add(currentModuleName);
        }
        result.scope.add("AZ");
        result.scope.add("ComponentDescriptorDefault<" + componentType + ">");
        return result;
    }

    private void renameModuleDescriptorFunction(
        Descriptor descriptor,
        DescriptorNames names,
        String jsonAddress,
        List<String> scope,
        String localName,
        Map<String, SlotGroup> slotGroups,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        SlotGroup group = slotGroups.get(addressKey(jsonAddress));
        if (group == null) {
            group = new SlotGroup(jsonAddress);
            group.slotNames.add(localName);
            group.useCount = 1;
        }

        if ("Reflect".equals(localName)) {
            Address reflectAddress = parseCaptureAddress(jsonAddress);
            Function reflectFunction = isProgramAddress(reflectAddress)
                ? currentProgram.getFunctionManager().getFunctionAt(reflectAddress)
                : null;
            processModuleDescriptorReflectThunkTarget(
                descriptor,
                names,
                reflectFunction,
                functionSeen,
                actions);
        }

        boolean shared = group.useCount != 1;
        BaseDescriptorName baseName = shared ? moduleDescriptorBaseName(group) : null;
        if (shared && baseName == null) {
            addModuleDescriptorFunctionAlias(
                jsonAddress,
                scope,
                localName,
                group,
                aliasSeen,
                actions);
            return;
        }

        List<String> targetScope = baseName != null ? baseName.scope : scope;
        String targetLocal = baseName != null ? baseName.localName : localName;
        String targetName = fullName(targetScope, targetLocal);

        String functionKey = addressKey(jsonAddress);
        if (functionSeen.contains(functionKey)) {
            if ("Reflect".equals(targetLocal)) {
                Address reflectAddress = parseCaptureAddress(jsonAddress);
                Function function = isProgramAddress(reflectAddress)
                    ? currentProgram.getFunctionManager().getFunctionAt(reflectAddress)
                    : null;
                processModuleDescriptorReflectThunkTarget(
                    descriptor,
                    names,
                    function,
                    functionSeen,
                    actions);
            }
            return;
        }
        functionSeen.add(functionKey);

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = moduleDescriptorFunctionAction(
            "module_descriptor_function",
            jsonAddress,
            targetName,
            group,
            shared);
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        boolean created = false;
        if (function == null && applyRenames) {
            function = createMissingFunction(address, targetLocal, action);
            created = function != null;
        }
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty(
                "reason",
                applyRenames ? "function-create-failed" : "function-missing");
            actions.add(action);
            return;
        }

        action.addProperty("oldName", function.getName(true));
        action.addProperty("created", created);
        boolean applied = false;
        if (applyRenames && !function.getName(true).equals(targetName)) {
            applied = applyFunctionRename(function, targetScope, targetLocal, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !function.getName(true).equals(targetName)) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
        if ("Reflect".equals(targetLocal)) {
            applyDescriptorReflectParameters(function, jsonAddress, targetName, actions);
            processModuleDescriptorReflectThunkTarget(
                descriptor,
                names,
                function,
                functionSeen,
                actions);
        }
    }

    private void processModuleDescriptorReflectThunkTarget(
        Descriptor descriptor,
        DescriptorNames names,
        Function descriptorReflect,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        if (descriptorReflect != null) {
            action.addProperty("thunkAddress", formatAddress(descriptorReflect.getEntryPoint()));
        }
        action.addProperty("componentName", descriptorComponentName(descriptor));
        action.addProperty("source", "module-descriptor-reflect-thunk");
        addModuleDescriptorEvidence(action);

        if (descriptorReflect == null) {
            action.addProperty("kind", "module_descriptor_reflect_thunk_target");
            action.addProperty("applied", false);
            action.addProperty("reason", "reflect-function-missing");
            actions.add(action);
            return;
        }

        Address targetAddress = moduleDescriptorReflectThunkTarget(descriptorReflect);
        if (!isProgramAddress(targetAddress)) {
            action.addProperty("kind", "module_descriptor_reflect_thunk_target");
            action.addProperty("applied", false);
            action.addProperty("reason", "thunk-target-not-detected");
            actions.add(action);
            return;
        }

        ArrayList<String> scope =
            new ArrayList<>(moduleDescriptorComponentScope(descriptor, names));
        String componentName = descriptorComponentName(descriptor);
        String address = formatAddress(targetAddress);

        Function target = currentProgram.getFunctionManager().getFunctionAt(targetAddress);
        if (target == null && applyRenames) {
            target = createMissingFunction(targetAddress, "Reflect", action);
        }
        if (target == null) {
            action.addProperty("kind", "module_descriptor_reflect_thunk_target");
            action.addProperty("address", address);
            action.addProperty("name", fullName(scope, "Reflect"));
            action.addProperty("applied", false);
            action.addProperty(
                "reason",
                applyRenames ? "function-create-failed" : "function-missing");
            if (!applyRenames) {
                action.addProperty("wouldApply", true);
            }
            actions.add(action);
            return;
        }

        String seenKey = "module_descriptor_reflect_thunk_target|" + addressKey(address);
        if (functionSeen.contains(seenKey)) {
            return;
        }
        functionSeen.add(seenKey);

        String targetName = fullName(scope, "Reflect");
        action.addProperty("kind", "module_descriptor_reflect_thunk_target");
        action.addProperty("address", address);
        action.addProperty("name", targetName);
        if (!reserveTargetName(targetName, address, action, actions)) {
            return;
        }
        action.addProperty("oldName", target.getName(true));
        boolean applied = false;
        if (applyRenames && !target.getName(true).equals(targetName)) {
            applied = applyFunctionRename(target, scope, "Reflect", action);
            if (!applied && action.has("reason")) {
                action.addProperty("applied", false);
                actions.add(action);
                return;
            }
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !target.getName(true).equals(targetName)) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);

        applyStaticReflectContextParameter(
            address,
            descriptor.componentUuid,
            componentName,
            scope,
            actions);
    }

    private Address moduleDescriptorReflectThunkTarget(Function function) throws Exception {
        if (function == null || function.getBody().getNumAddresses() > 0x20) {
            return null;
        }

        Listing listing = currentProgram.getListing();
        Instruction first = listing.getInstructionAt(function.getEntryPoint());
        if (first == null || !isMovRcxRdx(first)) {
            return null;
        }

        Instruction second = first.getNext();
        if (second == null || !function.getBody().contains(second.getMinAddress())) {
            return null;
        }
        Instruction third = second.getNext();
        if (third != null && function.getBody().contains(third.getMinAddress())) {
            return null;
        }
        if (!isDirectJump(second)) {
            return null;
        }

        Address[] flows = second.getFlows();
        if (flows == null || flows.length != 1 || !isExecutableAddress(flows[0])) {
            return null;
        }
        return flows[0];
    }

    private boolean isMovRcxRdx(Instruction instruction) {
        byte[] bytes = instructionBytes(instruction);
        return bytes.length == 3 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x8b &&
            unsignedByte(bytes[2]) == 0xca;
    }

    private boolean isDirectJump(Instruction instruction) {
        byte[] bytes = instructionBytes(instruction);
        return bytes.length >= 5 &&
            unsignedByte(bytes[0]) == 0xe9 &&
            instruction.getFlowType().isJump();
    }

    private void addModuleDescriptorFunctionAlias(
        String jsonAddress,
        List<String> scope,
        String localName,
        SlotGroup group,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        String targetName = fullName(scope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (aliasSeen.contains(key)) {
            return;
        }
        aliasSeen.add(key);

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = moduleDescriptorFunctionAction(
            "module_descriptor_function_alias",
            jsonAddress,
            targetName,
            group,
            true);
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(address));
        }
        boolean applied = false;
        if (applyRenames) {
            Function function = currentProgram.getFunctionManager().getFunctionAt(address);
            boolean created = false;
            if (function == null) {
                function = createMissingFunction(address, localName, action);
                created = function != null;
            }
            action.addProperty("created", created);
            if (function == null) {
                action.addProperty("applied", false);
                action.addProperty("reason", "function-create-failed");
                actions.add(action);
                return;
            }
            applied = ensurePrimaryLabel(address, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private JsonObject moduleDescriptorFunctionAction(
        String kind,
        String jsonAddress,
        String targetName,
        SlotGroup group,
        boolean shared) {

        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);
        addModuleDescriptorEvidence(action);
        action.addProperty("shared", shared);
        action.addProperty("useCount", group.useCount);
        JsonArray slots = new JsonArray();
        for (String slotName : group.slotNames) {
            slots.add(slotName);
        }
        action.add("slotNames", slots);
        return action;
    }

    private void renameModuleDescriptorLabel(
        String jsonAddress,
        List<String> scope,
        String localName,
        ActionSink actions) throws Exception {

        String targetName = fullName(scope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (labelSeen.contains(key)) {
            return;
        }
        labelSeen.add(key);

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = new JsonObject();
        action.addProperty("kind", "module_descriptor_label");
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);
        addModuleDescriptorEvidence(action);

        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(address));
        }
        boolean applied = false;
        if (applyRenames) {
            applied = ensurePrimaryLabel(address, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private Map<String, SlotGroup> moduleDescriptorSlotGroups(List<Descriptor> descriptors) {
        Map<String, SlotGroup> groups = new HashMap<>();
        for (Descriptor descriptor : descriptors) {
            if (descriptor.vtableSlots == null) {
                continue;
            }
            for (VTableSlot slot : descriptor.vtableSlots) {
                if (slot.address == null || slot.expected == null) {
                    continue;
                }
                String key = addressKey(slot.address);
                SlotGroup group = groups.get(key);
                if (group == null) {
                    group = new SlotGroup(slot.address);
                    groups.put(key, group);
                }
                group.slotNames.add(slot.expected);
                group.useCount++;
            }
        }
        return groups;
    }

    private Set<String> moduleDescriptorAmbiguousComponentNames(List<Descriptor> descriptors) {
        Map<String, Set<String>> idsByName = new HashMap<>();
        for (Descriptor descriptor : descriptors) {
            String name = descriptorComponentName(descriptor);
            if (name == null) {
                continue;
            }
            String id = descriptor.componentUuid;
            if (id == null || id.trim().isEmpty()) {
                id = "address:" + descriptor.addr;
            }
            Set<String> ids = idsByName.get(name);
            if (ids == null) {
                ids = new HashSet<>();
                idsByName.put(name, ids);
            }
            ids.add(id.toUpperCase(Locale.ROOT));
        }

        Set<String> ambiguous = new TreeSet<>();
        for (Map.Entry<String, Set<String>> entry : idsByName.entrySet()) {
            if (entry.getValue().size() > 1) {
                ambiguous.add(entry.getKey());
            }
        }
        return ambiguous;
    }

    private BaseDescriptorName moduleDescriptorBaseName(SlotGroup group) {
        if (group.slotNames.size() != 1) {
            return null;
        }
        String slotName = group.slotNames.iterator().next();
        if (!slotName.equals("GetDescriptor") &&
            !slotName.equals("ReleaseDescriptor") &&
            !slotName.equals("~ComponentDescriptor")) {
            return null;
        }
        BaseDescriptorName result = new BaseDescriptorName();
        result.scope = new ArrayList<>();
        result.scope.add("AZ");
        result.scope.add("ComponentDescriptor");
        result.localName = slotName;
        return result;
    }

    private void addModuleDescriptorEvidence(JsonObject action) {
        if (currentModuleSource != null) {
            action.addProperty("source", currentModuleSource);
        }
        if (currentModuleName != null) {
            action.addProperty("module", currentModuleName);
        }
    }

    private void processReflectedDatatypes(
        List<ClassData> classData,
        ActionSink actions) throws Exception {

        List<ClassData> canonicalClassData = canonicalClassDataByTypeId(classData);
        LinkedHashMap<String, Structure> declaredStructures = new LinkedHashMap<>();
        cleanupStaleNestedReflectedDatatypes(canonicalClassData, actions);
        if (applyRenames) {
            for (ClassData data : canonicalClassData) {
                String typeId = normalizeTypeId(data.typeId);
                if (typeId == null || declaredStructures.containsKey(typeId)) {
                    continue;
                }
                Structure structure = declareReflectedDatatype(data);
                if (structure != null) {
                    declaredStructures.put(typeId, structure);
                }
                if (monitor.isCancelled()) {
                    return;
                }
            }
        }

        Map<String, Structure> previousDeclaredStructures = declaredStructuresByTypeId;
        declaredStructuresByTypeId = declaredStructures;
        try {
            for (ClassData data : canonicalClassData) {
                processReflectedDatatype(data, actions);
                if (monitor.isCancelled()) {
                    return;
                }
            }
        }
        finally {
            declaredStructuresByTypeId = previousDeclaredStructures;
        }

        processMemberFunctionThisParameters(classData, declaredStructures, actions);
    }

    private void ensureCoreReflectionDatatypes(ActionSink actions) throws Exception {
        ensureCoreReflectionDatatype("ReflectContext", 0x10, null, actions);
        ensureCoreReflectionDatatype("SerializeContext", 0x10, "ReflectContext", actions);
        ensureCoreReflectionDatatype("BehaviorContext", 0x10, "ReflectContext", actions);
        ensureCoreReflectionDatatype("ComponentDescriptor", 0x8, null, actions);
    }

    private void ensureCoreReflectionDatatype(
        String name,
        int length,
        String baseName,
        ActionSink actions) throws Exception {

        CategoryPath categoryPath = new CategoryPath(CategoryPath.ROOT, "AZ");
        JsonObject action = new JsonObject();
        action.addProperty("kind", "core_reflection_datatype");
        action.addProperty("name", "AZ::" + name);
        action.addProperty("datatypePath", categoryPath.getPath());
        action.addProperty("length", length);
        if (baseName != null) {
            action.addProperty("base", "AZ::" + baseName);
        }

        DataType existing = dataTypeManager.getDataType(categoryPath, name);
        if (existing instanceof Structure && !isCoreReflectionDatatype(existing)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "existing-user-datatype");
            actions.add(action);
            return;
        }
        if (existing != null && !(existing instanceof Structure)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "existing-non-structure-datatype");
            actions.add(action);
            return;
        }

        Structure structure = coreReflectionStructure(name, length, baseName);
        if (existing instanceof Structure &&
            isCoreReflectionDatatype(existing) &&
            sameStructure((Structure) existing, structure)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "already-current");
            actions.add(action);
            return;
        }
        if (!applyRenames) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
            actions.add(action);
            return;
        }

        dataTypeManager.addDataType(
            structure,
            existing == null
                ? DataTypeConflictHandler.DEFAULT_HANDLER
                : DataTypeConflictHandler.REPLACE_HANDLER);
        action.addProperty("applied", true);
        actions.add(action);
    }

    private Structure coreReflectionStructure(
        String name,
        int length,
        String baseName) {

        CategoryPath categoryPath = new CategoryPath(CategoryPath.ROOT, "AZ");
        Structure structure =
            new StructureDataType(categoryPath, name, length, dataTypeManager);
        structure.setDescription(CORE_REFLECTION_DATATYPE_DESCRIPTION_PREFIX +
            ": AZ::" + name);

        if (baseName != null) {
            Structure base = coreReflectionDatatype(baseName);
            if (base != null && base.getLength() > 0 && base.getLength() <= length) {
                structure.replaceAtOffset(
                    0,
                    base,
                    base.getLength(),
                    "base",
                    "base class AZ::" + baseName);
                return structure;
            }
        }

        structure.replaceAtOffset(
            0,
            new PointerDataType(VoidDataType.dataType, dataTypeManager),
            currentProgram.getDefaultPointerSize(),
            "vftable",
            null);
        if ("ReflectContext".equals(name) && length > currentProgram.getDefaultPointerSize()) {
            structure.replaceAtOffset(
                currentProgram.getDefaultPointerSize(),
                new BooleanDataType(),
                1,
                "m_isRemoveReflection",
                "AZ::ReflectContext field from Lumberyard source");
        }
        return structure;
    }

    private boolean isCoreReflectionDatatype(DataType dataType) {
        String description = dataType.getDescription();
        return description != null &&
            description.startsWith(CORE_REFLECTION_DATATYPE_DESCRIPTION_PREFIX);
    }

    private Structure coreReflectionDatatype(String name) {
        DataType dataType = dataTypeManager.getDataType(
            new CategoryPath(CategoryPath.ROOT, "AZ"),
            name);
        return dataType instanceof Structure ? (Structure) dataType : null;
    }

    private DataType coreReflectionPointerType(String name) {
        Structure structure = coreReflectionDatatype(name);
        if (structure == null) {
            return null;
        }
        return new PointerDataType(structure, dataTypeManager);
    }

    private void processStaticReflectFunctionParameters(
        List<ClassData> classData,
        ActionSink actions) throws Exception {

        LinkedHashSet<String> seen = new LinkedHashSet<>();
        for (ClassData data : canonicalClassDataByTypeId(classData)) {
            String address = data.staticReflectFunctionAddress;
            if (!isAddressLike(address) || !seen.add(addressKey(address))) {
                continue;
            }
            applyStaticReflectContextParameter(
                address,
                data.typeId,
                data.targetTypeName,
                reflectedTypeScope(data.typeId, data.targetTypeName),
                actions);
        }
    }

    private void applyStaticReflectContextParameter(
        String address,
        String ownerTypeId,
        String ownerTypeName,
        List<String> scope,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "reflect_context_parameter");
        action.addProperty("address", address);
        action.addProperty("ownerTypeId", ownerTypeId);
        action.addProperty("ownerTypeName", ownerTypeName);
        action.addProperty("name", fullName(scope, "Reflect"));

        Function function = functionAtCaptureAddress(address, action);
        if (function == null) {
            actions.add(action);
            return;
        }
        action.addProperty("function", function.getName(true));

        if (!applyRenames) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
            actions.add(action);
            return;
        }

        DataType reflectContextPointer = coreReflectionPointerType("ReflectContext");
        if (reflectContextPointer == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "reflect-context-datatype-missing");
            actions.add(action);
            return;
        }

        try {
            boolean changed = replaceLeadingParameters(
                function,
                new String[] { "reflection" },
                new DataType[] { reflectContextPointer });
            action.addProperty("applied", changed);
            if (!changed) {
                action.addProperty("reason", "already-current");
            }
        }
        catch (Exception error) {
            action.addProperty("applied", false);
            action.addProperty("reason", "reflect-context-parameter-failed");
            action.addProperty("error", error.getMessage());
        }
        actions.add(action);
    }

    private void applyDescriptorReflectParameters(
        Function function,
        String address,
        String targetName,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "descriptor_reflect_parameter");
        action.addProperty("address", address);
        action.addProperty("name", targetName);
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "function-missing");
            actions.add(action);
            return;
        }
        action.addProperty("function", function.getName(true));

        if (!applyRenames) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
            actions.add(action);
            return;
        }

        DataType descriptorPointer = coreReflectionPointerType("ComponentDescriptor");
        DataType reflectContextPointer = coreReflectionPointerType("ReflectContext");
        if (descriptorPointer == null || reflectContextPointer == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "core-reflection-datatype-missing");
            actions.add(action);
            return;
        }

        try {
            boolean changed = replaceLeadingParameters(
                function,
                new String[] { "this", "reflection" },
                new DataType[] { descriptorPointer, reflectContextPointer });
            action.addProperty("applied", changed);
            if (!changed) {
                action.addProperty("reason", "already-current");
            }
        }
        catch (Exception error) {
            action.addProperty("applied", false);
            action.addProperty("reason", "descriptor-reflect-parameter-failed");
            action.addProperty("error", error.getMessage());
        }
        actions.add(action);
    }

    private Function functionAtCaptureAddress(String address, JsonObject action) {
        Address parsed = parseCaptureAddress(address);
        if (!isProgramAddress(parsed)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            return null;
        }
        Function function = currentProgram.getFunctionManager().getFunctionAt(parsed);
        if (function != null) {
            return function;
        }
        function = currentProgram.getFunctionManager().getFunctionContaining(parsed);
        if (function != null && function.getEntryPoint().equals(parsed)) {
            return function;
        }
        action.addProperty("applied", false);
        action.addProperty("reason", "function-missing");
        return null;
    }

    private boolean replaceLeadingParameters(
        Function function,
        String[] names,
        DataType[] types) throws Exception {

        Parameter[] existing = function.getParameters();
        boolean alreadyCurrent = existing.length >= names.length;
        if (alreadyCurrent) {
            for (int i = 0; i < names.length; i++) {
                if (!names[i].equals(existing[i].getName()) ||
                    !sameDataType(existing[i].getDataType(), types[i])) {
                    alreadyCurrent = false;
                    break;
                }
            }
        }
        if (alreadyCurrent) {
            return false;
        }

        int length = Math.max(existing.length, names.length);
        Parameter[] updated = new Parameter[length];
        for (int i = 0; i < length; i++) {
            if (i < names.length) {
                updated[i] = new ParameterImpl(names[i], types[i], currentProgram);
            }
            else {
                updated[i] = existing[i];
            }
        }
        function.replaceParameters(
            FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS,
            true,
            SourceType.USER_DEFINED,
            updated);
        return true;
    }

    private void cleanupStaleNestedReflectedDatatypes(
        List<ClassData> canonicalClassData,
        ActionSink actions) throws Exception {

        LinkedHashSet<String> reflectedTypeNames = new LinkedHashSet<>();
        LinkedHashSet<String> canonicalPaths = new LinkedHashSet<>();
        for (ClassData data : canonicalClassData) {
            DatatypeTarget target = datatypeTarget(data);
            if (target == null) {
                continue;
            }
            reflectedTypeNames.add(target.name);
            canonicalPaths.add(datatypePathName(target));
        }

        Iterator<DataType> iterator = dataTypeManager.getAllDataTypes();
        ArrayList<DataType> staleDatatypes = new ArrayList<>();
        LinkedHashMap<String, CategoryPath> staleCategoryPaths = new LinkedHashMap<>();
        rememberConflictingDatatypeCategories(staleCategoryPaths, canonicalPaths);
        while (iterator.hasNext()) {
            DataType dataType = iterator.next();
            if (!(dataType instanceof Structure)) {
                continue;
            }
            if (!reflectedTypeNames.contains(dataType.getName()) ||
                !isGeneratedReflectedDatatype(dataType)) {
                continue;
            }
            if (canonicalPaths.contains(dataType.getPathName())) {
                continue;
            }
            staleDatatypes.add(dataType);
            rememberCategoryPathAndAncestors(staleCategoryPaths, dataType.getCategoryPath());
        }

        for (DataType dataType : staleDatatypes) {
            JsonObject action = new JsonObject();
            action.addProperty("kind", "stale_datatype_cleanup");
            action.addProperty("name", dataType.getName());
            action.addProperty("datatypePath", dataType.getPathName());
            if (!applyRenames) {
                action.addProperty("applied", false);
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
                actions.add(action);
                continue;
            }
            try {
                dataTypeManager.remove(Collections.singletonList(dataType), monitor);
                action.addProperty("applied", true);
            }
            catch (Exception error) {
                action.addProperty("applied", false);
                action.addProperty("reason", "remove-failed");
                action.addProperty("error", error.getMessage());
            }
            actions.add(action);
        }

        cleanupEmptyStaleCategories(staleCategoryPaths, actions);
    }

    private void rememberConflictingDatatypeCategories(
        LinkedHashMap<String, CategoryPath> paths,
        Set<String> canonicalDatatypePaths) {

        for (String pathName : canonicalDatatypePaths) {
            Category category = dataTypeManager.getCategory(new CategoryPath(pathName));
            if (category != null) {
                rememberCategoryAndDescendants(paths, category);
            }
        }
    }

    private void rememberCategoryAndDescendants(
        LinkedHashMap<String, CategoryPath> paths,
        Category category) {

        if (category == null) {
            return;
        }
        CategoryPath path = category.getCategoryPath();
        if (path != null && !CategoryPath.ROOT.equals(path)) {
            paths.put(path.getPath(), path);
        }
        for (Category child : category.getCategories()) {
            rememberCategoryAndDescendants(paths, child);
        }
    }

    private void rememberCategoryPathAndAncestors(
        LinkedHashMap<String, CategoryPath> paths,
        CategoryPath path) {

        CategoryPath current = path;
        while (current != null && !CategoryPath.ROOT.equals(current)) {
            paths.put(current.getPath(), current);
            current = current.getParent();
        }
    }

    private void cleanupEmptyStaleCategories(
        LinkedHashMap<String, CategoryPath> staleCategoryPaths,
        ActionSink actions) throws Exception {

        ArrayList<CategoryPath> paths = new ArrayList<>(staleCategoryPaths.values());
        paths.sort((left, right) ->
            Integer.compare(right.asList().size(), left.asList().size()));

        for (CategoryPath path : paths) {
            JsonObject action = new JsonObject();
            action.addProperty("kind", "stale_category_cleanup");
            action.addProperty("categoryPath", path.getPath());

            if (!applyRenames) {
                action.addProperty("applied", false);
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
                actions.add(action);
                continue;
            }

            Category parent = dataTypeManager.getCategory(path.getParent());
            if (parent == null) {
                action.addProperty("applied", false);
                action.addProperty("reason", "parent-category-missing");
                actions.add(action);
                continue;
            }

            try {
                boolean removed = parent.removeEmptyCategory(path.getName(), monitor);
                action.addProperty("applied", removed);
                if (!removed) {
                    action.addProperty("reason", "category-not-empty");
                }
            }
            catch (Exception error) {
                action.addProperty("applied", false);
                action.addProperty("reason", "category-remove-failed");
                action.addProperty("error", error.getMessage());
            }
            actions.add(action);
        }
    }

    private boolean isGeneratedReflectedDatatype(DataType dataType) {
        String description = dataType.getDescription();
        return description != null &&
            description.startsWith("AZ SerializeContext reflected type");
    }

    private boolean sameStructure(Structure left, Structure right) {
        if (left == null || right == null) {
            return false;
        }
        if (left.getLength() != right.getLength()) {
            return false;
        }

        DataTypeComponent[] leftComponents = left.getDefinedComponents();
        DataTypeComponent[] rightComponents = right.getDefinedComponents();
        if (leftComponents.length != rightComponents.length) {
            return false;
        }
        for (int i = 0; i < leftComponents.length; i++) {
            if (!sameComponent(leftComponents[i], rightComponents[i])) {
                return false;
            }
        }
        return true;
    }

    private boolean sameComponent(DataTypeComponent left, DataTypeComponent right) {
        if (left.getOffset() != right.getOffset()) {
            return false;
        }
        if (left.getLength() != right.getLength()) {
            return false;
        }
        if (!sameText(left.getFieldName(), right.getFieldName())) {
            return false;
        }
        if (!sameText(left.getComment(), right.getComment())) {
            return false;
        }
        return sameDataType(left.getDataType(), right.getDataType());
    }

    private boolean sameDataType(DataType left, DataType right) {
        if (left == right) {
            return true;
        }
        if (left == null || right == null) {
            return false;
        }
        if (!sameText(left.getPathName(), right.getPathName())) {
            return false;
        }
        if (left.getLength() != right.getLength()) {
            return false;
        }
        try {
            return left.isEquivalent(right);
        }
        catch (Exception ignored) {
            return true;
        }
    }

    private boolean sameText(String left, String right) {
        if (left == null || left.isEmpty()) {
            return right == null || right.isEmpty();
        }
        return left.equals(right);
    }

    private String datatypePathName(DatatypeTarget target) {
        String categoryPath = target.categoryPath.getPath();
        return "/".equals(categoryPath)
            ? "/" + target.name
            : categoryPath + "/" + target.name;
    }

    private List<ClassData> canonicalClassDataByTypeId(List<ClassData> classData) {
        LinkedHashMap<String, ClassData> byTypeId = new LinkedHashMap<>();
        for (ClassData data : classData) {
            String typeId = normalizeTypeId(data.typeId);
            if (typeId == null) {
                continue;
            }
            byTypeId.put(typeId, preferredClassData(byTypeId.get(typeId), data));
        }
        return new ArrayList<>(byTypeId.values());
    }

    private ClassData preferredClassData(ClassData current, ClassData candidate) {
        if (current == null) {
            return candidate;
        }
        if (candidate == null) {
            return current;
        }

        int currentScore = reflectedClassDataScore(current);
        int candidateScore = reflectedClassDataScore(candidate);
        return candidateScore > currentScore ? candidate : current;
    }

    private int reflectedClassDataScore(ClassData data) {
        int score = 0;
        if (datatypeTarget(data) != null) {
            score += 1_000_000;
        }
        score += Math.max(0, reflectedLayoutFieldCount(data)) * 1_000;
        score += Math.max(0, reflectedLayoutSize(data));
        return score;
    }

    private Structure declareReflectedDatatype(ClassData data) throws Exception {
        DatatypeTarget target = datatypeTarget(data);
        if (target == null) {
            return null;
        }
        int layoutSize = reflectedLayoutSize(data);
        if (layoutSize <= 0) {
            return null;
        }

        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existing instanceof Structure) {
            Structure existingStructure = (Structure) existing;
            if (existingStructure.getLength() == layoutSize) {
                return existingStructure;
            }
            if (!isGeneratedReflectedDatatype(existingStructure)) {
                return null;
            }
            Structure replacement =
                new StructureDataType(target.categoryPath, target.name, layoutSize, dataTypeManager);
            replacement.setDescription(reflectedDatatypeDescription(data));
            if (!applyRenames) {
                return replacement;
            }
            createScope(target.scope);
            return (Structure) dataTypeManager.addDataType(
                replacement,
                DataTypeConflictHandler.REPLACE_HANDLER);
        }
        if (existing != null) {
            return null;
        }

        createScope(target.scope);
        Structure structure =
            new StructureDataType(target.categoryPath, target.name, layoutSize, dataTypeManager);
        structure.setDescription(reflectedDatatypeDescription(data));
        return (Structure) dataTypeManager.addDataType(
            structure,
            DataTypeConflictHandler.DEFAULT_HANDLER);
    }

    private void processReflectedDatatype(
        ClassData data,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "datatype_structure");
        action.addProperty("ownerTypeId", data.typeId);
        action.addProperty("ownerTypeName", data.targetTypeName);

        DatatypeTarget target = datatypeTarget(data);
        if (target == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-type-name");
            actions.add(action);
            return;
        }
        action.addProperty(
            "name",
            fullName(target.scope.subList(0, target.scope.size() - 1), target.name));
        action.addProperty("datatypePath", target.categoryPath.getPath());

        int layoutSize = reflectedLayoutSize(data);
        int fieldCount = reflectedLayoutFieldCount(data);
        action.addProperty("layoutSize", layoutSize);
        action.addProperty("fieldCount", fieldCount);
        if (layoutSize <= 0 || fieldCount == 0) {
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-layout-fields");
            actions.add(action);
            return;
        }

        Structure structure =
            new StructureDataType(target.categoryPath, target.name, layoutSize, dataTypeManager);
        structure.setDescription(reflectedDatatypeDescription(data));

        JsonArray fieldFailures = new JsonArray();
        int fieldsWritten = 0;
        List<ElementData> sortedElements = sortedLayoutElements(data);
        for (ElementData element : sortedElements) {
            Integer offset = parseLayoutInteger(element.offset);
            Integer dataSize = parseLayoutInteger(element.dataSize);
            if (offset == null || dataSize == null || dataSize <= 0) {
                addDatatypeFieldFailure(fieldFailures, element, "invalid-layout");
                continue;
            }
            try {
                DataType fieldType = fieldDataType(data, element, dataSize);
                int replaceLength = datatypeFieldReplaceLength(element, fieldType, dataSize);
                structure.replaceAtOffset(
                    offset,
                    fieldType,
                    replaceLength,
                    datatypeFieldName(element),
                    datatypeFieldComment(element));
                fieldsWritten++;
            }
            catch (Exception error) {
                JsonObject failure = datatypeFieldFailure(element, "field-apply-failed");
                failure.addProperty("error", error.getMessage());
                fieldFailures.add(failure);
            }
        }
        action.addProperty("fieldsWritten", fieldsWritten);
        action.addProperty("fieldFailures", fieldFailures.size());
        if (fieldFailures.size() > 0) {
            action.add("fieldFailureSamples", fieldFailures);
            action.addProperty("applied", false);
            action.addProperty("reason", "datatype-field-failed");
            actions.add(action);
            return;
        }

        boolean applied = false;
        if (applyRenames) {
            try {
                createScope(target.scope);
                DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
                if (existing instanceof Structure &&
                    isGeneratedReflectedDatatype(existing) &&
                    sameStructure((Structure) existing, structure)) {
                    action.addProperty("reason", "already-current");
                }
                else {
                    dataTypeManager.addDataType(structure, DataTypeConflictHandler.REPLACE_HANDLER);
                    applied = true;
                }
            }
            catch (Exception error) {
                action.addProperty("applied", false);
                action.addProperty("reason", "datatype-create-failed");
                action.addProperty("error", error.getMessage());
                actions.add(action);
                return;
            }
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private void processMemberFunctionThisParameters(
        List<ClassData> classData,
        Map<String, Structure> declaredStructures,
        ActionSink actions) throws Exception {

        LinkedHashMap<String, ArrayList<MemberFunctionUse>> usesByAddress =
            memberFunctionUsesByAddress(classData);
        for (Map.Entry<String, ArrayList<MemberFunctionUse>> entry : usesByAddress.entrySet()) {
            MemberFunctionTarget target = uniqueMemberFunctionTarget(entry.getValue());
            if (target == null) {
                JsonObject action = new JsonObject();
                action.addProperty("kind", "this_parameter");
                action.addProperty("address", entry.getKey());
                action.addProperty("applied", false);
                action.addProperty("reason", "ambiguous-member-owner");
                action.add("ownerCandidates", memberFunctionOwnerCandidates(entry.getValue()));
                actions.add(action);
                continue;
            }
            applyMemberFunctionThisParameter(target, declaredStructures, actions);
        }
    }

    private LinkedHashMap<String, ArrayList<MemberFunctionUse>> memberFunctionUsesByAddress(
        List<ClassData> classData) {

        LinkedHashMap<String, ArrayList<MemberFunctionUse>> result = new LinkedHashMap<>();
        for (ClassData data : classData) {
            for (ElementData element : data.elements) {
                for (ElementCallback callback : element.callbacks) {
                    if (!"memberFunction".equals(callback.sourceField)) {
                        continue;
                    }
                    String key = addressKey(callback.address);
                    ArrayList<MemberFunctionUse> uses = result.get(key);
                    if (uses == null) {
                        uses = new ArrayList<>();
                        result.put(key, uses);
                    }
                    MemberFunctionUse use = new MemberFunctionUse();
                    use.owner = data;
                    use.element = element;
                    use.callback = callback;
                    uses.add(use);
                }
            }
        }
        return result;
    }

    private MemberFunctionTarget uniqueMemberFunctionTarget(ArrayList<MemberFunctionUse> uses) {
        MemberFunctionTarget selected = null;
        for (MemberFunctionUse use : uses) {
            if (use == null || use.owner == null) {
                return null;
            }
            String ownerTypeId = normalizeTypeId(use.owner.typeId);
            String ownerTypeName = safeTypeName(use.owner.targetTypeName);
            if (ownerTypeId == null || ownerTypeName == null) {
                return null;
            }
            if (selected == null) {
                selected = new MemberFunctionTarget();
                selected.owner = use.owner;
                selected.element = use.element;
                selected.callback = use.callback;
                selected.ownerTypeId = ownerTypeId;
                selected.ownerTypeName = ownerTypeName;
                selected.address = use.callback == null ? null : use.callback.address;
                continue;
            }
            if (!sameTypeId(selected.ownerTypeId, ownerTypeId)) {
                return null;
            }
        }
        return selected;
    }

    private JsonArray memberFunctionOwnerCandidates(ArrayList<MemberFunctionUse> uses) {
        JsonArray result = new JsonArray();
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        for (MemberFunctionUse use : uses) {
            if (use == null || use.owner == null) {
                continue;
            }
            String ownerTypeId = normalizeTypeId(use.owner.typeId);
            String ownerTypeName = safeTypeName(use.owner.targetTypeName);
            String key = ownerTypeId + "\u001f" + ownerTypeName;
            if (!seen.add(key)) {
                continue;
            }
            JsonObject candidate = new JsonObject();
            candidate.addProperty("ownerTypeId", ownerTypeId);
            candidate.addProperty("ownerTypeName", ownerTypeName);
            result.add(candidate);
        }
        return result;
    }

    private void applyMemberFunctionThisParameter(
        MemberFunctionTarget target,
        Map<String, Structure> declaredStructures,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "this_parameter");
        action.addProperty("address", target.address);
        action.addProperty("ownerTypeId", target.ownerTypeId);
        action.addProperty("ownerTypeName", target.ownerTypeName);

        DatatypeTarget datatypeTarget = datatypeTarget(target.owner);
        if (datatypeTarget == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-type-name");
            actions.add(action);
            return;
        }
        action.addProperty(
            "name",
            fullName(datatypeTarget.scope, "this"));
        action.addProperty("datatypePath", datatypeTarget.categoryPath.getPath());

        Address address = parseCaptureAddress(target.address);
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function == null && applyRenames) {
            function = createMemberFunctionTargetFunction(target, address, action);
        }
        if (function == null) {
            Function containingFunction =
                currentProgram.getFunctionManager().getFunctionContaining(address);
            if (containingFunction != null) {
                action.addProperty("containingFunction", containingFunction.getName(true));
                action.addProperty(
                    "containingFunctionEntry",
                    containingFunction.getEntryPoint().toString());
            }
            Instruction instruction = currentProgram.getListing().getInstructionAt(address);
            action.addProperty("applied", false);
            action.addProperty(
                "reason",
                instruction == null
                    ? "member-function-pointer-not-instruction"
                    : "member-function-pointer-not-function-entry");
            actions.add(action);
            return;
        }
        action.addProperty("function", function.getName(true));

        if (reflectedLayoutSize(target.owner) <= 0 ||
            reflectedLayoutFieldCount(target.owner) == 0) {
            action.addProperty("applied", false);
            action.addProperty("reason", "datatype-missing-layout");
            actions.add(action);
            return;
        }

        if (!applyRenames) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
            actions.add(action);
            return;
        }

        Structure structure = reflectedStructure(target.owner);
        if (structure == null) {
            structure = declaredStructures.get(target.ownerTypeId);
        }
        if (structure == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "datatype-missing");
            actions.add(action);
            return;
        }

        boolean applied = false;
        try {
            Parameter[] parameters = function.getParameters();
            DataType thisPointer = new PointerDataType(structure, dataTypeManager);
            if (parameters.length > 0 &&
                "this".equals(parameters[0].getName()) &&
                sameDataType(parameters[0].getDataType(), thisPointer)) {
                action.addProperty("reason", "already-current");
                action.addProperty("applied", false);
                actions.add(action);
                return;
            }
            Parameter[] updated = parameters.length == 0
                ? new Parameter[1]
                : parameters.clone();
            updated[0] = new ParameterImpl(
                "this",
                thisPointer,
                currentProgram);
            function.replaceParameters(
                FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS,
                true,
                SourceType.USER_DEFINED,
                updated);
            applied = true;
        }
        catch (Exception error) {
            action.addProperty("applied", false);
            action.addProperty("reason", "this-parameter-failed");
            action.addProperty("error", error.getMessage());
            actions.add(action);
            return;
        }
        action.addProperty("applied", applied);
        actions.add(action);
    }

    private Function createMemberFunctionTargetFunction(
        MemberFunctionTarget target,
        Address address,
        JsonObject action) throws Exception {

        if (target.element == null || target.callback == null) {
            action.addProperty("createFailure", "missing-member-function-context");
            return null;
        }

        List<String> scope = attributeCallbackScope(target.owner, target.element, target.callback);
        String localName = attributeCallbackName(target.callback);
        action.addProperty("functionTargetName", fullName(scope, localName));

        Function function = createMissingFunction(address, localName, action);
        if (function == null) {
            return null;
        }

        action.addProperty("createdFunction", true);
        try {
            applyFunctionRename(function, scope, localName, action);
            action.addProperty("function", function.getName(true));
        }
        catch (Exception error) {
            action.addProperty("functionRenameFailure", error.getMessage());
        }
        return function;
    }

    private DatatypeTarget datatypeTarget(ClassData data) {
        if (data == null || data.targetTypeName == null) {
            return null;
        }
        return datatypeTarget(data.typeId, data.targetTypeName);
    }

    private DatatypeTarget datatypeTarget(String typeId, String typeName) {
        if (typeName == null) {
            return null;
        }
        ArrayList<String> scope = reflectedDatatypeScope(typeId, typeName);
        if (scope.isEmpty()) {
            return null;
        }
        String name = safeTypeName(scope.get(scope.size() - 1));
        if (name == null) {
            return null;
        }
        DatatypeTarget target = new DatatypeTarget();
        target.scope = scope;
        target.name = name;
        target.categoryPath = datatypeCategoryPath(scope);
        return target;
    }

    private CategoryPath datatypeCategoryPath(List<String> scope) {
        CategoryPath path = CategoryPath.ROOT;
        for (int i = 0; i < scope.size() - 1; i++) {
            String part = safeTypeName(scope.get(i));
            if (part == null) {
                continue;
            }
            path = new CategoryPath(path, part);
        }
        return path;
    }

    private ArrayList<String> reflectedDatatypeScope(String typeId, String typeName) {
        ArrayList<String> result = new ArrayList<>();
        ClassData data = null;
        if (typeEvidence != null) {
            data = typeEvidence.classBodiesByTypeId.get(normalizeTypeId(typeId));
        }
        if (data != null && shouldUseDatatypeOwnerScope(data)) {
            result.addAll(data.ownerScope);
        }
        String targetTypeName = data != null
            ? safeTypeName(data.targetTypeName)
            : safeTypeName(typeName);
        if (targetTypeName != null) {
            result.add(targetTypeName);
        }
        return result;
    }

    private boolean shouldUseDatatypeOwnerScope(ClassData data) {
        return data != null &&
            "module-descriptor".equals(data.ownerReason) &&
            hasOwner(data.ownerScope) &&
            !ownerScopeNamesType(data.ownerScope, data.targetTypeName);
    }

    private Structure reflectedStructure(ClassData data) {
        DatatypeTarget target = datatypeTarget(data);
        if (target == null) {
            return null;
        }
        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        return existing instanceof Structure ? (Structure) existing : null;
    }

    private int reflectedLayoutSize(ClassData data) {
        int result = 0;
        for (ElementData element : data.elements) {
            Integer offset = parseLayoutInteger(element.offset);
            Integer dataSize = parseLayoutInteger(element.dataSize);
            if (offset == null || dataSize == null || dataSize <= 0) {
                continue;
            }
            result = Math.max(result, offset + dataSize);
        }
        String typeId = normalizeTypeId(data.typeId);
        if (typeEvidence != null && typeId != null) {
            Integer nativeDataSize = typeEvidence.nativeDataSizesByTypeId.get(typeId);
            if (nativeDataSize != null) {
                result = Math.max(result, nativeDataSize);
            }
        }
        return result;
    }

    private int reflectedLayoutFieldCount(ClassData data) {
        int result = 0;
        for (ElementData element : data.elements) {
            Integer offset = parseLayoutInteger(element.offset);
            Integer dataSize = parseLayoutInteger(element.dataSize);
            if (offset != null && dataSize != null && dataSize > 0) {
                result++;
            }
        }
        return result;
    }

    private List<ElementData> sortedLayoutElements(ClassData data) {
        ArrayList<ElementData> result = new ArrayList<>(data.elements);
        result.sort((left, right) -> {
            Integer leftOffset = parseLayoutInteger(left.offset);
            Integer rightOffset = parseLayoutInteger(right.offset);
            int offsetCompare = Integer.compare(
                leftOffset == null ? Integer.MAX_VALUE : leftOffset,
                rightOffset == null ? Integer.MAX_VALUE : rightOffset);
            if (offsetCompare != 0) {
                return offsetCompare;
            }
            return left.safeName.compareTo(right.safeName);
        });
        return result;
    }

    private Integer parseLayoutInteger(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        if (trimmed.isEmpty()) {
            return null;
        }
        try {
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                return Integer.parseUnsignedInt(trimmed.substring(2), 16);
            }
            return Integer.parseUnsignedInt(trimmed);
        }
        catch (NumberFormatException ignored) {
            return null;
        }
    }

    private DataType fieldDataType(
        ClassData owner,
        ElementData element,
        int dataSize) {

        DataType nested = nestedFieldDataType(owner, element, dataSize);
        if (nested != null) {
            return nested;
        }
        DataType scalar = scalarFieldDataType(element, dataSize);
        if (scalar != null) {
            return scalar;
        }
        DataType named = namedFieldDataType(owner, element, dataSize);
        if (named != null) {
            return named;
        }
        return new ArrayDataType(new Undefined1DataType(), dataSize, 1);
    }

    private DataType nestedFieldDataType(
        ClassData owner,
        ElementData element,
        int dataSize) {

        String fieldTypeId = normalizeTypeId(element.typeId);
        String ownerTypeId = normalizeTypeId(owner.typeId);
        if (fieldTypeId == null || sameTypeId(fieldTypeId, ownerTypeId)) {
            return null;
        }
        Structure structure = declaredStructuresByTypeId.get(fieldTypeId);
        if (structure != null && fieldTypeFitsLayoutSlot(element, structure, dataSize)) {
            return structure;
        }
        ClassData fieldClassData = typeEvidence == null
            ? null
            : typeEvidence.classBodiesByTypeId.get(fieldTypeId);
        if (fieldClassData == null) {
            return null;
        }
        structure = reflectedStructure(fieldClassData);
        if (structure == null || !fieldTypeFitsLayoutSlot(element, structure, dataSize)) {
            return null;
        }
        return structure;
    }

    private DataType scalarFieldDataType(ElementData element, int dataSize) {
        String typeId = normalizeTypeId(element.typeId);
        if (typeId != null) {
            if ("A0CA880C-AFE4-43CB-926C-59AC48496112".equals(typeId) && dataSize == 1) {
                return new BooleanDataType();
            }
            if (("3AB0037F-AF8D-48CE-BCA0-A170D18B2C03".equals(typeId) ||
                "CFD606FE-41B8-4744-B79F-8A6BD97713D8".equals(typeId) ||
                "58422C0E-1E47-4854-98E6-34098F6FE12D".equals(typeId)) && dataSize == 1) {
                return new CharDataType();
            }
            if ("72B9409A-7D1A-4831-9CFE-FCB3FADD3426".equals(typeId) && dataSize == 1) {
                return new ByteDataType();
            }
            if ("B8A56D56-A10D-4DCE-9F63-405EE243DD3C".equals(typeId) && dataSize == 2) {
                return new ShortDataType();
            }
            if ("ECA0B403-C4F8-4B86-95FC-81688D046E40".equals(typeId) && dataSize == 2) {
                return new UnsignedShortDataType();
            }
            if ("72039442-EB38-4D42-A1AD-CB68F7E0EEF6".equals(typeId) && dataSize == 4) {
                return new IntegerDataType();
            }
            if (("43DA906B-7DEF-4CA8-9790-854106D3F983".equals(typeId) ||
                "9F4E062E-06A0-46D4-85DF-E0DA96467D3A".equals(typeId)) && dataSize == 4) {
                return new UnsignedIntegerDataType();
            }
            if ("EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D".equals(typeId) && dataSize == 4) {
                return new FloatDataType();
            }
            if ("110C4B14-11A8-4E9D-8638-5051013A56AC".equals(typeId) && dataSize == 8) {
                return new DoubleDataType();
            }
            if (("8F24B9AD-7C51-46CF-B2F8-277356957325".equals(typeId) ||
                "70D8A282-A1EA-462D-9D04-51EDE81FAC2F".equals(typeId)) && dataSize == 8) {
                return new LongLongDataType();
            }
            if (("D6597933-47CD-4FC8-B911-63F3E2B0993A".equals(typeId) ||
                "5EC2D6F7-6859-400F-9215-C106F5B10E53".equals(typeId) ||
                "6383F1D3-BB27-4E6B-A49A-6409B2059EAA".equals(typeId)) && dataSize == 8) {
                return new UnsignedLongLongDataType();
            }
        }

        String name = element.typeName == null ? null : element.typeName.replace(" ", "");
        if (name == null) {
            return null;
        }
        if (name.endsWith("*") && dataSize == currentProgram.getDefaultPointerSize()) {
            return new PointerDataType(VoidDataType.dataType, dataTypeManager);
        }
        if (("bool".equals(name) || "AZ::Platform::bool".equals(name)) && dataSize == 1) {
            return new BooleanDataType();
        }
        if (("char".equals(name) || "s8".equals(name)) && dataSize == 1) {
            return new CharDataType();
        }
        if ("u8".equals(name) && dataSize == 1) {
            return new ByteDataType();
        }
        if ("s16".equals(name) && dataSize == 2) {
            return new ShortDataType();
        }
        if ("u16".equals(name) && dataSize == 2) {
            return new UnsignedShortDataType();
        }
        if (("s32".equals(name) || "int".equals(name)) && dataSize == 4) {
            return new IntegerDataType();
        }
        if (("u32".equals(name) || "unsignedint".equals(name) ||
            "AZ::Crc32".equals(name) || "Crc32".equals(name)) && dataSize == 4) {
            return new UnsignedIntegerDataType();
        }
        if (("float".equals(name) || "f32".equals(name)) && dataSize == 4) {
            return new FloatDataType();
        }
        if (("double".equals(name) || "f64".equals(name)) && dataSize == 8) {
            return new DoubleDataType();
        }
        if ("s64".equals(name) && dataSize == 8) {
            return new LongLongDataType();
        }
        if (("u64".equals(name) || "AZ::EntityId".equals(name)) && dataSize == 8) {
            return new UnsignedLongLongDataType();
        }
        return null;
    }

    private DataType namedFieldDataType(
        ClassData owner,
        ElementData element,
        int dataSize) {

        String fieldTypeId = normalizeTypeId(element.typeId);
        String ownerTypeId = normalizeTypeId(owner.typeId);
        if (fieldTypeId == null || sameTypeId(fieldTypeId, ownerTypeId)) {
            return null;
        }

        String typeName = element.targetTypeName != null
            ? element.targetTypeName
            : safeTypeName(element.typeName);
        DatatypeTarget target = datatypeTarget(fieldTypeId, typeName);
        if (target == null) {
            return null;
        }

        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existing != null) {
            return fieldTypeFitsLayoutSlot(element, existing, dataSize) ? existing : null;
        }

        Structure structure =
            new StructureDataType(target.categoryPath, target.name, dataSize, dataTypeManager);
        structure.setDescription("AZ SerializeContext reflected field type: " +
            element.typeName + " (" + element.typeId + ")");
        if (!applyRenames) {
            return structure;
        }
        try {
            createScope(target.scope);
            DataType added = dataTypeManager.addDataType(
                structure,
                DataTypeConflictHandler.DEFAULT_HANDLER);
            return fieldTypeFitsLayoutSlot(element, added, dataSize) ? added : null;
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private boolean fieldTypeFitsLayoutSlot(ElementData element, DataType dataType, int dataSize) {
        int typeLength = dataType.getLength();
        if (typeLength == dataSize) {
            return true;
        }
        return element.isBaseClass && typeLength > 0 && typeLength <= dataSize;
    }

    private int datatypeFieldReplaceLength(
        ElementData element,
        DataType dataType,
        int dataSize) {

        int typeLength = dataType.getLength();
        if (element.isBaseClass && typeLength > 0 && typeLength <= dataSize) {
            return typeLength;
        }
        return dataSize;
    }

    private String datatypeFieldName(ElementData element) {
        String name = element.safeName == null ? "field" : element.safeName;
        name = name.replace("::", "_")
            .replace('<', '_')
            .replace('>', '_')
            .replace(',', '_')
            .replace('~', '_');
        name = UNDERSCORES_RE.matcher(name).replaceAll("_");
        name = trimUnderscores(name);
        return name.isEmpty() ? "field" : name;
    }

    private String datatypeFieldComment(ElementData element) {
        StringBuilder comment = new StringBuilder();
        if (element.typeName != null) {
            comment.append("AZ type: ").append(element.typeName);
        }
        if (element.typeId != null) {
            if (comment.length() > 0) {
                comment.append("; ");
            }
            comment.append("typeId: ").append(element.typeId);
        }
        if (element.isBaseClass) {
            if (comment.length() > 0) {
                comment.append("; ");
            }
            comment.append("base class");
        }
        return comment.length() == 0 ? null : comment.toString();
    }

    private String reflectedDatatypeDescription(ClassData data) {
        StringBuilder description = new StringBuilder();
        description.append("AZ SerializeContext reflected type");
        if (data.typeName != null) {
            description.append(": ").append(data.typeName);
        }
        if (data.typeId != null) {
            description.append(" (").append(data.typeId).append(")");
        }
        return description.toString();
    }

    private void addDatatypeFieldFailure(
        JsonArray failures,
        ElementData element,
        String reason) {

        failures.add(datatypeFieldFailure(element, reason));
    }

    private JsonObject datatypeFieldFailure(ElementData element, String reason) {
        JsonObject failure = new JsonObject();
        failure.addProperty("fieldName", element.name);
        failure.addProperty("fieldTypeId", element.typeId);
        failure.addProperty("fieldTypeName", element.typeName);
        failure.addProperty("fieldOffset", element.offset);
        failure.addProperty("dataSize", element.dataSize);
        failure.addProperty("reason", reason);
        return failure;
    }

    private Map<String, SlotGroup> rttiSlotGroups(Iterable<RttiType> types, String[] slotNames) {
        Map<String, SlotGroup> groups = new HashMap<>();
        for (RttiType type : types) {
            Address helperAddress = parseCaptureAddress(type.address);
            Address vtableAddress = readPointer(helperAddress);
            if (vtableAddress == null) {
                continue;
            }
            for (int slot = 0; slot < slotNames.length; slot++) {
                Address slotPointer = readPointer(vtableAddress.add(slot * 8L));
                if (slotPointer == null || !isProgramAddress(slotPointer)) {
                    continue;
                }
                SlotGroup group = groupFor(groups, formatAddress(slotPointer));
                group.slotNames.add(slotNames[slot]);
                group.useCount++;
            }
        }
        return groups;
    }

    private Map<String, SlotGroup> callbackGroups(List<ClassData> classData) {
        Map<String, SlotGroup> groups = new HashMap<>();
        for (ClassData data : classData) {
            for (Map.Entry<String, String> entry : data.callbacks.entrySet()) {
                String localName = FUNCTION_FIELDS.get(entry.getKey());
                if (localName == null) {
                    continue;
                }
                SlotGroup group = groupFor(groups, entry.getValue());
                group.slotNames.add(localName);
                group.useCount++;
            }
            for (ElementData element : data.elements) {
                for (ElementCallback callback : element.callbacks) {
                    SlotGroup group = groupFor(groups, callback.address);
                    group.slotNames.add(attributeCallbackName(callback));
                    group.useCount++;
                }
            }
        }
        return groups;
    }

    private Map<String, SlotGroup> classRegistrationFunctionGroups() {
        Map<String, SlotGroup> groups = new HashMap<>();
        if (classRegistrationEvidence == null) {
            return groups;
        }
        for (ClassRegistrationRecord record : classRegistrationEvidence.recordsByTypeId.values()) {
            rememberClassRegistrationFunction(groups, record.anyCreator, "AnyCreator");
        }
        return groups;
    }

    private void rememberClassRegistrationFunction(
        Map<String, SlotGroup> groups,
        String address,
        String slotName) {

        if (!isAddressLike(address)) {
            return;
        }
        SlotGroup group = groupFor(groups, address);
        group.slotNames.add(slotName);
        group.useCount++;
    }

    private void processClassRegistrationTraces(
        Map<String, SlotGroup> classRegistrationGroups,
        ClassRegistrationFunctionIndex classFunctionIndex,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (classRegistrationEvidence == null) {
            return;
        }
        for (ClassRegistrationRecord record :
            classRegistrationEvidence.recordsByTypeId.values()) {

            processClassRegistrationTrace(
                record,
                classRegistrationGroups,
                classFunctionIndex,
                functionSeen,
                aliasSeen,
                actions);
            if (monitor.isCancelled()) {
                return;
            }
        }
    }

    private void processClassRegistrationTrace(
        ClassRegistrationRecord record,
        Map<String, SlotGroup> classRegistrationGroups,
        ClassRegistrationFunctionIndex classFunctionIndex,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        String targetTypeName = classRegistrationTargetTypeName(record);
        if (targetTypeName == null) {
            addSkipped(
                actions,
                "class_registration_callsite",
                record.returnAddress,
                null,
                "missing-type-name");
            return;
        }

        List<String> scope = classRegistrationScope(targetTypeName);
        Address callsite = classRegistrationCallsite(record.returnAddress);
        renameClassRegistrationWrapperFunction(
            record,
            targetTypeName,
            callsite,
            classFunctionIndex,
            functionSeen,
            actions);
        renameCalledFunction(
            callsite,
            serializeContextScope(),
            "Class",
            "class_registration_helper_function",
            functionSeen,
            actions);
        renameClassRegistrationCallsite(record, scope, actions);

        if (isAddressLike(record.classDataAzRtti)) {
            renameLabel(
                record.classDataAzRtti,
                classRegistrationRttiScope(targetTypeName),
                "s_instance",
                "rtti_helper",
                actions);
        }
        if (isAddressLike(record.classDataFactory)) {
            renameClassRegistrationFactory(record.classDataFactory, targetTypeName, actions);
        }
        if (isAddressLike(record.anyCreator)) {
            renameFunction(
                record.anyCreator,
                scope,
                "AnyCreator",
                classRegistrationGroups,
                functionSeen,
                aliasSeen,
                actions);
        }
    }

    private void renameClassRegistrationFactory(
        String address,
        String targetTypeName,
        ActionSink actions) throws Exception {

        List<String> scope = classRegistrationFactoryScope(targetTypeName);
        renameLabel(
            address,
            scope,
            "s_instance",
            "class_registration_factory",
            actions);

        Address objectAddress = parseCaptureAddress(address);
        Address vtableAddress = readPointer(objectAddress);
        if (vtableAddress != null) {
            renameLabel(
                formatAddress(vtableAddress),
                scope,
                "vftable",
                "class_registration_factory_vftable",
                actions);
        }
    }

    private ArrayList<String> classRegistrationFactoryScope(String targetTypeName) {
        ArrayList<String> result = new ArrayList<>();
        result.add("AZ");
        result.add("Serialize");
        result.add("InstanceFactory<" + targetTypeName + ">");
        return result;
    }

    private void renameClassRegistrationWrapperFunction(
        ClassRegistrationRecord record,
        String targetTypeName,
        Address callsite,
        ClassRegistrationFunctionIndex classFunctionIndex,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", "class_registration_wrapper_function");
        action.addProperty("returnAddress", record.returnAddress);
        action.addProperty("callsite", formatAddress(callsite));
        action.addProperty("typeId", record.typeId);
        action.addProperty("typeName", targetTypeName);

        if (callsite == null || !isProgramAddress(callsite)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "callsite-not-found");
            actions.add(action);
            return;
        }

        Function function = currentProgram.getFunctionManager().getFunctionContaining(callsite);
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "function-missing");
            actions.add(action);
            return;
        }

        String functionAddress = formatAddress(function.getEntryPoint());
        action.addProperty("address", functionAddress);
        if (classFunctionIndex != null) {
            ArrayList<ClassRegistrationCallsite> peers =
                classFunctionIndex.recordsByFunction.get(functionAddress);
            LinkedHashSet<String> peerTypeIds = new LinkedHashSet<>();
            if (peers != null) {
                for (ClassRegistrationCallsite peer : peers) {
                    String peerTypeId = normalizeTypeId(peer.record.typeId);
                    if (peerTypeId != null) {
                        peerTypeIds.add(peerTypeId);
                    }
                }
            }
            if (peerTypeIds.size() > 1) {
                JsonArray peerIds = new JsonArray();
                for (String peerTypeId : peerTypeIds) {
                    peerIds.add(peerTypeId);
                }
                action.add("peerTypeIds", peerIds);
                action.addProperty("applied", false);
                action.addProperty("reason", "ambiguous-class-registration-function");
                actions.add(action);
                return;
            }
        }

        renameFunctionAt(
            function,
            classRegistrationWrapperScope(targetTypeName),
            "Register",
            "class_registration_wrapper_function",
            functionSeen,
            actions,
            action);
    }

    private String classRegistrationTargetTypeName(ClassRegistrationRecord record) {
        if (record == null) {
            return null;
        }
        String typeName = safeTypeName(record.typeName);
        if (typeName != null) {
            return typeName;
        }
        return safeTypeName(record.typeId);
    }

    private ArrayList<String> classRegistrationScope(String targetTypeName) {
        ArrayList<String> result = new ArrayList<>();
        result.add("AZ");
        result.add("SerializeContext");
        result.add("Class<" + targetTypeName + ">");
        return result;
    }

    private ArrayList<String> serializeContextScope() {
        ArrayList<String> result = new ArrayList<>();
        result.add("AZ");
        result.add("SerializeContext");
        return result;
    }

    private ArrayList<String> classBuilderScope() {
        ArrayList<String> result = serializeContextScope();
        result.add("ClassBuilder");
        return result;
    }

    private ArrayList<String> reflectedTypeScope(String typeId, String typeName) {
        ArrayList<String> result = new ArrayList<>();
        ClassData data = null;
        if (typeEvidence != null) {
            data = typeEvidence.classBodiesByTypeId.get(normalizeTypeId(typeId));
        }
        if (data != null && hasOwner(data.ownerScope)) {
            result.addAll(data.ownerScope);
        }
        String targetTypeName = data != null
            ? safeTypeName(data.targetTypeName)
            : safeTypeName(typeName);
        if (targetTypeName != null) {
            result.add(targetTypeName);
        }
        return result;
    }

    private ArrayList<String> classRegistrationWrapperScope(String targetTypeName) {
        ArrayList<String> result = new ArrayList<>();
        result.add("AZ");
        result.add("SerializeContext");
        result.add("Class<" + targetTypeName + ">");
        return result;
    }

    private ArrayList<String> classRegistrationRttiScope(String targetTypeName) {
        ArrayList<String> result = new ArrayList<>();
        result.add("AZ");
        result.add("Internal");
        result.add("RttiHelper<" + targetTypeName + ">");
        return result;
    }

    private void renameClassRegistrationCallsite(
        ClassRegistrationRecord record,
        List<String> scope,
        ActionSink actions) throws Exception {

        Address callsite = classRegistrationCallsite(record.returnAddress);
        String address = formatAddress(callsite);
        List<String> parentScope = scope.subList(0, scope.size() - 1);
        String localName = scope.get(scope.size() - 1);
        String targetName = fullName(parentScope, localName);
        JsonObject action = new JsonObject();
        action.addProperty("kind", "class_registration_callsite");
        action.addProperty("returnAddress", record.returnAddress);
        action.addProperty("address", address);
        action.addProperty("name", targetName);
        action.addProperty("typeId", record.typeId);

        if (callsite == null || !isProgramAddress(callsite)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "callsite-not-found");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, address, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(callsite));
        }
        boolean applied = false;
        if (applyRenames) {
            applied = ensureLabel(callsite, parentScope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private Address classRegistrationCallsite(String returnAddress) {
        Address address = parseCaptureAddress(returnAddress);
        if (!isProgramAddress(address)) {
            return null;
        }
        Instruction instruction = currentProgram.getListing().getInstructionBefore(address);
        if (instruction == null || !instruction.getFlowType().isCall()) {
            return null;
        }
        Address next = instruction.getMaxAddress().next();
        if (next != null && !next.equals(address)) {
            return null;
        }
        return instruction.getMinAddress();
    }

    private void processFieldRegistrationTraces(
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        if (fieldRegistrationEvidence == null) {
            return;
        }
        processFieldRegistrationOwnerFunctions(functionSeen, actions);
        for (FieldRegistrationRecord record : fieldRegistrationEvidence.records) {
            renameFieldRegistrationCallsite(record, functionSeen, actions);
            if (monitor.isCancelled()) {
                return;
            }
        }
    }

    private void processFieldRegistrationOwnerFunctions(
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        FieldRegistrationFunctionIndex index = fieldRegistrationFunctionIndex();
        for (FieldRegistrationFunctionGroup group : index.recordsByFunction.values()) {
            if (monitor.isCancelled()) {
                return;
            }
            JsonObject action = new JsonObject();
            action.addProperty("kind", "serialize_context_reflect_function");
            action.addProperty("address", group.functionAddress);
            action.addProperty("recordCount", group.recordCount);

            if (group.ownerTypeIds.size() != 1 || group.ownerTypeNames.size() != 1) {
                JsonArray ownerTypeIds = new JsonArray();
                for (String ownerTypeId : group.ownerTypeIds) {
                    ownerTypeIds.add(ownerTypeId);
                }
                JsonArray ownerTypeNames = new JsonArray();
                for (String ownerTypeName : group.ownerTypeNames) {
                    ownerTypeNames.add(ownerTypeName);
                }
                action.add("ownerTypeIds", ownerTypeIds);
                action.add("ownerTypeNames", ownerTypeNames);
                action.addProperty("applied", false);
                action.addProperty("reason", "ambiguous-reflect-owner");
                actions.add(action);
                continue;
            }

            String ownerTypeId = group.ownerTypeIds.iterator().next();
            String ownerTypeName = group.ownerTypeNames.iterator().next();
            action.addProperty("ownerTypeId", ownerTypeId);
            action.addProperty("ownerTypeName", ownerTypeName);
            Address functionAddress = parseCaptureAddress(group.functionAddress);
            if (!isProgramAddress(functionAddress)) {
                action.addProperty("applied", false);
                action.addProperty("reason", "address-not-in-program");
                actions.add(action);
                continue;
            }
            Function function = currentProgram.getFunctionManager()
                .getFunctionAt(functionAddress);
            if (function == null) {
                action.addProperty("applied", false);
                action.addProperty("reason", "function-missing");
                actions.add(action);
                continue;
            }

            renameFunctionAt(
                function,
                reflectedTypeScope(ownerTypeId, ownerTypeName),
                "Reflect",
                "serialize_context_reflect_function",
                functionSeen,
                actions,
                action);
            applyStaticReflectContextParameter(
                group.functionAddress,
                ownerTypeId,
                ownerTypeName,
                reflectedTypeScope(ownerTypeId, ownerTypeName),
                actions);
        }
    }

    private void renameFieldRegistrationCallsite(
        FieldRegistrationRecord record,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        ArrayList<String> scope = new ArrayList<>();
        scope.add("AZ");
        scope.add("SerializeContext");
        String localName = fieldRegistrationLocalName(record);
        String targetName = localName == null ? null : fullName(scope, localName);

        JsonObject action = new JsonObject();
        action.addProperty("kind", "field_registration_callsite");
        action.addProperty("returnAddress", record.fieldCallReturnAddress);
        action.addProperty("helperReturnAddress", record.helperReturnAddress);
        action.addProperty("name", targetName);
        action.addProperty("ownerTypeId", record.ownerTypeId);
        action.addProperty("ownerTypeName", record.ownerTypeName);
        action.addProperty("ownerSource", record.ownerSource);
        action.addProperty("ownerResolution", record.ownerResolution);
        action.addProperty("ownerFunctionAddress", record.ownerFunctionAddress);
        action.addProperty("fieldTypeId", record.fieldTypeId);
        action.addProperty("fieldTypeName", record.fieldTypeName);
        action.addProperty("fieldTypeNameSource", record.fieldTypeNameSource);
        action.addProperty("fieldOffset", record.fieldOffset);
        action.addProperty("fieldName", record.fieldName);
        action.addProperty("fieldNameSource", record.fieldNameSource);

        if (record.ownerTypeId == null || safeTypeName(record.ownerTypeName) == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-owner");
            actions.add(action);
            return;
        }
        if (safeTypeName(record.fieldName) == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-field-name");
            actions.add(action);
            return;
        }
        if (!isAddressLike(record.fieldCallReturnAddress)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "missing-field-call-return");
            actions.add(action);
            return;
        }

        Address callsite = callsiteBeforeReturn(record.fieldCallReturnAddress);
        String address = formatAddress(callsite);
        action.addProperty("address", address);

        if (callsite == null || !isProgramAddress(callsite)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "callsite-not-found");
            actions.add(action);
            return;
        }
        renameCalledFunction(
            callsite,
            classBuilderScope(),
            "Field",
            "field_registration_helper_function",
            functionSeen,
            actions);
        if (!reserveTargetName(targetName, address, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(callsite));
        }
        boolean applied = false;
        if (applyRenames) {
            applied = ensureLabel(callsite, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private String fieldRegistrationLocalName(FieldRegistrationRecord record) {
        String owner = safeTypeName(record.ownerTypeName);
        String field = safeTypeName(record.fieldName);
        String fieldType = safeTypeName(record.fieldTypeName);
        if (owner == null || field == null) {
            return null;
        }

        StringBuilder builder = new StringBuilder("Field<");
        builder.append(owner);
        builder.append(",");
        builder.append(field);
        if (fieldType != null) {
            builder.append(",");
            builder.append(fieldType);
        }
        builder.append(">");
        return builder.toString();
    }

    private Address callsiteBeforeReturn(String returnAddress) {
        Address address = parseCaptureAddress(returnAddress);
        if (!isProgramAddress(address)) {
            return null;
        }
        Instruction instruction = currentProgram.getListing().getInstructionBefore(address);
        if (instruction == null || !instruction.getFlowType().isCall()) {
            return null;
        }
        Address next = instruction.getMaxAddress().next();
        if (next != null && !next.equals(address)) {
            return null;
        }
        return instruction.getMinAddress();
    }

    private FieldRegistrationFunctionIndex fieldRegistrationFunctionIndex() {
        FieldRegistrationFunctionIndex result = new FieldRegistrationFunctionIndex();
        if (fieldRegistrationEvidence == null) {
            return result;
        }

        for (FieldRegistrationRecord record : fieldRegistrationEvidence.records) {
            if (record.ownerTypeId == null || safeTypeName(record.ownerTypeName) == null ||
                !isAddressLike(record.fieldCallReturnAddress)) {
                continue;
            }
            Address callsite = callsiteBeforeReturn(record.fieldCallReturnAddress);
            Function function = callsite == null
                ? null
                : currentProgram.getFunctionManager().getFunctionContaining(callsite);
            if (function == null) {
                continue;
            }

            String functionAddress = formatAddress(function.getEntryPoint());
            FieldRegistrationFunctionGroup group =
                result.recordsByFunction.get(functionAddress);
            if (group == null) {
                group = new FieldRegistrationFunctionGroup();
                group.functionAddress = functionAddress;
                result.recordsByFunction.put(functionAddress, group);
            }
            group.recordCount++;
            String ownerTypeId = normalizeTypeId(record.ownerTypeId);
            if (ownerTypeId != null) {
                group.ownerTypeIds.add(ownerTypeId);
            }
            String ownerTypeName = safeTypeName(record.ownerTypeName);
            if (ownerTypeName != null) {
                group.ownerTypeNames.add(ownerTypeName);
            }
        }
        return result;
    }

    private SlotGroup groupFor(Map<String, SlotGroup> groups, String address) {
        String key = addressKey(address);
        SlotGroup group = groups.get(key);
        if (group == null) {
            group = new SlotGroup(address);
            groups.put(key, group);
        }
        return group;
    }

    private void renameCalledFunction(
        Address callsite,
        List<String> scope,
        String localName,
        String kind,
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        action.addProperty("callsite", formatAddress(callsite));
        action.addProperty("name", fullName(scope, localName));

        Function function = calledFunctionAt(callsite);
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "called-function-missing");
            actions.add(action);
            return;
        }
        action.addProperty("address", formatAddress(function.getEntryPoint()));
        renameFunctionAt(
            function,
            scope,
            localName,
            kind,
            functionSeen,
            actions,
            action);
    }

    private Function calledFunctionAt(Address callsite) {
        if (callsite == null || !isProgramAddress(callsite)) {
            return null;
        }
        Instruction instruction = currentProgram.getListing().getInstructionAt(callsite);
        if (instruction == null || !instruction.getFlowType().isCall()) {
            return null;
        }
        for (Reference reference : instruction.getReferencesFrom()) {
            if (!reference.getReferenceType().isCall()) {
                continue;
            }
            Address target = reference.getToAddress();
            Function function = currentProgram.getFunctionManager().getFunctionAt(target);
            if (function != null) {
                return function;
            }
            function = currentProgram.getFunctionManager().getFunctionContaining(target);
            if (function != null) {
                return function;
            }
        }
        return null;
    }

    private void renameFunctionAt(
        Function function,
        List<String> scope,
        String localName,
        String kind,
        Set<String> functionSeen,
        ActionSink actions,
        JsonObject action) throws Exception {

        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "function-missing");
            actions.add(action);
            return;
        }

        Address address = function.getEntryPoint();
        String jsonAddress = formatAddress(address);
        String targetName = fullName(scope, localName);
        action.addProperty("kind", kind);
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);

        String functionKey = addressKey(jsonAddress);
        if (functionSeen.contains(functionKey)) {
            return;
        }
        functionSeen.add(functionKey);

        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        action.addProperty("oldName", function.getName(true));
        boolean applied = false;
        if (applyRenames && !function.getName(true).equals(targetName)) {
            applied = applyFunctionRename(function, scope, localName, action);
            if (!applied && action.has("reason")) {
                action.addProperty("applied", false);
                actions.add(action);
                return;
            }
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !function.getName(true).equals(targetName)) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private void renameFunction(
        String jsonAddress,
        List<String> scope,
        String localName,
        Map<String, SlotGroup> groups,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        renameFunction(
            "function",
            jsonAddress,
            scope,
            localName,
            groups,
            functionSeen,
            aliasSeen,
            actions);
    }

    private void renameFunction(
        String kind,
        String jsonAddress,
        List<String> scope,
        String localName,
        Map<String, SlotGroup> groups,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        SlotGroup group = groups.get(addressKey(jsonAddress));
        if (group == null) {
            group = new SlotGroup(jsonAddress);
            group.slotNames.add(localName);
            group.useCount = 1;
        }

        boolean shared = group.useCount != 1;
        if (shared) {
            addFunctionAlias(jsonAddress, scope, localName, group, aliasSeen, actions);
            return;
        }

        String targetName = fullName(scope, localName);
        String functionKey = addressKey(jsonAddress);
        if (functionSeen.contains(functionKey)) {
            return;
        }
        functionSeen.add(functionKey);

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = baseFunctionAction(kind, jsonAddress, targetName, group, false);
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        boolean created = false;
        if (function == null && applyRenames) {
            function = createMissingFunction(address, localName, action);
            created = function != null;
        }
        if (function == null) {
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", applyRenames ? "function-create-failed" : "function-missing");
            actions.add(action);
            return;
        }

        action.addProperty("oldName", function.getName(true));
        action.addProperty("created", created);
        boolean applied = false;
        if (applyRenames && !function.getName(true).equals(targetName)) {
            applied = applyFunctionRename(function, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !function.getName(true).equals(targetName)) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private boolean applyFunctionRename(
        Function function,
        List<String> scope,
        String localName,
        JsonObject action) {

        try {
            return renameFunctionSymbol(function, scope, localName, action);
        }
        catch (Exception error) {
            action.addProperty("reason", "function-rename-failed");
            action.addProperty("error", error.getMessage());
            return false;
        }
    }

    private boolean renameFunctionSymbol(
        Function function,
        List<String> scope,
        String localName,
        JsonObject action) throws Exception {

        String targetName = fullName(scope, localName);
        if (function.getName(true).equals(targetName)) {
            return false;
        }

        Namespace namespace = createScope(scope);
        Symbol blocker = symbols.getSymbol(localName, function.getEntryPoint(), namespace);
        if (blocker != null) {
            action.addProperty("blockingSymbol", blocker.getName(true));
            action.addProperty("blockingSymbolType", blocker.getSymbolType().toString());
            if (blocker.getSymbolType().equals(SymbolType.FUNCTION)) {
                action.addProperty("reason", "blocking-target-function-symbol");
                return false;
            }
            if (!blocker.delete()) {
                action.addProperty("reason", "blocking-target-symbol-delete-failed");
                return false;
            }
            action.addProperty("removedBlockingSymbol", true);
        }

        function.setName(localName, SourceType.USER_DEFINED);
        function.setParentNamespace(namespace);
        return true;
    }

    private boolean ensureLabel(
        Address address,
        List<String> scope,
        String localName,
        JsonObject action) throws Exception {

        Namespace namespace = createScope(scope);
        Symbol existing = symbols.getSymbol(localName, address, namespace);
        if (existing != null) {
            action.addProperty("reason", "already-current");
            return false;
        }

        symbols.createLabel(address, localName, namespace, SourceType.USER_DEFINED);
        return true;
    }

    private boolean ensurePrimaryLabel(
        Address address,
        List<String> scope,
        String localName,
        JsonObject action) throws Exception {

        Namespace namespace = createScope(scope);
        Symbol existing = symbols.getSymbol(localName, address, namespace);
        if (existing != null) {
            if (existing.isPrimary()) {
                action.addProperty("reason", "already-current");
                return false;
            }
            existing.setPrimary();
            return true;
        }

        Symbol symbol = symbols.createLabel(address, localName, namespace, SourceType.USER_DEFINED);
        symbol.setPrimary();
        return true;
    }

    private Function createMissingFunction(
        Address address,
        String localName,
        JsonObject action) throws Exception {

        action.addProperty("createAttempted", true);

        if (!isExecutableAddress(address)) {
            action.addProperty("createFailure", "non-executable-address");
            return null;
        }

        Function function = createFunction(address, localName);
        if (function != null) {
            action.addProperty("createPath", "direct");
            return function;
        }

        Listing listing = currentProgram.getListing();
        CodeUnit codeUnit = listing.getCodeUnitContaining(address);
        if (codeUnit != null && !codeUnit.getMinAddress().equals(address)) {
            action.addProperty("createFailure", "address-inside-existing-code-unit");
            action.addProperty("blockingCodeUnitStart", codeUnit.getMinAddress().toString());
            action.addProperty("blockingCodeUnitEnd", codeUnit.getMaxAddress().toString());
            action.addProperty("blockingCodeUnit", codeUnit.toString());
            return null;
        }

        if (codeUnit != null && !(codeUnit instanceof Instruction)) {
            action.addProperty("clearedCodeUnitStart", codeUnit.getMinAddress().toString());
            action.addProperty("clearedCodeUnitEnd", codeUnit.getMaxAddress().toString());
            action.addProperty("clearedCodeUnit", codeUnit.toString());
            clearListing(codeUnit.getMinAddress(), codeUnit.getMaxAddress());
        }

        boolean disassembled = disassemble(address);
        action.addProperty("disassembled", disassembled);
        if (!disassembled) {
            action.addProperty("createFailure", "disassemble-failed");
            return null;
        }

        function = createFunction(address, localName);
        if (function != null) {
            action.addProperty("createPath", "disassemble");
            return function;
        }

        action.addProperty("createFailure", "create-after-disassemble-failed");
        return null;
    }

    private boolean isExecutableAddress(Address address) {
        MemoryBlock block = currentProgram.getMemory().getBlock(address);
        return block != null && block.isExecute();
    }

    private void addFunctionAlias(
        String jsonAddress,
        List<String> scope,
        String localName,
        SlotGroup group,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        String targetName = fullName(scope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (aliasSeen.contains(key)) {
            return;
        }
        aliasSeen.add(key);

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = baseFunctionAction("function_alias", jsonAddress, targetName, group, true);
        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(address));
        }
        boolean applied = false;
        if (applyRenames) {
            applied = ensureLabel(address, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private JsonObject baseFunctionAction(
        String kind,
        String jsonAddress,
        String targetName,
        SlotGroup group,
        boolean shared) {
        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);
        action.addProperty("shared", shared);
        action.addProperty("useCount", group.useCount);
        JsonArray slots = new JsonArray();
        for (String slotName : group.slotNames) {
            slots.add(slotName);
        }
        action.add("slotNames", slots);
        return action;
    }

    private void renameLabel(
        String jsonAddress,
        List<String> scope,
        String localName,
        String kind,
        ActionSink actions) throws Exception {

        String targetName = fullName(scope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (labelSeen.contains(key)) {
            return;
        }
        labelSeen.add(key);

        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);

        if (!isProgramAddress(address)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "address-not-in-program");
            actions.add(action);
            return;
        }
        if (!reserveTargetName(targetName, jsonAddress, action, actions)) {
            return;
        }

        if (actions.keepsFullActions()) {
            action.add("oldNames", oldNames(address));
        }
        boolean applied = false;
        if (applyRenames) {
            applied = ensureLabel(address, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private boolean reserveTargetName(
        String targetName,
        String jsonAddress,
        JsonObject action,
        ActionSink actions) {

        String targetAddress = addressKey(jsonAddress);
        String existingAddress = targetNameOwners.get(targetName);
        if (existingAddress == null) {
            targetNameOwners.put(targetName, targetAddress);
            return true;
        }
        if (existingAddress.equals(targetAddress)) {
            return true;
        }

        action.addProperty("applied", false);
        action.addProperty("reason", "duplicate-target-name");
        action.addProperty("existingAddress", existingAddress);
        actions.add(action);
        return false;
    }

    private void addSkipped(
        ActionSink actions,
        String kind,
        String address,
        String name,
        String reason) {
        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        if (address != null) {
            action.addProperty("address", address);
        }
        if (name != null) {
            action.addProperty("name", name);
        }
        action.addProperty("applied", false);
        action.addProperty("reason", reason);
        actions.add(action);
    }

    private Namespace createScope(List<String> scope) throws Exception {
        Namespace parent = currentProgram.getGlobalNamespace();
        for (int i = 0; i < scope.size(); i++) {
            String part = scope.get(i);
            if (part == null || part.isEmpty()) {
                continue;
            }
            parent = getOrCreateScope(parent, part, isClassScope(scope, i));
        }
        return parent;
    }

    private Namespace getOrCreateScope(Namespace parent, String name, boolean classScope) throws Exception {
        Namespace namespace = symbols.getNamespace(name, parent);
        if (namespace == null) {
            if (classScope) {
                return symbols.createClass(parent, name, SourceType.USER_DEFINED);
            }
            return symbols.createNameSpace(parent, name, SourceType.USER_DEFINED);
        }
        if (classScope && !(namespace instanceof GhidraClass)) {
            return symbols.convertNamespaceToClass(namespace);
        }
        return namespace;
    }

    private boolean isClassScope(List<String> scope, int index) {
        String part = scope.get(index);
        int azIndex = scope.indexOf("AZ");
        if (isBehaviorEventScope(part)) {
            return false;
        }
        return (index == 0 && isModuleClassName(part)) ||
            (index + 1 < scope.size() && isBehaviorEventScope(scope.get(index + 1))) ||
            (azIndex > 0 && index == azIndex - 1) ||
            (azIndex < 0 && index == scope.size() - 1) ||
            part.equals("SerializeContext") ||
            part.equals("BehaviorContext") ||
            part.equals("VirtualDispatchThunk") ||
            part.equals("ClassBuilder") ||
            part.startsWith("RttiHelper<") ||
            part.startsWith("Class<") ||
            part.startsWith("ClassData<") ||
            part.startsWith("ClassElement<") ||
            part.startsWith("Attribute<") ||
            part.startsWith("Field<") ||
            part.startsWith("InstanceFactory<") ||
            part.startsWith("ComponentDescriptorDefault<") ||
            part.equals("ComponentDescriptor");
    }

    private boolean isBehaviorEventScope(String part) {
        return part.equals("Broadcast") ||
            part.equals("Event") ||
            part.equals("QueueBroadcast") ||
            part.equals("QueueEvent");
    }

    private JsonArray oldNames(Address address) {
        JsonArray result = new JsonArray();
        Symbol[] existing = symbols.getSymbols(address);
        for (Symbol symbol : existing) {
            result.add(symbol.getName(true));
        }
        return result;
    }

    private Address parseCaptureAddress(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        Matcher moduleMatch = MODULE_ADDR_RE.matcher(value);
        if (moduleMatch.matches()) {
            long offset = Long.parseUnsignedLong(moduleMatch.group("offset"), 16);
            return currentProgram.getImageBase().add(offset);
        }
        Matcher hexMatch = HEX_ADDR_RE.matcher(value);
        if (hexMatch.matches()) {
            long address = Long.parseUnsignedLong(hexMatch.group("addr"), 16);
            return currentProgram.getAddressFactory()
                .getDefaultAddressSpace()
                .getAddress(address);
        }
        return null;
    }

    private Address readPointer(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        try {
            long value = getLong(address);
            if (value == 0) {
                return null;
            }
            return currentProgram.getAddressFactory()
                .getDefaultAddressSpace()
                .getAddress(value);
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private Address absoluteAddress(long offset) {
        return currentProgram.getAddressFactory()
            .getDefaultAddressSpace()
            .getAddress(offset);
    }

    private boolean isProgramAddress(Address address) {
        return address != null && currentProgram.getMemory().contains(address);
    }

    private String readCString(Address address) throws Exception {
        if (!isProgramAddress(address)) {
            return null;
        }
        StringBuilder builder = new StringBuilder();
        for (int i = 0; i < 4096; i++) {
            byte value = getByte(address.add(i));
            if (value == 0) {
                return builder.toString();
            }
            int unsigned = unsignedByte(value);
            if (unsigned < 0x20 || unsigned > 0x7e) {
                return null;
            }
            builder.append((char) unsigned);
        }
        return null;
    }

    private int unsignedByte(Address address, int offset) throws Exception {
        return unsignedByte(getByte(address.add(offset)));
    }

    private int unsignedByte(byte value) {
        return value & 0xff;
    }

    private int signedByte(byte value) {
        return value;
    }

    private int int32(Address address, int offset) throws Exception {
        byte[] bytes = new byte[4];
        currentProgram.getMemory().getBytes(address.add(offset), bytes);
        return int32(bytes, 0);
    }

    private int int32(byte[] bytes, int offset) {
        return (unsignedByte(bytes[offset])) |
            (unsignedByte(bytes[offset + 1]) << 8) |
            (unsignedByte(bytes[offset + 2]) << 16) |
            (unsignedByte(bytes[offset + 3]) << 24);
    }

    private long uint32(Address address, int offset) throws Exception {
        return int32(address, offset) & 0xffff_ffffL;
    }

    private long int64(Address address, int offset) throws Exception {
        byte[] bytes = new byte[8];
        currentProgram.getMemory().getBytes(address.add(offset), bytes);
        long result = 0;
        for (int i = 7; i >= 0; i--) {
            result = (result << 8) | unsignedByte(bytes[i]);
        }
        return result;
    }

    private String formatAddress(Address address) {
        return address == null ? null : "0x" + Long.toHexString(address.getOffset());
    }

    private String addressKey(String address) {
        Address parsed = parseCaptureAddress(address);
        return parsed == null ? address : parsed.toString();
    }

    private String captureOffset(String value) {
        Matcher moduleMatch = MODULE_ADDR_RE.matcher(value);
        if (moduleMatch.matches()) {
            return moduleMatch.group("offset").toLowerCase(Locale.ROOT);
        }
        Matcher hexMatch = HEX_ADDR_RE.matcher(value);
        if (hexMatch.matches()) {
            long address = Long.parseUnsignedLong(hexMatch.group("addr"), 16);
            long base = currentProgram.getImageBase().getOffset();
            if (Long.compareUnsigned(address, base) >= 0) {
                return Long.toHexString(address - base);
            }
        }
        return null;
    }

    private boolean isAggregateModuleName(String name) {
        return "Module".equals(name);
    }

    private boolean isModuleClassName(String name) {
        return name.endsWith("Module") || name.endsWith("Gem");
    }

    private List<String> rttiScope(RttiType type) {
        ArrayList<String> result = new ArrayList<>();
        if (hasOwner(type.ownerScope)) {
            result.addAll(type.ownerScope);
        }
        result.add("AZ");
        result.add("Internal");
        result.add("RttiHelper<" + type.targetTypeName + ">");
        return result;
    }

    private List<String> classDataScope(ClassData data) {
        ArrayList<String> result = new ArrayList<>();
        if (hasOwner(data.ownerScope)) {
            result.addAll(data.ownerScope);
        }
        result.add("AZ");
        result.add("SerializeContext");
        result.add("ClassData<" + data.targetTypeName + ">");
        return result;
    }

    private List<String> classElementScope(ClassData data, ElementData element) {
        ArrayList<String> result = new ArrayList<>(classDataScope(data));
        String elementName = element.safeName;
        if (element.targetTypeName != null) {
            elementName += "," + element.targetTypeName;
        }
        result.add("ClassElement<" + elementName + ">");
        return result;
    }

    private List<String> attributeCallbackScope(
        ClassData data,
        ElementData element,
        ElementCallback callback) {

        ArrayList<String> result = new ArrayList<>(classElementScope(data, element));
        result.add("Attribute<" + callback.attributeName + ">");
        return result;
    }

    private String attributeCallbackName(ElementCallback callback) {
        if ("memberFunction".equals(callback.sourceField)) {
            return "MemberFunction";
        }
        if ("function".equals(callback.sourceField)) {
            return "Function";
        }
        return "Callback";
    }

    private String fullName(List<String> scope, String localName) {
        StringBuilder builder = new StringBuilder();
        for (String part : scope) {
            if (builder.length() > 0) {
                builder.append("::");
            }
            builder.append(part);
        }
        if (builder.length() > 0) {
            builder.append("::");
        }
        builder.append(localName);
        return builder.toString();
    }

    private String stringMember(JsonObject object, String name) {
        JsonElement value = resolveElement(object.get(name), jsonObjectsById);
        if (value == null || value.isJsonNull() || !value.isJsonPrimitive()) {
            return null;
        }
        String result;
        try {
            result = value.getAsString();
        }
        catch (Exception ignored) {
            return null;
        }
        return result == null || result.trim().isEmpty() ? null : result;
    }

    private Boolean boolMember(JsonObject object, String name) {
        JsonElement value = resolveElement(object.get(name), jsonObjectsById);
        if (value == null || value.isJsonNull() || !value.isJsonPrimitive()) {
            return null;
        }
        try {
            return value.getAsBoolean();
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private JsonObject objectMember(JsonObject object, String name) {
        JsonElement value = resolveElement(object.get(name), jsonObjectsById);
        if (value == null || value.isJsonNull() || !value.isJsonObject()) {
            return null;
        }
        return value.getAsJsonObject();
    }

    private JsonArray arrayMember(JsonObject object, String name) {
        JsonElement value = resolveElement(object.get(name), jsonObjectsById);
        if (value == null || value.isJsonNull() || !value.isJsonArray()) {
            return null;
        }
        return value.getAsJsonArray();
    }

    private String stringElement(JsonElement value) {
        JsonElement resolved = resolveElement(value, jsonObjectsById);
        if (resolved == null || resolved.isJsonNull() || !resolved.isJsonPrimitive()) {
            return null;
        }
        try {
            String result = resolved.getAsString();
            return result == null || result.trim().isEmpty() ? null : result;
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private String primitiveString(JsonElement value) {
        JsonElement resolved = resolveElement(value, jsonObjectsById);
        if (resolved == null || resolved.isJsonNull() || !resolved.isJsonPrimitive()) {
            return null;
        }
        try {
            String result = resolved.getAsString();
            return result == null || result.trim().isEmpty() ? null : result;
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private JsonObject resolveObject(JsonElement value, Map<String, JsonObject> objectsById) {
        JsonElement resolved = resolveElement(value, objectsById);
        if (resolved == null || resolved.isJsonNull() || !resolved.isJsonObject()) {
            return null;
        }
        return resolved.getAsJsonObject();
    }

    private JsonElement resolveElement(JsonElement value, Map<String, JsonObject> objectsById) {
        JsonElement current = value;
        Set<String> seen = new HashSet<>();
        for (int depth = 0; depth < MAX_REF_DEPTH; depth++) {
            if (current == null || current.isJsonNull() || !current.isJsonObject()) {
                return current;
            }
            JsonObject object = current.getAsJsonObject();
            String reference = stringMemberDirect(object, "$ref");
            if (reference == null) {
                return current;
            }
            String key = referenceKey(reference);
            if (key == null || !seen.add(key)) {
                return current;
            }
            JsonObject next = objectsById == null ? null : objectsById.get(key);
            if (next == null) {
                return current;
            }
            current = next;
        }
        return current;
    }

    private String stringMemberDirect(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull() || !value.isJsonPrimitive()) {
            return null;
        }
        try {
            String result = value.getAsString();
            return result == null || result.trim().isEmpty() ? null : result;
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private Long longMemberDirect(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull() || !value.isJsonPrimitive()) {
            return null;
        }
        try {
            return value.getAsLong();
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private String referenceKey(String reference) {
        if (reference == null) {
            return null;
        }
        String key = reference.trim();
        if (key.startsWith("#")) {
            key = key.substring(1);
        }
        return key.isEmpty() ? null : key;
    }

    private String normalizeTypeId(String typeId) {
        if (typeId == null) {
            return null;
        }
        String normalized = typeId.trim();
        if (normalized.isEmpty()) {
            return null;
        }
        return normalized.toUpperCase(Locale.ROOT);
    }

    private String exactFieldOwnerKey(String name, String typeId, String offset) {
        String normalizedName = normalizeFieldName(name);
        String normalizedTypeId = normalizeTypeId(typeId);
        String normalizedOffset = normalizeFieldOffset(offset);
        if (normalizedName == null || normalizedTypeId == null || normalizedOffset == null) {
            return null;
        }
        return normalizedName + "\u001f" + normalizedTypeId + "\u001f" + normalizedOffset;
    }

    private String nameTypeFieldOwnerKey(String name, String typeId) {
        String normalizedName = normalizeFieldName(name);
        String normalizedTypeId = normalizeTypeId(typeId);
        if (normalizedName == null || normalizedTypeId == null) {
            return null;
        }
        return normalizedName + "\u001f" + normalizedTypeId;
    }

    private String typeOffsetFieldOwnerKey(String typeId, String offset) {
        String normalizedTypeId = normalizeTypeId(typeId);
        String normalizedOffset = normalizeFieldOffset(offset);
        if (normalizedTypeId == null || normalizedOffset == null) {
            return null;
        }
        return normalizedTypeId + "\u001f" + normalizedOffset;
    }

    private String normalizeFieldName(String name) {
        if (name == null) {
            return null;
        }
        String normalized = name.trim();
        return normalized.isEmpty() ? null : normalized;
    }

    private String normalizeFieldOffset(String offset) {
        if (offset == null) {
            return null;
        }
        String normalized = offset.trim();
        return normalized.isEmpty() ? null : normalized;
    }

    private FieldOwner uniqueFieldOwner(ArrayList<FieldOwner> owners) {
        if (owners == null || owners.isEmpty()) {
            return null;
        }

        FieldOwner selected = null;
        for (FieldOwner owner : owners) {
            if (owner == null || owner.owner == null || owner.element == null) {
                return null;
            }
            if (selected == null) {
                selected = owner;
                continue;
            }
            if (!sameTypeId(selected.owner.typeId, owner.owner.typeId) ||
                !sameFieldOwnerElement(selected.element, owner.element)) {
                return null;
            }
        }
        return selected;
    }

    private boolean sameFieldOwnerElement(ElementData left, ElementData right) {
        if (left == null || right == null) {
            return left == right;
        }
        return equalNullable(normalizeFieldName(left.name), normalizeFieldName(right.name)) &&
            sameTypeId(left.typeId, right.typeId) &&
            equalNullable(normalizeFieldOffset(left.offset), normalizeFieldOffset(right.offset));
    }

    private boolean equalNullable(String left, String right) {
        if (left == null || right == null) {
            return left == right;
        }
        return left.equals(right);
    }

    private ArrayList<ElementData> elementDataFrom(
        JsonObject classObject,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId) {
        ArrayList<ElementData> result = new ArrayList<>();
        JsonElement elementsValue = resolveElement(classObject.get("elements"), jsonObjectsById);
        if (elementsValue == null || elementsValue.isJsonNull() || !elementsValue.isJsonArray()) {
            return result;
        }

        for (JsonElement elementValue : elementsValue.getAsJsonArray()) {
            JsonObject elementObject = resolveObject(elementValue, jsonObjectsById);
            if (elementObject == null) {
                continue;
            }
            String elementName = stringMember(elementObject, "name");
            String safeElementName = safeTypeName(elementName);
            if (safeElementName == null) {
                continue;
            }

            ElementData element = new ElementData();
            element.name = elementName;
            element.safeName = safeElementName;
            element.typeId = stringMember(elementObject, "typeId");
            element.typeName = elementTypeName(
                elementObject,
                genericInfoByTypeId,
                rawTypeNamesByTypeId);
            element.targetTypeName = safeTypeName(element.typeName);
            element.offset = stringMember(elementObject, "offset");
            element.dataSize = stringMember(elementObject, "dataSize");
            element.isBaseClass = Boolean.TRUE.equals(boolMember(elementObject, "is_base_class"));
            element.isPointer = Boolean.TRUE.equals(boolMember(elementObject, "is_pointer"));
            element.genericTypeIds = genericTypeIds(elementObject);
            element.callbacks = elementCallbacksFrom(elementObject);
            result.add(element);
        }
        return result;
    }

    private String elementTypeName(
        JsonObject elementObject,
        Map<String, JsonObject> genericInfoByTypeId,
        Map<String, String> rawTypeNamesByTypeId) {

        String displayName = displayMemberTypeName(
            elementObject,
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            new HashSet<>());
        if (displayName != null) {
            return displayName;
        }

        String typeName = stringFromAzRtti(elementObject, "typeName");
        return typeName != null ? typeName : stringMember(elementObject, "typeName");
    }

    private ArrayList<String> genericTypeIds(JsonObject elementObject) {
        ArrayList<String> result = new ArrayList<>();
        JsonObject genericInfo = objectMember(elementObject, "genericClassInfo");
        collectGenericTypeIds(genericInfo, result, new HashSet<>());
        return result;
    }

    private void collectGenericTypeIds(
        JsonObject genericInfo,
        ArrayList<String> result,
        Set<String> visitingTypeIds) {

        if (genericInfo == null) {
            return;
        }
        rememberGenericTypeId(result, visitingTypeIds, stringMember(genericInfo, "typeId"));
        rememberGenericTypeId(result, visitingTypeIds, stringMember(genericInfo, "specializedTypeId"));
        JsonArray elements = arrayMember(genericInfo, "elements");
        if (elements == null) {
            return;
        }
        for (JsonElement elementValue : elements) {
            JsonObject element = resolveObject(elementValue, jsonObjectsById);
            if (element == null) {
                continue;
            }
            rememberGenericTypeId(result, visitingTypeIds, stringMember(element, "typeId"));
            collectGenericTypeIds(objectMember(element, "genericClassInfo"), result, visitingTypeIds);
        }
    }

    private void rememberGenericTypeId(
        ArrayList<String> result,
        Set<String> visitingTypeIds,
        String typeId) {

        String normalizedTypeId = normalizeTypeId(typeId);
        if (normalizedTypeId == null || !visitingTypeIds.add(normalizedTypeId)) {
            return;
        }
        result.add(normalizedTypeId);
    }

    private ArrayList<ElementCallback> elementCallbacksFrom(JsonObject elementObject) {
        ArrayList<ElementCallback> result = new ArrayList<>();
        JsonElement attributesValue = resolveElement(elementObject.get("attributes"), jsonObjectsById);
        if (
            attributesValue == null ||
            attributesValue.isJsonNull() ||
            !attributesValue.isJsonArray()
        ) {
            return result;
        }

        for (JsonElement attributePairValue : attributesValue.getAsJsonArray()) {
            JsonElement resolvedAttributePairValue =
                resolveElement(attributePairValue, jsonObjectsById);
            if (
                resolvedAttributePairValue == null ||
                resolvedAttributePairValue.isJsonNull() ||
                !resolvedAttributePairValue.isJsonArray()
            ) {
                continue;
            }
            for (JsonElement attributeValue : resolvedAttributePairValue.getAsJsonArray()) {
                JsonObject attributeObject = resolveObject(attributeValue, jsonObjectsById);
                if (attributeObject == null) {
                    continue;
                }
                JsonObject callbackValue = objectMember(attributeObject, "value");
                if (callbackValue == null) {
                    continue;
                }
                String attributeName = safeTypeName(stringMember(attributeObject, "attributeName"));
                if (attributeName == null) {
                    continue;
                }

                addElementCallback(result, attributeName, callbackValue, "function");
                addElementCallback(result, attributeName, callbackValue, "memberFunction");
            }
        }
        return result;
    }

    private void addElementCallback(
        ArrayList<ElementCallback> result,
        String attributeName,
        JsonObject value,
        String fieldName) {
        String address = stringMember(value, fieldName);
        if (!isAddressLike(address)) {
            return;
        }
        ElementCallback callback = new ElementCallback();
        callback.attributeName = attributeName;
        callback.address = address;
        callback.sourceField = fieldName;
        result.add(callback);
    }

    private String stringFromAzRtti(JsonObject object, String field) {
        JsonObject azRtti = objectMember(object, "azRtti");
        if (azRtti == null) {
            return null;
        }
        return stringMember(azRtti, field);
    }

    private boolean isAddressLike(String value) {
        if (value == null) {
            return false;
        }
        return MODULE_ADDR_RE.matcher(value).matches() || HEX_ADDR_RE.matcher(value).matches();
    }

    private String safeTypeName(String name) {
        if (name == null) {
            return null;
        }
        String cleaned = BAD_NAME_CHARS_RE.matcher(name.trim()).replaceAll("_");
        cleaned = UNDERSCORES_RE.matcher(cleaned).replaceAll("_");
        cleaned = trimUnderscores(cleaned);
        return cleaned.isEmpty() ? null : cleaned;
    }

    private String reasonName(String name) {
        String safe = safeTypeName(name);
        return safe == null ? "<unknown>" : safe;
    }

    private String trimUnderscores(String value) {
        int start = 0;
        int end = value.length();
        while (start < end && value.charAt(start) == '_') {
            start++;
        }
        while (end > start && value.charAt(end - 1) == '_') {
            end--;
        }
        return value.substring(start, end);
    }

    private static Map<String, String> functionFieldNames() {
        LinkedHashMap<String, String> result = new LinkedHashMap<>();
        result.put("converter", "VersionConverter");
        result.put("persistentId", "PersistentId");
        result.put("doSave", "DoSave");
        return result;
    }

    private static class CoreRttiCastTarget {
        final String typeName;
        final String typeId;

        CoreRttiCastTarget(String typeName, String typeId) {
            this.typeName = typeName;
            this.typeId = typeId.toUpperCase(Locale.ROOT);
        }
    }

    private static class BehaviorContextInput {
        final File file;
        final String archiveEntryName;

        BehaviorContextInput(File file) {
            this.file = file;
            this.archiveEntryName = null;
        }

        BehaviorContextInput(File file, String archiveEntryName) {
            this.file = file;
            this.archiveEntryName = archiveEntryName;
        }

        String description() {
            if (archiveEntryName == null) {
                return file.getAbsolutePath();
            }
            return file.getAbsolutePath() + "!" + archiveEntryName;
        }

        String format() {
            return archiveEntryName == null ? "json" : "7z";
        }

        Reader openReader() throws Exception {
            if (archiveEntryName == null) {
                return new InputStreamReader(new FileInputStream(file), StandardCharsets.UTF_8);
            }

            SevenZFile archive = new SevenZFile(file);
            boolean closeArchive = true;
            try {
                SevenZArchiveEntry entry;
                while ((entry = archive.getNextEntry()) != null) {
                    if (entry.isDirectory() || !archiveEntryName.equals(entry.getName())) {
                        continue;
                    }
                    InputStream stream = archive.getInputStream(entry);
                    closeArchive = false;
                    return new InputStreamReader(
                        new SevenZEntryInputStream(archive, stream),
                        StandardCharsets.UTF_8);
                }
            }
            finally {
                if (closeArchive) {
                    archive.close();
                }
            }
            throw new IOException("archive entry not found: " + archiveEntryName);
        }
    }

    private static class SevenZEntryInputStream extends InputStream {
        private final SevenZFile archive;
        private final InputStream stream;

        SevenZEntryInputStream(SevenZFile archive, InputStream stream) {
            this.archive = archive;
            this.stream = stream;
        }

        @Override
        public int read() throws IOException {
            return stream.read();
        }

        @Override
        public int read(byte[] bytes, int offset, int length) throws IOException {
            return stream.read(bytes, offset, length);
        }

        @Override
        public void close() throws IOException {
            IOException failure = null;
            try {
                stream.close();
            }
            catch (IOException error) {
                failure = error;
            }
            try {
                archive.close();
            }
            catch (IOException error) {
                if (failure == null) {
                    failure = error;
                }
                else {
                    failure.addSuppressed(error);
                }
            }
            if (failure != null) {
                throw failure;
            }
        }
    }

    private static class BehaviorContextEvidence {
        String input;
        String inputFormat;
        String archiveEntry;
        int skippedInputs;
        int skippedRecords;
        int classCount;
        int globalMethodCount;
        int globalPropertyCount;
        int ebusCount;
        int typeToClassMapCount;
        int classMethodCount;
        int classPropertyCount;
        int constructorCount;
        int classBaseEdgeCount;
        int classRequestBusEdgeCount;
        int classNotificationBusEdgeCount;
        int ebusEventCount;
        int ebusVirtualPropertyCount;
        int virtualPropertyEventNameResolvedCount;
        int virtualPropertyEventNameMissingCount;
        int directEventFunctionCount;
        int virtualEventFunctionCount;
        int functionAddressCount;
        int duplicateFunctionAddressGroupCount;
        int safeDirectFunctionCount;
        int sharedFunctionGroupCount;
        int virtualDispatchGroupCount;
        final ArrayList<BehaviorFunctionCandidate> functionCandidates = new ArrayList<>();
        final LinkedHashMap<String, BehaviorFunctionGroup> groupsByAddress =
            new LinkedHashMap<>();
        final LinkedHashMap<String, RttiType> rttiTypes = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> kindCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> classNameCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> busNameCounts = new LinkedHashMap<>();
        final LinkedHashSet<String> classTypeIds = new LinkedHashSet<>();
        final LinkedHashSet<String> duplicateBusNames = new LinkedHashSet<>();
        final ArrayList<String> skippedReasons = new ArrayList<>();
    }

    private static class BehaviorFunctionCandidate {
        String address;
        ArrayList<String> scope;
        String localName;
        String kind;
        String slot;
        String methodName;
    }

    private static class BehaviorFunctionGroup {
        String address;
        final ArrayList<BehaviorFunctionCandidate> candidates = new ArrayList<>();
        final TreeSet<String> slots = new TreeSet<>();
        final TreeSet<String> targetNames = new TreeSet<>();
    }

    private static class ScanResult {
        LinkedHashMap<String, RttiType> rttiTypes;
        ArrayList<ClassData> classData;
        LinkedHashSet<String> collidingTypeNames;
        LinkedHashMap<String, LinkedHashSet<String>> collidingTypeIdsByName;
        LinkedHashSet<String> unresolvedCollidingTypeNames;
        LinkedHashMap<String, LinkedHashSet<String>> unresolvedCollidingTypeIdsByName;
        TypeEvidenceIndex typeEvidence;
    }

    private static class RttiType {
        String address;
        String typeName;
        String typeId;
        Boolean isAbstract;
        String targetTypeName;
        ArrayList<String> ownerScope;
        String ownerReason;
        String ownerComponentName;
        String ownerSource;
        String staticReflectFunctionAddress;
    }

    private static class ClassData {
        String typeName;
        String typeId;
        String rttiAddress;
        String targetTypeName;
        ArrayList<String> ownerScope;
        String ownerReason;
        String ownerComponentName;
        String ownerSource;
        String staticReflectFunctionAddress;
        LinkedHashMap<String, String> objects;
        LinkedHashMap<String, String> callbacks;
        ArrayList<ElementData> elements;
    }

    private static class ElementData {
        String name;
        String safeName;
        String typeId;
        String typeName;
        String targetTypeName;
        String offset;
        String dataSize;
        boolean isBaseClass;
        boolean isPointer;
        ArrayList<String> genericTypeIds;
        ArrayList<ElementCallback> callbacks;
    }

    private static class ElementCallback {
        String attributeName;
        String address;
        String sourceField;
    }

    private static class ActionStats {
        final LinkedHashMap<String, Integer> kindCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> reasonCounts = new LinkedHashMap<>();
        int wouldApplyCount;
        int appliedCount;
    }

    private class ActionSink {
        final ActionStats stats = new ActionStats();
        private final boolean keepFullActions;
        private final JsonArray fullActions;
        private final LinkedHashMap<String, JsonArray> samplesByReason =
            new LinkedHashMap<>();
        private int actionCount;

        ActionSink(boolean keepFullActions) {
            this.keepFullActions = keepFullActions;
            this.fullActions = keepFullActions ? new JsonArray() : null;
        }

        void add(JsonObject action) {
            actionCount++;
            increment(stats.kindCounts, stringMember(action, "kind"));
            String reason = stringMember(action, "reason");
            increment(stats.reasonCounts, reason);
            if (boolMember(action, "wouldApply") == Boolean.TRUE) {
                stats.wouldApplyCount++;
            }
            if (boolMember(action, "applied") == Boolean.TRUE) {
                stats.appliedCount++;
            }

            String sampleReason = reason == null ? "none" : reason;
            JsonArray samples = samplesByReason.get(sampleReason);
            if (samples == null) {
                samples = new JsonArray();
                samplesByReason.put(sampleReason, samples);
            }
            if (samples.size() < MAX_SUMMARY_ACTION_EXAMPLES) {
                samples.add(actionSample(action));
            }

            if (keepFullActions) {
                fullActions.add(action);
            }
        }

        int size() {
            return actionCount;
        }

        boolean keepsFullActions() {
            return keepFullActions;
        }

        JsonObject actionSamples() {
            JsonObject result = new JsonObject();
            for (Map.Entry<String, JsonArray> entry : samplesByReason.entrySet()) {
                result.add(entry.getKey(), entry.getValue());
            }
            return result;
        }

        JsonArray samplesForReason(String reason) {
            return samplesByReason.get(reason == null ? "none" : reason);
        }

        JsonArray fullActions() {
            return fullActions == null ? new JsonArray() : fullActions;
        }
    }

    private static class SlotGroup {
        final String jsonAddress;
        final TreeSet<String> slotNames = new TreeSet<>();
        int useCount;

        SlotGroup(String jsonAddress) {
            this.jsonAddress = jsonAddress;
        }
    }

    private static class CollisionReport {
        LinkedHashSet<String> names;
        LinkedHashMap<String, LinkedHashSet<String>> typeIdsByName;
    }

    private static class TypeEvidenceIndex {
        int classBodyCount;
        int usageCount;
        int staticOwnerCount;
        final LinkedHashMap<String, ClassData> classBodiesByTypeId = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> nativeDataSizesByTypeId = new LinkedHashMap<>();
        final LinkedHashMap<String, ArrayList<TypeUsage>> usagesByTypeId =
            new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> usageKindCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> staticOwnerScopeCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, ArrayList<FieldOwner>> fieldOwnersByExactKey =
            new LinkedHashMap<>();
        final LinkedHashMap<String, ArrayList<FieldOwner>> fieldOwnersByNameTypeKey =
            new LinkedHashMap<>();
        final LinkedHashMap<String, ArrayList<FieldOwner>> fieldOwnersByTypeOffsetKey =
            new LinkedHashMap<>();
        int fieldOwnerExactKeyCount;
        int fieldOwnerNameTypeKeyCount;
        int fieldOwnerTypeOffsetKeyCount;
        int ambiguousFieldOwnerExactKeyCount;
        int ambiguousFieldOwnerNameTypeKeyCount;
        int ambiguousFieldOwnerTypeOffsetKeyCount;
    }

    private static class TypeUsage {
        String ownerTypeId;
        String ownerTypeName;
        String fieldName;
        String kind;
    }

    private static class FieldOwner {
        ClassData owner;
        ElementData element;
    }

    private static class DatatypeTarget {
        ArrayList<String> scope;
        CategoryPath categoryPath;
        String name;
    }

    private static class MemberFunctionUse {
        ClassData owner;
        ElementData element;
        ElementCallback callback;
    }

    private static class MemberFunctionTarget {
        ClassData owner;
        ElementData element;
        ElementCallback callback;
        String address;
        String ownerTypeId;
        String ownerTypeName;
    }

    private static class FieldGraphResolution {
        ClassData owner;
        ElementData element;
        String reason;
        boolean ambiguous;

        static FieldGraphResolution found(
            ClassData owner,
            ElementData element,
            String reason) {

            FieldGraphResolution resolution = new FieldGraphResolution();
            resolution.owner = owner;
            resolution.element = element;
            resolution.reason = reason;
            return resolution;
        }

        static FieldGraphResolution missing(String reason) {
            FieldGraphResolution resolution = new FieldGraphResolution();
            resolution.reason = reason;
            return resolution;
        }

        static FieldGraphResolution ambiguous(String reason) {
            FieldGraphResolution resolution = new FieldGraphResolution();
            resolution.reason = reason;
            resolution.ambiguous = true;
            return resolution;
        }

        boolean resolvedOrAmbiguous() {
            return owner != null || ambiguous;
        }
    }

    private static class StaticOwner {
        ArrayList<String> scope;
        String reason;
        String source;
        String functionAddress;
    }

    private static class ClassRegistrationEvidenceIndex {
        String input;
        int skippedInputs;
        int recordCount;
        int skippedRecords;
        int recordsWithoutReturnAddress;
        int recordsWithoutClassDataAzRtti;
        int rttiBackedRecordCount;
        int duplicateTypeIds;
        final ArrayList<String> skippedReasons = new ArrayList<>();
        final ArrayList<ClassRegistrationRecord> records = new ArrayList<>();
        final LinkedHashMap<String, ClassRegistrationRecord> recordsByTypeId =
            new LinkedHashMap<>();
        final LinkedHashMap<String, LinkedHashSet<String>> duplicateReturnAddressesByTypeId =
            new LinkedHashMap<>();
    }

    private static class ClassRegistrationRecord {
        int lineNumber;
        Long sequence;
        String typeId;
        String typeName;
        String returnAddress;
        String classDataFactory;
        String classDataAzRtti;
        String anyCreator;
    }

    private static class FieldRegistrationEvidenceIndex {
        String input;
        int skippedInputs;
        int recordCount;
        int skippedRecords;
        int recordsWithoutOwner;
        int recordsWithLiveOwner;
        int recordsWithStaticOwner;
        int recordsWithoutFieldCallReturnAddress;
        int recordsWithoutFieldCallsite;
        int recordsWithoutFieldFunction;
        int recordsWithoutClassRegistrationOwner;
        int recordsWithAmbiguousStaticOwner;
        int recordsWithGraphOwner;
        int recordsWithGraphFieldName;
        int recordsWithGraphFieldTypeName;
        int recordsWithAmbiguousGraphOwner;
        int recordsWithoutGraphOwner;
        int recordsWithoutFieldTypeName;
        int recordsWithClassRegistrationFieldTypeName;
        int ownerFunctionCount;
        int ambiguousOwnerFunctionCount;
        int ownerClassRegistrationRecordsWithoutFunction;
        final ArrayList<String> skippedReasons = new ArrayList<>();
        final LinkedHashMap<String, Integer> ownerResolutionReasonCounts =
            new LinkedHashMap<>();
        final ArrayList<FieldRegistrationRecord> records = new ArrayList<>();
    }

    private static class ClassRegistrationFunctionIndex {
        int recordsWithoutFunction;
        final LinkedHashMap<String, ArrayList<ClassRegistrationCallsite>> recordsByFunction =
            new LinkedHashMap<>();
        final LinkedHashSet<String> ambiguousFunctions = new LinkedHashSet<>();
    }

    private static class ClassRegistrationCallsite {
        ClassRegistrationRecord record;
        Address callsite;
    }

    private static class FieldRegistrationFunctionIndex {
        final LinkedHashMap<String, FieldRegistrationFunctionGroup> recordsByFunction =
            new LinkedHashMap<>();
    }

    private static class FieldRegistrationFunctionGroup {
        String functionAddress;
        int recordCount;
        final LinkedHashSet<String> ownerTypeIds = new LinkedHashSet<>();
        final LinkedHashSet<String> ownerTypeNames = new LinkedHashSet<>();
    }

    private static class OwnerResolution {
        ClassRegistrationRecord record;
        String reason;
        String functionAddress;

        static OwnerResolution found(
            ClassRegistrationRecord record,
            String functionAddress) {

            OwnerResolution resolution = new OwnerResolution();
            resolution.record = record;
            resolution.reason = "class-registration-function";
            resolution.functionAddress = functionAddress;
            return resolution;
        }

        static OwnerResolution missing(String reason) {
            return missing(reason, null);
        }

        static OwnerResolution missing(String reason, String functionAddress) {
            OwnerResolution resolution = new OwnerResolution();
            resolution.reason = reason;
            resolution.functionAddress = functionAddress;
            return resolution;
        }
    }

    private static class FieldRegistrationRecord {
        int lineNumber;
        Long sequence;
        String fieldCallReturnAddress;
        String helperReturnAddress;
        String ownerTypeName;
        String ownerTypeId;
        String ownerSource;
        String ownerResolution;
        String ownerFunctionAddress;
        String fieldName;
        String fieldNameSource;
        String fieldTypeId;
        String fieldTypeName;
        String fieldTypeNameSource;
        String fieldOffset;
    }

    private static class ModuleEvidenceIndex {
        String input;
        int inputCount;
        int skippedInputs;
        int descriptorCount;
        int descriptorsWithoutTypeId;
        int duplicateTypeIds;
        final LinkedHashSet<String> moduleNames = new LinkedHashSet<>();
        final ArrayList<String> skippedReasons = new ArrayList<>();
        final ArrayList<ModuleCaptureInput> inputs = new ArrayList<>();
        final ArrayList<Descriptor> descriptors = new ArrayList<>();
        final LinkedHashMap<String, ModuleOwner> ownersByTypeId = new LinkedHashMap<>();
        final LinkedHashMap<String, LinkedHashSet<String>> duplicateModulesByTypeId =
            new LinkedHashMap<>();
    }

    private static class ModuleOwner {
        String typeId;
        String moduleName;
        String componentName;
        String source;
    }

    private static class ModuleCapture {
        String vftable;
        List<Descriptor> descriptors;
    }

    private static class ModuleCaptureInput {
        final File file;
        final ModuleCapture module;
        final String moduleName;

        ModuleCaptureInput(File file, ModuleCapture module, String moduleName) {
            this.file = file;
            this.module = module;
            this.moduleName = moduleName;
        }
    }

    private static class Descriptor {
        String addr;
        String vftable;
        String componentName;
        String componentUuid;
        AzRtti azRtti;
        List<VTableSlot> vtableSlots;
    }

    private static class AzRtti {
        String typeId;
        String typeName;
    }

    private static class VTableSlot {
        String expected;
        String address;
    }

    private static class InstanceVtableCandidate {
        Address vtableAddress;
        Address sourceInstruction;
        int vptrOffset;
    }

    private static class DescriptorNames {
        List<String> scope;
    }

    private static class BaseDescriptorName {
        List<String> scope;
        String localName;
    }

    private static Map<String, String> moduleNamesByVtable() {
        Map<String, String> names = new HashMap<>();
        names.put("836b7b0", "Module");
        names.put("8560a08", "JavelinComponentsCharacterModule");
        names.put("847d650", "JavelinComponentsAIModule");
        names.put("850d4a0", "WatermarkModule");
        names.put("850c860", "ScriptedEntityTweenerModule");
        names.put("84e0428", "ProfanityFilterModule");
        names.put("7f13960", "LyShineModule");
        names.put("7f5f980", "JavelinCollisionFiltersModule");
        names.put("844e030", "NewWorldDataSheetModule");
        names.put("8509650", "FootstepsModule");
        names.put("8507198", "CryLegacyModule");
        names.put("85098d8", "FrameProfilerEventHandlerModule");
        names.put("7faa6f0", "MaestroModule");
        names.put("850c470", "InputManagementFrameworkModule");
        names.put("850d138", "TextureAtlasModule");
        names.put("7f84eb8", "RockNRollModule");
        names.put("8509f50", "GraphicsReflectContextModule");
        names.put("7fb6490", "SnowGem");
        names.put("84e0168", "CryLegacyAnimationModule");
        names.put("7f832f0", "RoadsAndRiversModule");
        names.put("7fa8bf0", "ImGuiModule");
        names.put("84e0578", "RADTelemetryModule");
        names.put("7fb65a8", "WaterModule");
        names.put("84dfd90", "AmazonGamesSDKModule");
        names.put("7fb55e0", "RainGem");
        names.put("7f4e080", "CryHooksModule");
        names.put("7f12508", "CryHooksModule");
        names.put("85065c0", "BinkModule");
        names.put("84e0a98", "SpectatorModeModule");
        names.put("7f60c88", "LmbrCentralModule");
        names.put("850c518", "KragModule");
        names.put("7fa85a0", "CameraModule");
        names.put("7fa8a70", "HitchTrackerModule");
        names.put("84c5d48", "SlayerScriptGem");
        names.put("850b7f0", "HistoricalInputModule");
        names.put("84d97b0", "MusicSheetModule");
        names.put("7fb4b48", "PlatformServicesModule");
        return names;
    }
}
