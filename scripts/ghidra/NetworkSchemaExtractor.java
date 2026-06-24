// Extract network type and field registration evidence from typeregistry.json and Ghidra.
//@category NewWorld

import java.io.File;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.Reader;
import java.util.ArrayList;
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
    private static final long REGISTER_FIELD_RVA = 0x1775c60L;
    private static final long QUEUE_REGISTRATION_HOOK_RVA = 0x61a95c0L;
    private static final int BACKWARD_ARGUMENT_SCAN_LIMIT = 48;
    private static final int VTABLE_SCAN_LIMIT = 96;
    private static final int AZ_RTTI_VTABLE_SCAN_SLOTS = 24;
    private static final int TYPE_ID_PROVIDER_BYTES = 256;
    private static final int TYPE_NAME_PROVIDER_BYTES = 384;
    private static final Pattern MODULE_ADDR_RE =
        Pattern.compile("(?i)^NewWorld\\+0x(?<offset>[0-9a-f]+)$");
    private static final Pattern HEX_ADDR_RE =
        Pattern.compile("(?i)^0x(?<addr>[0-9a-f]+)$");
    private static final Pattern UUID_RE = Pattern.compile(
        "(?i)\\{?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\\}?");
    private static final Pattern INSTALL_REGISTRATION_HOOK_RE =
        Pattern.compile("InstallRegistrationHook<(?<type>[^>]+)>");

    private final Gson gson = new GsonBuilder()
        .disableHtmlEscaping()
        .setPrettyPrinting()
        .create();

    private final Map<String, Address> pointerReadCache = new HashMap<>();

    @Override
    protected void run() throws Exception {
        File input = inputFile();
        File output = outputFile(input);
        Address registerField = currentProgram.getImageBase().add(REGISTER_FIELD_RVA);

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
            registryJson.add(row);
        }

        JsonArray functionJson = new JsonArray();
        int dynamicFieldCount = 0;
        for (RegistrationFunction function : registrationFunctions.values()) {
            dynamicFieldCount += function.fields.size();
            functionJson.add(function.toJson());
        }

        JsonObject report = new JsonObject();
        report.addProperty("schema", "newworld.network_schema.static.v1");
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
        summary.addProperty("mappedRegistryEntries", mappedRegistryEntries);
        summary.addProperty("mappedRegistryFields", mappedFieldCount);
        report.add("summary", summary);

        report.add("registryEntries", registryJson);
        report.add("installRegistrationHooks", hookTypeNamesJson(hookTypeNamesById));
        report.add("fieldRegistrationFunctions", functionJson);

        try (FileWriter writer = new FileWriter(output)) {
            gson.toJson(report, writer);
        }

        println("Wrote network schema static report: " + output.getAbsolutePath());
        println("RegisterField functions: " + registrationFunctions.size() +
            ", calls: " + dynamicFieldCount +
            ", mapped registry entries: " + mappedRegistryEntries);
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
                return hook;
            }
        }
        return null;
    }

    private Address findRegistrationHelperTable(Function function) {
        int count = 0;
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(function.getBody(), true)) {
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
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(function.getBody(), true)) {
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

    private List<Address> unmarshalCallTargets(RegistryEntry entry) {
        ArrayList<Address> result = new ArrayList<>();
        Address unmarshalAddress = parseCapturedAddress(entry.unmarshal);
        Function unmarshal = functionAtOrContaining(unmarshalAddress);
        if (unmarshal == null) {
            return result;
        }
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(unmarshal.getBody(), true)) {
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

    private FieldCall recoverFieldCall(Function owner, Address callsite, int index) {
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

        FieldCall field = new FieldCall();
        field.index = index;
        field.callsite = callsite;
        field.nameAddress = state.nameAddress;
        field.name = state.name;
        field.group = state.groupKnown ? state.group : null;
        field.handlerOffset = state.handlerOffset;
        field.handlerExpression = state.handlerExpression;
        field.confidence = field.name == null
            ? "register-field-call-unresolved-name"
            : "register-field-call";
        return field;
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
                String value = readPrintableString(address);
                if (isFieldName(value)) {
                    state.nameAddress = address;
                    state.name = value;
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
        for (Instruction instruction :
            currentProgram.getListing().getInstructions(function.getBody(), true)) {
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
            JsonObject slot = new JsonObject();
            slot.addProperty("slot", i);
            slot.addProperty("slotOffset", "0x" + Integer.toHexString(i * 8));
            slot.addProperty("address", formatAddress(target));
            Function function = functionAtOrContaining(target);
            if (function != null) {
                slot.addProperty("function", fullFunctionName(function));
            }
            result.add(slot);
        }
        return result;
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
            return null;
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

    private Long immediateValue(Instruction instruction, int operandIndex) {
        for (Object object : operandObjects(instruction, operandIndex)) {
            if (object instanceof Scalar scalar) {
                return scalar.getUnsignedValue();
            }
        }
        return null;
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
        return currentProgram.getFunctionManager().getFunctionContaining(address);
    }

    private Function functionAtOrContaining(Address address) {
        if (address == null) {
            return null;
        }
        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function != null) {
            return function;
        }
        return currentProgram.getFunctionManager().getFunctionContaining(address);
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

    private Address readPointer(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        String key = address.toString();
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
        String namespace = function.getParentNamespace() == null
            ? null
            : function.getParentNamespace().getName(true);
        if (namespace == null || namespace.isEmpty() || "Global".equals(namespace)) {
            return function.getName();
        }
        return namespace + "::" + function.getName();
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
        Address nameAddress;
        String name;
        Integer group;
        Integer handlerOffset;
        String handlerExpression;
        String confidence;

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("index", index);
            object.addProperty("callsite", formatAddress(callsite));
            add(object, "name", name);
            add(object, "nameAddress", formatAddress(nameAddress));
            add(object, "group", group);
            if (handlerOffset != null) {
                object.addProperty("handlerOffset", "0x" + Integer.toHexString(handlerOffset));
            }
            add(object, "handlerExpression", handlerExpression);
            add(object, "confidence", confidence);
            return object;
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
