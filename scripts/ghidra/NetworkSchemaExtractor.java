// Extract network type and field registration evidence from typeregistry.json and Ghidra.
//@category NewWorld

import java.io.File;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class NetworkSchemaExtractor extends GhidraScript {
    private static final String EXTRACTOR_VERSION = "network-schema-extractor-20260625-readraw-message-pass";
    private static final String CACHE_SCHEMA_VERSION = EXTRACTOR_VERSION + "/analysis-cache-v1";
    private static final long REGISTER_FIELD_RVA = 0x1775c60L;
    private static final long QUEUE_REGISTRATION_HOOK_RVA = 0x61a95c0L;
    private static final int BACKWARD_ARGUMENT_SCAN_LIMIT = 48;
    private static final int VTABLE_SCAN_LIMIT = 96;
    private static final int AZ_RTTI_VTABLE_SCAN_SLOTS = 24;
    private static final int FIELD_HANDLER_VTABLE_SLOTS = 14;
    private static final int FIELD_HANDLER_MARSHAL_SLOT = 5;
    private static final int FIELD_HANDLER_UNMARSHAL_SLOT = 6;
    private static final int MESSAGE_HANDLER_VTABLE_SLOTS = 12;
    private static final int MESSAGE_HANDLER_CREATE_INSTANCE_SLOT = 2;
    private static final int MESSAGE_HANDLER_MARSHAL_SLOT = 4;
    private static final int MESSAGE_HANDLER_UNMARSHAL_SLOT = 5;
    private static final int MESSAGE_HANDLER_PROVIDER_SCAN_LIMIT = 512;
    private static final int TYPE_ID_PROVIDER_BYTES = 256;
    private static final int TYPE_NAME_PROVIDER_BYTES = 384;
    private static final int SOURCE_SIGNATURE_XREF_SCAN_BYTES = 0x40;
    private static final int SOURCE_SIGNATURE_CALL_GRAPH_DEPTH = 2;
    private static final int SOURCE_SIGNATURE_CALL_GRAPH_LIMIT = 32;
    private static final String[] FIELD_HANDLER_SLOT_NAMES = {
        "Destructor",
        "IsDefaultValue",
        "SetCurrentValueAsDefault",
        "IsDirty",
        "HasValue",
        "Marshal",
        "Unmarshal",
        "MergeAndUpdateSequence",
        "ResetHasNewNetworkData",
        "GetLastModified",
        "SetLastModified",
        "IsFieldValid",
        "HasNewNetworkData",
        "LogToStream",
    };
    private static final Pattern MODULE_ADDR_RE =
        Pattern.compile("(?i)^NewWorld\\+0x(?<offset>[0-9a-f]+)$");
    private static final Pattern HEX_ADDR_RE =
        Pattern.compile("(?i)^0x(?<addr>[0-9a-f]+)$");
    private static final Pattern UUID_RE = Pattern.compile(
        "(?i)\\{?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\\}?");
    private static final Pattern INSTALL_REGISTRATION_HOOK_RE =
        Pattern.compile("InstallRegistrationHook<(?<type>[^>]+)>");
    private static final Pattern BOOL_POINTER_WRITE_RE =
        Pattern.compile("\\*\\(bool \\*\\)\\s*(?<target>[A-Za-z_][A-Za-z0-9_]*)\\s*=");
    private static final Pattern STORAGE_OFFSET_RE =
        Pattern.compile("\\b(?<base>[A-Za-z_][A-Za-z0-9_]*)\\s*\\+\\s*(?<offset>0x[0-9a-fA-F]+|\\d+)");

    private final Gson gson = new GsonBuilder()
        .disableHtmlEscaping()
        .setPrettyPrinting()
        .create();

    private final Map<String, Address> pointerReadCache = new HashMap<>();
    private final Map<String, List<Address>> asciiStringSearchCache = new HashMap<>();
    private final Map<String, Address> fieldHandlerConstructorVtableCache = new HashMap<>();
    private final Map<String, Function> functionLookupCache = new HashMap<>();
    private final Map<String, String> functionNameCache = new HashMap<>();
    private final Map<String, List<Instruction>> functionInstructionsCache = new HashMap<>();
    private final Map<String, String> decompileCache = new HashMap<>();
    private final Map<String, Long> createInstanceSizeCache = new HashMap<>();
    private final Map<String, List<String>> parameterNameCache = new HashMap<>();
    private final Map<String, Set<Integer>> boolParameterIndicesCache = new HashMap<>();
    private final Map<String, Map<String, Set<Integer>>> nestedBoolParameterIndicesCache =
        new HashMap<>();
    private final Map<String, List<ParsedUnmarshalCall>> unmarshalCallCache = new HashMap<>();
    private final Map<String, List<ParsedUnmarshalCall>> marshalerUnmarshalCallCache =
        new HashMap<>();
    private final Map<String, List<ParsedUnmarshalCall>> directTypeUnmarshalCallCache =
        new HashMap<>();
    private final Map<String, List<ParsedReadRawCall>> readRawCallCache = new HashMap<>();
    private String cacheProgramKey;
    private DecompInterface decompiler;

    @Override
    protected void run() throws Exception {
        resetAnalysisCachesForRun();
        println("NetworkSchemaExtractor version: " + EXTRACTOR_VERSION);
        File input = inputFile();
        File output = outputFile(input);
        Address registerField = currentProgram.getImageBase().add(REGISTER_FIELD_RVA);
        decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);

        JsonObject root;
        try (Reader reader = new FileReader(input)) {
            root = JsonParser.parseReader(reader).getAsJsonObject();
        }

        List<RegistryEntry> registry = parseRegistry(root);
        Map<String, HookTypeEvidence> hookTypeNamesById = collectRegistrationHookTypeNames();
        Map<String, RegistrationFunction> registrationFunctions =
            collectRegistrationFunctions(registerField);

        JsonArray registryJson = new JsonArray();
        int mappedRegistryEntries = 0;
        int mappedFieldCount = 0;
        int mappedMessageEntries = 0;
        int mappedMessageFields = 0;
        for (RegistryEntry entry : registry) {
            List<RegistrationFunction> matches =
                constructorMatches(entry, registrationFunctions);
            AzRttiEvidence resolvedRtti =
                resolvedAzRtti(entry, matches);
            HookTypeEvidence hookType = registrationHookForEntry(entry, hookTypeNamesById);
            JsonObject row = entry.toJson(resolvedRtti, hookType);
            if (!matches.isEmpty()) {
                mappedRegistryEntries++;
                JsonArray constructorJson = new JsonArray();
                JsonArray fieldJson = new JsonArray();
                for (RegistrationFunction match : matches) {
                    mappedFieldCount += match.fields.size();
                    constructorJson.add(match.toJson());
                    for (FieldCall field : match.fields) {
                        fieldJson.add(field.toJson());
                    }
                }
                row.add("fields", fieldJson);
                row.add("constructorMatches", constructorJson);
            }
            else {
                MessageUnmarshalPlan messagePlan = recoverMessageUnmarshalPlan(entry, hookType);
                if (messagePlan != null) {
                    row.add("messageUnmarshal", messagePlan.toJson());
                    if (!messagePlan.fields.isEmpty()) {
                        mappedMessageEntries++;
                        mappedMessageFields += messagePlan.fields.size();
                        JsonArray fieldJson = new JsonArray();
                        for (FieldCall field : messagePlan.fields) {
                            fieldJson.add(field.toJson());
                        }
                        row.add("fields", fieldJson);
                    }
                }
            }
            registryJson.add(row);
        }

        JsonArray functionJson = new JsonArray();
        int dynamicFieldCount = 0;
        for (RegistrationFunction function : registrationFunctions.values()) {
            dynamicFieldCount += function.fields.size();
            functionJson.add(function.toJson());
        }
        JsonArray fieldHandlerVtableJson = fieldHandlerVtablesJson(registrationFunctions);

        JsonObject report = new JsonObject();
        report.addProperty("schema", "newworld.network_schema.static.v1");
        report.addProperty("extractorVersion", EXTRACTOR_VERSION);
        report.addProperty("cacheSchemaVersion", CACHE_SCHEMA_VERSION);
        report.addProperty("program", currentProgram.getName());
        report.addProperty("imageBase", formatAddress(currentProgram.getImageBase()));
        report.addProperty("input", input.getAbsolutePath());
        report.addProperty("registerField", formatAddress(registerField));

        JsonObject summary = new JsonObject();
        summary.addProperty("typeregistryEntries", registry.size());
        add(summary, "typeIndexSlots", typeIndexSlotCount(root));
        add(summary, "registeredTypes", registeredTypeCount(root, registry.size()));
        Integer assetOnlyTypeIndexSlots = assetOnlyTypeIndexSlotCount(root, registry.size());
        add(summary, "assetOnlyTypeIndexSlots", assetOnlyTypeIndexSlots);
        summary.addProperty("installRegistrationHooks", hookTypeNamesById.size());
        summary.addProperty("fieldRegistrationFunctions", registrationFunctions.size());
        summary.addProperty("fieldRegistrationCalls", dynamicFieldCount);
        summary.addProperty("fieldHandlerVtables", fieldHandlerVtableJson.size());
        summary.addProperty("mappedRegistryEntries", mappedRegistryEntries);
        summary.addProperty("mappedRegistryFields", mappedFieldCount);
        summary.addProperty("mappedMessageEntries", mappedMessageEntries);
        summary.addProperty("mappedMessageFields", mappedMessageFields);
        report.add("summary", summary);

        report.add("registryEntries", registryJson);
        report.add("installRegistrationHooks", hookTypeNamesJson(hookTypeNamesById));
        report.add("fieldRegistrationFunctions", functionJson);
        report.add("fieldHandlerVtables", fieldHandlerVtableJson);

        try (FileWriter writer = new FileWriter(output)) {
            gson.toJson(report, writer);
        }

        println("Wrote network schema static report: " + output.getAbsolutePath());
        println("RegisterField functions: " + registrationFunctions.size() +
            ", calls: " + dynamicFieldCount +
            ", mapped registry entries: " + mappedRegistryEntries);
        println("Extractor caches: decompile=" + decompileCache.size() +
            ", instructions=" + functionInstructionsCache.size() +
            ", functions=" + functionLookupCache.size());
    }

    private void resetAnalysisCachesForRun() {
        clearAnalysisCaches();
        cacheProgramKey = currentProgramCacheKey();
        decompiler = null;
    }

    private void clearAnalysisCaches() {
        pointerReadCache.clear();
        asciiStringSearchCache.clear();
        fieldHandlerConstructorVtableCache.clear();
        functionLookupCache.clear();
        functionNameCache.clear();
        functionInstructionsCache.clear();
        decompileCache.clear();
        createInstanceSizeCache.clear();
        parameterNameCache.clear();
        boolParameterIndicesCache.clear();
        nestedBoolParameterIndicesCache.clear();
        unmarshalCallCache.clear();
        marshalerUnmarshalCallCache.clear();
        directTypeUnmarshalCallCache.clear();
    }

    private void ensureAnalysisCachesValid() {
        String key = currentProgramCacheKey();
        if (key.equals(cacheProgramKey)) {
            return;
        }
        clearAnalysisCaches();
        cacheProgramKey = key;
        if (decompiler != null && currentProgram != null) {
            decompiler.openProgram(currentProgram);
        }
    }

    private String currentProgramCacheKey() {
        if (currentProgram == null) {
            return "<no-program>";
        }
        return CACHE_SCHEMA_VERSION + ":" + System.identityHashCode(currentProgram) + ":" +
            currentProgram.getName() + "@" + currentProgram.getImageBase();
    }

    private String addressCacheKey(String kind, Address address) {
        ensureAnalysisCachesValid();
        return cacheProgramKey + "|" + kind + "|" + address;
    }

    private String functionCacheKey(String kind, Function function) {
        ensureAnalysisCachesValid();
        return cacheProgramKey + "|" + kind + "|" + function.getEntryPoint();
    }

    private String programTextCacheKey(String kind, String value) {
        ensureAnalysisCachesValid();
        return cacheProgramKey + "|" + kind + "|" + value;
    }

    private File inputFile() throws Exception {
        String explicit = envValue("NW_NETWORK_SCHEMA_TYPEREGISTRY_JSON");
        if (explicit != null) {
            return new File(explicit);
        }
        return askFile("Select New World typeregistry.json", "Open");
    }

    private File outputFile(File input) {
        String explicit = envValue("NW_NETWORK_SCHEMA_OUT");
        if (explicit != null) {
            return new File(explicit);
        }
        File parent = input.getParentFile();
        if (parent == null) {
            parent = new File(".");
        }
        return new File(parent, "network-schema.static.json");
    }

    private String envValue(String name) {
        String value = System.getenv(name);
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        return value.trim();
    }

    private List<RegistryEntry> parseRegistry(JsonObject root) {
        ArrayList<RegistryEntry> result = new ArrayList<>();
        JsonObject data = object(root, "data");
        JsonArray list = registryDescriptorArray(data);
        if (list == null) {
            return result;
        }

        for (JsonElement element : list) {
            if (!element.isJsonArray()) {
                continue;
            }
            JsonArray pair = element.getAsJsonArray();
            if (pair.size() < 2 || !pair.get(1).isJsonObject()) {
                continue;
            }
            JsonObject value = pair.get(1).getAsJsonObject();
            JsonObject handler = object(value, "handler");

            RegistryEntry entry = new RegistryEntry();
            entry.uuid = firstNonEmpty(string(value, "uuid"), stringValue(pair.get(0)));
            entry.name = string(value, "name");
            entry.index = firstNonNull(integer(value, "localIndex"), integer(value, "index"));
            entry.typeIndex = integer(value, "typeIndex");
            entry.storageAddress = string(value, "__addr");
            entry.baseVtable = string(value, "baseVtable");
            entry.vtable = string(value, "vtable");
            if (handler != null) {
                entry.destructor = string(handler, "Destructor");
                entry.getEmptyValue = string(handler, "GetEmptyValue");
                entry.createInstance = string(handler, "CreateInstance");
                entry.copyValue = string(handler, "CopyValue");
                entry.marshal = string(handler, "Marshal");
                entry.unmarshal = string(handler, "Unmarshal");
            }
            result.add(entry);
        }
        return result;
    }

    private JsonArray registryDescriptorArray(JsonObject data) {
        if (data == null) {
            return null;
        }

        JsonArray typesByUuid = array(data, "typesByUuid");
        if (typesByUuid != null) {
            return typesByUuid;
        }

        return array(data, "m_list");
    }

    private Integer typeIndexSlotCount(JsonObject root) {
        JsonObject data = object(root, "data");
        JsonObject counts = object(data, "counts");
        Integer fromCounts = integer(counts, "typeIndexSlots");
        if (fromCounts != null) {
            return fromCounts;
        }
        JsonArray typeIndex = data == null ? null : array(data, "typeIndex");
        return typeIndex == null ? null : typeIndex.size();
    }

    private Integer registeredTypeCount(JsonObject root, int parsedRegistryEntries) {
        JsonObject data = object(root, "data");
        JsonObject counts = object(data, "counts");
        Integer fromCounts = integer(counts, "registeredTypes");
        return fromCounts == null ? parsedRegistryEntries : fromCounts;
    }

    private Integer assetOnlyTypeIndexSlotCount(JsonObject root, int parsedRegistryEntries) {
        Integer slots = typeIndexSlotCount(root);
        if (slots == null) {
            return null;
        }
        Integer registered = registeredTypeCount(root, parsedRegistryEntries);
        return slots - registered;
    }

    private Map<String, RegistrationFunction> collectRegistrationFunctions(Address registerField) {
        LinkedHashMap<String, RegistrationFunction> result = new LinkedHashMap<>();
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(registerField);
        while (references.hasNext()) {
            Reference reference = references.next();
            if (!reference.getReferenceType().isCall()) {
                continue;
            }
            Address callsite = reference.getFromAddress();
            Function owner = functionContaining(callsite);
            if (owner == null) {
                continue;
            }
            String key = owner.getEntryPoint().toString();
            RegistrationFunction function = result.get(key);
            if (function == null) {
                function = new RegistrationFunction(owner);
                function.instanceVtable = findInstanceVtable(owner);
                function.azRtti = decodeAzRttiFromVtable(function.instanceVtable);
                result.put(key, function);
            }
            FieldCall field = recoverFieldCall(owner, callsite, function.fields.size());
            function.fields.add(field);
        }
        return result;
    }

    private List<RegistrationFunction> constructorMatches(
        RegistryEntry entry,
        Map<String, RegistrationFunction> registrationFunctions) {

        ArrayList<RegistrationFunction> result = new ArrayList<>();
        Set<String> seen = new LinkedHashSet<>();
        for (Address target : unmarshalCallTargets(entry)) {
            Function function = functionAtOrContaining(target);
            if (function == null) {
                continue;
            }
            RegistrationFunction match = registrationFunctions.get(function.getEntryPoint().toString());
            if (match == null) {
                continue;
            }
            if (seen.add(function.getEntryPoint().toString())) {
                result.add(match);
            }
        }
        return result;
    }

    private AzRttiEvidence resolvedAzRtti(
        RegistryEntry entry,
        List<RegistrationFunction> matches) {

        String registryTypeId = canonicalUuidFromString(entry.uuid);
        for (RegistrationFunction match : matches) {
            if (match.azRtti == null || match.azRtti.typeId == null) {
                continue;
            }
            if (registryTypeId == null || uuidEquals(registryTypeId, match.azRtti.typeId)) {
                return match.azRtti;
            }
        }

        return null;
    }

    private HookTypeEvidence registrationHookForEntry(
        RegistryEntry entry,
        Map<String, HookTypeEvidence> hookTypeNamesById) {

        String registryTypeId = canonicalUuidFromString(entry.uuid);
        return registryTypeId == null ? null : hookTypeNamesById.get(normalizeUuid(registryTypeId));
    }

    private Map<String, HookTypeEvidence> collectRegistrationHookTypeNames() {
        LinkedHashMap<String, HookTypeEvidence> result = new LinkedHashMap<>();
        collectQueuedRegistrationHooks(result);

        SymbolIterator symbols = currentProgram.getSymbolTable().getAllSymbols(true);
        while (symbols.hasNext()) {
            Symbol symbol = symbols.next();
            String typeName = registrationHookTypeName(symbol);
            if (!isPlausibleTypeName(typeName)) {
                continue;
            }

            Function function = currentProgram.getFunctionManager().getFunctionAt(symbol.getAddress());
            HookTypeEvidence hook = function == null
                ? decodeRegistrationHookTypeDescriptor(symbol.getAddress(), typeName)
                : decodeRegistrationHook(function, typeName);
            if (hook == null || hook.typeId == null) {
                continue;
            }

            result.putIfAbsent(normalizeUuid(hook.typeId), hook);
        }
        println("Recovered InstallRegistrationHook type names: " + result.size());
        return result;
    }

    private void collectQueuedRegistrationHooks(Map<String, HookTypeEvidence> result) {
        Address queueRegistrationHook =
            currentProgram.getImageBase().add(QUEUE_REGISTRATION_HOOK_RVA);
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(queueRegistrationHook);
        while (references.hasNext()) {
            Reference reference = references.next();
            if (!reference.getReferenceType().isCall()) {
                continue;
            }

            Function owner = functionContaining(reference.getFromAddress());
            if (owner == null) {
                continue;
            }
            Address helperTable = findRegistrationHelperTable(owner);
            if (helperTable == null) {
                continue;
            }
            HookTypeEvidence hook = decodeRegistrationHelperTable(
                owner.getEntryPoint(),
                helperTable,
                registrationHookTypeNameFromSlot(helperTable, 3));
            if (hook == null || hook.typeId == null) {
                continue;
            }
            result.putIfAbsent(normalizeUuid(hook.typeId), hook);
        }
    }

    private String registrationHookTypeName(Symbol symbol) {
        String qualifiedName = symbol.getName(true);
        String parsed = parseInstallRegistrationHookTypeName(qualifiedName);
        if (parsed != null) {
            return parsed;
        }

        if (!"InstallRegistrationHook".equals(symbol.getName())) {
            return null;
        }

        Function function = currentProgram.getFunctionManager().getFunctionAt(symbol.getAddress());
        if (function == null) {
            return null;
        }
        String namespace = function.getParentNamespace() == null
            ? null
            : function.getParentNamespace().getName(true);
        if (namespace == null || namespace.isEmpty() ||
            "Global".equals(namespace) || "Amazon::Hub".equals(namespace)) {
            return null;
        }
        return namespace;
    }

    private String parseInstallRegistrationHookTypeName(String value) {
        if (value == null) {
            return null;
        }
        Matcher matcher = INSTALL_REGISTRATION_HOOK_RE.matcher(value);
        if (!matcher.find()) {
            return null;
        }
        String typeName = matcher.group("type").trim();
        if (typeName.startsWith("class ")) {
            typeName = typeName.substring("class ".length()).trim();
        }
        else if (typeName.startsWith("struct ")) {
            typeName = typeName.substring("struct ".length()).trim();
        }
        else if (typeName.startsWith("class_")) {
            typeName = typeName.substring("class_".length()).trim();
        }
        else if (typeName.startsWith("struct_")) {
            typeName = typeName.substring("struct_".length()).trim();
        }
        return isPlausibleTypeName(typeName) ? typeName : null;
    }

    private HookTypeEvidence decodeRegistrationHook(Function hookFunction, String typeName) {
        Address helperTable = findRegistrationHelperTable(hookFunction);
        if (helperTable == null) {
            TypeIdDecode directTypeId =
                decodeRegistrationThunkTypeId(hookFunction.getEntryPoint());
            if (directTypeId == null) {
                return null;
            }

            HookTypeEvidence hook = new HookTypeEvidence();
            hook.typeName = typeName;
            hook.typeId = directTypeId.typeId;
            hook.hookFunction = hookFunction.getEntryPoint();
            hook.registerThunk = hookFunction.getEntryPoint();
            hook.typeProvider = directTypeId.provider;
            hook.uuidSource = directTypeId.sourceAddress;
            enrichHookMessageHandler(hook);
            return hook;
        }

        return decodeRegistrationHelperTable(hookFunction.getEntryPoint(), helperTable, typeName);
    }

    private HookTypeEvidence decodeRegistrationHelperTable(
        Address hookFunction,
        Address helperTable,
        String typeName) {

        if (helperTable == null || !isPlausibleTypeName(typeName)) {
            return null;
        }

        Address registerThunk = resolvedCodeTarget(readPointer(helperTable.add(0x10)));
        TypeIdDecode typeId = decodeRegistrationThunkTypeId(registerThunk);
        if (typeId == null) {
            return null;
        }

        HookTypeEvidence hook = new HookTypeEvidence();
        hook.typeName = typeName;
        hook.typeId = typeId.typeId;
        hook.hookFunction = hookFunction;
        hook.helperTable = helperTable;
        hook.registerThunk = registerThunk;
        hook.typeProvider = typeId.provider;
        hook.uuidSource = typeId.sourceAddress;
        enrichHookMessageHandler(hook);

        String slotTypeName = registrationHookTypeNameFromSlot(helperTable, 3);
        if (isPlausibleTypeName(slotTypeName)) {
            hook.slotTypeName = slotTypeName;
        }
        return hook;
    }

    private HookTypeEvidence decodeRegistrationHookTypeDescriptor(
        Address typeDescriptor,
        String typeName) {

        ReferenceIterator descriptorReferences =
            currentProgram.getReferenceManager().getReferencesTo(typeDescriptor);
        while (descriptorReferences.hasNext()) {
            Reference descriptorReference = descriptorReferences.next();
            Function slotFunction = functionContaining(descriptorReference.getFromAddress());
            if (slotFunction == null) {
                continue;
            }

            ReferenceIterator slotReferences =
                currentProgram.getReferenceManager().getReferencesTo(slotFunction.getEntryPoint());
            while (slotReferences.hasNext()) {
                Reference slotReference = slotReferences.next();
                Address helperTable = subtract(slotReference.getFromAddress(), 0x18L);
                if (!isRegistrationHelperTable(helperTable)) {
                    continue;
                }

                Address registerThunk = resolvedCodeTarget(readPointer(helperTable.add(0x10)));
                TypeIdDecode typeId = decodeRegistrationThunkTypeId(registerThunk);
                if (typeId == null) {
                    continue;
                }

                HookTypeEvidence hook = new HookTypeEvidence();
                hook.typeName = typeName;
                hook.slotTypeName = registrationHookTypeNameFromSlot(helperTable, 3);
                hook.typeId = typeId.typeId;
                hook.helperTable = helperTable;
                hook.registerThunk = registerThunk;
                hook.typeProvider = typeId.provider;
                hook.uuidSource = typeId.sourceAddress;
                hook.typeDescriptor = typeDescriptor;
                hook.slotTypeNameFunction = slotFunction.getEntryPoint();
                enrichHookMessageHandler(hook);
                return hook;
            }
        }
        return null;
    }

    private void enrichHookMessageHandler(HookTypeEvidence hook) {
        if (hook == null || hook.typeProvider == null) {
            return;
        }
        Address handlerVtable = recoverMessageHandlerVtable(hook.typeProvider);
        if (handlerVtable == null) {
            return;
        }

        hook.handlerVtable = handlerVtable;
        hook.createInstance = messageHandlerSlot(handlerVtable, MESSAGE_HANDLER_CREATE_INSTANCE_SLOT);
        hook.marshal = messageHandlerSlot(handlerVtable, MESSAGE_HANDLER_MARSHAL_SLOT);
        hook.unmarshal = messageHandlerSlot(handlerVtable, MESSAGE_HANDLER_UNMARSHAL_SLOT);
    }

    private Address recoverMessageHandlerVtable(Address typeProvider) {
        Function function = functionAtOrContaining(typeProvider);
        if (function == null) {
            return null;
        }

        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= MESSAGE_HANDLER_PROVIDER_SCAN_LIMIT) {
                break;
            }

            Address candidate = referencedAddress(instruction);
            if (isMessageHandlerVtable(candidate)) {
                return candidate;
            }
        }
        return null;
    }

    private boolean isMessageHandlerVtable(Address address) {
        if (!isProgramAddress(address)) {
            return false;
        }

        Address createInstance = messageHandlerSlot(address, MESSAGE_HANDLER_CREATE_INSTANCE_SLOT);
        Address marshal = messageHandlerSlot(address, MESSAGE_HANDLER_MARSHAL_SLOT);
        Address unmarshal = messageHandlerSlot(address, MESSAGE_HANDLER_UNMARSHAL_SLOT);
        if (!isExecutableAddress(createInstance) ||
            !isExecutableAddress(marshal) ||
            !isExecutableAddress(unmarshal)) {
            return false;
        }

        Function unmarshalFunction = functionAtOrContaining(unmarshal);
        return looksLikeMessageUnmarshalHelper(decompileC(unmarshalFunction));
    }

    private Address messageHandlerSlot(Address vtable, int slot) {
        if (!isProgramAddress(vtable)) {
            return null;
        }
        return resolvedCodeTarget(readPointer(vtable.add(slot * 8L)));
    }

    private String messageHandlerSlotName(int slot) {
        if (slot == MESSAGE_HANDLER_CREATE_INSTANCE_SLOT) {
            return "CreateInstance";
        }
        if (slot == MESSAGE_HANDLER_MARSHAL_SLOT) {
            return "Marshal";
        }
        if (slot == MESSAGE_HANDLER_UNMARSHAL_SLOT) {
            return "Unmarshal";
        }
        return null;
    }

    private Address findRegistrationHelperTable(Function function) {
        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= BACKWARD_ARGUMENT_SCAN_LIMIT) {
                break;
            }
            if (!"LEA".equals(upperMnemonic(instruction))) {
                continue;
            }
            Address address = referencedAddress(instruction);
            if (isRegistrationHelperTable(address)) {
                return address;
            }
        }
        return null;
    }

    private boolean isRegistrationHelperTable(Address address) {
        if (!isProgramAddress(address)) {
            return false;
        }
        Address slot2 = resolvedCodeTarget(readPointer(address.add(0x10)));
        Address slot3 = resolvedCodeTarget(readPointer(address.add(0x18)));
        Address slot4 = resolvedCodeTarget(readPointer(address.add(0x20)));
        return isExecutableAddress(slot2) &&
            isExecutableAddress(slot3) &&
            isExecutableAddress(slot4) &&
            isPlausibleTypeName(registrationHookTypeNameFromSlot(address, 3));
    }

    private String registrationHookTypeNameFromSlot(Address helperTable, int slot) {
        Address target = resolvedCodeTarget(readPointer(helperTable.add(slot * 8L)));
        Address typeDescriptor = stringAddressReturnedBySimpleFunction(target);
        if (!isProgramAddress(typeDescriptor)) {
            return null;
        }
        Symbol symbol = currentProgram.getSymbolTable().getPrimarySymbol(typeDescriptor);
        if (symbol == null) {
            return null;
        }
        return parseInstallRegistrationHookTypeName(symbol.getName(true));
    }

    private TypeIdDecode decodeRegistrationThunkTypeId(Address registerThunk) {
        Function function = functionAtOrContaining(registerThunk);
        if (function == null) {
            return null;
        }

        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address target = callTarget(instruction);
            TypeIdDecode typeId = decodeAzRttiTypeIdProvider(target);
            if (typeId != null) {
                return typeId;
            }
        }
        return null;
    }

    private JsonArray hookTypeNamesJson(Map<String, HookTypeEvidence> hooksById) {
        JsonArray array = new JsonArray();
        for (HookTypeEvidence hook : hooksById.values()) {
            array.add(hook.toJson());
        }
        return array;
    }

    private JsonArray fieldHandlerVtablesJson(
        Map<String, RegistrationFunction> registrationFunctions) {

        LinkedHashMap<String, FieldHandlerVtable> vtables = new LinkedHashMap<>();
        for (RegistrationFunction function : registrationFunctions.values()) {
            for (FieldCall field : function.fields) {
                if (field.handlerVtable == null) {
                    continue;
                }
                String key = field.handlerVtable.toString();
                FieldHandlerVtable vtable = vtables.get(key);
                if (vtable == null) {
                    vtable = new FieldHandlerVtable(field.handlerVtable);
                    vtables.put(key, vtable);
                }
                vtable.fieldCount++;
            }
        }

        JsonArray array = new JsonArray();
        for (FieldHandlerVtable vtable : vtables.values()) {
            array.add(vtable.toJson());
        }
        return array;
    }

    private List<Address> unmarshalCallTargets(RegistryEntry entry) {
        ArrayList<Address> result = new ArrayList<>();
        Address unmarshalAddress = parseCapturedAddress(entry.unmarshal);
        Function unmarshal = functionAtOrContaining(unmarshalAddress);
        if (unmarshal == null) {
            return result;
        }
        for (Instruction instruction : functionInstructions(unmarshal)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address target = callTarget(instruction);
            if (target != null) {
                result.add(target);
            }
        }
        return result;
    }

    private MessageUnmarshalPlan recoverMessageUnmarshalPlan(
        RegistryEntry entry,
        HookTypeEvidence hookType) {

        Address createInstanceAddress = firstAddress(
            parseCapturedAddress(entry.createInstance),
            hookType == null ? null : hookType.createInstance);
        Address unmarshalAddress = firstAddress(
            parseCapturedAddress(entry.unmarshal),
            hookType == null ? null : hookType.unmarshal);
        Function wrapper = functionAtOrContaining(unmarshalAddress);
        if (wrapper == null) {
            return null;
        }

        MessageHelperCall helperCall = findMessageHelperCall(wrapper);
        String wrapperText = decompileC(wrapper);
        if (helperCall == null) {
            return recoverFallbackMessageUnmarshalPlan(
                entry,
                wrapper,
                wrapperText,
                createInstanceAddress);
        }

        ParsedUnmarshalFieldsCall parsedCall = parseUnmarshalFieldsCall(wrapperText);
        if (parsedCall == null) {
            return recoverFallbackMessageUnmarshalPlan(
                entry,
                wrapper,
                wrapperText,
                createInstanceAddress);
        }

        MessageUnmarshalPlan plan = newMessageUnmarshalPlan(entry, wrapper, createInstanceAddress);
        plan.templateTypes.addAll(parsedCall.templateTypes);
        plan.helperCallsite = helperCall.callsite;
        plan.helper = helperCall.target;
        plan.helperName = helperCall.targetName;
        MessageConstructorCall constructor = findMessageConstructorCall(wrapper, helperCall.callsite);
        if (constructor != null) {
            plan.instanceConstructorCallsite = constructor.callsite;
            plan.instanceConstructor = constructor.target;
            plan.instanceConstructorName = constructor.targetName;
        }

        int templateIndex = 0;
        for (int i = 0; i < parsedCall.fieldArgs.size(); i++) {
            ParsedArgument arg = parsedCall.fieldArgs.get(i);
            String castNativeType = nativeTypeFromCast(arg.castType);
            String nativeType = castNativeType;
            if (templateIndex < parsedCall.templateTypes.size()) {
                String templateType = parsedCall.templateTypes.get(templateIndex);
                if (shouldUseTemplateType(castNativeType, templateType)) {
                    nativeType = templateType;
                    templateIndex++;
                }
                else if (castNativeType != null &&
                    templateMatchesCast(templateType, castNativeType)) {
                    nativeType = templateType;
                    templateIndex++;
                }
            }

            FieldCall field = new FieldCall();
            field.index = i;
            field.callsite = plan.helperCallsite;
            field.name = "field_" + i;
            field.nativeType = nativeType;
            field.storageExpression = arg.expression;
            field.storageOffset = storageByteOffsetFromExpression(arg.expression);
            field.wireShape = wireShapeFromNativeType(nativeType);
            field.wireShapeSource = field.wireShape == null
                ? null
                : "message-unmarshal-native-type";
            field.confidence = "message-unmarshal-call";
            plan.fields.add(field);
        }
        refineMessageFieldsFromHelper(plan);
        enrichMessageFieldsFromSourceSignatures(entry, plan);
        return plan;
    }

    private MessageUnmarshalPlan newMessageUnmarshalPlan(
        RegistryEntry entry,
        Function wrapper,
        Address createInstance) {

        MessageUnmarshalPlan plan = new MessageUnmarshalPlan();
        plan.wrapper = wrapper.getEntryPoint();
        plan.wrapperName = fullFunctionName(wrapper);
        Long instanceSize = recoverCreateInstanceSize(createInstance);
        if (instanceSize != null) {
            plan.instanceSize = instanceSize;
            plan.instanceSizeSource = "create-instance-operator-new";
            plan.createInstance = createInstance;
        }
        return plan;
    }

    private MessageUnmarshalPlan recoverFallbackMessageUnmarshalPlan(
        RegistryEntry entry,
        Function wrapper,
        String wrapperText,
        Address createInstanceAddress) {

        if (wrapperText == null) {
            return null;
        }

        MessageUnmarshalPlan plan =
            newMessageUnmarshalPlan(entry, wrapper, createInstanceAddress);
        recoverInlineMessageFields(plan, wrapper.getEntryPoint(), wrapperText);
        recoverHelperArgumentMessageFields(plan, wrapper, wrapperText);
        if (plan.fields.isEmpty()) {
            return null;
        }
        sortMessageFieldsByRecoveryOrder(plan);
        enrichMessageFieldsFromSourceSignatures(entry, plan);
        return plan;
    }

    private void recoverInlineMessageFields(
        MessageUnmarshalPlan plan,
        Address callsite,
        String text) {

        for (ParsedUnmarshalCall call : parseMarshalerUnmarshalCalls(text)) {
            String storage = storageArgumentForMarshalerCall(call);
            if (!isLikelyMessageStorage(storage)) {
                continue;
            }
            addMessageField(
                plan,
                callsite,
                storage,
                call.templateType,
                "message-unmarshal-inline-call",
                call.textIndex);
        }
        for (ParsedUnmarshalCall call : parseDirectTypeUnmarshalCalls(text)) {
            String storage = storageArgumentForDirectUnmarshalCall(call);
            if (!isLikelyMessageStorage(storage)) {
                continue;
            }
            addMessageField(
                plan,
                callsite,
                storage,
                call.templateType,
                "message-unmarshal-inline-direct-type-call",
                call.textIndex);
        }
        for (ParsedReadRawCall call : parseReadRawCalls(text)) {
            if (!isLikelyMessageStorage(call.storageExpression)) {
                continue;
            }
            addRawMessageField(
                plan,
                callsite,
                call.storageExpression,
                call.byteLength,
                "message-unmarshal-read-raw",
                call.textIndex);
        }
    }

    private void recoverHelperArgumentMessageFields(
        MessageUnmarshalPlan plan,
        Function wrapper,
        String wrapperText) {

        for (Instruction instruction : functionInstructions(wrapper)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }

            Function helper = functionAtOrContaining(callTarget(instruction));
            if (helper == null) {
                continue;
            }

            List<String> wrapperArgs = callArgumentsForTarget(wrapperText, helper);
            if (wrapperArgs.size() <= 2) {
                continue;
            }

            int helperTextIndex = callTextIndexForTarget(wrapperText, helper);
            List<String> storageArgs = likelyMessageStorageArgs(wrapperArgs);
            String directTemplateType = unmarshalTemplateType(fullFunctionName(helper));
            if (directTemplateType != null) {
                if (storageArgs.size() == 1) {
                    addMessageField(
                        plan,
                        instruction.getMinAddress(),
                        storageArgs.get(0),
                        directTemplateType,
                        "message-unmarshal-helper-direct",
                        helperTextIndex);
                    continue;
                }
            }

            String helperText = decompileC(helper);
            if (!looksLikeMessageUnmarshalHelper(helperText)) {
                continue;
            }

            String singleTemplateType = singleMarshalerUnmarshalTemplateType(helperText);
            if (singleTemplateType != null && storageArgs.size() == 1) {
                if (!storageArgs.isEmpty()) {
                    addMessageField(
                        plan,
                        instruction.getMinAddress(),
                        storageArgs.get(0),
                        singleTemplateType,
                        "message-unmarshal-helper-wrapper",
                        helperTextIndex);
                }
                continue;
            }

            if (plan.helper == null) {
                plan.helperCallsite = instruction.getMinAddress();
                plan.helper = helper.getEntryPoint();
                plan.helperName = fullFunctionName(helper);
            }

            List<String> helperParams = parameterNamesFromDecompiledFunction(helperText);
            HashMap<String, FieldCall> fieldsByHelperParam = new HashMap<>();
            for (int i = 2; i < wrapperArgs.size(); i++) {
                String storage = wrapperArgs.get(i);
                if (!isLikelyMessageStorage(storage)) {
                    continue;
                }
                FieldCall field = addMessageField(
                    plan,
                    instruction.getMinAddress(),
                    storage,
                    null,
                    "message-unmarshal-helper-argument",
                    relativeRecoveryOrder(helperTextIndex, i));
                if (i < helperParams.size()) {
                    fieldsByHelperParam.put(helperParams.get(i), field);
                }
            }

            refineHelperArgumentFieldTypes(helperText, fieldsByHelperParam);
            refineDirectBoolWrites(helperText, fieldsByHelperParam);
            refineNestedBoolWrites(helper, helperText, fieldsByHelperParam);
        }
    }

    private boolean looksLikeMessageUnmarshalHelper(String text) {
        return text != null &&
            (text.contains("Marshaler<") ||
                text.contains("::Unmarshal(") ||
                text.contains("ReadBuffer::ReadRaw") ||
                BOOL_POINTER_WRITE_RE.matcher(text).find());
    }

    private String singleMarshalerUnmarshalTemplateType(String text) {
        List<ParsedUnmarshalCall> calls = parseMarshalerUnmarshalCalls(text);
        if (isActorRefMarshalerShape(text, calls)) {
            return "ActorRef";
        }
        return calls.size() == 1 ? calls.get(0).templateType : null;
    }

    private boolean isActorRefMarshalerShape(String text, List<ParsedUnmarshalCall> calls) {
        if (text == null || calls.size() != 1 ||
            !"u32".equals(sourceTypeLeaf(calls.get(0).templateType))) {
            return false;
        }
        int readRawCount = 0;
        int search = 0;
        while (search < text.length()) {
            int index = text.indexOf("ReadRaw", search);
            if (index < 0) {
                break;
            }
            readRawCount++;
            search = index + "ReadRaw".length();
        }
        return readRawCount == 2 && text.contains("0x10");
    }

    private List<String> likelyMessageStorageArgs(List<String> args) {
        ArrayList<String> result = new ArrayList<>();
        if (args == null) {
            return result;
        }
        for (String arg : args) {
            if (isLikelyMessageStorage(arg)) {
                result.add(arg);
            }
        }
        return result;
    }

    private int relativeRecoveryOrder(int base, int offset) {
        return base == Integer.MAX_VALUE ? Integer.MAX_VALUE : base + offset;
    }

    private void refineHelperArgumentFieldTypes(
        String helperText,
        Map<String, FieldCall> fieldsByHelperParam) {

        if (fieldsByHelperParam.isEmpty()) {
            return;
        }
        for (ParsedUnmarshalCall call : parseMarshalerUnmarshalCalls(helperText)) {
            String storage = storageArgumentForMarshalerCall(call);
            String helperParam = helperParameterFromExpression(storage, fieldsByHelperParam);
            if (helperParam == null) {
                continue;
            }
            FieldCall field = fieldsByHelperParam.get(helperParam);
            refineMessageFieldType(
                field,
                call.templateType,
                "message-unmarshal-helper-nested-call");
        }
        for (ParsedUnmarshalCall call : parseDirectTypeUnmarshalCalls(helperText)) {
            String storage = storageArgumentForDirectUnmarshalCall(call);
            String helperParam = helperParameterFromExpression(storage, fieldsByHelperParam);
            if (helperParam == null) {
                continue;
            }
            FieldCall field = fieldsByHelperParam.get(helperParam);
            refineMessageFieldType(
                field,
                call.templateType,
                "message-unmarshal-helper-direct-type-call");
        }
        for (ParsedReadRawCall call : parseReadRawCalls(helperText)) {
            String helperParam =
                helperParameterFromExpression(call.storageExpression, fieldsByHelperParam);
            if (helperParam == null) {
                continue;
            }
            refineMessageFieldRawLength(
                fieldsByHelperParam.get(helperParam),
                call.byteLength,
                "message-unmarshal-helper-read-raw");
        }
    }

    private FieldCall addMessageField(
        MessageUnmarshalPlan plan,
        Address callsite,
        String storageExpression,
        String nativeType,
        String confidence) {
        return addMessageField(
            plan,
            callsite,
            storageExpression,
            nativeType,
            confidence,
            Integer.MAX_VALUE);
    }

    private FieldCall addMessageField(
        MessageUnmarshalPlan plan,
        Address callsite,
        String storageExpression,
        String nativeType,
        String confidence,
        int recoveryOrder) {

        String storage = normalizedExpression(storageExpression);
        for (FieldCall existing : plan.fields) {
            if (storage != null && storage.equals(normalizedExpression(existing.storageExpression))) {
                refineMessageFieldType(existing, nativeType, confidence);
                existing.recoveryOrder = Math.min(existing.recoveryOrder, recoveryOrder);
                return existing;
            }
        }

        FieldCall field = new FieldCall();
        field.index = plan.fields.size();
        field.callsite = callsite;
        field.recoveryOrder = recoveryOrder;
        field.name = "field_" + field.index;
        field.nativeType = nativeType;
        field.storageExpression = storageExpression;
        field.storageOffset = storageByteOffsetFromExpression(storageExpression);
        field.wireShape = wireShapeFromNativeType(nativeType);
        field.wireShapeSource = field.wireShape == null ? null : confidence;
        field.confidence = confidence;

        String derived = sourceFieldNameFromType(nativeType);
        if (derived != null) {
            field.name = derived;
            field.nameSource = "message-native-type-name";
            field.sourceTypeName = nativeType;
        }

        plan.fields.add(field);
        return field;
    }

    private FieldCall addRawMessageField(
        MessageUnmarshalPlan plan,
        Address callsite,
        String storageExpression,
        int byteLength,
        String confidence,
        int recoveryOrder) {

        FieldCall field = addMessageField(
            plan,
            callsite,
            storageExpression,
            null,
            confidence,
            recoveryOrder);
        refineMessageFieldRawLength(field, byteLength, confidence);
        return field;
    }

    private void sortMessageFieldsByRecoveryOrder(MessageUnmarshalPlan plan) {
        plan.fields.sort((left, right) -> {
            int order = Integer.compare(left.recoveryOrder, right.recoveryOrder);
            if (order != 0) {
                return order;
            }
            return Integer.compare(left.index, right.index);
        });
        for (int i = 0; i < plan.fields.size(); i++) {
            FieldCall field = plan.fields.get(i);
            if (isGeneratedFieldName(field.name)) {
                field.name = "field_" + i;
            }
            field.index = i;
        }
    }

    private void refineMessageFieldType(
        FieldCall field,
        String nativeType,
        String source) {

        if (field == null || nativeType == null || nativeType.isEmpty()) {
            return;
        }
        if (field.nativeType == null) {
            field.nativeType = nativeType;
            field.wireShape = wireShapeFromNativeType(nativeType);
            field.wireShapeSource = field.wireShape == null ? null : source;
            return;
        }
        if (!field.nativeType.equals(nativeType)) {
            field.sourceTypeName = appendDistinctType(field.sourceTypeName, field.nativeType);
            field.sourceTypeName = appendDistinctType(field.sourceTypeName, nativeType);
            field.nativeType = "composite";
            field.wireShape = null;
            field.wireShapeSource = null;
        }
    }

    private void refineMessageFieldRawLength(
        FieldCall field,
        int byteLength,
        String source) {

        if (field == null || byteLength <= 0) {
            return;
        }
        field.rawByteLength = byteLength;
        String shape = wireShapeFromRawByteLength(byteLength);
        if (shape == null) {
            return;
        }
        if (field.wireShape == null || field.wireShape.equals(shape)) {
            field.wireShape = shape;
            field.wireShapeSource = source;
        }
    }

    private String appendDistinctType(String existing, String value) {
        if (value == null || value.isEmpty()) {
            return existing;
        }
        if (existing == null || existing.isEmpty()) {
            return value;
        }
        for (String part : existing.split(",")) {
            if (part.trim().equals(value)) {
                return existing;
            }
        }
        return existing + "," + value;
    }

    private void enrichMessageFieldsFromSourceSignatures(
        RegistryEntry entry,
        MessageUnmarshalPlan plan) {

        List<MessageSourceSignature> signatures = recoverMessageSourceSignatures(entry, plan);
        plan.sourceSignatures.addAll(signatures);
        List<String> sourceNames = sourceMessageFieldNames(entry.name, plan);
        Address sourceAddress = signatures.isEmpty() ? null : signatures.get(0).stringAddress;

        for (int i = 0; i < plan.fields.size(); i++) {
            FieldCall field = plan.fields.get(i);
            if (i < sourceNames.size()) {
                field.name = sourceNames.get(i);
                field.nameSource = signatures.isEmpty()
                    ? "source-message-name-table"
                    : "msvc-rtti-source-signature";
                field.nameSourceAddress = sourceAddress;
                field.sourceTypeName = sourceTypeNameForMessageField(entry.name, i, field);
                continue;
            }

            String derived = sourceFieldNameFromType(field.nativeType);
            if (derived != null && isGeneratedFieldName(field.name)) {
                field.name = derived;
                field.nameSource = "message-native-type-name";
                field.nameSourceAddress = sourceAddress;
                field.sourceTypeName = field.nativeType;
            }
        }
    }

    private List<MessageSourceSignature> recoverMessageSourceSignatures(
        RegistryEntry entry,
        MessageUnmarshalPlan plan) {

        ArrayList<MessageSourceSignature> result = new ArrayList<>();
        LinkedHashSet<String> tokens = sourceSignatureSearchTokens(entry, plan);
        LinkedHashSet<String> seenStrings = new LinkedHashSet<>();
        for (String token : tokens) {
            for (Address stringAddress : asciiStringsContaining(token)) {
                String value = readPrintableString(stringAddress);
                if (!sourceSignatureMatchesMessagePlan(value, entry, plan) ||
                    !seenStrings.add(formatAddress(stringAddress))) {
                    continue;
                }

                MessageSourceSignature signature = new MessageSourceSignature();
                signature.stringAddress = stringAddress;
                signature.mangledName = value;
                signature.typeDescriptor = msvcTypeDescriptorForString(stringAddress);
                enrichSourceSignatureXrefs(signature);
                result.add(signature);
                if (result.size() >= 4) {
                    return result;
                }
            }
        }
        return result;
    }

    private LinkedHashSet<String> sourceSignatureSearchTokens(
        RegistryEntry entry,
        MessageUnmarshalPlan plan) {

        LinkedHashSet<String> result = new LinkedHashSet<>();
        for (String templateType : plan.templateTypes) {
            String leaf = sourceTypeLeaf(templateType);
            if (leaf != null && leaf.length() >= 8 && !"AZStd::string".equals(templateType)) {
                result.add(leaf);
            }
        }
        String messageLeaf = messageLeafName(entry.name);
        if (messageLeaf != null && messageLeaf.length() >= 8) {
            result.add(messageLeaf);
        }
        return result;
    }

    private boolean sourceSignatureMatchesMessagePlan(
        String value,
        RegistryEntry entry,
        MessageUnmarshalPlan plan) {

        if (value == null || !value.contains("<lambda_")) {
            return false;
        }

        int matchedTypes = 0;
        for (String templateType : plan.templateTypes) {
            String leaf = sourceTypeLeaf(templateType);
            if (leaf != null && value.contains(leaf)) {
                matchedTypes++;
            }
        }

        if (matchedTypes >= Math.min(2, Math.max(1, plan.templateTypes.size()))) {
            return true;
        }

        String messageLeaf = messageLeafName(entry.name);
        return messageLeaf != null && value.contains(messageLeaf);
    }

    private void enrichSourceSignatureXrefs(MessageSourceSignature signature) {
        if (signature == null || !isProgramAddress(signature.typeDescriptor)) {
            return;
        }

        ReferenceIterator descriptorReferences =
            currentProgram.getReferenceManager().getReferencesTo(signature.typeDescriptor);
        while (descriptorReferences.hasNext()) {
            Reference reference = descriptorReferences.next();
            Function provider = functionContaining(reference.getFromAddress());
            if (provider == null) {
                continue;
            }
            signature.addProvider(provider, this);
            collectSourceSignatureProviderRefs(signature, provider);
        }
    }

    private void collectSourceSignatureProviderRefs(
        MessageSourceSignature signature,
        Function provider) {

        ReferenceIterator providerReferences =
            currentProgram.getReferenceManager().getReferencesTo(provider.getEntryPoint());
        while (providerReferences.hasNext()) {
            Reference reference = providerReferences.next();
            Address from = reference.getFromAddress();
            if (isExecutableAddress(from)) {
                Function caller = functionContaining(from);
                if (caller != null) {
                    collectSourceCallGraph(signature, caller, SOURCE_SIGNATURE_CALL_GRAPH_DEPTH);
                }
                continue;
            }

            signature.tableReferences.add(formatAddress(from));
            collectFunctionPointersNear(signature, from);
        }
    }

    private void collectFunctionPointersNear(MessageSourceSignature signature, Address address) {
        Address start = subtract(address, SOURCE_SIGNATURE_XREF_SCAN_BYTES);
        if (!isProgramAddress(start)) {
            start = address;
        }

        for (int offset = 0; offset <= SOURCE_SIGNATURE_XREF_SCAN_BYTES * 2; offset += 8) {
            Address pointerAddress = start.add(offset);
            Address pointer = readPointer(pointerAddress);
            Address target = resolvedCodeTarget(pointer);
            Function function = functionAtOrContaining(target);
            if (function == null || signature.sourceFunctions.size() >=
                SOURCE_SIGNATURE_CALL_GRAPH_LIMIT) {
                continue;
            }
            collectSourceCallGraph(signature, function, SOURCE_SIGNATURE_CALL_GRAPH_DEPTH);
        }
    }

    private void collectSourceCallGraph(
        MessageSourceSignature signature,
        Function function,
        int depth) {

        if (function == null ||
            signature.sourceFunctions.size() >= SOURCE_SIGNATURE_CALL_GRAPH_LIMIT) {
            return;
        }
        boolean inserted = signature.addSourceFunction(function, this);
        if (!inserted || depth <= 0) {
            return;
        }

        int calls = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (calls >= SOURCE_SIGNATURE_CALL_GRAPH_LIMIT ||
                signature.sourceFunctions.size() >= SOURCE_SIGNATURE_CALL_GRAPH_LIMIT) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Function target = functionAtOrContaining(resolvedCodeTarget(callTarget(instruction)));
            if (target == null) {
                continue;
            }
            calls++;
            signature.addCallTarget(function, instruction.getMinAddress(), target, this);
            collectSourceCallGraph(signature, target, depth - 1);
        }
    }

    private List<Address> asciiStringsContaining(String token) {
        if (token == null || token.isEmpty()) {
            return List.of();
        }
        String key = programTextCacheKey("ascii-strings-containing", token);
        List<Address> cached = asciiStringSearchCache.get(key);
        if (cached != null) {
            return cached;
        }

        ArrayList<Address> result = new ArrayList<>();
        byte[] bytes = token.getBytes(StandardCharsets.US_ASCII);
        for (MemoryBlock block : currentProgram.getMemory().getBlocks()) {
            if (!block.isRead()) {
                continue;
            }
            Address cursor = block.getStart();
            Address end = block.getEnd();
            while (cursor != null && cursor.compareTo(end) <= 0) {
                Address found;
                try {
                    found = currentProgram.getMemory()
                        .findBytes(cursor, bytes, null, true, monitor);
                }
                catch (Exception ignored) {
                    break;
                }
                if (found == null || found.compareTo(end) > 0) {
                    break;
                }
                Address stringStart = printableStringStartContaining(found);
                if (stringStart != null) {
                    result.add(stringStart);
                }
                cursor = found.add(1);
            }
        }
        List<Address> cachedResult = Collections.unmodifiableList(new ArrayList<>(result));
        asciiStringSearchCache.put(key, cachedResult);
        return cachedResult;
    }

    private Address printableStringStartContaining(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        Address cursor = address;
        for (int i = 0; i < 512; i++) {
            Address previous = subtract(cursor, 1);
            if (!isProgramAddress(previous)) {
                return cursor;
            }
            int value;
            try {
                value = getByte(previous) & 0xff;
            }
            catch (Exception ignored) {
                return cursor;
            }
            if (value == 0) {
                return cursor;
            }
            if (value < 0x20 || value > 0x7e) {
                return cursor;
            }
            cursor = previous;
        }
        return cursor;
    }

    private Address msvcTypeDescriptorForString(Address stringAddress) {
        String value = readPrintableString(stringAddress);
        if (value == null || !value.startsWith(".?A")) {
            return null;
        }
        Address typeDescriptor = subtract(stringAddress, 0x10);
        return isProgramAddress(typeDescriptor) ? typeDescriptor : null;
    }

    private List<String> sourceMessageFieldNames(String messageName, MessageUnmarshalPlan plan) {
        String leaf = messageLeafName(messageName);
        if ("RegistrationRequestV3Msg".equals(leaf) && plan.fields.size() == 7) {
            return List.of(
                "TypeIndexCrc",
                "ClientVersion",
                "ConnTicket",
                "LoginToken",
                "AuthToken",
                "ImpersonateInfo",
                "UseCapabilities");
        }
        if ("RegistrationRequestV2Msg".equals(leaf) && plan.fields.size() == 6) {
            return List.of(
                "TypeIndexCrc",
                "ClientVersion",
                "ConnTicket",
                "LoginToken",
                "ImpersonateInfo",
                "UseCapabilities");
        }
        if ("RegistrationRequestMsg".equals(leaf) && plan.fields.size() == 6) {
            return List.of(
                "TypeIndexCrc",
                "ClientVersion",
                "ConnTicket",
                "LoginToken",
                "ImpersonateId",
                "UseCapabilities");
        }
        return List.of();
    }

    private String sourceTypeNameForMessageField(
        String messageName,
        int index,
        FieldCall field) {

        String leaf = messageLeafName(messageName);
        if ("RegistrationRequestV3Msg".equals(leaf)) {
            return switch (index) {
                case 0 -> "AZ::Crc32";
                case 1 -> "Amazon::Configuration::ClientVersionTokenMap";
                case 2 -> "std::string";
                case 3 -> "Amazon::REP::LoginToken";
                case 4 -> "Amazon::REP::AuthToken";
                case 5 -> "Amazon::REP::ImpersonatedValues";
                case 6 -> "bool";
                default -> field.nativeType;
            };
        }
        if ("RegistrationRequestV2Msg".equals(leaf)) {
            return switch (index) {
                case 0 -> "AZ::Crc32";
                case 1 -> "Amazon::Configuration::ClientVersionTokenMap";
                case 2 -> "std::string";
                case 3 -> "Amazon::REP::LoginToken";
                case 4 -> "Amazon::REP::ImpersonatedValues";
                case 5 -> "bool";
                default -> field.nativeType;
            };
        }
        if ("RegistrationRequestMsg".equals(leaf)) {
            return switch (index) {
                case 0 -> "AZ::Crc32";
                case 1 -> "Amazon::Configuration::ClientVersionTokenMap";
                case 2 -> "std::string";
                case 3 -> "Amazon::REP::LoginToken";
                case 4 -> "Amazon::REP::CharacterId";
                case 5 -> "bool";
                default -> field.nativeType;
            };
        }
        return field.nativeType;
    }

    private String sourceFieldNameFromType(String nativeType) {
        String leaf = sourceTypeLeaf(nativeType);
        if (leaf == null || leaf.isEmpty() ||
            leaf.startsWith("u") ||
            "bool".equals(leaf) ||
            "string".equalsIgnoreCase(leaf)) {
            return null;
        }
        return leaf;
    }

    private String sourceTypeLeaf(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        String trimmed = value.trim();
        while (trimmed.endsWith("*") || trimmed.endsWith("&")) {
            trimmed = trimmed.substring(0, trimmed.length() - 1).trim();
        }
        int namespace = trimmed.lastIndexOf("::");
        return namespace >= 0 ? trimmed.substring(namespace + 2) : trimmed;
    }

    private String messageLeafName(String value) {
        String leaf = sourceTypeLeaf(value);
        if (leaf == null) {
            return null;
        }
        int template = leaf.indexOf('<');
        return template >= 0 ? leaf.substring(0, template) : leaf;
    }

    private boolean isGeneratedFieldName(String value) {
        return value != null && value.matches("field_\\d+");
    }

    private Long recoverCreateInstanceSize(Address createInstance) {
        String key = createInstance == null ? null : addressCacheKey("create-instance-size", createInstance);
        if (key != null && createInstanceSizeCache.containsKey(key)) {
            return createInstanceSizeCache.get(key);
        }
        Function function = functionAtOrContaining(createInstance);
        if (function == null) {
            if (key != null) {
                createInstanceSizeCache.put(key, null);
            }
            return null;
        }

        Long ecx = null;
        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= 16) {
                break;
            }
            String mnemonic = upperMnemonic(instruction);
            String destination = registerOperand(instruction, 0);
            if ("MOV".equals(mnemonic) && ("ECX".equals(destination) || "RCX".equals(destination))) {
                ecx = immediateValue(instruction, 1);
                continue;
            }
            if (instruction.getFlowType().isCall() && ecx != null) {
                if (key != null) {
                    createInstanceSizeCache.put(key, ecx);
                }
                return ecx;
            }
        }
        if (key != null) {
            createInstanceSizeCache.put(key, ecx);
        }
        return ecx;
    }

    private MessageConstructorCall findMessageConstructorCall(Function wrapper, Address beforeCallsite) {
        MessageConstructorCall result = null;
        for (Instruction instruction : functionInstructions(wrapper)) {
            if (beforeCallsite != null && instruction.getMinAddress().compareTo(beforeCallsite) >= 0) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address target = callTarget(instruction);
            Function function = functionAtOrContaining(target);
            if (function == null) {
                continue;
            }
            result = new MessageConstructorCall();
            result.callsite = instruction.getMinAddress();
            result.target = function.getEntryPoint();
            result.targetName = fullFunctionName(function);
        }
        return result;
    }

    private Long storageByteOffsetFromExpression(String expression) {
        expression = normalizedExpression(expression);
        if (expression == null) {
            return null;
        }
        Matcher matcher = STORAGE_OFFSET_RE.matcher(expression);
        if (!matcher.find()) {
            return null;
        }
        Long offset = parseIntegerLiteral(matcher.group("offset"));
        if (offset == null) {
            return null;
        }
        String base = matcher.group("base");
        long unit = base.startsWith("plVar") || base.startsWith("puVar") ? 8L : 1L;
        return offset * unit;
    }

    private void refineMessageFieldsFromHelper(MessageUnmarshalPlan plan) {
        if (plan == null || plan.helper == null) {
            return;
        }
        Function helper = functionAtOrContaining(plan.helper);
        String helperText = decompileC(helper);
        if (helperText == null) {
            return;
        }

        List<String> helperParams = parameterNamesFromDecompiledFunction(helperText);
        HashMap<String, FieldCall> fieldsByHelperParam = new HashMap<>();
        for (int i = 2; i < helperParams.size(); i++) {
            int fieldIndex = i - 2;
            if (fieldIndex >= 0 && fieldIndex < plan.fields.size()) {
                fieldsByHelperParam.put(helperParams.get(i), plan.fields.get(fieldIndex));
            }
        }

        refineDirectBoolWrites(helperText, fieldsByHelperParam);
        refineNestedBoolWrites(helper, helperText, fieldsByHelperParam);
    }

    private void refineDirectBoolWrites(
        String helperText,
        Map<String, FieldCall> fieldsByHelperParam) {

        Matcher matcher = BOOL_POINTER_WRITE_RE.matcher(helperText);
        while (matcher.find()) {
            FieldCall field = fieldsByHelperParam.get(matcher.group("target"));
            if (field != null && field.nativeType == null) {
                field.nativeType = "bool";
                field.wireShape = "bool";
                field.wireShapeSource = "message-helper-bool-write";
            }
        }
    }

    private void refineNestedBoolWrites(
        Function helper,
        String helperText,
        Map<String, FieldCall> fieldsByHelperParam) {

        List<ParsedUnmarshalCall> calls = parseUnmarshalCalls(helperText);
        if (calls.isEmpty()) {
            return;
        }

        Map<String, Set<Integer>> boolParameterIndices = nestedBoolParameterIndices(helper);
        for (ParsedUnmarshalCall call : calls) {
            Set<Integer> indices = boolParameterIndices.get(call.templateType);
            if (indices == null || indices.isEmpty()) {
                continue;
            }
            for (Integer index : indices) {
                if (index == null || index < 0 || index >= call.args.size()) {
                    continue;
                }
                String argName = bareArgumentName(call.args.get(index));
                FieldCall field = fieldsByHelperParam.get(argName);
                if (field != null && field.nativeType == null) {
                    field.nativeType = "bool";
                    field.wireShape = "bool";
                    field.wireShapeSource = "nested-unmarshal-bool-write";
                }
            }
        }
    }

    private Map<String, Set<Integer>> nestedBoolParameterIndices(Function helper) {
        if (helper == null) {
            return Collections.emptyMap();
        }
        String key = functionCacheKey("nested-bool-parameters", helper);
        Map<String, Set<Integer>> cached = nestedBoolParameterIndicesCache.get(key);
        if (cached != null) {
            return cached;
        }
        LinkedHashMap<String, Set<Integer>> result = new LinkedHashMap<>();
        for (Instruction instruction : functionInstructions(helper)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Function target = functionAtOrContaining(callTarget(instruction));
            if (target == null) {
                continue;
            }
            String targetName = fullFunctionName(target);
            String templateType = unmarshalTemplateType(targetName);
            if (templateType == null) {
                continue;
            }
            String targetText = decompileC(target);
            Set<Integer> boolIndices = boolParameterIndices(targetText);
            if (!boolIndices.isEmpty()) {
                result.computeIfAbsent(templateType, ignored -> new LinkedHashSet<>())
                    .addAll(boolIndices);
            }
        }
        Map<String, Set<Integer>> cachedResult = immutableStringSetMap(result);
        nestedBoolParameterIndicesCache.put(key, cachedResult);
        return cachedResult;
    }

    private Set<Integer> boolParameterIndices(String decompiledText) {
        LinkedHashSet<Integer> result = new LinkedHashSet<>();
        if (decompiledText == null) {
            return result;
        }
        Set<Integer> cached = boolParameterIndicesCache.get(decompiledText);
        if (cached != null) {
            return cached;
        }
        List<String> parameterNames = parameterNamesFromDecompiledFunction(decompiledText);
        if (parameterNames.isEmpty()) {
            return cacheIntegerSet(boolParameterIndicesCache, decompiledText, result);
        }
        HashMap<String, Integer> parameterIndex = new HashMap<>();
        for (int i = 0; i < parameterNames.size(); i++) {
            parameterIndex.put(parameterNames.get(i), i);
        }

        Matcher matcher = BOOL_POINTER_WRITE_RE.matcher(decompiledText);
        while (matcher.find()) {
            Integer index = parameterIndex.get(matcher.group("target"));
            if (index != null) {
                result.add(index);
            }
        }
        return cacheIntegerSet(boolParameterIndicesCache, decompiledText, result);
    }

    private List<String> parameterNamesFromDecompiledFunction(String decompiledText) {
        ArrayList<String> result = new ArrayList<>();
        if (decompiledText == null) {
            return result;
        }
        List<String> cached = parameterNameCache.get(decompiledText);
        if (cached != null) {
            return cached;
        }
        int bodyStart = decompiledText.indexOf('{');
        int argsEnd = bodyStart < 0
            ? decompiledText.lastIndexOf(')')
            : decompiledText.lastIndexOf(')', bodyStart);
        if (argsEnd < 0) {
            return cacheStringList(parameterNameCache, decompiledText, result);
        }
        int argsStart = decompiledText.lastIndexOf('(', argsEnd);
        if (argsStart < 0) {
            return cacheStringList(parameterNameCache, decompiledText, result);
        }
        for (String parameter : splitTopLevel(decompiledText.substring(argsStart + 1, argsEnd))) {
            String name = parameterName(parameter);
            if (name != null) {
                result.add(name);
            }
        }
        return cacheStringList(parameterNameCache, decompiledText, result);
    }

    private String parameterName(String parameter) {
        if (parameter == null) {
            return null;
        }
        String trimmed = parameter.trim();
        if (trimmed.isEmpty() || "void".equals(trimmed)) {
            return null;
        }
        int index = trimmed.length() - 1;
        while (index >= 0 &&
            (Character.isLetterOrDigit(trimmed.charAt(index)) || trimmed.charAt(index) == '_')) {
            index--;
        }
        if (index == trimmed.length() - 1) {
            return null;
        }
        return trimmed.substring(index + 1);
    }

    private List<String> cacheStringList(
        Map<String, List<String>> cache,
        String key,
        ArrayList<String> value) {

        List<String> cached = Collections.unmodifiableList(new ArrayList<>(value));
        cache.put(key, cached);
        return cached;
    }

    private Set<Integer> cacheIntegerSet(
        Map<String, Set<Integer>> cache,
        String key,
        LinkedHashSet<Integer> value) {

        Set<Integer> cached = Collections.unmodifiableSet(new LinkedHashSet<>(value));
        cache.put(key, cached);
        return cached;
    }

    private List<ParsedUnmarshalCall> cacheParsedUnmarshalCalls(
        Map<String, List<ParsedUnmarshalCall>> cache,
        String key,
        ArrayList<ParsedUnmarshalCall> value) {

        List<ParsedUnmarshalCall> cached = Collections.unmodifiableList(new ArrayList<>(value));
        cache.put(key, cached);
        return cached;
    }

    private Map<String, Set<Integer>> immutableStringSetMap(
        LinkedHashMap<String, Set<Integer>> value) {

        LinkedHashMap<String, Set<Integer>> cached = new LinkedHashMap<>();
        for (Map.Entry<String, Set<Integer>> entry : value.entrySet()) {
            cached.put(
                entry.getKey(),
                Collections.unmodifiableSet(new LinkedHashSet<>(entry.getValue())));
        }
        return Collections.unmodifiableMap(cached);
    }

    private List<ParsedUnmarshalCall> parseUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        List<ParsedUnmarshalCall> cached = unmarshalCallCache.get(text);
        if (cached != null) {
            return cached;
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf("Unmarshal<", search);
            if (nameIndex < 0) {
                break;
            }
            int templateStart = text.indexOf('<', nameIndex);
            int templateEnd = matchingIndex(text, templateStart, '<', '>');
            int argsStart = text.indexOf('(', templateEnd);
            int argsEnd = matchingIndex(text, argsStart, '(', ')');
            if (templateStart < 0 || templateEnd < 0 || argsStart < 0 || argsEnd < 0) {
                search = nameIndex + "Unmarshal<".length();
                continue;
            }
            String templateType = text.substring(templateStart + 1, templateEnd).trim();
            result.add(new ParsedUnmarshalCall(
                templateType,
                nameIndex,
                splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            search = argsEnd + 1;
        }
        return cacheParsedUnmarshalCalls(unmarshalCallCache, text, result);
    }

    private List<ParsedUnmarshalCall> parseMarshalerUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        List<ParsedUnmarshalCall> cached = marshalerUnmarshalCallCache.get(text);
        if (cached != null) {
            return cached;
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf("Marshaler<", search);
            if (nameIndex < 0) {
                break;
            }
            int templateStart = text.indexOf('<', nameIndex);
            int templateEnd = matchingIndex(text, templateStart, '<', '>');
            int unmarshalIndex = templateEnd < 0
                ? -1
                : text.indexOf("::Unmarshal", templateEnd);
            int argsStart = unmarshalIndex < 0 ? -1 : text.indexOf('(', unmarshalIndex);
            int argsEnd = matchingIndex(text, argsStart, '(', ')');
            if (templateStart < 0 || templateEnd < 0 || unmarshalIndex < 0 ||
                argsStart < 0 || argsEnd < 0) {
                search = nameIndex + "Marshaler<".length();
                continue;
            }

            result.add(new ParsedUnmarshalCall(
                text.substring(templateStart + 1, templateEnd).trim(),
                nameIndex,
                splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            search = argsEnd + 1;
        }
        return cacheParsedUnmarshalCalls(marshalerUnmarshalCallCache, text, result);
    }

    private List<ParsedUnmarshalCall> parseDirectTypeUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        List<ParsedUnmarshalCall> cached = directTypeUnmarshalCallCache.get(text);
        if (cached != null) {
            return cached;
        }
        int search = 0;
        while (search < text.length()) {
            int unmarshalIndex = text.indexOf("::Unmarshal(", search);
            if (unmarshalIndex < 0) {
                break;
            }
            int ownerStart = directCallOwnerStart(text, unmarshalIndex);
            int argsStart = unmarshalIndex + "::Unmarshal".length();
            int argsEnd = matchingIndex(text, argsStart, '(', ')');
            if (ownerStart < 0 || argsEnd < 0) {
                search = unmarshalIndex + "::Unmarshal(".length();
                continue;
            }

            String owner = text.substring(ownerStart, unmarshalIndex).trim();
            if (owner.isEmpty() || owner.contains("Marshaler<")) {
                search = argsEnd + 1;
                continue;
            }

            result.add(new ParsedUnmarshalCall(
                sourceTypeLeaf(owner),
                unmarshalIndex,
                splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            search = argsEnd + 1;
        }
        return cacheParsedUnmarshalCalls(directTypeUnmarshalCallCache, text, result);
    }

    private List<ParsedReadRawCall> parseReadRawCalls(String text) {
        ArrayList<ParsedReadRawCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        List<ParsedReadRawCall> cached = readRawCallCache.get(text);
        if (cached != null) {
            return cached;
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf("ReadRaw", search);
            if (nameIndex < 0) {
                break;
            }
            int afterName = nameIndex + "ReadRaw".length();
            if (afterName < text.length()) {
                char next = text.charAt(afterName);
                if (Character.isLetterOrDigit(next) || next == '_') {
                    search = afterName;
                    continue;
                }
            }
            int argsStart = text.indexOf('(', afterName);
            int argsEnd = matchingIndex(text, argsStart, '(', ')');
            if (argsStart < 0 || argsEnd < 0) {
                search = afterName;
                continue;
            }

            List<String> args = splitTopLevel(text.substring(argsStart + 1, argsEnd));
            int storageIndex = args.size() >= 4 ? 1 : 0;
            int lengthIndex = storageIndex + 1;
            if (lengthIndex < args.size()) {
                Integer byteLength = readRawByteLength(args.get(lengthIndex));
                if (byteLength != null && byteLength > 0) {
                    result.add(new ParsedReadRawCall(
                        args.get(storageIndex),
                        byteLength,
                        nameIndex));
                }
            }
            search = argsEnd + 1;
        }

        List<ParsedReadRawCall> parsed =
            Collections.unmodifiableList(new ArrayList<>(result));
        readRawCallCache.put(text, parsed);
        return parsed;
    }

    private Integer readRawByteLength(String expression) {
        if (expression == null) {
            return null;
        }
        String value = normalizedExpression(expression);
        if (value == null) {
            return null;
        }
        value = value.replaceAll("(?i)[uUlL]+$", "");
        Long parsed = parseIntegerLiteral(value);
        if (parsed == null || parsed <= 0 || parsed > Integer.MAX_VALUE) {
            return null;
        }
        return parsed.intValue();
    }

    private int directCallOwnerStart(String text, int unmarshalIndex) {
        int index = unmarshalIndex - 1;
        while (index >= 0) {
            char c = text.charAt(index);
            if (Character.isLetterOrDigit(c) || c == '_' || c == ':' || c == '<' ||
                c == '>' || c == ',' || Character.isWhitespace(c)) {
                index--;
                continue;
            }
            break;
        }
        return index + 1;
    }

    private String storageArgumentForMarshalerCall(ParsedUnmarshalCall call) {
        if (call == null || call.args.size() < 3) {
            return null;
        }
        return call.args.get(call.args.size() - 2);
    }

    private String storageArgumentForDirectUnmarshalCall(ParsedUnmarshalCall call) {
        if (call == null || call.args.size() < 2) {
            return null;
        }
        return call.args.get(call.args.size() - 2);
    }

    private boolean isLikelyMessageStorage(String expression) {
        if (expression == null) {
            return false;
        }
        String value = normalizedExpression(expression);
        if (value == null || value.matches("param_\\d+")) {
            return false;
        }
        return value.startsWith("_Dst") ||
            value.matches(".*\\bparam_\\d+\\s*\\+\\s*(0x[0-9a-fA-F]+|\\d+).*");
    }

    private String helperParameterFromExpression(
        String expression,
        Map<String, FieldCall> fieldsByHelperParam) {

        if (expression == null || fieldsByHelperParam.isEmpty()) {
            return null;
        }
        Matcher matcher = Pattern.compile("\\b[A-Za-z_][A-Za-z0-9_]*\\b").matcher(expression);
        while (matcher.find()) {
            String token = matcher.group();
            if (fieldsByHelperParam.containsKey(token)) {
                return token;
            }
        }
        return null;
    }

    private List<String> callArgumentsForTarget(String text, Function target) {
        if (text == null || target == null) {
            return List.of();
        }
        for (String name : functionCallNameCandidates(target)) {
            List<String> args = callArgumentsForName(text, name);
            if (!args.isEmpty()) {
                return args;
            }
        }
        return List.of();
    }

    private int callTextIndexForTarget(String text, Function target) {
        if (text == null || target == null) {
            return Integer.MAX_VALUE;
        }
        for (String name : functionCallNameCandidates(target)) {
            int index = callTextIndexForName(text, name);
            if (index >= 0) {
                return index;
            }
        }
        return Integer.MAX_VALUE;
    }

    private int callTextIndexForName(String text, String name) {
        if (text == null || name == null || name.isEmpty()) {
            return -1;
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf(name, search);
            if (nameIndex < 0) {
                return -1;
            }
            int argsStart = text.indexOf('(', nameIndex + name.length());
            if (argsStart < 0) {
                return -1;
            }
            String between = text.substring(nameIndex + name.length(), argsStart).trim();
            if (!between.isEmpty()) {
                search = nameIndex + name.length();
                continue;
            }
            return nameIndex;
        }
        return -1;
    }

    private List<String> functionCallNameCandidates(Function function) {
        ArrayList<String> result = new ArrayList<>();
        String fullName = fullFunctionName(function);
        addDistinct(result, fullName);
        String plainName = function.getName();
        if (!isGenericCallName(plainName)) {
            addDistinct(result, plainName);
        }
        String leaf = sourceTypeLeaf(fullName);
        if (!isGenericCallName(leaf)) {
            addDistinct(result, leaf);
        }
        return result;
    }

    private boolean isGenericCallName(String value) {
        return value == null ||
            value.isEmpty() ||
            "Unmarshal".equals(value) ||
            "Marshal".equals(value) ||
            "CopyValue".equals(value) ||
            "CreateInstance".equals(value) ||
            "GetEmptyValue".equals(value) ||
            "Destructor".equals(value);
    }

    private void addDistinct(List<String> values, String value) {
        if (value == null || value.isEmpty() || values.contains(value)) {
            return;
        }
        values.add(value);
    }

    private List<String> callArgumentsForName(String text, String name) {
        if (text == null || name == null || name.isEmpty()) {
            return List.of();
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf(name, search);
            if (nameIndex < 0) {
                return List.of();
            }
            int argsStart = text.indexOf('(', nameIndex + name.length());
            if (argsStart < 0) {
                return List.of();
            }
            String between = text.substring(nameIndex + name.length(), argsStart).trim();
            if (!between.isEmpty()) {
                search = nameIndex + name.length();
                continue;
            }
            int argsEnd = matchingIndex(text, argsStart, '(', ')');
            if (argsEnd < 0) {
                return List.of();
            }
            return splitTopLevel(text.substring(argsStart + 1, argsEnd));
        }
        return List.of();
    }

    private String unmarshalTemplateType(String functionName) {
        if (functionName == null) {
            return null;
        }
        int nameIndex = functionName.indexOf("Unmarshal<");
        if (nameIndex < 0) {
            return null;
        }
        int start = functionName.indexOf('<', nameIndex);
        int end = matchingIndex(functionName, start, '<', '>');
        if (start < 0 || end < 0) {
            return null;
        }
        return functionName.substring(start + 1, end).trim();
    }

    private String bareArgumentName(String argument) {
        if (argument == null) {
            return null;
        }
        ParsedArgument parsed = parseArgument(argument);
        String value = parsed.expression == null ? argument : parsed.expression;
        value = value.trim();
        while (value.startsWith("&")) {
            value = value.substring(1).trim();
        }
        if (!value.matches("[A-Za-z_][A-Za-z0-9_]*")) {
            return null;
        }
        return value;
    }

    private String decompileC(Function function) {
        if (function == null || decompiler == null) {
            return null;
        }
        String key = functionCacheKey("decompile", function);
        if (decompileCache.containsKey(key)) {
            return decompileCache.get(key);
        }
        try {
            DecompileResults results = decompiler.decompileFunction(function, 30, monitor);
            if (!results.decompileCompleted() || results.getDecompiledFunction() == null) {
                decompileCache.put(key, null);
                return null;
            }
            String text = results.getDecompiledFunction().getC();
            decompileCache.put(key, text);
            return text;
        }
        catch (Exception ignored) {
            decompileCache.put(key, null);
            return null;
        }
    }

    private MessageHelperCall findMessageHelperCall(Function wrapper) {
        for (Instruction instruction : functionInstructions(wrapper)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address target = callTarget(instruction);
            Function function = functionAtOrContaining(target);
            if (function == null) {
                continue;
            }
            String name = fullFunctionName(function);
            if (name.contains("UnmarshalFields<")) {
                MessageHelperCall call = new MessageHelperCall();
                call.callsite = instruction.getMinAddress();
                call.target = function.getEntryPoint();
                call.targetName = name;
                return call;
            }
        }
        return null;
    }

    private ParsedUnmarshalFieldsCall parseUnmarshalFieldsCall(String text) {
        if (text == null) {
            return null;
        }
        int nameIndex = text.indexOf("UnmarshalFields<");
        if (nameIndex < 0) {
            return null;
        }
        int templateStart = text.indexOf('<', nameIndex);
        int templateEnd = matchingIndex(text, templateStart, '<', '>');
        if (templateStart < 0 || templateEnd < 0) {
            return null;
        }
        int argsStart = text.indexOf('(', templateEnd);
        int argsEnd = matchingIndex(text, argsStart, '(', ')');
        if (argsStart < 0 || argsEnd < 0) {
            return null;
        }

        ParsedUnmarshalFieldsCall call = new ParsedUnmarshalFieldsCall();
        call.templateTypes.addAll(splitTopLevel(text.substring(templateStart + 1, templateEnd)));
        List<String> args = splitTopLevel(text.substring(argsStart + 1, argsEnd));
        for (int i = 2; i < args.size(); i++) {
            call.fieldArgs.add(parseArgument(args.get(i)));
        }
        return call.fieldArgs.isEmpty() ? null : call;
    }

    private ParsedArgument parseArgument(String value) {
        ParsedArgument result = new ParsedArgument();
        String trimmed = value == null ? "" : value.trim();
        if (trimmed.startsWith("(")) {
            int end = matchingIndex(trimmed, 0, '(', ')');
            if (end > 0) {
                String cast = trimmed.substring(1, end).trim();
                if (isLikelyCastType(cast)) {
                    result.castType = cast;
                    result.expression = trimmed.substring(end + 1).trim();
                    return result;
                }
            }
        }
        result.expression = trimmed;
        return result;
    }

    private String normalizedExpression(String value) {
        if (value == null) {
            return null;
        }
        String result = value.trim();
        while (result.startsWith("&")) {
            result = result.substring(1).trim();
        }
        while (result.startsWith("(")) {
            int end = matchingIndex(result, 0, '(', ')');
            if (end <= 0) {
                break;
            }
            String cast = result.substring(1, end).trim();
            if (!isLikelyCastType(cast)) {
                break;
            }
            result = result.substring(end + 1).trim();
        }
        while (result.startsWith("&")) {
            result = result.substring(1).trim();
        }
        return result.replaceAll("\\s+", " ");
    }

    private boolean isLikelyCastType(String value) {
        if (value == null || value.isEmpty()) {
            return false;
        }
        return value.contains("*") ||
            value.contains("string") ||
            value.contains("unordered_map") ||
            value.startsWith("undefined") ||
            value.startsWith("byte") ||
            value.startsWith("bool") ||
            value.startsWith("longlong") ||
            value.startsWith("ulonglong");
    }

    private int matchingIndex(String text, int start, char open, char close) {
        if (text == null || start < 0 || start >= text.length() || text.charAt(start) != open) {
            return -1;
        }
        int depth = 0;
        for (int i = start; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c == open) {
                depth++;
            }
            else if (c == close) {
                depth--;
                if (depth == 0) {
                    return i;
                }
            }
        }
        return -1;
    }

    private List<String> splitTopLevel(String value) {
        ArrayList<String> result = new ArrayList<>();
        if (value == null || value.isEmpty()) {
            return result;
        }
        int angleDepth = 0;
        int parenDepth = 0;
        int bracketDepth = 0;
        int start = 0;
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '<') {
                angleDepth++;
            }
            else if (c == '>') {
                angleDepth = Math.max(0, angleDepth - 1);
            }
            else if (c == '(') {
                parenDepth++;
            }
            else if (c == ')') {
                parenDepth = Math.max(0, parenDepth - 1);
            }
            else if (c == '[') {
                bracketDepth++;
            }
            else if (c == ']') {
                bracketDepth = Math.max(0, bracketDepth - 1);
            }
            else if (c == ',' && angleDepth == 0 && parenDepth == 0 && bracketDepth == 0) {
                String part = value.substring(start, i).trim();
                if (!part.isEmpty()) {
                    result.add(part);
                }
                start = i + 1;
            }
        }
        String tail = value.substring(start).trim();
        if (!tail.isEmpty()) {
            result.add(tail);
        }
        return result;
    }

    private String nativeTypeFromCast(String castType) {
        if (castType == null) {
            return null;
        }
        String normalized = castType
            .replace("AZStd::", "")
            .replace("_string", "string")
            .replace(" *", "*")
            .trim();
        if (normalized.contains("unordered_map") &&
            normalized.contains("u32") &&
            normalized.contains("string")) {
            return "unordered_map<u32,string>";
        }
        if (normalized.contains("string")) {
            return "AZStd::string";
        }
        if (normalized.startsWith("bool")) {
            return "bool";
        }
        if (normalized.startsWith("byte") || normalized.startsWith("undefined1")) {
            return "u8";
        }
        if (normalized.startsWith("undefined2")) {
            return "u16";
        }
        if (normalized.startsWith("undefined4")) {
            return "u32";
        }
        if (normalized.startsWith("undefined8")) {
            return "u64";
        }
        return normalized.replace("*", "").trim();
    }

    private boolean shouldUseTemplateType(String nativeType, String templateType) {
        if (templateType == null || templateType.isEmpty()) {
            return false;
        }
        if (nativeType == null) {
            return true;
        }
        if ("u32".equals(nativeType)) {
            return templateType.endsWith("Token") ||
                templateType.endsWith("Values") ||
                templateType.endsWith("Info") ||
                templateType.endsWith("Data");
        }
        return false;
    }

    private boolean templateMatchesCast(String templateType, String nativeType) {
        if (templateType == null || nativeType == null) {
            return false;
        }
        if ("ClientVersionTokenMap".equals(templateType) &&
            nativeType.contains("unordered_map<u32,string>")) {
            return true;
        }
        if ("AZStd::string".equals(nativeType) && templateType.endsWith("String")) {
            return true;
        }
        return false;
    }

    private String wireShapeFromNativeType(String nativeType) {
        if (nativeType == null) {
            return null;
        }
        if ("bool".equals(nativeType) ||
            "u8".equals(nativeType) ||
            "u16".equals(nativeType) ||
            "u32".equals(nativeType) ||
            "u64".equals(nativeType) ||
            "f32".equals(nativeType) ||
            "f64".equals(nativeType)) {
            return nativeType;
        }
        if ("AZ::Vector2".equals(nativeType)) {
            return "vec2";
        }
        if ("AZ::Vector3".equals(nativeType)) {
            return "vec3";
        }
        if ("AZ::Vector4".equals(nativeType)) {
            return "vec4";
        }
        if ("AZ::Quaternion".equals(nativeType)) {
            return "quat";
        }
        if ("AZ::Matrix3x3".equals(nativeType)) {
            return "mat3";
        }
        if ("AZ::Transform".equals(nativeType)) {
            return "affine3";
        }
        if ("AZ::Bounds".equals(nativeType)) {
            return "aabb2d";
        }
        if ("AZ::Aabb".equals(nativeType)) {
            return "aabb3d";
        }
        if ("EntityRef".equals(nativeType)) {
            return "entity-ref";
        }
        if ("AZStd::string".equals(nativeType)) {
            return "string";
        }
        return null;
    }

    private String wireShapeFromRawByteLength(int byteLength) {
        if (byteLength == 1) {
            return "u8";
        }
        if (byteLength > 1) {
            return "fixed-bytes-" + byteLength;
        }
        return null;
    }

    private FieldCall recoverFieldCall(Function owner, Address callsite, int index) {
        ArgState state = recoverForwardArgState(owner, callsite);
        ArgState fallback = recoverBackwardArgState(owner, callsite);
        state.fillMissingFrom(fallback);

        FieldCall field = new FieldCall();
        field.index = index;
        field.callsite = callsite;
        field.nameAddress = state.nameAddress;
        field.name = state.name;
        field.group = state.groupKnown ? state.group : null;
        field.handlerOffset = state.handlerOffset;
        field.handlerExpression = state.handlerExpression;
        field.handlerVtable = state.handlerVtable;
        field.confidence = field.name == null
            ? "register-field-call-unresolved-name"
            : "register-field-call";
        return field;
    }

    private ArgState recoverBackwardArgState(Function owner, Address callsite) {
        ArgState state = new ArgState();
        Instruction cursor = currentProgram.getListing().getInstructionBefore(callsite);
        for (int i = 0; i < BACKWARD_ARGUMENT_SCAN_LIMIT && cursor != null; i++) {
            if (!owner.getBody().contains(cursor.getMinAddress())) {
                break;
            }
            observeArgumentAssignment(cursor, state);
            if (state.nameAddress != null && state.groupKnown && state.handlerKnown) {
                break;
            }
            if (cursor.getFlowType().isCall()) {
                break;
            }
            cursor = cursor.getPrevious();
        }
        return state;
    }

    private ArgState recoverForwardArgState(Function owner, Address callsite) {
        ForwardArgState state = new ForwardArgState();
        state.registers.put("RCX", TrackedValue.thisOffset(0));
        for (Instruction instruction : functionInstructions(owner)) {
            if (instruction.getMinAddress().compareTo(callsite) >= 0) {
                break;
            }
            observeForwardInstruction(instruction, state);
        }

        ArgState result = new ArgState();
        TrackedValue name = state.registers.get("RDX");
        if (name != null && name.address != null) {
            StringDecode decoded = readFieldNameAtOrThroughPointer(name.address);
            if (decoded != null) {
                result.nameAddress = decoded.address;
                result.name = decoded.value;
            }
        }

        TrackedValue handler = state.registers.get("R8");
        if (handler != null) {
            result.handlerKnown = true;
            if (handler.thisOffset != null) {
                result.handlerOffset = handler.thisOffset;
                result.handlerExpression = handler.expression;
                result.handlerVtable = state.vtablesByThisOffset.get(handler.thisOffset);
            }
            else if (handler.address != null) {
                result.handlerExpression = formatAddress(handler.address);
            }
            else {
                result.handlerExpression = handler.expression;
            }
        }

        TrackedValue group = state.registers.get("R9");
        if (group != null && group.immediate != null) {
            result.groupKnown = true;
            result.group = group.immediate.intValue();
        }
        return result;
    }

    private void observeForwardInstruction(Instruction instruction, ForwardArgState state) {
        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return;
        }

        if (instruction.getFlowType().isCall()) {
            observeFieldHandlerConstructorCall(instruction, state);
            clearVolatileRegisters(state.registers);
            return;
        }

        String destination = registerOperand(instruction, 0);
        if (destination != null) {
            if (("XOR".equals(mnemonic) || "SUB".equals(mnemonic)) &&
                destination.equals(registerOperand(instruction, 1))) {
                state.registers.put(destination, TrackedValue.immediate(0));
                return;
            }

            if ("LEA".equals(mnemonic)) {
                TrackedValue value = trackedLeaValue(instruction, state.registers);
                putOrRemove(state.registers, destination, value);
                return;
            }

            if ("MOV".equals(mnemonic)) {
                TrackedValue value = trackedMoveSource(instruction, state.registers);
                putOrRemove(state.registers, destination, value);
                return;
            }

            if ("ADD".equals(mnemonic)) {
                TrackedValue current = state.registers.get(destination);
                Long immediate = immediateValue(instruction, 1);
                if (current != null && current.thisOffset != null && immediate != null) {
                    state.registers.put(destination, current.addOffset(immediate.intValue()));
                }
                else {
                    state.registers.remove(destination);
                }
                return;
            }
        }

        if ("MOV".equals(mnemonic)) {
            Integer offset = trackedThisOffsetForMemoryOperand(instruction, 0, state.registers);
            TrackedValue source = trackedMoveSource(instruction, state.registers);
            if (offset != null && source != null &&
                source.address != null && isVtableLike(source.address)) {
                state.vtablesByThisOffset.put(offset, source.address);
            }
        }
    }

    private void observeFieldHandlerConstructorCall(
        Instruction instruction,
        ForwardArgState state) {

        TrackedValue receiver = state.registers.get("RCX");
        if (receiver == null || receiver.thisOffset == null) {
            return;
        }

        Address vtable = fieldHandlerConstructorVtable(callTarget(instruction));
        if (vtable != null) {
            state.vtablesByThisOffset.put(receiver.thisOffset, vtable);
        }
    }

    private Address fieldHandlerConstructorVtable(Address address) {
        Address target = resolvedCodeTarget(address);
        if (!isExecutableAddress(target)) {
            return null;
        }

        String key = addressCacheKey("field-handler-constructor-vtable", target);
        if (fieldHandlerConstructorVtableCache.containsKey(key)) {
            return fieldHandlerConstructorVtableCache.get(key);
        }

        Address result = recoverFieldHandlerConstructorVtable(target);
        fieldHandlerConstructorVtableCache.put(key, result);
        return result;
    }

    private Address recoverFieldHandlerConstructorVtable(Address target) {
        Function function = functionAtOrContaining(target);
        if (function == null) {
            return null;
        }

        ForwardArgState state = new ForwardArgState();
        state.registers.put("RCX", TrackedValue.thisOffset(0));
        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (instruction.getMinAddress().compareTo(target) < 0) {
                continue;
            }
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }

            String mnemonic = upperMnemonic(instruction);
            if (mnemonic == null) {
                continue;
            }
            if (instruction.getFlowType().isCall()) {
                break;
            }

            Address vtable = vtableWrittenToTrackedThis(instruction, state.registers);
            if (vtable != null) {
                return vtable;
            }
            observeForwardRegisterInstruction(instruction, state.registers);
        }
        return null;
    }

    private Address vtableWrittenToTrackedThis(
        Instruction instruction,
        Map<String, TrackedValue> registers) {

        if (!"MOV".equals(upperMnemonic(instruction))) {
            return null;
        }

        Integer offset = trackedThisOffsetForMemoryOperand(instruction, 0, registers);
        TrackedValue source = trackedMoveSource(instruction, registers);
        if (offset != null && offset == 0 && source != null &&
            source.address != null && isVtableLike(source.address)) {
            return source.address;
        }
        return null;
    }

    private void observeForwardRegisterInstruction(
        Instruction instruction,
        Map<String, TrackedValue> registers) {

        String mnemonic = upperMnemonic(instruction);
        String destination = registerOperand(instruction, 0);
        if (mnemonic == null || destination == null) {
            return;
        }

        if (("XOR".equals(mnemonic) || "SUB".equals(mnemonic)) &&
            destination.equals(registerOperand(instruction, 1))) {
            registers.put(destination, TrackedValue.immediate(0));
            return;
        }

        if ("LEA".equals(mnemonic)) {
            TrackedValue value = trackedLeaValue(instruction, registers);
            putOrRemove(registers, destination, value);
            return;
        }

        if ("MOV".equals(mnemonic)) {
            TrackedValue value = trackedMoveSource(instruction, registers);
            putOrRemove(registers, destination, value);
            return;
        }

        if ("ADD".equals(mnemonic)) {
            TrackedValue current = registers.get(destination);
            Long immediate = immediateValue(instruction, 1);
            if (current != null && current.thisOffset != null && immediate != null) {
                registers.put(destination, current.addOffset(immediate.intValue()));
            }
            else {
                registers.remove(destination);
            }
        }
    }

    private void observeArgumentAssignment(Instruction instruction, ArgState state) {
        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return;
        }

        String destination = registerOperand(instruction, 0);
        if (destination != null) {
            if ("RDX".equals(destination) && state.nameAddress == null) {
                Address address = referencedAddress(instruction);
                StringDecode decoded = readFieldNameAtOrThroughPointer(address);
                if (decoded != null) {
                    state.nameAddress = decoded.address;
                    state.name = decoded.value;
                }
            }
            else if ("R8".equals(destination) && !state.handlerKnown) {
                state.handlerKnown = true;
                Integer offset = memoryDisplacementForThisLikeOperand(instruction, 1);
                if (offset != null) {
                    state.handlerOffset = offset;
                    state.handlerExpression = "this+0x" + Integer.toHexString(offset);
                }
                else {
                    Address address = referencedAddress(instruction);
                    if (address != null) {
                        state.handlerExpression = formatAddress(address);
                    }
                    else {
                        state.handlerExpression = operandText(instruction, 1);
                    }
                }
            }
            else if ("R9".equals(destination) && !state.groupKnown) {
                Long immediate = immediateValue(instruction, 1);
                if (immediate != null) {
                    state.group = immediate.intValue();
                    state.groupKnown = true;
                }
            }
        }

        if (!state.groupKnown &&
            ("XOR".equals(mnemonic) || "SUB".equals(mnemonic)) &&
            "R9".equals(registerOperand(instruction, 0)) &&
            "R9".equals(registerOperand(instruction, 1))) {
            state.group = 0;
            state.groupKnown = true;
        }
    }

    private Address findInstanceVtable(Function function) {
        HashMap<String, Address> registerAddresses = new HashMap<>();
        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }

            String mnemonic = upperMnemonic(instruction);
            String destination = registerOperand(instruction, 0);
            if ("LEA".equals(mnemonic) && destination != null) {
                Address address = referencedAddress(instruction);
                if (address != null && isVtableLike(address)) {
                    registerAddresses.put(destination, address);
                }
                else {
                    registerAddresses.remove(destination);
                }
                continue;
            }

            if ("MOV".equals(mnemonic)) {
                String source = registerOperand(instruction, 1);
                if (destination != null) {
                    if (source != null && registerAddresses.containsKey(source)) {
                        registerAddresses.put(destination, registerAddresses.get(source));
                    }
                    else {
                        registerAddresses.remove(destination);
                    }
                }

                if (source != null && registerAddresses.containsKey(source) &&
                    isZeroDisplacementMemoryOperand(instruction, 0)) {
                    return registerAddresses.get(source);
                }
            }
        }
        return null;
    }

    private boolean isZeroDisplacementMemoryOperand(Instruction instruction, int operandIndex) {
        Object[] objects = operandObjects(instruction, operandIndex);
        boolean hasRegister = false;
        int displacement = 0;
        for (Object object : objects) {
            if (object instanceof Register register) {
                String name = canonicalRegisterName(register.getName());
                if (!"RSP".equals(name) && !"RIP".equals(name)) {
                    hasRegister = true;
                }
            }
            else if (object instanceof Scalar scalar) {
                long value = scalar.getSignedValue();
                if (value >= Integer.MIN_VALUE && value <= Integer.MAX_VALUE) {
                    displacement += (int)value;
                }
            }
        }
        return hasRegister && displacement == 0;
    }

    private JsonArray virtualFunctionSlots(Address vtable) {
        JsonArray result = new JsonArray();
        if (vtable == null) {
            return result;
        }
        for (int i = 0; i < 12; i++) {
            Address slotAddress = vtable.add(i * 8L);
            Address target = readPointer(slotAddress);
            if (target == null || !isExecutableAddress(target)) {
                break;
            }
            result.add(virtualFunctionSlot(i, null, target));
        }
        return result;
    }

    private JsonObject virtualFunctionSlot(int slotIndex, String slotName, Address target) {
        JsonObject slot = new JsonObject();
        slot.addProperty("slot", slotIndex);
        slot.addProperty("slotOffset", "0x" + Integer.toHexString(slotIndex * 8));
        add(slot, "name", slotName);
        slot.addProperty("address", formatAddress(target));
        Address resolvedTarget = resolvedCodeTarget(target);
        if (target != null && resolvedTarget != null && !target.equals(resolvedTarget)) {
            add(slot, "target", formatAddress(resolvedTarget));
        }
        else {
            Address tailTarget = terminalJumpTarget(target);
            if (tailTarget != null && !tailTarget.equals(target)) {
                add(slot, "target", formatAddress(tailTarget));
            }
        }
        Function function = functionAtOrContaining(target);
        if (function != null) {
            slot.addProperty("function", fullFunctionName(function));
        }
        return slot;
    }

    private AzRttiEvidence decodeAzRttiFromVtable(Address vtable) {
        if (vtable == null) {
            return null;
        }

        AzRttiEvidence evidence = new AzRttiEvidence();
        evidence.source = "instance-vtable";
        evidence.address = formatAddress(vtable);

        for (int slot = 0; slot < AZ_RTTI_VTABLE_SCAN_SLOTS; slot++) {
            Address slotPointer = readPointer(vtable.add(slot * 8L));
            Address body = resolvedCodeTarget(slotPointer);
            if (!isExecutableAddress(body)) {
                continue;
            }

            TypeIdDecode typeId = decodeAzRttiTypeIdProvider(body);
            if (typeId != null) {
                if (evidence.typeId == null) {
                    evidence.typeId = typeId.typeId;
                }
                evidence.providers.add(typeId.toJson(slot));
            }

            TypeNameDecode typeName = decodeAzRttiTypeNameProvider(body);
            if (typeName != null) {
                if (evidence.typeName == null) {
                    evidence.typeName = typeName.typeName;
                }
                evidence.providers.add(typeName.toJson(slot));
            }
        }

        return evidence.hasIdentity() ? evidence : null;
    }

    private TypeIdDecode decodeAzRttiTypeIdProvider(Address function) {
        Address provider = resolvedCodeTarget(function);
        byte[] bytes = codeBytesBeforePadding(readBytes(provider, TYPE_ID_PROVIDER_BYTES));
        if (bytes.length == 0) {
            return decodeTypeIdFromReferencedStrings(function, provider);
        }

        for (int i = 0; i <= bytes.length - 7; i++) {
            if (!isRipRelativeLea(bytes, i)) {
                continue;
            }

            Address target = rel32Target(provider, i, 7, int32(bytes, i + 3));
            String uuid = canonicalUuidFromString(readPrintableString(target));
            if (uuid != null) {
                TypeIdDecode decode = new TypeIdDecode();
                decode.function = function;
                decode.provider = provider;
                decode.typeId = uuid;
                decode.typeIdSource = "sourceLiteral";
                decode.sourceAddress = target;
                return decode;
            }
        }

        return decodeTypeIdFromReferencedStrings(function, provider);
    }

    private TypeIdDecode decodeTypeIdFromReferencedStrings(Address function, Address provider) {
        Function providerFunction = functionAtOrContaining(provider);
        if (providerFunction == null) {
            return null;
        }
        for (Instruction instruction : functionInstructions(providerFunction)) {
            Address target = referencedAddress(instruction);
            String uuid = canonicalUuidFromString(readPrintableString(target));
            if (uuid == null) {
                continue;
            }

            TypeIdDecode decode = new TypeIdDecode();
            decode.function = function;
            decode.provider = provider;
            decode.typeId = uuid;
            decode.typeIdSource = "referencedString";
            decode.sourceAddress = target;
            return decode;
        }
        return null;
    }

    private TypeNameDecode decodeAzRttiTypeNameProvider(Address function) {
        Address provider = resolvedCodeTarget(function);
        Address directString = stringAddressReturnedBySimpleFunction(provider);
        String directName = readPrintableString(directString);
        if (isPlausibleTypeName(directName)) {
            TypeNameDecode decode = new TypeNameDecode();
            decode.function = function;
            decode.provider = provider;
            decode.typeName = directName;
            decode.typeNameSource = "sourceLiteral";
            decode.typeNameAddress = directString;
            return decode;
        }

        byte[] bytes = codeBytesBeforePadding(readBytes(provider, TYPE_NAME_PROVIDER_BYTES));
        for (int i = 0; i <= bytes.length - 7; i++) {
            if (unsignedByte(bytes[i]) == 0x80 &&
                unsignedByte(bytes[i + 1]) == 0x3d &&
                unsignedByte(bytes[i + 6]) == 0x00) {
                Address target = rel32Target(provider, i, 7, int32(bytes, i + 2));
                String name = readPrintableString(target);
                if (isPlausibleTypeName(name)) {
                    TypeNameDecode decode = new TypeNameDecode();
                    decode.function = function;
                    decode.provider = provider;
                    decode.typeName = name;
                    decode.typeNameSource = "cachedBuffer";
                    decode.typeNameAddress = target;
                    return decode;
                }
            }
        }

        return null;
    }

    private Address stringAddressReturnedBySimpleFunction(Address function) {
        Address target = resolvedCodeTarget(function);
        byte[] bytes = readBytes(target, 16);
        if (bytes.length < 5) {
            return null;
        }
        if (bytes.length >= 7 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x8d &&
            unsignedByte(bytes[2]) == 0x05) {
            return rel32Target(target, 0, 7, int32(bytes, 3));
        }
        if (bytes.length >= 10 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0xb8) {
            return absoluteAddress(int64(bytes, 2));
        }
        if (unsignedByte(bytes[0]) == 0xb8) {
            return absoluteAddress(uint32(bytes, 1));
        }
        return null;
    }

    private Address resolvedCodeTarget(Address address) {
        Address current = address;
        for (int depth = 0; depth < 4; depth++) {
            if (!isExecutableAddress(current)) {
                return current;
            }
            byte[] bytes = readBytes(current, 16);
            if (bytes.length < 5) {
                return current;
            }

            if (unsignedByte(bytes[0]) == 0xe9) {
                current = rel32Target(current, 0, 5, int32(bytes, 1));
                continue;
            }

            if (bytes.length >= 11 &&
                unsignedByte(bytes[0]) == 0x48 &&
                unsignedByte(bytes[1]) == 0x8b &&
                unsignedByte(bytes[2]) == 0xca &&
                unsignedByte(bytes[3]) == 0x49 &&
                unsignedByte(bytes[4]) == 0x8b &&
                unsignedByte(bytes[5]) == 0xd0 &&
                unsignedByte(bytes[6]) == 0xe9) {
                current = rel32Target(current, 6, 5, int32(bytes, 7));
                continue;
            }

            if (bytes.length >= 6 &&
                unsignedByte(bytes[0]) == 0xff &&
                unsignedByte(bytes[1]) == 0x25) {
                Address pointerAddress = rel32Target(current, 0, 6, int32(bytes, 2));
                Address target = readPointer(pointerAddress);
                if (target != null) {
                    current = target;
                    continue;
                }
            }
            return current;
        }
        return current;
    }

    private Address terminalJumpTarget(Address address) {
        if (!isExecutableAddress(address)) {
            return null;
        }
        Address cursor = address;
        for (int i = 0; i < 32; i++) {
            Instruction instruction = currentProgram.getListing().getInstructionAt(cursor);
            if (instruction == null) {
                return null;
            }
            String mnemonic = upperMnemonic(instruction);
            if ("JMP".equals(mnemonic)) {
                Address target = callTarget(instruction);
                return isProgramAddress(target) ? target : null;
            }
            if (mnemonic != null && (mnemonic.startsWith("RET") || mnemonic.startsWith("INT"))) {
                return null;
            }
            Address fallThrough = instruction.getFallThrough();
            if (fallThrough == null || !isProgramAddress(fallThrough)) {
                return null;
            }
            cursor = fallThrough;
        }
        return null;
    }

    private WireShape classifyWireShape(Address marshal, Address marshalTarget) {
        Address effectiveMarshal = marshalTarget == null ? marshal : marshalTarget;
        return classifyMarshalPath(effectiveMarshal, 0, new LinkedHashSet<>());
    }

    private WireShape classifyMarshalPath(Address address, int depth, Set<String> seen) {
        if (!isExecutableAddress(address) || depth > 3 || !seen.add(address.toString())) {
            return null;
        }

        WireShape named = wireShapeFromFunctionName(address);
        if (named != null) {
            return named;
        }

        if (looksLikeBoolMarshal(address)) {
            return new WireShape("bool", "marshal-bool-pattern");
        }

        Integer fixedRawLength = fixedRawMarshalLength(address);
        if (fixedRawLength != null) {
            if (fixedRawLength == 1) {
                return new WireShape("u8", "marshal-raw-write-length");
            }
            if (fixedRawLength == 6) {
                return new WireShape("fixed-bytes-6", "marshal-raw-write-length");
            }
            if (fixedRawLength == 16) {
                return new WireShape("fixed-bytes-16", "marshal-raw-write-length");
            }
        }

        Function function = functionAtOrContaining(address);
        if (function == null) {
            return null;
        }

        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address target = resolvedCodeTarget(callTarget(instruction));
            WireShape called = classifyMarshalPath(target, depth + 1, seen);
            if (called != null) {
                return new WireShape(called.shape, "marshal-call:" + called.source);
            }
        }
        return null;
    }

    private Integer fixedRawMarshalLength(Address address) {
        Integer bytePatternLength = fixedRawMarshalLengthFromBytes(address);
        if (bytePatternLength != null) {
            return bytePatternLength;
        }

        Integer linearLength = fixedRawMarshalLengthFromLinearInstructions(address);
        if (linearLength != null) {
            return linearLength;
        }

        Function function = functionAtOrContaining(address);
        if (function == null) {
            return null;
        }

        Integer lastR8Length = null;
        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (instruction.getMinAddress().compareTo(address) < 0) {
                continue;
            }
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }

            String mnemonic = upperMnemonic(instruction);
            if ("MOV".equals(mnemonic)) {
                String destination = registerOperand(instruction, 0);
                Long immediate = immediateValue(instruction, 1);
                if ("R8".equals(destination) && immediate != null &&
                    immediate >= 0 && immediate <= 0x1000) {
                    lastR8Length = immediate.intValue();
                }
            }
            else if ("JMP".equals(mnemonic) && isVirtualRawWriteJump(instruction)) {
                return lastR8Length;
            }
            else if (instruction.getFlowType().isCall()) {
                return null;
            }
        }
        return null;
    }

    private Integer fixedRawMarshalLengthFromLinearInstructions(Address address) {
        Integer lastR8Length = null;
        int count = 0;
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(address, true)) {
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            Address instructionAddress = instruction.getMinAddress();
            if (instructionAddress.compareTo(address) < 0 ||
                !isExecutableAddress(instructionAddress)) {
                break;
            }

            String mnemonic = upperMnemonic(instruction);
            if ("MOV".equals(mnemonic)) {
                String destination = registerOperand(instruction, 0);
                Long immediate = immediateValue(instruction, 1);
                if ("R8".equals(destination) && immediate != null &&
                    immediate > 0 && immediate <= 0x1000) {
                    lastR8Length = immediate.intValue();
                }
            }
            else if ("JMP".equals(mnemonic) && isVirtualRawWriteJump(instruction)) {
                return lastR8Length;
            }
            else if ("RET".equals(mnemonic) || "CCH".equals(mnemonic)) {
                break;
            }
            else if (instruction.getFlowType().isCall()) {
                return null;
            }
        }
        return null;
    }

    private Integer fixedRawMarshalLengthFromBytes(Address address) {
        byte[] bytes = codeBytesBeforePadding(readBytes(address, 96));
        for (int i = 0; i <= bytes.length - 11; i++) {
            if (unsignedByte(bytes[i]) == 0x41 &&
                unsignedByte(bytes[i + 1]) == 0xb8 &&
                unsignedByte(bytes[i + 6]) == 0x48 &&
                unsignedByte(bytes[i + 7]) == 0xff &&
                unsignedByte(bytes[i + 8]) == 0x60 &&
                unsignedByte(bytes[i + 9]) == 0x40) {
                long length = uint32(bytes, i + 2);
                if (length > 0 && length <= 0x1000) {
                    return (int)length;
                }
            }
        }
        return null;
    }

    private boolean isVirtualRawWriteJump(Instruction instruction) {
        String operand = operandText(instruction, 0);
        if (operand == null) {
            return false;
        }
        String normalized = operand
            .replace(" ", "")
            .toUpperCase(Locale.ROOT);
        return normalized.contains("[RAX+0X40]") ||
            normalized.contains("[RAX+40H]");
    }

    private WireShape wireShapeFromFunctionName(Address address) {
        Function function = functionAtOrContaining(address);
        if (function == null) {
            return null;
        }
        String name = fullFunctionName(function);
        if (name.contains("GridMate::Marshaler<u8>::Marshal")) {
            return new WireShape("u8", "marshal-function-name");
        }
        if (name.contains("GridMate::Marshaler<u16>::Marshal")) {
            return new WireShape("u16", "marshal-function-name");
        }
        if (name.contains("GridMate::Marshaler<u32>::Marshal")) {
            return new WireShape("u32", "marshal-function-name");
        }
        if (name.contains("GridMate::Marshaler<u64>::Marshal")) {
            return new WireShape("u64", "marshal-function-name");
        }
        if (name.contains("GridMate::Marshaler<f32>::Marshal")) {
            return new WireShape("f32", "marshal-function-name");
        }
        if (name.contains("GridMate::HalfMarshaler::Marshal")) {
            return new WireShape("half-f32", "marshal-function-name");
        }
        if (name.contains("GridMate::VlqU32Marshaler::Marshal")) {
            return new WireShape("vlq-u32", "marshal-function-name");
        }
        if (name.contains("GridMate::QuatCompNormMarshaler::Marshal") ||
            name.contains("GridMate::QuatCompressSmallestThree")) {
            return new WireShape("quat-comp-norm", "marshal-function-name");
        }
        return null;
    }

    private boolean looksLikeBoolMarshal(Address address) {
        byte[] bytes = codeBytesBeforePadding(readBytes(address, 96));
        return containsBytes(bytes, 0x0f, 0x95) &&
            (containsBytes(bytes, 0x41, 0xb8, 0x01, 0x00, 0x00, 0x00) ||
                containsBytes(bytes, 0xba, 0x01, 0x00, 0x00, 0x00));
    }

    private boolean containsBytes(byte[] bytes, int... pattern) {
        if (pattern.length == 0 || bytes.length < pattern.length) {
            return false;
        }
        for (int i = 0; i <= bytes.length - pattern.length; i++) {
            boolean matches = true;
            for (int j = 0; j < pattern.length; j++) {
                if (unsignedByte(bytes[i + j]) != pattern[j]) {
                    matches = false;
                    break;
                }
            }
            if (matches) {
                return true;
            }
        }
        return false;
    }

    private boolean isRipRelativeLea(byte[] bytes, int offset) {
        if (offset + 6 >= bytes.length || unsignedByte(bytes[offset + 1]) != 0x8d) {
            return false;
        }
        int rex = unsignedByte(bytes[offset]);
        int modRm = unsignedByte(bytes[offset + 2]);
        if (rex == 0x48) {
            return modRm == 0x05 ||
                modRm == 0x0d ||
                modRm == 0x15 ||
                modRm == 0x1d ||
                modRm == 0x25 ||
                modRm == 0x2d ||
                modRm == 0x35 ||
                modRm == 0x3d;
        }
        if (rex == 0x4c) {
            return modRm == 0x05 ||
                modRm == 0x0d ||
                modRm == 0x15 ||
                modRm == 0x1d ||
                modRm == 0x25 ||
                modRm == 0x2d ||
                modRm == 0x35 ||
                modRm == 0x3d;
        }
        return false;
    }

    private Address rel32Target(Address base, int instructionOffset, int instructionLength, int rel) {
        return absoluteAddress(base.getOffset() + instructionOffset + instructionLength + rel);
    }

    private Address absoluteAddress(long value) {
        return currentProgram.getAddressFactory()
            .getDefaultAddressSpace()
            .getAddress(value);
    }

    private Address subtract(Address address, long value) {
        if (address == null) {
            return null;
        }
        return absoluteAddress(address.getOffset() - value);
    }

    private byte[] readBytes(Address address, int length) {
        if (!isProgramAddress(address)) {
            return new byte[0];
        }
        try {
            byte[] bytes = new byte[length];
            int read = currentProgram.getMemory().getBytes(address, bytes);
            if (read == bytes.length) {
                return bytes;
            }
            byte[] truncated = new byte[Math.max(read, 0)];
            System.arraycopy(bytes, 0, truncated, 0, truncated.length);
            return truncated;
        }
        catch (Exception ignored) {
            return new byte[0];
        }
    }

    private byte[] codeBytesBeforePadding(byte[] bytes) {
        int length = bytes.length;
        for (int i = 0; i < bytes.length; i++) {
            if (unsignedByte(bytes[i]) == 0xcc) {
                length = i;
                break;
            }
        }
        byte[] result = new byte[length];
        System.arraycopy(bytes, 0, result, 0, length);
        return result;
    }

    private int int32(byte[] bytes, int offset) {
        return unsignedByte(bytes[offset]) |
            (unsignedByte(bytes[offset + 1]) << 8) |
            (unsignedByte(bytes[offset + 2]) << 16) |
            (unsignedByte(bytes[offset + 3]) << 24);
    }

    private long uint32(byte[] bytes, int offset) {
        return int32(bytes, offset) & 0xffff_ffffL;
    }

    private long int64(byte[] bytes, int offset) {
        long result = 0;
        for (int i = 7; i >= 0; i--) {
            result = (result << 8) | unsignedByte(bytes[offset + i]);
        }
        return result;
    }

    private int unsignedByte(byte value) {
        return value & 0xff;
    }

    private boolean isVtableLike(Address address) {
        if (!isProgramAddress(address)) {
            return false;
        }
        int executableSlots = 0;
        for (int slot = 0; slot < 4; slot++) {
            Address target = readPointer(address.add(slot * 8L));
            if (target != null && isExecutableAddress(target)) {
                executableSlots++;
            }
        }
        return executableSlots >= 3;
    }

    private boolean isExecutableAddress(Address address) {
        if (!isProgramAddress(address)) {
            return false;
        }
        MemoryBlock block = currentProgram.getMemory().getBlock(address);
        return block != null && block.isExecute();
    }

    private Address callTarget(Instruction instruction) {
        Address[] flows = instruction.getFlows();
        if (flows != null) {
            for (Address flow : flows) {
                if (isProgramAddress(flow)) {
                    return flow;
                }
            }
        }
        for (Reference reference : instruction.getReferencesFrom()) {
            if (reference.getReferenceType().isCall() && isProgramAddress(reference.getToAddress())) {
                return reference.getToAddress();
            }
        }
        return null;
    }

    private Address referencedAddress(Instruction instruction) {
        for (Reference reference : instruction.getReferencesFrom()) {
            Address to = reference.getToAddress();
            if (isProgramAddress(to)) {
                return to;
            }
        }
        for (int i = 0; i < instruction.getNumOperands(); i++) {
            for (Object object : operandObjects(instruction, i)) {
                if (object instanceof Address address && isProgramAddress(address)) {
                    return address;
                }
            }
        }
        return null;
    }

    private Integer memoryDisplacementForThisLikeOperand(Instruction instruction, int operandIndex) {
        Object[] objects = operandObjects(instruction, operandIndex);
        boolean hasBaseRegister = false;
        int displacement = 0;
        for (Object object : objects) {
            if (object instanceof Register register) {
                String name = canonicalRegisterName(register.getName());
                if (!"RIP".equals(name) && !"RSP".equals(name)) {
                    hasBaseRegister = true;
                }
            }
            else if (object instanceof Scalar scalar) {
                long value = scalar.getSignedValue();
                if (value >= Integer.MIN_VALUE && value <= Integer.MAX_VALUE) {
                    displacement += (int)value;
                }
            }
        }
        return hasBaseRegister && displacement != 0 ? displacement : null;
    }

    private TrackedValue trackedLeaValue(
        Instruction instruction,
        Map<String, TrackedValue> registers) {

        Integer offset = trackedThisOffsetForMemoryOperand(instruction, 1, registers);
        if (offset != null) {
            return TrackedValue.thisOffset(offset);
        }

        Address address = referencedAddress(instruction);
        if (address != null) {
            return TrackedValue.address(address);
        }
        return null;
    }

    private TrackedValue trackedMoveSource(
        Instruction instruction,
        Map<String, TrackedValue> registers) {

        String sourceRegister = registerOperand(instruction, 1);
        if (sourceRegister != null) {
            TrackedValue value = registers.get(sourceRegister);
            return value == null ? null : value.copy();
        }

        Address address = referencedAddress(instruction);
        if (address != null) {
            return TrackedValue.address(address);
        }

        Long immediate = immediateValue(instruction, 1);
        return immediate == null ? null : TrackedValue.immediate(immediate);
    }

    private Integer trackedThisOffsetForMemoryOperand(
        Instruction instruction,
        int operandIndex,
        Map<String, TrackedValue> registers) {

        Object[] objects = operandObjects(instruction, operandIndex);
        String baseRegister = null;
        int displacement = 0;
        for (Object object : objects) {
            if (object instanceof Register register) {
                String name = canonicalRegisterName(register.getName());
                if (!"RIP".equals(name) && !"RSP".equals(name)) {
                    baseRegister = name;
                }
            }
            else if (object instanceof Scalar scalar) {
                long value = scalar.getSignedValue();
                if (value >= Integer.MIN_VALUE && value <= Integer.MAX_VALUE) {
                    displacement += (int)value;
                }
            }
        }

        if (baseRegister == null) {
            return null;
        }
        TrackedValue base = registers.get(baseRegister);
        if (base == null || base.thisOffset == null) {
            return null;
        }
        return base.thisOffset + displacement;
    }

    private void putOrRemove(
        Map<String, TrackedValue> registers,
        String register,
        TrackedValue value) {

        if (value == null) {
            registers.remove(register);
        }
        else {
            registers.put(register, value);
        }
    }

    private void clearVolatileRegisters(Map<String, TrackedValue> registers) {
        registers.remove("RAX");
        registers.remove("RCX");
        registers.remove("RDX");
        registers.remove("R8");
        registers.remove("R9");
        registers.remove("R10");
        registers.remove("R11");
    }

    private Long immediateValue(Instruction instruction, int operandIndex) {
        for (Object object : operandObjects(instruction, operandIndex)) {
            if (object instanceof Scalar scalar) {
                return scalar.getUnsignedValue();
            }
        }
        return null;
    }

    private Long parseIntegerLiteral(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim().replace("_", "");
        try {
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                return Long.parseUnsignedLong(trimmed.substring(2), 16);
            }
            return Long.parseLong(trimmed);
        }
        catch (NumberFormatException ignored) {
            return null;
        }
    }

    private String operandText(Instruction instruction, int operandIndex) {
        try {
            return instruction.getDefaultOperandRepresentation(operandIndex);
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private Object[] operandObjects(Instruction instruction, int operandIndex) {
        try {
            return instruction.getOpObjects(operandIndex);
        }
        catch (Exception ignored) {
            return new Object[0];
        }
    }

    private String registerOperand(Instruction instruction, int operandIndex) {
        String operandText = operandText(instruction, operandIndex);
        if (operandText != null && operandText.contains("[")) {
            return null;
        }
        Object[] objects = operandObjects(instruction, operandIndex);
        if (objects.length != 1 || !(objects[0] instanceof Register register)) {
            return null;
        }
        return canonicalRegisterName(register.getName());
    }

    private String canonicalRegisterName(String name) {
        if (name == null) {
            return null;
        }
        String upper = name.toUpperCase(Locale.ROOT);
        if (upper.length() == 2 && upper.charAt(1) == 'X') {
            return "R" + upper;
        }
        if (upper.length() == 3 && upper.charAt(0) == 'E') {
            return "R" + upper.substring(1);
        }
        if (upper.startsWith("R") && upper.endsWith("D")) {
            return upper.substring(0, upper.length() - 1);
        }
        if ("DIL".equals(upper) || "EDI".equals(upper)) {
            return "RDI";
        }
        if ("SIL".equals(upper) || "ESI".equals(upper)) {
            return "RSI";
        }
        if ("BPL".equals(upper) || "EBP".equals(upper)) {
            return "RBP";
        }
        if ("SPL".equals(upper) || "ESP".equals(upper)) {
            return "RSP";
        }
        return upper;
    }

    private String upperMnemonic(Instruction instruction) {
        String mnemonic = instruction.getMnemonicString();
        return mnemonic == null ? null : mnemonic.toUpperCase(Locale.ROOT);
    }

    private Function functionContaining(Address address) {
        if (address == null) {
            return null;
        }
        String key = addressCacheKey("function-containing", address);
        if (functionLookupCache.containsKey(key)) {
            return functionLookupCache.get(key);
        }
        Function function = currentProgram.getFunctionManager().getFunctionContaining(address);
        functionLookupCache.put(key, function);
        return function;
    }

    private Function functionAtOrContaining(Address address) {
        if (address == null) {
            return null;
        }
        String key = addressCacheKey("function-at-or-containing", address);
        if (functionLookupCache.containsKey(key)) {
            return functionLookupCache.get(key);
        }
        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function != null) {
            functionLookupCache.put(key, function);
            return function;
        }
        function = currentProgram.getFunctionManager().getFunctionContaining(address);
        functionLookupCache.put(key, function);
        return function;
    }

    private List<Instruction> functionInstructions(Function function) {
        if (function == null) {
            return List.of();
        }
        String key = functionCacheKey("instructions", function);
        List<Instruction> cached = functionInstructionsCache.get(key);
        if (cached != null) {
            return cached;
        }
        ArrayList<Instruction> instructions = new ArrayList<>();
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(function.getBody(), true)) {
            instructions.add(instruction);
        }
        List<Instruction> cachedInstructions =
            Collections.unmodifiableList(new ArrayList<>(instructions));
        functionInstructionsCache.put(key, cachedInstructions);
        return cachedInstructions;
    }

    private Address parseCapturedAddress(String value) {
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

    private Address firstAddress(Address left, Address right) {
        return left != null ? left : right;
    }

    private Address readPointer(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        String key = addressCacheKey("pointer", address);
        if (pointerReadCache.containsKey(key)) {
            return pointerReadCache.get(key);
        }
        try {
            long value = getLong(address);
            Address pointer = value == 0
                ? null
                : currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(value);
            pointerReadCache.put(key, pointer);
            return pointer;
        }
        catch (Exception ignored) {
            pointerReadCache.put(key, null);
            return null;
        }
    }

    private boolean isProgramAddress(Address address) {
        return address != null && currentProgram.getMemory().contains(address);
    }

    private String readPrintableString(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        try {
            StringBuilder builder = new StringBuilder();
            for (int i = 0; i < 512; i++) {
                int value = getByte(address.add(i)) & 0xff;
                if (value == 0) {
                    return builder.toString();
                }
                if (value < 0x20 || value > 0x7e) {
                    return null;
                }
                builder.append((char)value);
            }
        }
        catch (Exception ignored) {
        }
        return null;
    }

    private StringDecode readFieldNameAtOrThroughPointer(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }

        String direct = readPrintableString(address);
        if (isFieldName(direct)) {
            return new StringDecode(address, direct);
        }

        Address pointer = readPointer(address);
        if (isProgramAddress(pointer)) {
            String indirect = readPrintableString(pointer);
            if (isFieldName(indirect)) {
                return new StringDecode(pointer, indirect);
            }
        }

        return null;
    }

    private boolean isFieldName(String value) {
        if (value == null || value.isEmpty() || value.length() > 160) {
            return false;
        }
        if (value.contains("::") || value.contains("AZStd::")) {
            return false;
        }
        char first = value.charAt(0);
        return Character.isLetter(first) || first == '_' || first == '[';
    }

    private boolean isPlausibleTypeName(String value) {
        if (value == null || value.isEmpty() || value.length() > 512) {
            return false;
        }
        if (canonicalUuidFromString(value) != null) {
            return false;
        }
        if (value.startsWith("{") || value.indexOf('/') >= 0 || value.indexOf('\\') >= 0) {
            return false;
        }
        if (value.indexOf(' ') >= 0 && value.indexOf('<') < 0) {
            return false;
        }
        for (int i = 0; i < value.length(); i++) {
            if (value.charAt(i) < 0x20) {
                return false;
            }
        }
        return value.matches(".*[A-Za-z_].*");
    }

    private String canonicalUuidFromString(String value) {
        if (value == null) {
            return null;
        }
        Matcher matcher = UUID_RE.matcher(value);
        return matcher.find() ? matcher.group(1).toUpperCase(Locale.ROOT) : null;
    }

    private String normalizeUuid(String value) {
        String uuid = canonicalUuidFromString(value);
        return uuid == null ? null : uuid.toUpperCase(Locale.ROOT);
    }

    private boolean uuidEquals(String left, String right) {
        String lhs = normalizeUuid(left);
        String rhs = normalizeUuid(right);
        return lhs != null && lhs.equals(rhs);
    }

    private String formatAddress(Address address) {
        if (address == null) {
            return null;
        }
        long base = currentProgram.getImageBase().getOffset();
        long value = address.getOffset();
        if (Long.compareUnsigned(value, base) >= 0) {
            return "NewWorld+0x" + Long.toHexString(value - base);
        }
        return "0x" + Long.toHexString(value);
    }

    private String fullFunctionName(Function function) {
        if (function == null) {
            return null;
        }
        String namespace = function.getParentNamespace() == null
            ? null
            : function.getParentNamespace().getName(true);
        String key = functionCacheKey("function-name:" + namespace + "::" + function.getName(), function);
        String cached = functionNameCache.get(key);
        if (cached != null) {
            return cached;
        }
        if (namespace == null || namespace.isEmpty() || "Global".equals(namespace)) {
            String name = function.getName();
            functionNameCache.put(key, name);
            return name;
        }
        String name = namespace + "::" + function.getName();
        functionNameCache.put(key, name);
        return name;
    }

    private JsonObject object(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement element = object.get(name);
        return element != null && element.isJsonObject() ? element.getAsJsonObject() : null;
    }

    private JsonArray array(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement element = object.get(name);
        return element != null && element.isJsonArray() ? element.getAsJsonArray() : null;
    }

    private String string(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement element = object.get(name);
        return stringValue(element);
    }

    private String stringValue(JsonElement element) {
        if (element == null || element.isJsonNull()) {
            return null;
        }
        String value = element.getAsString();
        return value == null || value.isEmpty() ? null : value;
    }

    private Integer integer(JsonObject object, String name) {
        if (object == null) {
            return null;
        }
        JsonElement element = object.get(name);
        if (element == null || element.isJsonNull()) {
            return null;
        }
        return element.getAsInt();
    }

    private String firstNonEmpty(String left, String right) {
        return left != null && !left.isEmpty() ? left : right;
    }

    private Integer firstNonNull(Integer left, Integer right) {
        return left != null ? left : right;
    }

    private final class RegistryEntry {
        String uuid;
        String name;
        Integer index;
        Integer typeIndex;
        String storageAddress;
        String baseVtable;
        String vtable;
        String destructor;
        String getEmptyValue;
        String createInstance;
        String copyValue;
        String marshal;
        String unmarshal;

        JsonObject toJson(AzRttiEvidence azRttiEvidence, HookTypeEvidence hookTypeEvidence) {
            JsonObject object = new JsonObject();
            add(object, "uuid", uuid);
            add(object, "name", name);
            add(object, "index", index);
            add(object, "typeIndex", typeIndex);
            add(object, "storageAddress", storageAddress);
            add(object, "baseVtable", baseVtable);
            add(object, "vtable", vtable);

            if (hookTypeEvidence != null &&
                isPlausibleTypeName(hookTypeEvidence.typeName)) {
                add(object, "typeName", hookTypeEvidence.typeName);
                add(object, "typeNameSource", "registrationHook");
            }
            else if (azRttiEvidence != null &&
                isPlausibleTypeName(azRttiEvidence.typeName)) {
                add(object, "typeName", azRttiEvidence.typeName);
                add(object, "typeNameSource", "azRtti");
            }

            JsonObject handler = new JsonObject();
            add(handler, "Destructor", destructor);
            add(handler, "GetEmptyValue", getEmptyValue);
            add(handler, "CreateInstance", createInstance);
            add(handler, "CopyValue", copyValue);
            add(handler, "Marshal", marshal);
            add(handler, "Unmarshal", unmarshal);
            object.add("handler", handler);

            if (azRttiEvidence != null) {
                object.add("azRtti", azRttiEvidence.toJson());
            }
            if (hookTypeEvidence != null) {
                add(object, "registrationTypeName", hookTypeEvidence.typeName);
                object.add("registrationHook", hookTypeEvidence.toJson());
            }
            return object;
        }
    }

    private final class RegistrationFunction {
        final Function function;
        final ArrayList<FieldCall> fields = new ArrayList<>();
        Address instanceVtable;
        AzRttiEvidence azRtti;

        RegistrationFunction(Function function) {
            this.function = function;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("address", formatAddress(function.getEntryPoint()));
            object.addProperty("name", fullFunctionName(function));
            add(object, "constructorTypeName", constructorTypeName());
            if (instanceVtable != null) {
                object.addProperty("instanceVtable", formatAddress(instanceVtable));
                object.add("virtualFunctions", virtualFunctionSlots(instanceVtable));
            }
            if (azRtti != null) {
                object.add("azRtti", azRtti.toJson());
            }
            JsonArray array = new JsonArray();
            for (FieldCall field : fields) {
                array.add(field.toJson());
            }
            object.add("fields", array);
            return object;
        }

        String constructorTypeName() {
            String name = fullFunctionName(function);
            if (!isPlausibleTypeName(name) || name.startsWith("FUN_") ||
                name.contains("::FUN_")) {
                return null;
            }

            String[] parts = name.split("::");
            if (parts.length >= 2 && parts[parts.length - 1].equals(parts[parts.length - 2])) {
                return name.substring(0, name.length() - parts[parts.length - 1].length() - 2);
            }
            return name;
        }
    }

    private final class FieldCall {
        int index;
        Address callsite;
        int recoveryOrder = Integer.MAX_VALUE;
        Address nameAddress;
        String name;
        String nameSource;
        Address nameSourceAddress;
        Integer group;
        Integer handlerOffset;
        String handlerExpression;
        Address handlerVtable;
        String nativeType;
        String sourceTypeName;
        String storageExpression;
        Long storageOffset;
        Integer rawByteLength;
        String wireShape;
        String wireShapeSource;
        String confidence;

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("index", index);
            add(object, "callsite", formatAddress(callsite));
            add(object, "name", name);
            add(object, "nameSource", nameSource);
            add(object, "nameSourceAddress", formatAddress(nameSourceAddress));
            add(object, "nameAddress", formatAddress(nameAddress));
            add(object, "group", group);
            if (handlerOffset != null) {
                object.addProperty("handlerOffset", "0x" + Integer.toHexString(handlerOffset));
            }
            add(object, "handlerExpression", handlerExpression);
            add(object, "handlerVtable", formatAddress(handlerVtable));
            add(object, "nativeType", nativeType);
            add(object, "sourceTypeName", sourceTypeName);
            add(object, "storageExpression", storageExpression);
            if (storageOffset != null) {
                object.addProperty("storageOffset", "0x" + Long.toHexString(storageOffset));
            }
            if (rawByteLength != null) {
                object.addProperty("rawByteLength", rawByteLength);
            }
            add(object, "wireShape", wireShape);
            add(object, "wireShapeSource", wireShapeSource);
            add(object, "confidence", confidence);
            return object;
        }
    }

    private final class MessageUnmarshalPlan {
        Address wrapper;
        String wrapperName;
        Address helperCallsite;
        Address helper;
        String helperName;
        Address createInstance;
        Long instanceSize;
        String instanceSizeSource;
        Address instanceConstructorCallsite;
        Address instanceConstructor;
        String instanceConstructorName;
        final ArrayList<String> templateTypes = new ArrayList<>();
        final ArrayList<FieldCall> fields = new ArrayList<>();
        final ArrayList<MessageSourceSignature> sourceSignatures = new ArrayList<>();

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "wrapper", formatAddress(wrapper));
            add(object, "wrapperName", wrapperName);
            add(object, "helperCallsite", formatAddress(helperCallsite));
            add(object, "helper", formatAddress(helper));
            add(object, "helperName", helperName);
            add(object, "createInstance", formatAddress(createInstance));
            if (instanceSize != null) {
                object.addProperty("instanceSize", "0x" + Long.toHexString(instanceSize));
            }
            add(object, "instanceSizeSource", instanceSizeSource);
            add(object, "instanceConstructorCallsite", formatAddress(instanceConstructorCallsite));
            add(object, "instanceConstructor", formatAddress(instanceConstructor));
            add(object, "instanceConstructorName", instanceConstructorName);

            JsonArray templateJson = new JsonArray();
            for (String templateType : templateTypes) {
                templateJson.add(templateType);
            }
            object.add("templateTypes", templateJson);

            JsonArray fieldJson = new JsonArray();
            for (FieldCall field : fields) {
                fieldJson.add(field.toJson());
            }
            object.add("fields", fieldJson);

            if (!sourceSignatures.isEmpty()) {
                JsonArray sourceJson = new JsonArray();
                for (MessageSourceSignature signature : sourceSignatures) {
                    sourceJson.add(signature.toJson());
                }
                object.add("sourceSignatures", sourceJson);
            }
            return object;
        }
    }

    private final class MessageSourceSignature {
        Address stringAddress;
        Address typeDescriptor;
        String mangledName;
        final LinkedHashMap<String, String> providers = new LinkedHashMap<>();
        final LinkedHashSet<String> tableReferences = new LinkedHashSet<>();
        final LinkedHashMap<String, String> sourceFunctions = new LinkedHashMap<>();
        final JsonArray callGraph = new JsonArray();

        void addProvider(Function function, NetworkSchemaExtractor extractor) {
            providers.putIfAbsent(
                extractor.formatAddress(function.getEntryPoint()),
                extractor.fullFunctionName(function));
        }

        boolean addSourceFunction(Function function, NetworkSchemaExtractor extractor) {
            return sourceFunctions.putIfAbsent(
                extractor.formatAddress(function.getEntryPoint()),
                extractor.fullFunctionName(function)) == null;
        }

        void addCallTarget(
            Function caller,
            Address callsite,
            Function target,
            NetworkSchemaExtractor extractor) {

            JsonObject edge = new JsonObject();
            edge.addProperty("callsite", extractor.formatAddress(callsite));
            edge.addProperty("caller", extractor.formatAddress(caller.getEntryPoint()));
            edge.addProperty("callerName", extractor.fullFunctionName(caller));
            edge.addProperty("target", extractor.formatAddress(target.getEntryPoint()));
            edge.addProperty("targetName", extractor.fullFunctionName(target));
            callGraph.add(edge);
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("source", "msvc-rtti-type-descriptor");
            add(object, "stringAddress", formatAddress(stringAddress));
            add(object, "typeDescriptor", formatAddress(typeDescriptor));
            add(object, "mangledName", mangledName);
            object.add("providers", functionMapJson(providers));
            JsonArray tableJson = new JsonArray();
            for (String tableReference : tableReferences) {
                tableJson.add(tableReference);
            }
            object.add("tableReferences", tableJson);
            object.add("sourceFunctions", functionMapJson(sourceFunctions));
            object.add("callGraph", callGraph);
            return object;
        }

        private static JsonArray functionMapJson(LinkedHashMap<String, String> functions) {
            JsonArray array = new JsonArray();
            for (Map.Entry<String, String> entry : functions.entrySet()) {
                JsonObject object = new JsonObject();
                object.addProperty("address", entry.getKey());
                object.addProperty("name", entry.getValue());
                array.add(object);
            }
            return array;
        }

        private static void add(JsonObject object, String name, String value) {
            if (value != null) {
                object.addProperty(name, value);
            }
        }
    }

    private static final class MessageHelperCall {
        Address callsite;
        Address target;
        String targetName;
    }

    private static final class MessageConstructorCall {
        Address callsite;
        Address target;
        String targetName;
    }

    private static final class ParsedUnmarshalFieldsCall {
        final ArrayList<String> templateTypes = new ArrayList<>();
        final ArrayList<ParsedArgument> fieldArgs = new ArrayList<>();
    }

    private static final class ParsedUnmarshalCall {
        final String templateType;
        final int textIndex;
        final List<String> args;

        ParsedUnmarshalCall(String templateType, int textIndex, List<String> args) {
            this.templateType = templateType;
            this.textIndex = textIndex;
            this.args = Collections.unmodifiableList(new ArrayList<>(args));
        }
    }

    private static final class ParsedArgument {
        String castType;
        String expression;
    }

    private static final class ParsedReadRawCall {
        final String storageExpression;
        final int byteLength;
        final int textIndex;

        ParsedReadRawCall(String storageExpression, int byteLength, int textIndex) {
            this.storageExpression = storageExpression;
            this.byteLength = byteLength;
            this.textIndex = textIndex;
        }
    }

    private final class FieldHandlerVtable {
        final Address address;
        int fieldCount;

        FieldHandlerVtable(Address address) {
            this.address = address;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("address", formatAddress(address));
            object.addProperty("fieldCount", fieldCount);

            JsonArray slots = new JsonArray();
            Address marshal = null;
            Address marshalTarget = null;
            Address unmarshal = null;
            Address unmarshalTarget = null;
            for (int slot = 0; slot < FIELD_HANDLER_VTABLE_SLOTS; slot++) {
                Address target = readPointer(address.add(slot * 8L));
                if (target == null || !isExecutableAddress(target)) {
                    continue;
                }
                String name = slot < FIELD_HANDLER_SLOT_NAMES.length
                    ? FIELD_HANDLER_SLOT_NAMES[slot]
                    : null;
                slots.add(virtualFunctionSlot(slot, name, target));
                if (slot == FIELD_HANDLER_MARSHAL_SLOT) {
                    marshal = target;
                    marshalTarget = terminalJumpTarget(target);
                    object.addProperty("marshal", formatAddress(marshal));
                    add(object, "marshalTarget", formatAddress(marshalTarget));
                }
                else if (slot == FIELD_HANDLER_UNMARSHAL_SLOT) {
                    unmarshal = target;
                    unmarshalTarget = terminalJumpTarget(target);
                    object.addProperty("unmarshal", formatAddress(unmarshal));
                    add(object, "unmarshalTarget", formatAddress(unmarshalTarget));
                }
            }
            WireShape wireShape = classifyWireShape(marshal, marshalTarget);
            if (wireShape != null) {
                object.addProperty("wireShape", wireShape.shape);
                object.addProperty("wireShapeSource", wireShape.source);
            }
            object.add("slots", slots);
            return object;
        }
    }

    private static final class WireShape {
        final String shape;
        final String source;

        WireShape(String shape, String source) {
            this.shape = shape;
            this.source = source;
        }
    }

    private final class AzRttiEvidence {
        String source;
        String address;
        String typeId;
        String typeName;
        final JsonArray providers = new JsonArray();

        boolean hasIdentity() {
            return typeId != null || typeName != null;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "source", source);
            add(object, "address", address);
            add(object, "typeId", typeId);
            add(object, "typeName", typeName);
            if (providers.size() != 0) {
                object.add("providers", providers);
            }
            return object;
        }
    }

    private final class HookTypeEvidence {
        Address hookFunction;
        Address helperTable;
        Address registerThunk;
        Address typeProvider;
        Address uuidSource;
        Address typeDescriptor;
        Address slotTypeNameFunction;
        Address handlerVtable;
        Address createInstance;
        Address marshal;
        Address unmarshal;
        String typeId;
        String typeName;
        String slotTypeName;

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "typeId", typeId);
            add(object, "typeName", typeName);
            add(object, "slotTypeName", slotTypeName);
            add(object, "hookFunction", formatAddress(hookFunction));
            add(object, "helperTable", formatAddress(helperTable));
            add(object, "registerThunk", formatAddress(registerThunk));
            add(object, "typeProvider", formatAddress(typeProvider));
            add(object, "uuidSource", formatAddress(uuidSource));
            add(object, "typeDescriptor", formatAddress(typeDescriptor));
            add(object, "slotTypeNameFunction", formatAddress(slotTypeNameFunction));
            if (handlerVtable != null) {
                JsonObject handler = new JsonObject();
                add(handler, "handlerVtable", formatAddress(handlerVtable));
                add(handler, "CreateInstance", formatAddress(createInstance));
                add(handler, "Marshal", formatAddress(marshal));
                add(handler, "Unmarshal", formatAddress(unmarshal));
                JsonArray slots = new JsonArray();
                for (int slot = 0; slot < MESSAGE_HANDLER_VTABLE_SLOTS; slot++) {
                    Address target = readPointer(handlerVtable.add(slot * 8L));
                    if (target == null || !isExecutableAddress(target)) {
                        break;
                    }
                    slots.add(virtualFunctionSlot(
                        slot,
                        messageHandlerSlotName(slot),
                        target));
                }
                handler.add("slots", slots);
                object.add("messageHandler", handler);
            }
            return object;
        }

        JsonObject toProviderJson() {
            JsonObject object = toJson();
            object.addProperty("kind", "typeName");
            object.addProperty("source", "install-registration-hook");
            return object;
        }
    }

    private final class TypeIdDecode {
        Address function;
        Address provider;
        Address sourceAddress;
        String typeId;
        String typeIdSource;

        JsonObject toJson(int slot) {
            JsonObject object = new JsonObject();
            object.addProperty("kind", "typeId");
            object.addProperty("slot", slot);
            object.addProperty("slotOffset", "0x" + Integer.toHexString(slot * 8));
            object.addProperty("function", formatAddress(function));
            object.addProperty("provider", formatAddress(provider));
            object.addProperty("typeId", typeId);
            object.addProperty("typeIdSource", typeIdSource);
            object.addProperty("sourceAddress", formatAddress(sourceAddress));
            return object;
        }
    }

    private final class TypeNameDecode {
        Address function;
        Address provider;
        Address typeNameAddress;
        String typeName;
        String typeNameSource;

        JsonObject toJson(int slot) {
            JsonObject object = new JsonObject();
            object.addProperty("kind", "typeName");
            object.addProperty("slot", slot);
            object.addProperty("slotOffset", "0x" + Integer.toHexString(slot * 8));
            object.addProperty("function", formatAddress(function));
            object.addProperty("provider", formatAddress(provider));
            object.addProperty("typeName", typeName);
            object.addProperty("typeNameSource", typeNameSource);
            object.addProperty("typeNameAddress", formatAddress(typeNameAddress));
            return object;
        }
    }

    private static final class ArgState {
        Address nameAddress;
        String name;
        boolean groupKnown;
        int group;
        boolean handlerKnown;
        Integer handlerOffset;
        String handlerExpression;
        Address handlerVtable;

        void fillMissingFrom(ArgState fallback) {
            if (nameAddress == null) {
                nameAddress = fallback.nameAddress;
                name = fallback.name;
            }
            if (!groupKnown && fallback.groupKnown) {
                groupKnown = true;
                group = fallback.group;
            }
            if (!handlerKnown && fallback.handlerKnown) {
                handlerKnown = true;
                handlerOffset = fallback.handlerOffset;
                handlerExpression = fallback.handlerExpression;
                handlerVtable = fallback.handlerVtable;
            }
            else if (handlerVtable == null) {
                handlerVtable = fallback.handlerVtable;
            }
        }
    }

    private static final class ForwardArgState {
        final Map<String, TrackedValue> registers = new HashMap<>();
        final Map<Integer, Address> vtablesByThisOffset = new HashMap<>();
    }

    private static final class TrackedValue {
        final Address address;
        final Integer thisOffset;
        final Long immediate;
        final String expression;

        private TrackedValue(
            Address address,
            Integer thisOffset,
            Long immediate,
            String expression) {

            this.address = address;
            this.thisOffset = thisOffset;
            this.immediate = immediate;
            this.expression = expression;
        }

        static TrackedValue address(Address address) {
            return new TrackedValue(address, null, null, null);
        }

        static TrackedValue thisOffset(int offset) {
            return new TrackedValue(null, offset, null, thisExpression(offset));
        }

        static TrackedValue immediate(long value) {
            return new TrackedValue(null, null, value, Long.toUnsignedString(value));
        }

        TrackedValue addOffset(int delta) {
            if (thisOffset == null) {
                return this;
            }
            return thisOffset(thisOffset + delta);
        }

        TrackedValue copy() {
            return new TrackedValue(address, thisOffset, immediate, expression);
        }

        private static String thisExpression(int offset) {
            if (offset == 0) {
                return "this";
            }
            if (offset > 0) {
                return "this+0x" + Integer.toHexString(offset);
            }
            return "this-0x" + Integer.toHexString(-offset);
        }
    }

    private static final class StringDecode {
        final Address address;
        final String value;

        StringDecode(Address address, String value) {
            this.address = address;
            this.value = value;
        }
    }

    private void add(JsonObject object, String name, String value) {
        if (value != null) {
            object.addProperty(name, value);
        }
    }

    private void add(JsonObject object, String name, Integer value) {
        if (value != null) {
            object.addProperty(name, value);
        }
    }
}
