// Extract network type and field registration evidence from typeregistry.json and Ghidra.
//@category NewWorld

import java.io.File;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Deque;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
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
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeComponent;
import ghidra.program.model.data.Structure;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Parameter;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.PcodeOpAST;
import ghidra.program.model.pcode.Varnode;
import ghidra.program.model.pcode.VarnodeAST;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class NetworkSchemaExtractor extends GhidraScript {
    private static final String EXTRACTOR_VERSION = "network-schema-extractor-20260629-direct-unmarshal-fields-pass";
    private static final String CACHE_SCHEMA_VERSION = EXTRACTOR_VERSION + "/analysis-cache-v1";
    private static final long REGISTER_FIELD_RVA = 0x1775c60L;
    private static final long ADD_FILTER_GROUP_RVA = 0x1677dd0L;
    private static final long FIND_ACTOR_FACET_TARGET_RVA = 0x16bcba0L;
    private static final long FIND_ACTOR_FACET_TARGET_MESSAGE_COPY_RVA = 0x16bcbc2L;
    private static final long FIND_ACTOR_FACET_TARGET_TARGET_OFFSET_RVA = 0x16bcc0aL;
    private static final long FIND_ACTOR_FACET_TARGET_TARGET_ADD_RVA = 0x16bcc10L;
    private static final long FIND_ACTOR_FACET_TARGET_TARGET_LOAD_RVA = 0x16bcd01L;
    private static final long FIND_ACTOR_FACET_TARGET_TARGET_NAME_RVA = 0x16bcd04L;
    private static final long REPLICATED_STATE_CONSTRUCTOR_RVA = 0x162f3e0L;
    private static final long TYPE_REGISTRY_REGISTER_TYPE_RVA = 0x61a9740L;
    private static final long QUEUE_REGISTRATION_HOOK_RVA = 0x61a95c0L;
    private static final int BACKWARD_ARGUMENT_SCAN_LIMIT = 48;
    private static final int VTABLE_SCAN_LIMIT = 96;
    private static final int DIRECT_FIELD_TABLE_SCAN_LIMIT = 12000;
    private static final int FIXED_FIELD_TABLE_ENTRY_SIZE = 0x10;
    private static final int REPLICATED_STATE_ATTRIBUTE_VECTOR_OFFSET = 0x680;
    private static final int VECTOR_CURRENT_POINTER_OFFSET = 0x8;
    private static final int AZ_RTTI_VTABLE_SCAN_SLOTS = 24;
    private static final int FIELD_HANDLER_SCALAR_VTABLE_SLOTS = 14;
    private static final int FIELD_HANDLER_CONTAINER_VTABLE_SLOTS = 20;
    private static final int FIELD_HANDLER_CONSTRUCTOR_WRITE_SPAN = 0x200;
    private static final int FIELD_HANDLER_MARSHAL_SLOT = 5;
    private static final int FIELD_HANDLER_UNMARSHAL_SLOT = 6;
    private static final int FIELD_HANDLER_MARSHAL_FULL_SLOT = 14;
    private static final int FIELD_HANDLER_UNMARSHAL_FULL_SLOT = 15;
    private static final int MESSAGE_HANDLER_VTABLE_SLOTS = 12;
    private static final int MESSAGE_HANDLER_CREATE_INSTANCE_SLOT = 2;
    private static final int MESSAGE_HANDLER_MARSHAL_SLOT = 4;
    private static final int MESSAGE_HANDLER_UNMARSHAL_SLOT = 5;
    private static final int MESSAGE_HANDLER_PROVIDER_SCAN_LIMIT = 512;
    private static final int I_FRAGMENT_IS_METADATA_SLOT = 12;
    private static final int I_FRAGMENT_GET_CATEGORY_SLOT = 13;
    private static final int I_FRAGMENT_VTABLE_SCAN_SLOTS = 24;
    private static final int CONSTANT_RETURN_SCAN_LIMIT = 12;
    private static final int REGISTRY_HANDLER_RTTI_CALL_DEPTH = 3;
    private static final int REGISTRY_HANDLER_RTTI_FUNCTION_LIMIT = 96;
    private static final int REGISTRY_HANDLER_RTTI_SCAN_LIMIT = 160;
    private static final int REGISTRATION_FAILURE_SAMPLE_LIMIT = 256;
    private static final int SOURCE_SIGNATURE_XREF_SCAN_BYTES = 0x40;
    private static final int SOURCE_SIGNATURE_CALL_GRAPH_DEPTH = 2;
    private static final int SOURCE_SIGNATURE_CALL_GRAPH_LIMIT = 32;
    private static final int PCODE_VALUE_DEPTH_LIMIT = 16;
    private static final int PCODE_ALIAS_DESCENDANT_LIMIT = 96;
    private static final int NESTED_DIRECT_TYPE_DEPTH_LIMIT = 4;
    private static final int NESTED_DIRECT_TYPE_MEMBER_LIMIT = 32;
    private static final int CONSTRUCTOR_VTABLE_RECURSION_LIMIT = 48;
    private static final int INHERITED_FORWARD_STATE_RECURSION_LIMIT = 48;
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
        "MarshalFull",
        "UnmarshalFull",
        "CopyContainer",
        "ApplyChangeSet",
        "SummarizeChanges",
        "GetSize",
    };
    private static final Pattern MODULE_ADDR_RE =
        Pattern.compile("(?i)^NewWorld\\+0x(?<offset>[0-9a-f]+)$");
    private static final Pattern HEX_ADDR_RE =
        Pattern.compile("(?i)^0x(?<addr>[0-9a-f]+)$");
    private static final Pattern UUID_RE = Pattern.compile(
        "(?i)\\{?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\\}?");
    private static final Pattern INSTALL_REGISTRATION_HOOK_RE =
        Pattern.compile("InstallRegistrationHook<(?<type>[^>]+)>");
    private static final Pattern RTTI_HELPER_NAME_RE =
        Pattern.compile("AZ::Internal::RttiHelper<(?<type>.+)>::");
    private static final Pattern BOOL_POINTER_WRITE_RE =
        Pattern.compile("\\*\\(bool \\*\\)\\s*(?<target>[A-Za-z_][A-Za-z0-9_]*)\\s*=");
    private static final Pattern STORAGE_OFFSET_RE =
        Pattern.compile("\\b(?<base>[A-Za-z_][A-Za-z0-9_]*)\\s*\\+\\s*(?<offset>0x[0-9a-fA-F]+|\\d+)");
    private static final Pattern POINTER_STORE_RE =
        Pattern.compile("\\*\\((?<type>[^)]*\\*)\\)\\s*(?<target>[^=;]+?)\\s*=\\s*(?<rhs>[^;]+);");
    private static final String NIL_TYPE_ID = "00000000-0000-0000-0000-000000000000";
    private static final String CHAR_TYPE_ID = "3AB0037F-AF8D-48CE-BCA0-A170D18B2C03";
    private static final String S8_TYPE_ID = "58422C0E-1E47-4854-98E6-34098F6FE12D";
    private static final String SHORT_TYPE_ID = "B8A56D56-A10D-4DCE-9F63-405EE243DD3C";
    private static final String INT_TYPE_ID = "72039442-EB38-4D42-A1AD-CB68F7E0EEF6";
    private static final String LONG_TYPE_ID = "8F24B9AD-7C51-46CF-B2F8-277356957325";
    private static final String S64_TYPE_ID = "70D8A282-A1EA-462D-9D04-51EDE81FAC2F";
    private static final String U8_TYPE_ID = "72B9409A-7D1A-4831-9CFE-FCB3FADD3426";
    private static final String U16_TYPE_ID = "ECA0B403-C4F8-4B86-95FC-81688D046E40";
    private static final String U32_TYPE_ID = "43DA906B-7DEF-4CA8-9790-854106D3F983";
    private static final String ULONG_TYPE_ID = "5EC2D6F7-6859-400F-9215-C106F5B10E53";
    private static final String U64_TYPE_ID = "D6597933-47CD-4FC8-B911-63F3E2B0993A";
    private static final String FLOAT_TYPE_ID = "EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D";
    private static final String DOUBLE_TYPE_ID = "110C4B14-11A8-4E9D-8638-5051013A56AC";
    private static final String BOOL_TYPE_ID = "A0CA880C-AFE4-43CB-926C-59AC48496112";
    private static final String AZ_UUID_TYPE_ID = "E152C105-A133-4D03-BBF8-3D4B2FBA3E2A";
    private static final String ENTITY_ID_TYPE_ID = "6383F1D3-BB27-4E6B-A49A-6409B2059EAA";
    private static final String CRC32_TYPE_ID = "9F4E062E-06A0-46D4-85DF-E0DA96467D3A";
    private static final String VECTOR2_TYPE_ID = "3D80F623-C85C-4741-90D0-E4E66164E6BF";
    private static final String VECTOR3_TYPE_ID = "8379EB7D-01FA-4538-B64B-A6543B4BE73D";
    private static final String VECTOR4_TYPE_ID = "0CE9FA36-1E3A-4C06-9254-B7C73A732053";
    private static final String TRANSFORM_TYPE_ID = "5D9958E9-9F1E-4985-B532-FFFDE75FEDFD";
    private static final String QUATERNION_TYPE_ID = "73103120-3DD3-4873-BAB3-9713FA2804FB";
    private static final String COLOR_TYPE_ID = "7894072A-9050-4F0F-901B-34B1A0D29417";
    private static final String COLORF_TYPE_ID = "63782551-A309-463B-A301-3A360800DF1E";
    private static final String COLORB_TYPE_ID = "6F0CC2C0-0CC6-4DBF-9297-B043F270E6A4";
    private static final String AABB_TYPE_ID = "A54C2B36-D5B8-46A1-A529-4EBDBD2450E7";
    private static final String AZSTD_ALLOCATOR_TYPE_NAME = "AZStd::allocator";
    private static final String AZSTD_ALLOCATOR_TYPE_ID = "E9F5A3BE-2B3D-4C62-9E6B-4E00A13AB452";
    private static final String AZSTD_LESS_TYPE_ID = "41B40AFC-68FD-4ED9-9EC7-BA9992802E1B";
    private static final String AZSTD_EQUAL_TO_TYPE_ID = "4377BCED-F78C-4016-80BB-6AFACE6E5137";
    private static final String AZSTD_HASH_TYPE_ID = "EFA74E54-BDFA-47BE-91A7-5A05DA0306D7";
    private static final String AZSTD_CHAR_TRAITS_TYPE_ID = "9B018C0C-022E-4BA4-AE91-2C1E8592DBB2";
    private static final String AZSTD_BASIC_STRING_TYPE_ID = "C26397ED-8F60-4DF6-8320-0D0C592DA3CD";
    private static final String AZSTD_STRING_TYPE_ID = "03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9";
    private static final String AZSTD_PAIR_TYPE_ID = "919645C1-E464-482B-A69B-04AA688B6847";
    private static final String AZSTD_VECTOR_TYPE_ID = "A60E3E61-1FF6-4982-B6B8-9E4350C4C679";
    private static final String AZSTD_LIST_TYPE_ID = "E1E05843-BB02-4F43-B7DC-3ADB28DF42AC";
    private static final String AZSTD_FORWARD_LIST_TYPE_ID = "D7E91EA3-326F-4019-87F0-6F45924B909A";
    private static final String AZSTD_SET_TYPE_ID = "6C51837F-B0C9-40A3-8D52-2143341EDB07";
    private static final String AZSTD_UNORDERED_SET_TYPE_ID = "8D60408E-DA65-4670-99A2-8ABB574625AE";
    private static final String AZSTD_MAP_TYPE_ID = "F8ECF58D-D33E-49DC-BF34-8FA499AC3AE1";
    private static final String AZSTD_UNORDERED_MAP_TYPE_ID = "41171F6F-9E5E-4227-8420-289F1DD5D005";
    private static final String AZSTD_SHARED_PTR_TYPE_ID = "FE61C84E-149D-43FD-88BA-1C3DB7E548B4";
    private static final String AZSTD_INTRUSIVE_PTR_TYPE_ID = "530F8502-309E-4EE1-9AEF-5C0456B1F502";
    private static final String AZSTD_UNIQUE_PTR_TYPE_ID = "B55F90DA-C21E-4EB4-9857-87BE6529BA6D";
    private static final String AZSTD_OPTIONAL_TYPE_ID = "AB8C50C0-23A7-4333-81CD-46F648938B1C";
    private static final String AZSTD_FIXED_VECTOR_TYPE_ID = "74044B6F-E922-4FD7-915D-EFC5D1DC59AE";
    private static final String AZSTD_ARRAY_TYPE_ID = "911B2EA8-CCB1-4F0C-A535-540AD00173AE";
    private static final String AZSTD_BITSET_TYPE_ID = "6BAE9836-EC49-466A-85F2-F4B1B70839FB";
    private static final String AZSTD_TUPLE_TYPE_ID = "F99F9308-DC3E-4384-9341-89CBF1ABD51E";
    private static final String AZSTD_UNORDERED_FLAT_MAP_TYPE_ID = "AA6CB2BA-A6FA-43A3-B08C-4B6E0D751068";
    private static final String AZ_DATA_ASSET_TYPE_ID = "C891BF19-B60C-45E2-BFD0-027D15DDC939";
    private static final String AZ_DATA_ASSET_ID_TYPE_ID = "652ED536-3402-439B-AEBE-4A5DBC554085";
    private static final String AZ_INTERNAL_RVALUE_TO_LVALUE_WRAPPER_TYPE_ID =
        "2590807F-5748-4CD0-A475-83EF5FD216CF";
    private static final String MB_REPLICATED_FIELD_TYPE_ID = "5C059EC7-44B0-4666-9FC9-674192338F39";
    private static final String AMAZON_PERVASIVES_UID_TYPE_ID = "DFE50973-EA0B-4616-833A-B60B5E2E71DF";

    private final Gson gson = new GsonBuilder()
        .disableHtmlEscaping()
        .setPrettyPrinting()
        .create();

    private final Map<String, Address> pointerReadCache = new HashMap<>();
    private final Map<String, List<Address>> asciiStringSearchCache = new HashMap<>();
    private final Map<String, Address> fieldHandlerConstructorVtableCache = new HashMap<>();
    private final Map<String, List<VtableWrite>> constructorVtableWritesCache = new HashMap<>();
    private final Map<String, Function> functionLookupCache = new HashMap<>();
    private final Map<String, Function> functionByFullNameCache = new HashMap<>();
    private final Map<String, String> functionNameCache = new HashMap<>();
    private final Map<String, List<Instruction>> functionInstructionsCache = new HashMap<>();
    private final Map<String, Boolean> allocatorReturnFunctionCache = new HashMap<>();
    private final Map<String, ForwardArgState> inheritedForwardStateCache = new HashMap<>();
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
    private final Map<String, HighFunction> highFunctionCache = new HashMap<>();
    private final Map<String, Integer> returnedCallInputSlotCache = new HashMap<>();
    private final Map<String, FieldHandlerShape> fieldHandlerShapeCache = new HashMap<>();
    private final Map<String, NestedTypeShape> nestedTypeShapeCache = new HashMap<>();
    private final Map<String, List<Structure>> nestedTypeStructureCache = new HashMap<>();
    private final Map<String, List<SerializeTypeInfo>> serializeTypesByName = new HashMap<>();
    private final Map<String, List<SerializeTypeInfo>> serializeTypesByLeafName = new HashMap<>();
    private final Map<String, SerializeTypeInfo> serializeTypesById = new HashMap<>();
    private final LinkedHashSet<String> registryTypeIds = new LinkedHashSet<>();
    private final Map<String, Integer> typeIdSourceCounts = new LinkedHashMap<>();
    private final Map<String, Integer> nativeUuidRejectCounts = new LinkedHashMap<>();
    private String cacheProgramKey;
    private DecompInterface decompiler;
    private boolean functionByFullNameCacheLoaded;
    private int pcodeMessageFieldCandidateCount;
    private int pcodeMessageFieldAcceptedCount;
    private int pcodeMessageFieldRejectedCount;
    private final Map<String, Integer> pcodeMessageFieldRejectCounts = new LinkedHashMap<>();
    private int nestedTypeShapesRecovered;
    private int nestedTypeShapeFailures;
    private final Map<String, Integer> nestedTypeShapeRejectCounts = new LinkedHashMap<>();
    private int recoveredFunctionCount;
    private int recoveredFunctionFailureCount;
    private int queuedRegistrationReferenceCount;
    private int queuedRegistrationDecodedCount;
    private int queuedRegistrationNoFunctionCount;
    private int queuedRegistrationNoHelperCount;
    private int queuedRegistrationNoTypeIdCount;
    private int directRegisterTypeReferenceCount;
    private int directRegisterTypeDecodedCount;
    private int directRegisterTypeNoFunctionCount;
    private int directRegisterTypeNoTypeIdCount;
    private JsonArray queuedRegistrationFailureSamples = new JsonArray();
    private JsonArray directRegisterTypeFailureSamples = new JsonArray();

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
        loadSerializeTypeIndex(input);

        List<RegistryEntry> registry = parseRegistry(root);
        Map<String, HookTypeEvidence> hookTypeNamesById =
            collectRegistrationHookTypeNames(registry);
        Map<String, RegistrationFunction> registrationFunctions =
            collectRegistrationFunctions(registerField);
        collectDirectFieldTableRegistrationFunctions(
            registry,
            hookTypeNamesById,
            registrationFunctions);

        JsonArray registryJson = new JsonArray();
        int mappedRegistryEntries = 0;
        int mappedFieldCount = 0;
        int mappedMessageEntries = 0;
        int mappedMessageFields = 0;
        int registryEntriesWithAzRtti = 0;
        int registryEntriesWithValueAzRtti = 0;
        int registryEntriesWithRegistrationTypeName = 0;
        int registryEntriesWithRecoveredTypeName = 0;
        int baseOrInterfaceRegistryEntries = 0;
        JsonArray identityFallbackJson = new JsonArray();
        JsonArray identityBlockerJson = new JsonArray();
        LinkedHashMap<String, Integer> identityBlockerCounts = new LinkedHashMap<>();
        for (RegistryEntry entry : registry) {
            List<RegistrationFunction> matches =
                constructorMatches(entry, registrationFunctions);
            AzRttiEvidence resolvedRtti =
                resolvedAzRtti(entry, matches);
            HookTypeEvidence hookType = registrationHookForEntry(entry, hookTypeNamesById);
            TypeNameEvidence recoveredTypeName =
                resolvedTypeName(entry, resolvedRtti, hookType, matches);
            AzRttiEvidence valueRtti =
                resolvedValueAzRtti(entry, resolvedRtti, hookType, recoveredTypeName);
            if (resolvedRtti != null) {
                registryEntriesWithAzRtti++;
            }
            if (valueRtti != null) {
                registryEntriesWithValueAzRtti++;
            }
            if (hookType != null && isPlausibleTypeName(hookType.typeName)) {
                registryEntriesWithRegistrationTypeName++;
            }
            if (recoveredTypeName != null &&
                isPlausibleTypeName(recoveredTypeName.typeName)) {
                registryEntriesWithRecoveredTypeName++;
            }
            if (isBaseOrInterfaceRegistryEntry(entry, resolvedRtti)) {
                baseOrInterfaceRegistryEntries++;
            }
            if (isRegistryTypeNameFallback(hookType)) {
                identityFallbackJson.add(identityFallbackJson(
                    entry,
                    resolvedRtti,
                    hookType,
                    recoveredTypeName,
                    matches));
            }
            String identityBlockerReason =
                identityBlockerReason(
                    entry,
                    resolvedRtti,
                    valueRtti,
                    hookType,
                    recoveredTypeName,
                    matches);
            if (identityBlockerReason != null) {
                incrementCount(identityBlockerCounts, identityBlockerReason);
                identityBlockerJson.add(identityBlockerJson(
                    entry,
                    resolvedRtti,
                    valueRtti,
                    hookType,
                    recoveredTypeName,
                    matches,
                    identityBlockerReason));
            }
            JsonObject row = entry.toJson(resolvedRtti, valueRtti, hookType, recoveredTypeName);
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
        summary.addProperty("pcodeMessageFieldCandidates", pcodeMessageFieldCandidateCount);
        summary.addProperty("pcodeMessageFieldsAccepted", pcodeMessageFieldAcceptedCount);
        summary.addProperty("pcodeMessageFieldsRejected", pcodeMessageFieldRejectedCount);
        summary.add("pcodeMessageFieldRejectSummary", countMapJson(pcodeMessageFieldRejectCounts));
        summary.addProperty("nestedTypeShapesRecovered", nestedTypeShapesRecovered);
        summary.addProperty("nestedTypeShapeFailures", nestedTypeShapeFailures);
        summary.add("nestedTypeShapeRejectSummary", countMapJson(nestedTypeShapeRejectCounts));
        summary.addProperty("recoveredFunctions", recoveredFunctionCount);
        summary.addProperty("recoveredFunctionFailures", recoveredFunctionFailureCount);
        summary.addProperty("registryEntriesWithAzRtti", registryEntriesWithAzRtti);
        summary.addProperty("registryEntriesWithValueAzRtti", registryEntriesWithValueAzRtti);
        summary.addProperty(
            "registryEntriesWithRegistrationTypeName",
            registryEntriesWithRegistrationTypeName);
        summary.addProperty(
            "registryEntriesWithRecoveredTypeName",
            registryEntriesWithRecoveredTypeName);
        summary.addProperty(
            "baseOrInterfaceRegistryEntries",
            baseOrInterfaceRegistryEntries);
        summary.addProperty("registryTypeNameFallbacks", identityFallbackJson.size());
        summary.addProperty("identityBlockers", identityBlockerJson.size());
        report.add("summary", summary);

        JsonObject identityDiagnostics = new JsonObject();
        identityDiagnostics.add("registryTypeNameFallbacks", identityFallbackJson);
        identityDiagnostics.add("identityBlockers", identityBlockerJson);
        identityDiagnostics.add("identityBlockerSummary", countMapJson(identityBlockerCounts));
        report.add("identityDiagnostics", identityDiagnostics);

        JsonObject registrationEvidenceDiagnostics = new JsonObject();
        registrationEvidenceDiagnostics.addProperty(
            "queuedReferenceCount",
            queuedRegistrationReferenceCount);
        registrationEvidenceDiagnostics.addProperty(
            "queuedDecodedCount",
            queuedRegistrationDecodedCount);
        registrationEvidenceDiagnostics.addProperty(
            "queuedNoFunctionCount",
            queuedRegistrationNoFunctionCount);
        registrationEvidenceDiagnostics.addProperty(
            "queuedNoHelperCount",
            queuedRegistrationNoHelperCount);
        registrationEvidenceDiagnostics.addProperty(
            "queuedNoTypeIdCount",
            queuedRegistrationNoTypeIdCount);
        registrationEvidenceDiagnostics.add("queuedFailureSamples", queuedRegistrationFailureSamples);
        registrationEvidenceDiagnostics.addProperty(
            "directReferenceCount",
            directRegisterTypeReferenceCount);
        registrationEvidenceDiagnostics.addProperty(
            "directDecodedCount",
            directRegisterTypeDecodedCount);
        registrationEvidenceDiagnostics.addProperty(
            "directNoFunctionCount",
            directRegisterTypeNoFunctionCount);
        registrationEvidenceDiagnostics.addProperty(
            "directNoTypeIdCount",
            directRegisterTypeNoTypeIdCount);
        registrationEvidenceDiagnostics.add(
            "directFailureSamples",
            directRegisterTypeFailureSamples);
        report.add("registrationEvidenceDiagnostics", registrationEvidenceDiagnostics);

        JsonObject registrationInvariants =
            registrationInvariants(hookTypeNamesById, registry);
        report.add("registrationInvariants", registrationInvariants);

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
        println("Registration invariants: hook UUIDs not in registry=" +
            registrationInvariants.get("hookUuidsNotInRegistryCount").getAsInt() +
            ", zero UUIDs=" +
            registrationInvariants.get("zeroUuidDecodedCount").getAsInt() +
            ", name mismatches=" +
            registrationInvariants.get("decodedButNameMismatchCount").getAsInt() +
            ", native rejects=" + countMapJson(nativeUuidRejectCounts));
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
        constructorVtableWritesCache.clear();
        functionLookupCache.clear();
        functionByFullNameCache.clear();
        functionNameCache.clear();
        functionInstructionsCache.clear();
        allocatorReturnFunctionCache.clear();
        inheritedForwardStateCache.clear();
        decompileCache.clear();
        createInstanceSizeCache.clear();
        parameterNameCache.clear();
        boolParameterIndicesCache.clear();
        nestedBoolParameterIndicesCache.clear();
        unmarshalCallCache.clear();
        marshalerUnmarshalCallCache.clear();
        directTypeUnmarshalCallCache.clear();
        readRawCallCache.clear();
        highFunctionCache.clear();
        returnedCallInputSlotCache.clear();
        fieldHandlerShapeCache.clear();
        nestedTypeShapeCache.clear();
        serializeTypesByName.clear();
        serializeTypesByLeafName.clear();
        serializeTypesById.clear();
        registryTypeIds.clear();
        typeIdSourceCounts.clear();
        nativeUuidRejectCounts.clear();
        functionByFullNameCacheLoaded = false;
        pcodeMessageFieldCandidateCount = 0;
        pcodeMessageFieldAcceptedCount = 0;
        pcodeMessageFieldRejectedCount = 0;
        pcodeMessageFieldRejectCounts.clear();
        nestedTypeShapesRecovered = 0;
        nestedTypeShapeFailures = 0;
        nestedTypeShapeRejectCounts.clear();
        recoveredFunctionCount = 0;
        recoveredFunctionFailureCount = 0;
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

    private void loadSerializeTypeIndex(File typeregistryInput) {
        File serialize = serializeJsonFile(typeregistryInput);
        if (serialize == null || !serialize.isFile()) {
            println("Serialize type index not loaded; set NW_NETWORK_SCHEMA_SERIALIZE_JSON for exact field type-id joins.");
            return;
        }

        int loaded = 0;
        try (Reader reader = new FileReader(serialize)) {
            JsonObject root = JsonParser.parseReader(reader).getAsJsonObject();
            JsonObject uuidMap = object(root, "uuidMap");
            if (uuidMap == null) {
                println("Serialize type index has no uuidMap: " + serialize);
                return;
            }
            for (Map.Entry<String, JsonElement> entry : uuidMap.entrySet()) {
                if (!entry.getValue().isJsonObject()) {
                    continue;
                }
                JsonObject value = entry.getValue().getAsJsonObject();
                String typeId = canonicalUuidFromString(
                    firstNonEmpty(string(value, "typeId"), entry.getKey()));
                String name = string(value, "name");
                if (typeId == null || name == null || name.trim().isEmpty()) {
                    continue;
                }
                SerializeTypeInfo info = new SerializeTypeInfo();
                info.typeId = typeId;
                info.name = name.trim();
                info.factory = string(value, "factory");
                JsonObject azRtti = object(value, "azRtti");
                info.azRttiAddress = azRtti == null ? null : string(azRtti, "address");

                serializeTypesById.put(normalizeUuid(typeId), info);
                serializeTypesByName.computeIfAbsent(info.name, ignored -> new ArrayList<>()).add(info);
                serializeTypesByLeafName
                    .computeIfAbsent(sourceTypeLeaf(info.name), ignored -> new ArrayList<>())
                    .add(info);
                loaded++;
            }
            println("Loaded serialize type index: " + loaded + " types from " + serialize);
        }
        catch (Exception exception) {
            println("Serialize type index load failed: " + exception.getMessage());
        }
    }

    private File serializeJsonFile(File typeregistryInput) {
        String explicit = envValue("NW_NETWORK_SCHEMA_SERIALIZE_JSON");
        if (explicit != null) {
            return new File(explicit);
        }
        File dir = typeregistryInput == null ? null : typeregistryInput.getParentFile();
        for (int i = 0; i < 4 && dir != null; i++) {
            File candidate = new File(dir, "serialize.json");
            if (candidate.isFile()) {
                return candidate;
            }
            dir = dir.getParentFile();
        }
        return null;
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

    private void collectDirectFieldTableRegistrationFunctions(
        List<RegistryEntry> registry,
        Map<String, HookTypeEvidence> hookTypeNamesById,
        Map<String, RegistrationFunction> registrationFunctions) {

        LinkedHashSet<Address> candidates = new LinkedHashSet<>();
        for (RegistryEntry entry : registry) {
            HookTypeEvidence hook = registrationHookForEntry(entry, hookTypeNamesById);
            collectConstructorCandidates(candidates, parseCapturedAddress(entry.createInstance));
            collectConstructorCandidates(candidates, parseCapturedAddress(entry.unmarshal));
            if (hook != null) {
                collectConstructorCandidates(candidates, hook.createInstance);
                collectConstructorCandidates(candidates, hook.unmarshal);
            }
        }

        for (Address candidate : candidates) {
            Function function = functionAtOrContaining(candidate);
            if (function == null) {
                continue;
            }
            String key = function.getEntryPoint().toString();
            if (registrationFunctions.containsKey(key)) {
                continue;
            }

            List<FieldCall> fields = recoverDirectFieldTableAppends(function);
            if (fields.isEmpty()) {
                continue;
            }

            RegistrationFunction registration = new RegistrationFunction(function);
            registration.instanceVtable = findInstanceVtable(function);
            registration.azRtti = decodeAzRttiFromVtable(registration.instanceVtable);
            registration.fields.addAll(fields);
            registrationFunctions.put(key, registration);
        }
    }

    private void collectConstructorCandidates(Set<Address> candidates, Address seed) {
        Address target = resolvedCodeTarget(seed);
        if (!isExecutableAddress(target)) {
            return;
        }

        Function function = functionAtOrContaining(target);
        if (function == null) {
            return;
        }
        candidates.add(function.getEntryPoint());

        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (instruction.getMinAddress().compareTo(target) < 0) {
                continue;
            }
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address call = resolvedCodeTarget(callTarget(instruction));
            if (isExecutableAddress(call)) {
                candidates.add(call);
            }
        }
    }

    private List<FieldCall> recoverDirectFieldTableAppends(Function owner) {
        ForwardArgState state = newForwardArgState();

        HashMap<String, Integer> cursorOffsetByRegister = new HashMap<>();
        HashMap<Integer, DirectFieldAppend> pendingByCursorOffset = new HashMap<>();
        HashMap<String, FixedNamedFieldValue> fixedNamedFieldsBySimdRegister = new HashMap<>();
        ArrayList<FieldCall> fields = new ArrayList<>();

        int count = 0;
        for (Instruction instruction : functionInstructions(owner)) {
            if (count++ >= DIRECT_FIELD_TABLE_SCAN_LIMIT) {
                break;
            }

            observeDirectFieldTableAppend(
                instruction,
                state,
                cursorOffsetByRegister,
                pendingByCursorOffset,
                fixedNamedFieldsBySimdRegister,
                fields);
            observeForwardInstruction(instruction, state);
        }

        if (fields.isEmpty()) {
            return fields;
        }

        LinkedHashMap<Integer, Integer> groupByCursorOffset = new LinkedHashMap<>();
        ArrayList<Integer> cursorOffsets = new ArrayList<>();
        for (FieldCall field : fields) {
            if (isDirectAttributeRegistration(field)) {
                continue;
            }
            if (field.groupCursorOffset != null &&
                !cursorOffsets.contains(field.groupCursorOffset)) {
                cursorOffsets.add(field.groupCursorOffset);
            }
        }
        Collections.sort(cursorOffsets);
        for (int i = 0; i < cursorOffsets.size(); i++) {
            groupByCursorOffset.put(cursorOffsets.get(i), i);
        }

        for (int i = 0; i < fields.size(); i++) {
            FieldCall field = fields.get(i);
            field.index = i;
            if (isDirectAttributeRegistration(field)) {
                field.group = null;
            }
            else if (field.group == null && field.groupCursorOffset != null) {
                field.group = groupByCursorOffset.get(field.groupCursorOffset);
            }
        }
        return fields;
    }

    private boolean isDirectAttributeRegistration(FieldCall field) {
        return field != null && "attribute".equals(field.registrationKind);
    }

    private void observeDirectFieldTableAppend(
        Instruction instruction,
        ForwardArgState state,
        Map<String, Integer> cursorOffsetByRegister,
        Map<Integer, DirectFieldAppend> pendingByCursorOffset,
        Map<String, FixedNamedFieldValue> fixedNamedFieldsBySimdRegister,
        List<FieldCall> fields) {

        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return;
        }

        observeStackStagedFixedFieldAppend(
            instruction,
            state,
            cursorOffsetByRegister,
            fixedNamedFieldsBySimdRegister,
            fields);

        if (instruction.getFlowType().isCall()) {
            clearVolatileCursorRegisters(cursorOffsetByRegister);
            fixedNamedFieldsBySimdRegister.clear();
            return;
        }

        String destination = registerOperand(instruction, 0);
        if ("MOV".equals(mnemonic) && destination != null) {
            Integer cursorOffset = directFieldCursorOffsetFromMoveSource(
                instruction,
                state,
                cursorOffsetByRegister);
            if (cursorOffset != null) {
                cursorOffsetByRegister.put(destination, cursorOffset);
            }
            else {
                cursorOffsetByRegister.remove(destination);
            }
        }
        else if (destination != null && "LEA".equals(mnemonic)) {
            TrackedValue value = trackedLeaValue(instruction, state);
            if (value != null && value.thisOffset != null) {
                cursorOffsetByRegister.put(destination, value.thisOffset);
            }
            else {
                cursorOffsetByRegister.remove(destination);
            }
        }
        else if (destination != null && ("ADD".equals(mnemonic) ||
            "XOR".equals(mnemonic) ||
            "SUB".equals(mnemonic))) {
            cursorOffsetByRegister.remove(destination);
        }

        MemoryReference destinationMemory = memoryReference(instruction, 0);
        if (!"MOV".equals(mnemonic) || destinationMemory == null ||
            destinationMemory.baseRegister == null) {
            return;
        }

        Integer cursorOffset = cursorOffsetByRegister.get(destinationMemory.baseRegister);
        if (cursorOffset == null) {
            return;
        }

        DirectFieldAppend pending = pendingByCursorOffset.computeIfAbsent(
            cursorOffset,
            ignored -> new DirectFieldAppend(cursorOffset));
        if (destinationMemory.displacement == 0) {
            TrackedValue value = trackedOperandValue(instruction, 1, state);
            Address address = value == null ? null : value.address;
            StringDecode decoded = readFieldNameAtOrThroughPointer(address);
            if (decoded != null) {
                pending.nameAddress = decoded.address;
                pending.name = decoded.value;
                pending.nameWrite = instruction.getMinAddress();
            }
        }
        else if (destinationMemory.displacement == 8) {
            TrackedValue value = trackedOperandValue(instruction, 1, state);
            if (value != null && value.thisOffset != null) {
                pending.handlerOffset = value.thisOffset;
                pending.handlerExpression = value.expression;
                pending.handlerWrite = instruction.getMinAddress();
            }
        }
        else if (destinationMemory.displacement == 0x10) {
            TrackedValue value = trackedOperandValue(instruction, 1, state);
            if (value != null && value.immediate != null) {
                pending.filterGroupAttribute = value.immediate != 0;
            }
        }

        FieldCall field = pending.toFieldCall(state);
        if (field != null) {
            fields.add(field);
            pendingByCursorOffset.remove(cursorOffset);
        }
    }

    private void observeStackStagedFixedFieldAppend(
        Instruction instruction,
        ForwardArgState state,
        Map<String, Integer> cursorOffsetByRegister,
        Map<String, FixedNamedFieldValue> fixedNamedFieldsBySimdRegister,
        List<FieldCall> fields) {

        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return;
        }

        if ("MOV".equals(mnemonic)) {
            MemoryReference destinationMemory = memoryReference(instruction, 0);
            Integer widthBits = memoryWriteWidthBits(instruction);
            if (destinationMemory != null && isStackRegister(destinationMemory.baseRegister) &&
                widthBits != null && widthBits == 64) {
                TrackedValue value = trackedOperandValue(instruction, 1, state);
                putOrRemove(state.valuesByStackSlot, stackSlotOffset(destinationMemory, state), value);
                return;
            }
        }

        String destinationRegister = registerOperand(instruction, 0);
        if (!isPackedMoveMnemonic(mnemonic)) {
            if (destinationRegister != null && destinationRegister.startsWith("XMM")) {
                fixedNamedFieldsBySimdRegister.remove(destinationRegister);
            }
            return;
        }

        if (destinationRegister != null && destinationRegister.startsWith("XMM")) {
            MemoryReference sourceMemory = memoryReference(instruction, 1);
            if (sourceMemory == null || !isStackRegister(sourceMemory.baseRegister)) {
                fixedNamedFieldsBySimdRegister.remove(destinationRegister);
                return;
            }

            FixedNamedFieldValue field =
                fixedNamedFieldFromStack(sourceMemory, state, instruction.getMinAddress());
            if (field == null) {
                fixedNamedFieldsBySimdRegister.remove(destinationRegister);
            }
            else {
                fixedNamedFieldsBySimdRegister.put(destinationRegister, field);
            }
            return;
        }

        MemoryReference destinationMemory = memoryReference(instruction, 0);
        String sourceRegister = registerOperand(instruction, 1);
        if (destinationMemory == null || sourceRegister == null ||
            !sourceRegister.startsWith("XMM") ||
            destinationMemory.baseRegister == null ||
            destinationMemory.displacement != 0) {
            return;
        }

        Integer cursorOffset = cursorOffsetByRegister.get(destinationMemory.baseRegister);
        if (cursorOffset == null) {
            return;
        }

        FixedNamedFieldValue value = fixedNamedFieldsBySimdRegister.get(sourceRegister);
        if (value == null) {
            return;
        }

        DirectFieldAppend append = new DirectFieldAppend(cursorOffset);
        append.nameAddress = value.nameAddress;
        append.name = value.name;
        append.nameWrite = value.nameWrite;
        append.handlerOffset = value.handlerOffset;
        append.handlerExpression = value.handlerExpression;
        append.handlerWrite = instruction.getMinAddress();

        FieldCall field = append.toFieldCall(state);
        if (field != null) {
            fields.add(field);
        }
    }

    private FixedNamedFieldValue fixedNamedFieldFromStack(
        MemoryReference sourceMemory,
        ForwardArgState state,
        Address readAddress) {

        Integer nameSlot = stackSlotOffset(sourceMemory, state);
        Integer handlerSlot = nameSlot == null ? null : nameSlot + 8;
        TrackedValue nameValue = nameSlot == null ? null : state.valuesByStackSlot.get(nameSlot);
        TrackedValue handlerValue = handlerSlot == null ? null : state.valuesByStackSlot.get(handlerSlot);
        if (nameValue == null || handlerValue == null ||
            nameValue.address == null || handlerValue.thisOffset == null) {
            return null;
        }

        StringDecode decoded = readFieldNameAtOrThroughPointer(nameValue.address);
        if (decoded == null) {
            return null;
        }

        FixedNamedFieldValue field = new FixedNamedFieldValue();
        field.nameAddress = decoded.address;
        field.name = decoded.value;
        field.nameWrite = readAddress;
        field.handlerOffset = handlerValue.thisOffset;
        field.handlerExpression = handlerValue.expression;
        return field;
    }

    private boolean isPackedMoveMnemonic(String mnemonic) {
        return "MOVUPS".equals(mnemonic) ||
            "MOVAPS".equals(mnemonic) ||
            "MOVUPD".equals(mnemonic) ||
            "MOVAPD".equals(mnemonic) ||
            "MOVDQA".equals(mnemonic) ||
            "MOVDQU".equals(mnemonic) ||
            "MOVDQA32".equals(mnemonic) ||
            "MOVDQA64".equals(mnemonic) ||
            "MOVDQU32".equals(mnemonic) ||
            "MOVDQU64".equals(mnemonic) ||
            "VMOVUPS".equals(mnemonic) ||
            "VMOVAPS".equals(mnemonic) ||
            "VMOVUPD".equals(mnemonic) ||
            "VMOVAPD".equals(mnemonic) ||
            "VMOVDQA".equals(mnemonic) ||
            "VMOVDQU".equals(mnemonic) ||
            "VMOVDQA32".equals(mnemonic) ||
            "VMOVDQA64".equals(mnemonic) ||
            "VMOVDQU32".equals(mnemonic) ||
            "VMOVDQU64".equals(mnemonic);
    }

    private boolean isMemoryWriteMnemonic(String mnemonic) {
        return "MOV".equals(mnemonic) ||
            "MOVD".equals(mnemonic) ||
            "MOVQ".equals(mnemonic) ||
            "MOVSS".equals(mnemonic) ||
            "MOVSD".equals(mnemonic) ||
            "VMOVD".equals(mnemonic) ||
            "VMOVQ".equals(mnemonic) ||
            "VMOVSS".equals(mnemonic) ||
            "VMOVSD".equals(mnemonic) ||
            isPackedMoveMnemonic(mnemonic);
    }

    private boolean writesRegister(String mnemonic) {
        if (mnemonic == null) {
            return false;
        }
        if (mnemonic.startsWith("SET") || mnemonic.startsWith("CMOV")) {
            return true;
        }
        return "AND".equals(mnemonic) ||
            "OR".equals(mnemonic) ||
            "XOR".equals(mnemonic) ||
            "IMUL".equals(mnemonic) ||
            "MUL".equals(mnemonic) ||
            "IDIV".equals(mnemonic) ||
            "DIV".equals(mnemonic) ||
            "SHL".equals(mnemonic) ||
            "SHR".equals(mnemonic) ||
            "SAR".equals(mnemonic) ||
            "SAL".equals(mnemonic) ||
            "NEG".equals(mnemonic) ||
            "NOT".equals(mnemonic) ||
            "INC".equals(mnemonic) ||
            "DEC".equals(mnemonic) ||
            "MOVZX".equals(mnemonic) ||
            "MOVSX".equals(mnemonic) ||
            "MOVSXD".equals(mnemonic);
    }

    private boolean isStackRegister(String register) {
        return "RBP".equals(register) || "RSP".equals(register);
    }

    private Integer directFieldCursorOffsetFromMoveSource(
        Instruction instruction,
        ForwardArgState state,
        Map<String, Integer> cursorOffsetByRegister) {

        String sourceRegister = registerOperand(instruction, 1);
        if (sourceRegister != null && cursorOffsetByRegister.containsKey(sourceRegister)) {
            return cursorOffsetByRegister.get(sourceRegister);
        }

        MemoryReference sourceMemory = memoryReference(instruction, 1);
        if (sourceMemory != null) {
            Integer cursorOffset = cursorOffsetByRegister.get(sourceMemory.baseRegister);
            if (cursorOffset != null &&
                sourceMemory.displacement == VECTOR_CURRENT_POINTER_OFFSET) {
                return cursorOffset;
            }

            TrackedValue base = state.registers.get(sourceMemory.baseRegister);
            if (base != null && base.thisOffset != null &&
                sourceMemory.displacement == VECTOR_CURRENT_POINTER_OFFSET) {
                return base.thisOffset;
            }
        }

        return trackedThisOffsetForMemoryOperand(instruction, 1, state.registers);
    }

    private void clearVolatileCursorRegisters(Map<String, Integer> cursorOffsetByRegister) {
        cursorOffsetByRegister.remove("RAX");
        cursorOffsetByRegister.remove("RCX");
        cursorOffsetByRegister.remove("RDX");
        cursorOffsetByRegister.remove("R8");
        cursorOffsetByRegister.remove("R9");
        cursorOffsetByRegister.remove("R10");
        cursorOffsetByRegister.remove("R11");
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
                enrichAzRttiWithConstructorName(match.azRtti, match);
                return match.azRtti;
            }
        }

        return decodeAzRttiFromRegistryHandler(entry, registryTypeId, true);
    }

    private AzRttiEvidence resolvedValueAzRtti(
        RegistryEntry entry,
        AzRttiEvidence identityRtti,
        HookTypeEvidence hookType,
        TypeNameEvidence recoveredTypeName) {

        String registryTypeId = canonicalUuidFromString(entry.uuid);
        String expectedTypeName =
            expectedRuntimeTypeName(entry, identityRtti, hookType, recoveredTypeName);
        AzRttiEvidence valueRtti =
            decodeValueAzRttiFromUnmarshal(entry, registryTypeId, expectedTypeName);
        if (valueRtti == null || valueRtti.typeId == null) {
            return null;
        }
        if (identityRtti != null && uuidEquals(identityRtti.typeId, valueRtti.typeId)) {
            return null;
        }
        if (registryTypeId != null && uuidEquals(registryTypeId, valueRtti.typeId)) {
            return null;
        }
        return valueRtti;
    }

    private String expectedRuntimeTypeName(
        RegistryEntry entry,
        AzRttiEvidence identityRtti,
        HookTypeEvidence hookType,
        TypeNameEvidence recoveredTypeName) {

        if (identityRtti != null && isLikelyRuntimeTypeName(identityRtti.typeName)) {
            return identityRtti.typeName;
        }
        if (hookType != null && isLikelyRuntimeTypeName(hookType.typeName)) {
            return hookType.typeName;
        }
        if (recoveredTypeName != null && isLikelyRuntimeTypeName(recoveredTypeName.typeName)) {
            return recoveredTypeName.typeName;
        }
        return isLikelyRuntimeTypeName(entry.name) ? entry.name : null;
    }

    private AzRttiEvidence decodeValueAzRttiFromUnmarshal(
        RegistryEntry entry,
        String registryTypeId,
        String expectedTypeName) {

        Address unmarshal = parseCapturedAddress(entry.unmarshal);
        if (!isExecutableAddress(unmarshal)) {
            return null;
        }

        AzRttiEvidence best =
            decodeValueAzRttiFromVptrWrites(
                unmarshal,
                registryTypeId,
                expectedTypeName,
                "unmarshal-vptr-write-value",
                0);
        int bestScore = best == null
            ? Integer.MIN_VALUE
            : valueAzRttiEvidenceScore(best, registryTypeId, expectedTypeName, 0, 0);

        int callOrder = 0;
        for (Instruction instruction : linearInstructions(unmarshal, REGISTRY_HANDLER_RTTI_SCAN_LIMIT)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }

            Address target = resolvedCodeTarget(callTarget(instruction));
            if (!isExecutableAddress(target)) {
                continue;
            }

            AzRttiEvidence candidate =
                decodeValueAzRttiFromVptrWrites(
                    target,
                    registryTypeId,
                    expectedTypeName,
                    "unmarshal-constructor-vtable-write-value",
                    1);
            if (candidate == null || candidate.typeId == null) {
                callOrder++;
                continue;
            }

            int score =
                valueAzRttiEvidenceScore(candidate, registryTypeId, expectedTypeName, 1, callOrder);
            if (score >= bestScore) {
                best = candidate;
                bestScore = score;
            }
            callOrder++;
        }

        return best;
    }

    private AzRttiEvidence decodeValueAzRttiFromVptrWrites(
        Address functionAddress,
        String registryTypeId,
        String expectedTypeName,
        String source,
        int depth) {

        AzRttiEvidence best = null;
        int bestScore = Integer.MIN_VALUE;
        ArrayList<JsonObject> vptrWriteObjects = new ArrayList<>();
        for (VtableWrite write : handlerVtableWrites(functionAddress)) {
            AzRttiEvidence evidence = decodeAzRttiFromVtable(write.vtable);
            if (evidence == null || evidence.typeId == null) {
                continue;
            }
            vptrWriteObjects.add(vptrWriteJson(write, evidence, expectedTypeName));
            if (expectedTypeName != null &&
                isLikelyRuntimeTypeName(evidence.typeName) &&
                !typeNamesMatch(expectedTypeName, evidence.typeName)) {
                continue;
            }

            int score =
                valueAzRttiEvidenceScore(evidence, registryTypeId, expectedTypeName, depth, write.order);
            if (score >= bestScore) {
                evidence.source = source;
                evidence.sourceInstruction = write.instruction;
                best = evidence;
                bestScore = score;
            }
        }
        JsonArray vptrWrites = constructorVptrChainJson(vptrWriteObjects);
        if (best != null && vptrWrites.size() != 0) {
            best.constructorVptrWrites = vptrWrites;
        }
        return best;
    }

    private JsonArray constructorVptrChainJson(List<JsonObject> writes) {
        JsonArray array = new JsonArray();
        for (int i = 0; i < writes.size(); i++) {
            JsonObject object = writes.get(i);
            object.addProperty(
                "constructorRole",
                i + 1 == writes.size() ? "concrete" : "base-or-intermediate");
            array.add(object);
        }
        return array;
    }

    private JsonObject vptrWriteJson(
        VtableWrite write,
        AzRttiEvidence evidence,
        String expectedTypeName) {

        JsonObject object = new JsonObject();
        object.addProperty("order", write.order);
        add(object, "constructorFunction", formatAddress(write.function));
        add(object, "constructorFunctionName", fullFunctionName(functionAtOrContaining(write.function)));
        add(object, "sourceInstruction", formatAddress(write.instruction));
        add(object, "vtable", formatAddress(write.vtable));
        if (write.thisOffset != null) {
            object.addProperty("thisOffset", "0x" + Integer.toHexString(write.thisOffset));
        }
        add(object, "baseKey", write.baseKey);
        if (write.baseOffset != null) {
            object.addProperty("baseOffset", "0x" + Integer.toHexString(write.baseOffset));
        }
        add(object, "pattern", write.pattern);
        add(object, "typeId", evidence.typeId);
        add(object, "typeName", evidence.typeName);
        if (expectedTypeName != null) {
            object.addProperty("matchesExpectedTypeName",
                typeNamesMatch(expectedTypeName, evidence.typeName));
        }
        if (evidence.providers.size() != 0) {
            object.add("providers", evidence.providers.deepCopy());
        }
        return object;
    }

    private int valueAzRttiEvidenceScore(
        AzRttiEvidence evidence,
        String registryTypeId,
        String expectedTypeName,
        int depth,
        int order) {

        int score = order * 20 - depth * 100;
        if (evidence.typeId != null) {
            score += 40;
        }
        if (isLikelyRuntimeTypeName(evidence.typeName)) {
            score += 100 + runtimeTypeNameScore(evidence.typeName);
        }
        if (expectedTypeName != null && typeNamesMatch(expectedTypeName, evidence.typeName)) {
            score += 10_000;
        }
        if (registryTypeId != null && uuidEquals(registryTypeId, evidence.typeId)) {
            score -= 500;
        }
        return score;
    }

    private void enrichAzRttiWithConstructorName(
        AzRttiEvidence evidence,
        RegistrationFunction match) {

        if (evidence == null) {
            return;
        }

        String typeName = match.constructorTypeName();
        if (!isLikelyRuntimeTypeName(typeName)) {
            return;
        }
        if (isPlausibleTypeName(evidence.typeName)) {
            return;
        }

        TypeNameDecode decode = new TypeNameDecode();
        decode.function = match.function.getEntryPoint();
        decode.provider = match.function.getEntryPoint();
        decode.typeName = typeName;
        decode.typeNameSource = "constructorFunctionName";
        evidence.typeName = typeName;
        evidence.typeNameSource = decode.typeNameSource;
        evidence.providers.add(decode.toJson(-1));
    }

    private AzRttiEvidence decodeAzRttiFromRegistryHandler(
        RegistryEntry entry,
        String registryTypeId,
        boolean identityOnly) {

        return decodeAzRttiFromHandlerAddresses(
            registryHandlerFunctionAddresses(entry),
            registryTypeId,
            identityOnly);
    }

    private AzRttiEvidence decodeAzRttiFromHandlerAddresses(
        List<Address> handlers,
        String registryTypeId,
        boolean identityOnly) {

        AzRttiEvidence bestExact = null;
        int bestExactScore = Integer.MIN_VALUE;
        AzRttiEvidence bestNameOnly = null;
        int bestNameOnlyScore = Integer.MIN_VALUE;
        for (Address handler : handlers) {
            AzRttiScanResult result =
                decodeAzRttiFromHandlerRoot(handler, registryTypeId, identityOnly);
            if (result.exact != null && result.exactScore >= bestExactScore) {
                bestExact = result.exact;
                bestExactScore = result.exactScore;
            }
            if (result.nameOnly != null && result.nameOnlyScore >= bestNameOnlyScore) {
                bestNameOnly = result.nameOnly;
                bestNameOnlyScore = result.nameOnlyScore;
            }
        }

        if (bestExact != null) {
            return bestExact;
        }
        return bestNameOnly;
    }

    private AzRttiScanResult decodeAzRttiFromHandlerRoot(
        Address handler,
        String registryTypeId,
        boolean identityOnly) {

        AzRttiScanResult result = new AzRttiScanResult();
        Deque<HandlerScanFrame> stack = new ArrayDeque<>();
        stack.addFirst(new HandlerScanFrame(handler, 0));

        LinkedHashSet<String> seenFunctions = new LinkedHashSet<>();
        int bestNameOnlyScore = Integer.MIN_VALUE;

        while (!stack.isEmpty() && seenFunctions.size() < REGISTRY_HANDLER_RTTI_FUNCTION_LIMIT) {
            HandlerScanFrame frame = stack.removeFirst();
            Address functionAddress = resolvedCodeTarget(frame.address);
            if (!isExecutableAddress(functionAddress) ||
                !seenFunctions.add(functionAddress.toString())) {
                continue;
            }

            for (VtableWrite write : handlerVtableWrites(functionAddress)) {
                AzRttiEvidence evidence = decodeAzRttiFromVtable(write.vtable);
                if (evidence == null || !evidence.hasIdentity()) {
                    continue;
                }
                boolean matchesRegistryTypeId =
                    registryTypeId != null && evidenceHasTypeId(evidence, registryTypeId);
                if (identityOnly && registryTypeId != null &&
                    !matchesRegistryTypeId) {
                    continue;
                }
                if (identityOnly && matchesRegistryTypeId) {
                    evidence.typeId = registryTypeId;
                    String foldedTypeName = providerTypeNameForTypeId(evidence, registryTypeId);
                    if (isConcreteNetworkTypeName(foldedTypeName)) {
                        evidence.typeName = foldedTypeName;
                        evidence.typeNameSource = "rtti-provider-graph";
                    }
                    else {
                        evidence.typeName = null;
                        evidence.typeNameSource = null;
                    }
                }
                if (evidence.typeId != null) {
                    int score = azRttiEvidenceScore(
                        evidence,
                        write,
                        registryTypeId,
                        frame.depth);
                    if (score >= result.exactScore) {
                        evidence.source =
                            azRttiSourceForWrite(identityOnly, false, write);
                        evidence.sourceInstruction = write.instruction;
                        evidence.constructorVptrWrites =
                            singleVptrWriteChain(write, evidence, null);
                        result.exact = evidence;
                        result.exactScore = score;
                    }
                    continue;
                }

                if (isLikelyRuntimeTypeName(evidence.typeName)) {
                    int score = azRttiEvidenceScore(
                        evidence,
                        write,
                        registryTypeId,
                        frame.depth);
                    if (score >= bestNameOnlyScore) {
                        evidence.source =
                            azRttiSourceForWrite(identityOnly, true, write);
                        evidence.sourceInstruction = write.instruction;
                        evidence.constructorVptrWrites =
                            singleVptrWriteChain(write, evidence, null);
                        result.nameOnly = evidence;
                        result.nameOnlyScore = score;
                        bestNameOnlyScore = score;
                    }
                }
            }

            if (frame.depth >= REGISTRY_HANDLER_RTTI_CALL_DEPTH) {
                continue;
            }

            ArrayList<Address> callees = new ArrayList<>();
            for (Instruction instruction :
                linearInstructions(functionAddress, REGISTRY_HANDLER_RTTI_SCAN_LIMIT)) {
                if (!instruction.getFlowType().isCall()) {
                    continue;
                }

                Address target = resolvedCodeTarget(callTarget(instruction));
                if (isExecutableAddress(target)) {
                    callees.add(target);
                }
            }
            for (int i = callees.size() - 1; i >= 0; i--) {
                stack.addFirst(new HandlerScanFrame(callees.get(i), frame.depth + 1));
            }
        }

        return result;
    }

    private String azRttiSourceForWrite(
        boolean identityOnly,
        boolean nameOnly,
        VtableWrite write) {

        String source = write != null &&
            write.pattern != null &&
            write.pattern.contains("argument-object-vtable-write")
            ? "argument-object-vtable-write"
            : "constructor-vtable-write";
        if (!identityOnly) {
            source += "-value";
        }
        if (nameOnly) {
            source += "-name-only";
        }
        return source;
    }

    private JsonArray singleVptrWriteChain(
        VtableWrite write,
        AzRttiEvidence evidence,
        String expectedTypeName) {

        ArrayList<JsonObject> writes = new ArrayList<>();
        writes.add(vptrWriteJson(write, evidence, expectedTypeName));
        return constructorVptrChainJson(writes);
    }

    private boolean evidenceHasTypeId(AzRttiEvidence evidence, String typeId) {
        if (evidence == null || typeId == null) {
            return false;
        }
        if (uuidEquals(typeId, evidence.typeId)) {
            return true;
        }
        for (JsonElement element : evidence.providers) {
            if (!element.isJsonObject()) {
                continue;
            }
            String providerTypeId = string(element.getAsJsonObject(), "typeId");
            if (uuidEquals(typeId, providerTypeId)) {
                return true;
            }
        }
        return false;
    }

    private String providerTypeNameForTypeId(AzRttiEvidence evidence, String typeId) {
        if (evidence == null || typeId == null) {
            return null;
        }

        LinkedHashSet<Integer> matchingSlots = providerSlotsForTypeId(evidence, typeId);

        for (Integer slot : matchingSlots) {
            String sameSlotName = providerTypeNameAtSlot(evidence, slot);
            if (isConcreteNetworkTypeName(sameSlotName)) {
                return sameSlotName;
            }
        }

        if (matchingSlots.contains(2)) {
            String actualTypeName = providerTypeNameAtSlot(evidence, 1);
            if (isConcreteNetworkTypeName(actualTypeName)) {
                return actualTypeName;
            }
        }
        if (matchingSlots.contains(0)) {
            String actualTypeName = providerTypeNameAtSlot(evidence, 1);
            if (isConcreteNetworkTypeName(actualTypeName)) {
                return actualTypeName;
            }
        }
        return null;
    }

    private String providerAnyTypeNameForTypeId(AzRttiEvidence evidence, String typeId) {
        if (evidence == null || typeId == null) {
            return null;
        }

        LinkedHashSet<Integer> matchingSlots = providerSlotsForTypeId(evidence, typeId);
        for (Integer slot : matchingSlots) {
            String sameSlotName = providerTypeNameAtSlot(evidence, slot);
            if (isLikelyRuntimeTypeName(sameSlotName)) {
                return sameSlotName;
            }
        }

        if (matchingSlots.contains(2)) {
            String actualTypeName = providerTypeNameAtSlot(evidence, 1);
            if (isLikelyRuntimeTypeName(actualTypeName)) {
                return actualTypeName;
            }
        }
        if (matchingSlots.contains(0)) {
            String actualTypeName = providerTypeNameAtSlot(evidence, 1);
            if (isLikelyRuntimeTypeName(actualTypeName)) {
                return actualTypeName;
            }
        }
        return null;
    }

    private LinkedHashSet<Integer> providerSlotsForTypeId(
        AzRttiEvidence evidence,
        String typeId) {

        LinkedHashSet<Integer> slots = new LinkedHashSet<>();
        if (evidence == null || typeId == null) {
            return slots;
        }
        for (JsonElement element : evidence.providers) {
            if (!element.isJsonObject()) {
                continue;
            }
            JsonObject provider = element.getAsJsonObject();
            if (uuidEquals(typeId, string(provider, "typeId"))) {
                Integer slot = integer(provider, "slot");
                if (slot != null) {
                    slots.add(slot);
                }
            }
        }
        return slots;
    }

    private String providerTypeNameAtSlot(AzRttiEvidence evidence, int slot) {
        if (evidence == null) {
            return null;
        }
        for (JsonElement element : evidence.providers) {
            if (!element.isJsonObject()) {
                continue;
            }
            JsonObject provider = element.getAsJsonObject();
            Integer providerSlot = integer(provider, "slot");
            if (providerSlot != null && providerSlot == slot) {
                String typeName = string(provider, "typeName");
                if (typeName != null) {
                    return typeName;
                }
            }
        }
        return null;
    }

    private String providerTypeIdAtSlot(AzRttiEvidence evidence, int slot) {
        if (evidence == null) {
            return null;
        }
        for (JsonElement element : evidence.providers) {
            if (!element.isJsonObject()) {
                continue;
            }
            JsonObject provider = element.getAsJsonObject();
            Integer providerSlot = integer(provider, "slot");
            if (providerSlot != null && providerSlot == slot) {
                String providerTypeId = string(provider, "typeId");
                if (providerTypeId != null) {
                    return providerTypeId;
                }
            }
        }
        return null;
    }

    private boolean isBaseOrInterfaceRegistryEntry(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti) {

        String registryTypeId = canonicalUuidFromString(entry.uuid);
        if (registryTypeId == null || resolvedRtti == null ||
            !evidenceHasTypeId(resolvedRtti, registryTypeId)) {
            return false;
        }
        return isBaseNetworkTypeName(
            providerAnyTypeNameForTypeId(resolvedRtti, registryTypeId));
    }

    private int azRttiEvidenceScore(
        AzRttiEvidence evidence,
        VtableWrite write,
        String registryTypeId,
        int frameDepth) {

        int score = write.order * 20 - frameDepth * 200;
        if (evidence.typeId != null) {
            score += 40;
        }
        if (isLikelyRuntimeTypeName(evidence.typeName)) {
            score += 100 + runtimeTypeNameScore(evidence.typeName);
        }
        if (registryTypeId != null && uuidEquals(registryTypeId, evidence.typeId)) {
            score += 1000;
        }
        return score;
    }

    private List<VtableWrite> constructorVtableWrites(Address functionAddress) {
        return constructorVtableWrites(functionAddress, new LinkedHashSet<>(), false);
    }

    private List<VtableWrite> handlerVtableWrites(Address functionAddress) {
        return constructorVtableWrites(functionAddress, new LinkedHashSet<>(), true);
    }

    private List<VtableWrite> constructorVtableWrites(
        Address functionAddress,
        Set<String> activeConstructors,
        boolean includeArgumentObjectWrites) {

        Address target = resolvedCodeTarget(functionAddress);
        if (!isExecutableAddress(target)) {
            return List.of();
        }

        String key = addressCacheKey(
            includeArgumentObjectWrites
                ? "handler-vtable-writes"
                : "constructor-vtable-writes",
            target);
        List<VtableWrite> cached = constructorVtableWritesCache.get(key);
        if (cached != null) {
            return cached;
        }
        if (activeConstructors.size() >= CONSTRUCTOR_VTABLE_RECURSION_LIMIT) {
            return List.of();
        }

        String activeKey = target.toString();
        if (!activeConstructors.add(activeKey)) {
            return List.of();
        }
        try {
            List<VtableWrite> recovered =
                recoverConstructorVtableWrites(
                    target,
                    activeConstructors,
                    includeArgumentObjectWrites);
            List<VtableWrite> immutable =
                Collections.unmodifiableList(new ArrayList<>(recovered));
            constructorVtableWritesCache.put(key, immutable);
            return immutable;
        }
        finally {
            activeConstructors.remove(activeKey);
        }
    }

    private List<VtableWrite> recoverConstructorVtableWrites(
        Address functionAddress,
        Set<String> activeConstructors,
        boolean includeArgumentObjectWrites) {

        functionAtOrContaining(functionAddress);
        ArrayList<VtableWrite> result = new ArrayList<>();
        ForwardArgState state = newForwardArgState();

        for (Instruction instruction :
            linearInstructions(functionAddress, REGISTRY_HANDLER_RTTI_SCAN_LIMIT)) {

            String mnemonic = upperMnemonic(instruction);
            if (mnemonic == null) {
                continue;
            }

            if (instruction.getFlowType().isCall()) {
                collectVectorConstructorVtableWrites(
                    instruction,
                    state,
                    result,
                    activeConstructors);
                collectDirectConstructorCallVtableWrites(
                    instruction,
                    state,
                    result,
                    activeConstructors);
                observeForwardInstruction(instruction, state, false);
                continue;
            }

            if (isMemoryWriteMnemonic(mnemonic)) {
                Integer offset =
                    trackedThisOffsetForMemoryOperand(instruction, 0, state.registers);
                TrackedValue baseOffset = includeArgumentObjectWrites
                    ? trackedBaseOffsetForMemoryOperand(instruction, 0, state.registers)
                    : null;
                TrackedValue source = "MOV".equals(mnemonic)
                    ? trackedOperandValue(instruction, 1, state)
                    : null;
                if (offset != null && source != null &&
                    source.address != null && isVtableLike(source.address)) {
                    result.add(new VtableWrite(
                        functionAddress,
                        instruction.getMinAddress(),
                        source.address,
                        result.size(),
                        offset,
                        "constructor-vtable-write"));
                }
                else if (source != null &&
                    source.address != null &&
                    isVtableLike(source.address) &&
                    isArgumentObjectVtableWrite(baseOffset)) {
                    result.add(new VtableWrite(
                        functionAddress,
                        instruction.getMinAddress(),
                        source.address,
                        result.size(),
                        null,
                        baseOffset.baseKey,
                        baseOffset.baseOffset,
                        "argument-object-vtable-write"));
                }
            }

            observeForwardInstruction(instruction, state, false);
        }

        return result;
    }

    private boolean isArgumentObjectVtableWrite(TrackedValue value) {
        return value != null &&
            "arg:R8".equals(value.baseKey) &&
            value.baseOffset != null &&
            value.baseOffset >= 0 &&
            value.baseOffset <= 0x1000 &&
            value.baseOffset % 8 == 0;
    }

    private void collectDirectConstructorCallVtableWrites(
        Instruction instruction,
        ForwardArgState state,
        List<VtableWrite> result,
        Set<String> activeConstructors) {

        TrackedValue receiver = state.registers.get("RCX");
        if (receiver == null || receiver.thisOffset == null) {
            return;
        }

        Address constructor = resolvedCodeTarget(callTarget(instruction));
        if (!isExecutableAddress(constructor)) {
            return;
        }
        if (isVectorConstructorIteratorCall(constructor)) {
            return;
        }

        for (VtableWrite write : constructorVtableWrites(
            constructor,
            activeConstructors,
            false)) {
            if (write.thisOffset == null) {
                continue;
            }
            Integer projected =
                addOffsets(receiver.thisOffset, write.thisOffset.longValue());
            if (projected == null) {
                continue;
            }
            result.add(new VtableWrite(
                write.function,
                write.instruction,
                write.vtable,
                result.size(),
                projected,
                prefixPattern("constructor-call", write.pattern)));
        }
    }

    private void collectVectorConstructorVtableWrites(
        Instruction instruction,
        ForwardArgState state,
        List<VtableWrite> result,
        Set<String> activeConstructors) {

        Address call = resolvedCodeTarget(callTarget(instruction));
        if (!isVectorConstructorIteratorCall(call)) {
            return;
        }

        TrackedValue base = state.registers.get("RCX");
        TrackedValue elementSize = state.registers.get("RDX");
        TrackedValue elementCount = state.registers.get("R8");
        TrackedValue constructorValue = state.registers.get("R9");
        if (base == null || base.thisOffset == null ||
            elementSize == null || elementSize.immediate == null ||
            elementCount == null || elementCount.immediate == null ||
            constructorValue == null || constructorValue.address == null) {
            return;
        }

        long count = elementCount.immediate;
        long size = elementSize.immediate;
        if (count <= 0 || count > 256 || size <= 0 || size > 0x1000) {
            return;
        }

        Address constructor = resolvedCodeTarget(constructorValue.address);
        List<VtableWrite> writes = constructorVtableWrites(
            constructor,
            activeConstructors,
            false);
        if (writes.isEmpty()) {
            return;
        }

        for (long i = 0; i < count; i++) {
            long elementOffset = base.thisOffset.longValue() + (i * size);
            if (elementOffset < Integer.MIN_VALUE || elementOffset > Integer.MAX_VALUE) {
                continue;
            }
            for (VtableWrite write : writes) {
                if (write.thisOffset == null) {
                    continue;
                }
                Integer projected = addOffsets((int)elementOffset, write.thisOffset.longValue());
                if (projected == null) {
                    continue;
                }
                result.add(new VtableWrite(
                    write.function,
                    write.instruction,
                    write.vtable,
                    result.size(),
                    projected,
                    prefixPattern("vector-constructor", write.pattern)));
            }
        }
    }

    private Integer addOffsets(int base, long delta) {
        long value = (long)base + delta;
        if (value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) {
            return null;
        }
        return (int)value;
    }

    private String prefixPattern(String prefix, String pattern) {
        return pattern == null ? prefix : prefix + ":" + pattern;
    }

    private boolean isVectorConstructorIteratorCall(Address target) {
        Address resolved = resolvedCodeTarget(target);
        if (!isExecutableAddress(resolved)) {
            return false;
        }
        String name = fullFunctionName(functionAtOrContaining(resolved));
        return name != null &&
            name.toLowerCase(Locale.ROOT).contains("eh_vector_constructor_iterator");
    }

    private boolean isMemoryOperand(Instruction instruction, int operandIndex) {
        String text = operandText(instruction, operandIndex);
        return text != null && text.contains("[") && text.contains("]");
    }

    private void clearVolatileAddressRegisters(Map<String, Address> registers) {
        registers.remove("RAX");
        registers.remove("RCX");
        registers.remove("RDX");
        registers.remove("R8");
        registers.remove("R9");
        registers.remove("R10");
        registers.remove("R11");
    }

    private TypeNameEvidence resolvedTypeName(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti,
        HookTypeEvidence hookType,
        List<RegistrationFunction> matches) {

        if ((hookType != null && isPlausibleTypeName(hookType.typeName)) ||
            (resolvedRtti != null && isConcreteNetworkTypeName(resolvedRtti.typeName))) {
            return null;
        }

        for (RegistrationFunction match : matches) {
            String typeName = match.constructorTypeName();
            if (!isLikelyRuntimeTypeName(typeName)) {
                continue;
            }
            TypeNameEvidence evidence = new TypeNameEvidence();
            evidence.source = "constructorFunctionName";
            evidence.function = match.function.getEntryPoint();
            evidence.typeName = typeName;
            return evidence;
        }

        return typeNameFromCreateInstanceAllocation(entry);
    }

    private TypeNameEvidence typeNameFromCreateInstanceAllocation(RegistryEntry entry) {
        Address createInstance = parseCapturedAddress(entry.createInstance);
        if (!isExecutableAddress(createInstance)) {
            return null;
        }

        String bestName = null;
        String bestSource = null;
        int bestScore = Integer.MIN_VALUE;
        Address bestAddress = null;
        for (Instruction instruction :
            linearInstructions(createInstance, REGISTRY_HANDLER_RTTI_SCAN_LIMIT)) {

            for (Address target : referencedAddresses(instruction)) {
                String name = readPrintableString(target);
                TypeNameCandidate candidate =
                    typeNameCandidate(name, "createInstanceAllocationLabel", target);
                if (candidate == null || candidate.score < bestScore) {
                    continue;
                }
                bestName = candidate.typeName;
                bestSource = candidate.source;
                bestAddress = candidate.address;
                bestScore = candidate.score;
            }
        }

        TypeNameCandidate decompiledCandidate =
            typeNameFromCreateInstanceDecompile(createInstance);
        if (decompiledCandidate != null && decompiledCandidate.score >= bestScore) {
            bestName = decompiledCandidate.typeName;
            bestSource = decompiledCandidate.source;
            bestAddress = decompiledCandidate.address;
            bestScore = decompiledCandidate.score;
        }

        if (bestName == null) {
            return null;
        }

        TypeNameEvidence evidence = new TypeNameEvidence();
        evidence.source = bestSource == null ? "createInstanceAllocationLabel" : bestSource;
        evidence.function = createInstance;
        evidence.typeName = bestName;
        evidence.typeNameAddress = bestAddress;
        return evidence;
    }

    private TypeNameCandidate typeNameFromCreateInstanceDecompile(Address createInstance) {
        Function function = functionAtOrContaining(createInstance);
        String text = decompileC(function);
        if (text == null) {
            return null;
        }

        TypeNameCandidate best = null;
        Matcher matcher = Pattern
            .compile("\"(?<value>[A-Za-z_][A-Za-z0-9_:<>.$-]{0,255})\"")
            .matcher(text);
        while (matcher.find()) {
            TypeNameCandidate candidate =
                typeNameCandidate(matcher.group("value"), "createInstanceAllocationLiteral", null);
            if (candidate == null || (best != null && candidate.score < best.score)) {
                continue;
            }
            best = candidate;
        }
        return best;
    }

    private TypeNameCandidate typeNameCandidate(String name, String source, Address address) {
        if (!isLikelyRuntimeTypeName(name)) {
            return null;
        }

        return new TypeNameCandidate(name, source, address, runtimeTypeNameScore(name));
    }

    private boolean isLikelyRuntimeTypeName(String name) {
        if (!isPlausibleTypeName(name)) {
            return false;
        }
        if (isAllocatorTypeName(name)) {
            return false;
        }
        if (name.startsWith("m_") || name.startsWith("[") || name.startsWith("FUN_")) {
            return false;
        }
        return Character.isUpperCase(name.charAt(0)) ||
            name.contains("::") ||
            hasRuntimeTypeSuffix(name);
    }

    private boolean isConcreteNetworkTypeName(String name) {
        if (!isLikelyRuntimeTypeName(name)) {
            return false;
        }
        return !isBaseNetworkTypeName(name);
    }

    private boolean isBaseNetworkTypeName(String name) {
        if (name == null) {
            return false;
        }
        String simple = simpleTypeName(name);
        return "IMessage".equals(simple) ||
            "IFragment".equals(simple) ||
            "TraitState".equals(simple) ||
            "ReplicatedState".equals(simple) ||
            "ReplicatedFieldHandlerBase".equals(simple);
    }

    private int runtimeTypeNameScore(String name) {
        if (name == null) {
            return Integer.MIN_VALUE;
        }

        int score = 0;
        if (name.contains("::")) {
            score += 80;
        }
        if (!name.isEmpty() && Character.isUpperCase(name.charAt(0))) {
            score += 20;
        }
        if (hasRuntimeTypeSuffix(name)) {
            score += 40;
        }
        score += Math.min(name.length(), 80);
        return score;
    }

    private boolean isAllocatorTypeName(String name) {
        if (name == null) {
            return false;
        }
        String lower = name.toLowerCase(Locale.ROOT);
        return "systemallocator".equals(lower) ||
            "az::systemallocator".equals(lower) ||
            lower.startsWith("azstd::allocator") ||
            lower.startsWith("azstd::static_buffer_allocator") ||
            lower.contains("allocator_deallocate");
    }

    private boolean typeNamesMatch(String expected, String actual) {
        if (expected == null || actual == null) {
            return false;
        }
        if (expected.equals(actual)) {
            return true;
        }
        return simpleTypeName(expected).equals(simpleTypeName(actual));
    }

    private String simpleTypeName(String name) {
        if (name == null) {
            return "";
        }
        String value = name;
        int namespace = value.lastIndexOf("::");
        if (namespace >= 0) {
            value = value.substring(namespace + 2);
        }
        int dot = value.lastIndexOf('.');
        if (dot >= 0) {
            value = value.substring(dot + 1);
        }
        return value;
    }

    private boolean hasRuntimeTypeSuffix(String name) {
        if (name == null) {
            return false;
        }
        String[] suffixes = {
            "Msg",
            "State",
            "Trait",
            "Fragment",
            "Component",
            "ReplicatedState",
            "Params",
            "Data",
            "Request",
            "Response",
            "Notification",
        };
        for (String suffix : suffixes) {
            if (name.endsWith(suffix)) {
                return true;
            }
        }
        return false;
    }

    private String typeNameFromRttiHelperFunctionName(Address function) {
        Function providerFunction = functionAtOrContaining(function);
        if (providerFunction == null) {
            return null;
        }

        Matcher matcher = RTTI_HELPER_NAME_RE.matcher(fullFunctionName(providerFunction));
        if (!matcher.find()) {
            return null;
        }

        String typeName = matcher.group("type");
        return isLikelyRuntimeTypeName(typeName) ? typeName : null;
    }

    private TypeNameDecode typeNameDecodeFromFunctionName(Address function, Address provider) {
        String typeName = typeNameFromRttiHelperFunctionName(provider);
        if (typeName == null) {
            typeName = typeNameFromRttiHelperFunctionName(function);
        }
        if (typeName == null) {
            return null;
        }

        TypeNameDecode decode = new TypeNameDecode();
        decode.function = function;
        decode.provider = provider;
        decode.typeName = typeName;
        decode.typeNameSource = "rttiHelperFunctionName";
        return decode;
    }

    private List<Address> registryHandlerFunctionAddresses(RegistryEntry entry) {
        ArrayList<Address> result = new ArrayList<>();
        addRegistryHandlerAddress(result, entry.unmarshal);
        addRegistryHandlerAddress(result, entry.marshal);
        addRegistryHandlerAddress(result, entry.destructor);
        addRegistryHandlerAddress(result, entry.createInstance);
        return result;
    }

    private void addRegistryHandlerAddress(List<Address> result, String value) {
        Address address = parseCapturedAddress(value);
        if (isExecutableAddress(address)) {
            result.add(address);
        }
    }

    private HookTypeEvidence registrationHookForEntry(
        RegistryEntry entry,
        Map<String, HookTypeEvidence> hookTypeNamesById) {

        String registryTypeId = canonicalUuidFromString(entry.uuid);
        return registryTypeId == null ? null : hookTypeNamesById.get(normalizeUuid(registryTypeId));
    }

    private boolean isRegistryTypeNameFallback(HookTypeEvidence hookType) {
        return hookType != null && "typeregistry-entry".equals(hookType.source);
    }

    private JsonObject identityFallbackJson(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti,
        HookTypeEvidence hookType,
        TypeNameEvidence recoveredTypeName,
        List<RegistrationFunction> matches) {

        JsonObject object = registryIdentityJson(entry);
        object.addProperty("typeName", hookType.typeName);
        object.addProperty("typeNameSource", hookType.typeNameSource());
        object.addProperty(
            "reason",
            "no install-registration-hook evidence matched this UUID; using the registry/typeindex name");
        object.addProperty("constructorMatchCount", matches.size());
        object.addProperty(
            "handlerAddressCount",
            registryHandlerFunctionAddresses(entry).size());
        add(object, "azRttiSource", resolvedRtti == null ? null : resolvedRtti.source);
        add(object, "azRttiTypeId", resolvedRtti == null ? null : resolvedRtti.typeId);
        add(object, "azRttiTypeName", resolvedRtti == null ? null : resolvedRtti.typeName);
        add(object, "azRttiTypeNameSource",
            resolvedRtti == null ? null : resolvedRtti.typeNameSource);
        if (recoveredTypeName != null) {
            object.add("recoveredTypeName", recoveredTypeName.toJson());
        }
        object.add("missingEvidence", fallbackMissingEvidence(entry, resolvedRtti, matches));
        object.add("handler", registryHandlerJson(entry));
        object.add("constructorMatches", constructorMatchAddressesJson(matches));
        return object;
    }

    private JsonObject identityBlockerJson(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti,
        AzRttiEvidence valueRtti,
        HookTypeEvidence hookType,
        TypeNameEvidence recoveredTypeName,
        List<RegistrationFunction> matches,
        String reason) {

        JsonObject object = registryIdentityJson(entry);
        object.addProperty("reason", reason);
        object.addProperty("hasRegistryName", isPlausibleTypeName(entry.name));
        object.addProperty("hasHookEvidence", hookType != null);
        object.addProperty(
            "hasHookTypeName",
            hookType != null && isPlausibleTypeName(hookType.typeName));
        object.addProperty("hasAzRtti", resolvedRtti != null);
        object.addProperty(
            "hasAzRttiTypeId",
            resolvedRtti != null && resolvedRtti.typeId != null);
        object.addProperty(
            "hasAzRttiTypeName",
            resolvedRtti != null && isPlausibleTypeName(resolvedRtti.typeName));
        object.addProperty(
            "hasConcreteAzRttiTypeName",
            resolvedRtti != null && isConcreteNetworkTypeName(resolvedRtti.typeName));
        object.addProperty(
            "hasRecoveredTypeName",
            recoveredTypeName != null && isPlausibleTypeName(recoveredTypeName.typeName));
        object.addProperty("hasValueAzRtti", valueRtti != null);
        object.addProperty(
            "hasConcreteValueTypeName",
            valueRtti != null && isConcreteNetworkTypeName(valueRtti.typeName));
        object.addProperty("constructorMatchCount", matches.size());
        object.addProperty(
            "handlerAddressCount",
            registryHandlerFunctionAddresses(entry).size());
        add(object, "hookSource", hookType == null ? null : hookType.typeNameSource());
        add(object, "hookTypeName", hookType == null ? null : hookType.typeName);
        add(object, "azRttiSource", resolvedRtti == null ? null : resolvedRtti.source);
        add(object, "azRttiTypeId", resolvedRtti == null ? null : resolvedRtti.typeId);
        add(object, "azRttiTypeName", resolvedRtti == null ? null : resolvedRtti.typeName);
        add(object, "azRttiTypeNameSource",
            resolvedRtti == null ? null : resolvedRtti.typeNameSource);
        add(object, "azRttiSelectedProviderTypeName",
            providerAnyTypeNameForTypeId(
                resolvedRtti,
                canonicalUuidFromString(entry.uuid)));
        add(object, "valueAzRttiSource", valueRtti == null ? null : valueRtti.source);
        add(object, "valueAzRttiTypeId", valueRtti == null ? null : valueRtti.typeId);
        add(object, "valueAzRttiTypeName", valueRtti == null ? null : valueRtti.typeName);
        add(object, "valueAzRttiTypeNameSource",
            valueRtti == null ? null : valueRtti.typeNameSource);
        if (valueRtti != null) {
            object.add("valueAzRtti", valueRtti.toJson());
        }
        if (recoveredTypeName != null) {
            object.add("recoveredTypeName", recoveredTypeName.toJson());
        }
        object.add("missingEvidence", blockerMissingEvidence(
            entry,
            resolvedRtti,
            valueRtti,
            hookType,
            recoveredTypeName,
            matches));
        object.add("handler", registryHandlerJson(entry));
        object.add("constructorMatches", constructorMatchAddressesJson(matches));
        return object;
    }

    private String identityBlockerReason(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti,
        AzRttiEvidence valueRtti,
        HookTypeEvidence hookType,
        TypeNameEvidence recoveredTypeName,
        List<RegistrationFunction> matches) {

        if ((hookType != null && isPlausibleTypeName(hookType.typeName)) ||
            (resolvedRtti != null && isConcreteNetworkTypeName(resolvedRtti.typeName)) ||
            (recoveredTypeName != null && isPlausibleTypeName(recoveredTypeName.typeName)) ||
            (valueRtti != null && isConcreteNetworkTypeName(valueRtti.typeName))) {
            return null;
        }

        String registryTypeId = canonicalUuidFromString(entry.uuid);
        if (registryTypeId == null) {
            return "missing-or-invalid-uuid";
        }
        if (isZeroUuid(registryTypeId)) {
            return "zero-uuid";
        }
        if (isBaseOrInterfaceRegistryEntry(entry, resolvedRtti)) {
            return null;
        }
        if (hookType != null && !isPlausibleTypeName(hookType.typeName)) {
            return "hook-evidence-without-semantic-type-name";
        }
        if (resolvedRtti != null && resolvedRtti.typeId != null &&
            !isPlausibleTypeName(resolvedRtti.typeName)) {
            return "az-rtti-uuid-only";
        }
        if (!isPlausibleTypeName(entry.name)) {
            if (matches.isEmpty()) {
                if (registryHandlerFunctionAddresses(entry).isEmpty()) {
                    return "no-registry-name-no-hook-no-constructor-match-no-handler-address";
                }
                return "no-registry-name-no-hook-no-constructor-match-handler-rtti-missing";
            }
            return "no-registry-name-constructor-rtti-missing-or-mismatch";
        }
        if (matches.isEmpty()) {
            if (registryHandlerFunctionAddresses(entry).isEmpty()) {
                return "registry-name-no-hook-no-constructor-match-no-handler-address";
            }
            return "registry-name-no-hook-no-constructor-match-handler-rtti-missing";
        }
        return "registry-name-constructor-rtti-missing-or-mismatch";
    }

    private JsonObject registryIdentityJson(RegistryEntry entry) {
        JsonObject object = new JsonObject();
        add(object, "uuid", entry.uuid);
        add(object, "name", entry.name);
        add(object, "index", entry.index);
        add(object, "typeIndex", entry.typeIndex);
        add(object, "storageAddress", entry.storageAddress);
        add(object, "baseVtable", entry.baseVtable);
        add(object, "vtable", entry.vtable);
        return object;
    }

    private JsonObject registryHandlerJson(RegistryEntry entry) {
        JsonObject handler = new JsonObject();
        add(handler, "Destructor", entry.destructor);
        add(handler, "GetEmptyValue", entry.getEmptyValue);
        add(handler, "CreateInstance", entry.createInstance);
        add(handler, "CopyValue", entry.copyValue);
        add(handler, "Marshal", entry.marshal);
        add(handler, "Unmarshal", entry.unmarshal);

        JsonArray parsed = new JsonArray();
        for (Address address : registryHandlerFunctionAddresses(entry)) {
            parsed.add(formatAddress(address));
        }
        handler.add("parsedFunctionAddresses", parsed);
        return handler;
    }

    private JsonArray constructorMatchAddressesJson(List<RegistrationFunction> matches) {
        JsonArray array = new JsonArray();
        for (RegistrationFunction match : matches) {
            JsonObject object = new JsonObject();
            object.addProperty("address", formatAddress(match.function.getEntryPoint()));
            object.addProperty("name", fullFunctionName(match.function));
            if (match.azRtti != null) {
                object.add("azRtti", match.azRtti.toJson());
            }
            array.add(object);
        }
        return array;
    }

    private JsonArray fallbackMissingEvidence(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti,
        List<RegistrationFunction> matches) {

        JsonArray array = new JsonArray();
        array.add("install-registration-hook");
        if (resolvedRtti == null) {
            if (matches.isEmpty()) {
                array.add("constructor-register-field-match");
            }
            if (registryHandlerFunctionAddresses(entry).isEmpty()) {
                array.add("registry-handler-address");
            }
            else {
                array.add("constructor-vtable-write-az-rtti");
            }
        }
        else if (!isConcreteNetworkTypeName(resolvedRtti.typeName)) {
            array.add("az-rtti-semantic-type-name");
        }
        return array;
    }

    private JsonArray blockerMissingEvidence(
        RegistryEntry entry,
        AzRttiEvidence resolvedRtti,
        AzRttiEvidence valueRtti,
        HookTypeEvidence hookType,
        TypeNameEvidence recoveredTypeName,
        List<RegistrationFunction> matches) {

        JsonArray array = new JsonArray();
        String registryTypeId = canonicalUuidFromString(entry.uuid);
        if (registryTypeId == null || isZeroUuid(registryTypeId)) {
            array.add("usable-uuid");
        }
        if (!isPlausibleTypeName(entry.name)) {
            array.add("registry-debug-name");
        }
        if (hookType == null) {
            array.add("install-registration-hook");
        }
        else if (!isPlausibleTypeName(hookType.typeName)) {
            array.add("registration-hook-semantic-type-name");
        }
        if (matches.isEmpty()) {
            array.add("constructor-register-field-match");
        }
        if (registryHandlerFunctionAddresses(entry).isEmpty()) {
            array.add("registry-handler-address");
        }
        if (resolvedRtti == null) {
            array.add("az-rtti");
        }
        else if (!isConcreteNetworkTypeName(resolvedRtti.typeName)) {
            array.add("az-rtti-semantic-type-name");
        }
        if (recoveredTypeName == null) {
            array.add("recovered-type-name");
        }
        if (valueRtti == null) {
            array.add("value-az-rtti");
        }
        else if (!isConcreteNetworkTypeName(valueRtti.typeName)) {
            array.add("concrete-value-type-name");
        }
        return array;
    }

    private JsonObject countMapJson(Map<String, Integer> counts) {
        JsonObject object = new JsonObject();
        for (Map.Entry<String, Integer> entry : counts.entrySet()) {
            object.addProperty(entry.getKey(), entry.getValue());
        }
        return object;
    }

    private JsonObject registrationInvariants(
        Map<String, HookTypeEvidence> hooksById,
        List<RegistryEntry> registry) {

        LinkedHashMap<String, String> registryNamesById = new LinkedHashMap<>();
        for (RegistryEntry entry : registry) {
            String typeId = normalizeUuid(entry.uuid);
            if (typeId != null) {
                registryNamesById.put(typeId, entry.name);
            }
        }

        JsonArray hookUuidsNotInRegistry = new JsonArray();
        JsonArray zeroUuidDecoded = new JsonArray();
        JsonArray decodedButNameMismatch = new JsonArray();
        int hookUuidsNotInRegistryCount = 0;
        int zeroUuidDecodedCount = 0;
        int decodedButNameMismatchCount = 0;
        int unresolvedHookNameCount = 0;
        for (HookTypeEvidence hook : hooksById.values()) {
            if (hook == null || "typeregistry-entry".equals(hook.source)) {
                continue;
            }
            String typeId = normalizeUuid(hook.typeId);
            if (typeId == null) {
                continue;
            }
            if (isZeroUuid(typeId)) {
                zeroUuidDecodedCount++;
                addInvariantSample(zeroUuidDecoded, hook, null);
                continue;
            }
            if (!registryTypeIds.contains(typeId)) {
                hookUuidsNotInRegistryCount++;
                addInvariantSample(hookUuidsNotInRegistry, hook, null);
                continue;
            }

            String registryName = registryNamesById.get(typeId);
            if (registryName == null || registryName.isEmpty()) {
                continue;
            }
            if (!isPlausibleTypeName(hook.typeName) &&
                !isPlausibleTypeName(hook.slotTypeName)) {
                unresolvedHookNameCount++;
                continue;
            }
            if (!semanticTypeNameMatches(registryName, hook.typeName) &&
                !semanticTypeNameMatches(registryName, hook.slotTypeName)) {
                decodedButNameMismatchCount++;
                addInvariantSample(decodedButNameMismatch, hook, registryName);
            }
        }

        JsonObject object = new JsonObject();
        object.addProperty("hookUuidsNotInRegistryCount", hookUuidsNotInRegistryCount);
        object.add("hookUuidsNotInRegistry", hookUuidsNotInRegistry);
        object.addProperty("zeroUuidDecodedCount", zeroUuidDecodedCount);
        object.add("zeroUuidDecoded", zeroUuidDecoded);
        object.addProperty("decodedButNameMismatchCount", decodedButNameMismatchCount);
        object.add("decodedButNameMismatch", decodedButNameMismatch);
        object.addProperty("unresolvedHookNameCount", unresolvedHookNameCount);
        object.add("typeIdSourceStats", countMapJson(typeIdSourceCounts));
        object.add("nativeUuidRejectSummary", countMapJson(nativeUuidRejectCounts));
        object.addProperty("registryTypeIdCount", registryTypeIds.size());
        return object;
    }

    private void addInvariantSample(
        JsonArray samples,
        HookTypeEvidence hook,
        String registryName) {

        if (samples.size() >= REGISTRATION_FAILURE_SAMPLE_LIMIT) {
            return;
        }
        JsonObject object = new JsonObject();
        add(object, "typeId", hook.typeId);
        add(object, "typeName", hook.typeName);
        add(object, "slotTypeName", hook.slotTypeName);
        add(object, "registryName", registryName);
        add(object, "source", hook.source == null ? "install-registration-hook" : hook.source);
        add(object, "helperTable", formatAddress(hook.helperTable));
        add(object, "registerThunk", formatAddress(hook.registerThunk));
        add(object, "typeProvider", formatAddress(hook.typeProvider));
        add(object, "uuidSource", formatAddress(hook.uuidSource));
        samples.add(object);
    }

    private boolean semanticTypeNameMatches(String expected, String observed) {
        if (expected == null || observed == null) {
            return false;
        }
        if (expected.equals(observed)) {
            return true;
        }
        return observed.endsWith("::" + expected) || expected.endsWith("::" + observed);
    }

    private void incrementCount(Map<String, Integer> counts, String key) {
        counts.put(key, counts.getOrDefault(key, 0) + 1);
    }

    private Map<String, HookTypeEvidence> collectRegistrationHookTypeNames(
        List<RegistryEntry> registry) {

        LinkedHashMap<String, HookTypeEvidence> result = new LinkedHashMap<>();
        seedRegistryTypeIds(registry);
        collectQueuedRegistrationHooks(result);
        int queuedHookEvidenceCount = result.size();

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
        int installHookEvidenceCount = result.size();
        collectDirectRegisterTypeProviders(result);
        int directRegisterTypeEvidenceCount = result.size() - installHookEvidenceCount;
        int hookEvidenceCount = result.size();
        seedRegistryTypeIdentities(result, registry);
        println("Recovered registration type names: " + result.size() +
            " (hook evidence: " + hookEvidenceCount +
            ", queued hooks: " + queuedHookEvidenceCount +
            ", direct RegisterType: " + directRegisterTypeEvidenceCount +
            ", registry fallback: " + (result.size() - hookEvidenceCount) + ")");
        println("Registration evidence xrefs: queued refs=" + queuedRegistrationReferenceCount +
            ", queued decoded=" + queuedRegistrationDecodedCount +
            ", queued no function=" + queuedRegistrationNoFunctionCount +
            ", queued no helper=" + queuedRegistrationNoHelperCount +
            ", queued no type-id=" + queuedRegistrationNoTypeIdCount +
            ", direct refs=" + directRegisterTypeReferenceCount +
            ", direct decoded=" + directRegisterTypeDecodedCount +
            ", direct no function=" + directRegisterTypeNoFunctionCount +
            ", direct no type-id=" + directRegisterTypeNoTypeIdCount);
        return result;
    }

    private void seedRegistryTypeIds(List<RegistryEntry> registry) {
        registryTypeIds.clear();
        for (RegistryEntry entry : registry) {
            String registryTypeId = canonicalUuidFromString(entry.uuid);
            if (registryTypeId != null) {
                registryTypeIds.add(normalizeUuid(registryTypeId));
            }
        }
    }

    private void seedRegistryTypeIdentities(
        Map<String, HookTypeEvidence> result,
        List<RegistryEntry> registry) {

        for (RegistryEntry entry : registry) {
            String typeId = canonicalUuidFromString(entry.uuid);
            if (typeId == null || isZeroUuid(typeId) || !isPlausibleTypeName(entry.name)) {
                continue;
            }

            String key = normalizeUuid(typeId);
            if (result.containsKey(key)) {
                continue;
            }

            HookTypeEvidence hook = new HookTypeEvidence();
            hook.typeId = typeId;
            hook.typeName = entry.name;
            hook.source = "typeregistry-entry";
            result.put(key, hook);
        }
    }

    private boolean isZeroUuid(String uuid) {
        String normalized = normalizeUuid(uuid);
        return NIL_TYPE_ID.equals(normalized);
    }

    private void collectQueuedRegistrationHooks(Map<String, HookTypeEvidence> result) {
        queuedRegistrationReferenceCount = 0;
        queuedRegistrationDecodedCount = 0;
        queuedRegistrationNoFunctionCount = 0;
        queuedRegistrationNoHelperCount = 0;
        queuedRegistrationNoTypeIdCount = 0;
        queuedRegistrationFailureSamples = new JsonArray();
        Address queueRegistrationHook =
            currentProgram.getImageBase().add(QUEUE_REGISTRATION_HOOK_RVA);
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(queueRegistrationHook);
        while (references.hasNext()) {
            Reference reference = references.next();
            queuedRegistrationReferenceCount++;

            Function owner = functionContaining(reference.getFromAddress());
            if (owner == null) {
                queuedRegistrationNoFunctionCount++;
                recordRegistrationFailure(
                    queuedRegistrationFailureSamples,
                    "queued-hook",
                    "no-function",
                    reference,
                    null,
                    null,
                    null);
                continue;
            }
            Address helperTable =
                findRegistrationHelperTable(owner, reference.getFromAddress());
            if (helperTable == null) {
                queuedRegistrationNoHelperCount++;
                recordRegistrationFailure(
                    queuedRegistrationFailureSamples,
                    "queued-hook",
                    "no-helper-table",
                    reference,
                    owner,
                    null,
                    null);
                continue;
            }
            HookTypeEvidence hook = decodeRegistrationHelperTable(
                owner.getEntryPoint(),
                helperTable,
                registrationHookTypeNameFromSlot(helperTable, 3));
            if (hook == null || hook.typeId == null) {
                queuedRegistrationNoTypeIdCount++;
                recordRegistrationFailure(
                    queuedRegistrationFailureSamples,
                    "queued-hook",
                    "no-type-id",
                    reference,
                    owner,
                    helperTable,
                    hook);
                continue;
            }
            queuedRegistrationDecodedCount++;
            result.putIfAbsent(normalizeUuid(hook.typeId), hook);
        }
    }

    private void collectDirectRegisterTypeProviders(Map<String, HookTypeEvidence> result) {
        directRegisterTypeReferenceCount = 0;
        directRegisterTypeDecodedCount = 0;
        directRegisterTypeNoFunctionCount = 0;
        directRegisterTypeNoTypeIdCount = 0;
        directRegisterTypeFailureSamples = new JsonArray();
        Address registerType =
            currentProgram.getImageBase().add(TYPE_REGISTRY_REGISTER_TYPE_RVA);
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(registerType);
        while (references.hasNext()) {
            Reference reference = references.next();
            directRegisterTypeReferenceCount++;

            Function owner = functionContaining(reference.getFromAddress());
            if (owner == null) {
                directRegisterTypeNoFunctionCount++;
                recordRegistrationFailure(
                    directRegisterTypeFailureSamples,
                    "direct-register-type",
                    "no-function",
                    reference,
                    null,
                    null,
                    null);
                continue;
            }

            HookTypeEvidence hook =
                decodeDirectRegisterTypeProvider(owner, reference.getFromAddress());
            if (hook == null || hook.typeId == null) {
                directRegisterTypeNoTypeIdCount++;
                HookTypeEvidence partialHook =
                    hook == null ? partialDirectRegisterTypeProvider(owner, reference.getFromAddress()) : hook;
                recordRegistrationFailure(
                    directRegisterTypeFailureSamples,
                    "direct-register-type",
                    "no-type-id",
                    reference,
                    owner,
                    null,
                    partialHook);
                continue;
            }

            directRegisterTypeDecodedCount++;
            result.putIfAbsent(normalizeUuid(hook.typeId), hook);
        }
    }

    private void recordRegistrationFailure(
        JsonArray samples,
        String scan,
        String reason,
        Reference reference,
        Function owner,
        Address helperTable,
        HookTypeEvidence partialHook) {

        if (samples.size() >= REGISTRATION_FAILURE_SAMPLE_LIMIT) {
            return;
        }

        JsonObject object = new JsonObject();
        object.addProperty("scan", scan);
        object.addProperty("reason", reason);
        if (reference != null) {
            add(object, "callsite", formatAddress(reference.getFromAddress()));
            add(object, "target", formatAddress(reference.getToAddress()));
        }
        if (owner != null) {
            add(object, "owner", formatAddress(owner.getEntryPoint()));
            add(object, "ownerName", fullFunctionName(owner));
        }
        add(object, "helperTable", formatAddress(helperTable));
        if (partialHook != null) {
            add(object, "partialTypeName", partialHook.typeName);
            add(object, "partialSlotTypeName", partialHook.slotTypeName);
            add(object, "partialTypeProvider", formatAddress(partialHook.typeProvider));
            add(object, "partialUuidSource", formatAddress(partialHook.uuidSource));
            add(object, "partialTypeDescriptor", formatAddress(partialHook.typeDescriptor));
        }
        samples.add(object);
    }

    private HookTypeEvidence decodeDirectRegisterTypeProvider(
        Function owner,
        Address registerTypeCallsite) {

        List<Instruction> instructions = functionInstructions(owner);
        int registerTypeInstruction = -1;
        for (int i = 0; i < instructions.size(); i++) {
            Address instructionAddress = instructions.get(i).getMinAddress();
            if (instructionAddress.equals(registerTypeCallsite)) {
                registerTypeInstruction = i;
                break;
            }
            if (instructionAddress.compareTo(registerTypeCallsite) > 0) {
                break;
            }
        }
        if (registerTypeInstruction < 0) {
            return null;
        }

        int firstCandidate =
            Math.max(0, registerTypeInstruction - BACKWARD_ARGUMENT_SCAN_LIMIT);
        for (int i = registerTypeInstruction - 1; i >= firstCandidate; i--) {
            Instruction instruction = instructions.get(i);
            if (!instruction.getFlowType().isCall()) {
                continue;
            }

            Address target = callTarget(instruction);
            TypeIdDecode typeId = decodeDirectTypeIdProvider(target);
            if (typeId == null) {
                continue;
            }

            TypeNameDecode typeName = decodeTypeNameFromReferencedStrings(
                typeId.function,
                typeId.provider);

            HookTypeEvidence hook = new HookTypeEvidence();
            hook.source = "direct-register-type";
            hook.typeId = typeId.typeId;
            hook.typeName = typeName == null ? null : typeName.typeName;
            hook.hookFunction = owner.getEntryPoint();
            hook.registerThunk = owner.getEntryPoint();
            hook.typeProvider = typeId.provider;
            hook.uuidSource = typeId.sourceAddress;
            enrichHookMessageHandler(hook);
            return hook;
        }
        return null;
    }

    private HookTypeEvidence partialDirectRegisterTypeProvider(
        Function owner,
        Address registerTypeCallsite) {

        Address typeProvider = typeProviderCandidateBeforeRegisterType(
            owner == null ? null : owner.getEntryPoint(),
            registerTypeCallsite);
        if (typeProvider == null) {
            return null;
        }

        HookTypeEvidence hook = new HookTypeEvidence();
        hook.source = "direct-register-type-partial";
        hook.hookFunction = owner.getEntryPoint();
        hook.registerThunk = owner.getEntryPoint();
        hook.typeProvider = typeProvider;
        return hook;
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
            HookTypeEvidence hook = new HookTypeEvidence();
            hook.typeName = typeName;
            hook.hookFunction = hookFunction;
            hook.helperTable = helperTable;
            hook.registerThunk = registerThunk;
            hook.typeProvider = typeProviderCandidateBeforeRegisterType(registerThunk, null);
            String slotTypeName = registrationHookTypeNameFromSlot(helperTable, 3);
            if (isPlausibleTypeName(slotTypeName)) {
                hook.slotTypeName = slotTypeName;
            }
            return hook;
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

    private Address typeProviderCandidateBeforeRegisterType(
        Address registerThunk,
        Address registerTypeCallsite) {

        Function function = functionAtOrContaining(registerThunk);
        if (function == null) {
            return null;
        }

        List<Instruction> instructions = functionInstructions(function);
        int registerTypeInstruction = -1;
        Address registerType = currentProgram.getImageBase().add(TYPE_REGISTRY_REGISTER_TYPE_RVA);
        for (int i = 0; i < instructions.size(); i++) {
            Instruction instruction = instructions.get(i);
            if (registerTypeCallsite != null && instruction.getMinAddress().equals(registerTypeCallsite)) {
                registerTypeInstruction = i;
                break;
            }
            if (instruction.getFlowType().isCall() &&
                registerType.equals(resolvedCodeTarget(callTarget(instruction)))) {
                registerTypeInstruction = i;
                break;
            }
        }

        if (registerTypeInstruction >= 0) {
            int firstCandidate =
                Math.max(0, registerTypeInstruction - BACKWARD_ARGUMENT_SCAN_LIMIT);
            for (int i = registerTypeInstruction - 1; i >= firstCandidate; i--) {
                Instruction instruction = instructions.get(i);
                if (!instruction.getFlowType().isCall()) {
                    continue;
                }
                Address target = resolvedCodeTarget(callTarget(instruction));
                if (isExecutableAddress(target)) {
                    return target;
                }
            }
        }

        int count = 0;
        for (Instruction instruction : instructions) {
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Address target = resolvedCodeTarget(callTarget(instruction));
            if (isExecutableAddress(target)) {
                return target;
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

            for (Address candidate : referencedAddresses(instruction)) {
                if (isMessageHandlerVtable(candidate)) {
                    return candidate;
                }
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
            for (Address address : referencedAddresses(instruction)) {
                if (isRegistrationHelperTable(address)) {
                    return address;
                }
            }
        }
        return null;
    }

    private Address findRegistrationHelperTable(Function function, Address callsite) {
        List<Instruction> instructions = functionInstructions(function);
        int callIndex = instructionIndexAtOrBefore(instructions, callsite);
        if (callIndex < 0) {
            return findRegistrationHelperTable(function);
        }

        int first = Math.max(0, callIndex - BACKWARD_ARGUMENT_SCAN_LIMIT);
        for (int i = callIndex; i >= first; i--) {
            Instruction instruction = instructions.get(i);
            for (Address address : referencedAddresses(instruction)) {
                if (isRegistrationHelperTable(address)) {
                    return address;
                }
            }
        }

        return null;
    }

    private int instructionIndexAtOrBefore(List<Instruction> instructions, Address address) {
        if (address == null) {
            return -1;
        }
        for (int i = 0; i < instructions.size(); i++) {
            Instruction instruction = instructions.get(i);
            Address start = instruction.getMinAddress();
            Address end = instruction.getMaxAddress();
            if (start.compareTo(address) <= 0 && end.compareTo(address) >= 0) {
                return i;
            }
            if (start.compareTo(address) > 0) {
                break;
            }
        }
        return -1;
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
        if (symbol != null) {
            String typeName = parseInstallRegistrationHookTypeName(symbol.getName(true));
            if (typeName != null) {
                return typeName;
            }
        }

        return parseMsvcInstallRegistrationHookTypeDescriptor(typeDescriptor);
    }

    private String parseMsvcInstallRegistrationHookTypeDescriptor(Address typeDescriptor) {
        if (!isProgramAddress(typeDescriptor)) {
            return null;
        }

        String descriptorName = readPrintableString(typeDescriptor.add(0x10L));
        if (descriptorName == null) {
            return null;
        }

        String marker = "InstallRegistrationHook@";
        int markerIndex = descriptorName.indexOf(marker);
        if (markerIndex < 0) {
            return null;
        }
        int start = markerIndex + marker.length();
        int end = descriptorName.indexOf("@Hub@Amazon@@", start);
        if (end < 0 || end <= start) {
            return null;
        }

        String encoded = descriptorName.substring(start, end);
        if (encoded.startsWith("V") || encoded.startsWith("U")) {
            encoded = encoded.substring(1);
        }
        while (encoded.endsWith("@")) {
            encoded = encoded.substring(0, encoded.length() - 1);
        }
        if (encoded.isEmpty()) {
            return null;
        }

        String[] parts = encoded.split("@");
        ArrayList<String> names = new ArrayList<>();
        for (int i = parts.length - 1; i >= 0; i--) {
            if (!parts[i].isEmpty()) {
                names.add(parts[i]);
            }
        }
        if (names.isEmpty()) {
            return null;
        }

        String typeName = String.join("::", names);
        return isPlausibleTypeName(typeName) ? typeName : null;
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
            TypeIdDecode typeId = decodeDirectTypeIdProvider(target);
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

        LinkedHashMap<String, Address> vtableAddresses = new LinkedHashMap<>();
        LinkedHashMap<String, Integer> fieldCounts = new LinkedHashMap<>();
        for (RegistrationFunction function : registrationFunctions.values()) {
            for (FieldCall field : function.fields) {
                if (field.handlerVtable == null) {
                    continue;
                }
                String key = field.handlerVtable.toString();
                vtableAddresses.putIfAbsent(key, field.handlerVtable);
                fieldCounts.put(key, fieldCounts.getOrDefault(key, 0) + 1);
            }
        }

        JsonArray array = new JsonArray();
        for (Map.Entry<String, Address> entry : vtableAddresses.entrySet()) {
            int fieldCount = fieldCounts.getOrDefault(entry.getKey(), 0);
            array.add(fieldHandlerVtableJson(entry.getValue(), fieldCount));
        }
        return array;
    }

    private JsonObject fieldHandlerVtableJson(Address address, int fieldCount) {
        JsonObject object = new JsonObject();
        object.addProperty("address", formatAddress(address));
        object.addProperty("fieldCount", fieldCount);
        FieldHandlerShape shape = fieldHandlerShape(address);
        if (shape != null) {
            object.addProperty("handlerKind", shape.kind);
            object.addProperty("vtableSlots", shape.vtableSlots);
        }

        JsonArray slots = new JsonArray();
        Address marshal = null;
        Address marshalTarget = null;
        int slotCount = shape == null
            ? FIELD_HANDLER_CONTAINER_VTABLE_SLOTS
            : shape.vtableSlots;
        for (int slot = 0; slot < slotCount; slot++) {
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
                Address unmarshal = target;
                Address unmarshalTarget = terminalJumpTarget(target);
                object.addProperty("unmarshal", formatAddress(unmarshal));
                add(object, "unmarshalTarget", formatAddress(unmarshalTarget));
            }
        }
        ContainerWireShape containerWireShape = shape == null ? null : shape.containerWireShape;
        WireShape wireShape = shape == null ? classifyWireShape(marshal, marshalTarget) : shape.wireShape;
        if (wireShape != null) {
            object.addProperty("wireShape", wireShape.shape);
            object.addProperty("wireShapeSource", wireShape.source);
        }
        if (containerWireShape != null) {
            add(object, "deltaWireShape", containerWireShape.deltaShape);
            add(object, "fullWireShape", containerWireShape.fullShape);
            object.add("deltaMarshalShapes", stringArray(containerWireShape.deltaMarshalShapes));
            object.add("fullMarshalShapes", stringArray(containerWireShape.fullMarshalShapes));
        }
        object.add("vtableDiagnostics", vtableDiagnostics(
            address,
            shape,
            slotCount,
            null));
        object.add("slots", slots);
        return object;
    }

    private FieldHandlerShape fieldHandlerShape(Address vtable) {
        if (!isProgramAddress(vtable)) {
            return null;
        }

        String key = addressCacheKey("field-handler-shape", vtable);
        if (fieldHandlerShapeCache.containsKey(key)) {
            return fieldHandlerShapeCache.get(key);
        }

        FieldHandlerShape result = recoverFieldHandlerShape(vtable);
        fieldHandlerShapeCache.put(key, result);
        return result;
    }

    private FieldHandlerShape recoverFieldHandlerShape(Address vtable) {
        String handlerTypeName = fieldHandlerTypeName(null, vtable);
        NetworkTemplateType handlerType = parseNetworkTemplateType(handlerTypeName);
        WireShape templateWireShape = wireShapeFromHandlerTypeName(handlerTypeName);

        ContainerWireShape container = classifyReplicatedContainerWireShape(vtable);
        if (container != null) {
            return new FieldHandlerShape(
                "replicated-container",
                FIELD_HANDLER_CONTAINER_VTABLE_SLOTS,
                container.primaryShape,
                container);
        }

        if (isReplicatedContainerHandlerType(handlerType)) {
            return new FieldHandlerShape(
                "replicated-container",
                FIELD_HANDLER_CONTAINER_VTABLE_SLOTS,
                templateWireShape,
                null);
        }

        Address marshal = readPointer(vtable.add(FIELD_HANDLER_MARSHAL_SLOT * 8L));
        Address marshalTarget = terminalJumpTarget(marshal);
        WireShape wireShape = classifyWireShape(marshal, marshalTarget);
        if (wireShape == null) {
            wireShape = templateWireShape;
        }
        return new FieldHandlerShape(
            fieldHandlerKindFromType(handlerType),
            FIELD_HANDLER_SCALAR_VTABLE_SLOTS,
            wireShape,
            null);
    }

    private boolean isReplicatedContainerHandlerType(NetworkTemplateType type) {
        return type != null &&
            (type.simpleName.equals("ReplicatedContainer") ||
                type.simpleName.equals("ReplicatedMapFieldHandler") ||
                type.simpleName.equals("ReplicatedVectorFieldHandler") ||
                type.simpleName.equals("ReplicatedSetFieldHandler"));
    }

    private String fieldHandlerKindFromType(NetworkTemplateType type) {
        if (type == null) {
            return "replicated-field";
        }
        if (type.simpleName.equals("DeltaCompressedReplicatedFieldHandler") ||
            type.simpleName.equals("DeltaCompressedReplicatedFieldHandlerBase")) {
            return "delta-compressed-replicated-field";
        }
        if (type.simpleName.equals("DynamicDeltaReplicatedFieldHandler")) {
            return "dynamic-delta-replicated-field";
        }
        if (type.simpleName.equals("FixedReplicatedState")) {
            return "fixed-replicated-state";
        }
        return "replicated-field";
    }

    private String fieldHandlerTypeName(
        HandlerConstruction construction,
        Address vtable) {

        if (construction != null) {
            String typeName = handlerTypeNameFromQualifiedName(construction.constructorName);
            if (typeName != null) {
                return typeName;
            }
        }

        if (isProgramAddress(vtable)) {
            Symbol symbol = currentProgram.getSymbolTable().getPrimarySymbol(vtable);
            if (symbol != null) {
                String typeName = handlerTypeNameFromQualifiedName(symbol.getName(true));
                if (typeName != null) {
                    return typeName;
                }
            }
        }
        return null;
    }

    private String handlerTypeNameFromQualifiedName(String qualifiedName) {
        if (qualifiedName == null || qualifiedName.isBlank()) {
            return null;
        }

        String name = normalizeQualifiedNetworkTypeName(qualifiedName);
        if (name == null) {
            return null;
        }
        int vftableIndex = name.indexOf("::vftable");
        if (vftableIndex > 0) {
            name = name.substring(0, vftableIndex);
        }

        NetworkTemplateType networkType = parseNetworkTemplateType(name);
        if (networkType != null) {
            return networkType.qualifiedName;
        }

        if (name.contains("ReplicatedFieldHandlerBase")) {
            return "MB::ReplicatedFieldHandlerBase";
        }
        if (name.contains("Amazon::Hub::IFragment") || name.endsWith("IFragment")) {
            return "Amazon::Hub::IFragment";
        }
        if (name.contains("Amazon::Hub::IMessage") || name.endsWith("IMessage")) {
            return "Amazon::Hub::IMessage";
        }

        if (!isNetworkTemplateOwnerName(name)) {
            return null;
        }

        int split = name.lastIndexOf("::");
        if (split > 0) {
            String owner = name.substring(0, split);
            String leaf = name.substring(split + 2);
            NetworkTemplateType ownerType = parseNetworkTemplateType(owner);
            if (ownerType != null && !leaf.contains("<")) {
                return ownerType.qualifiedName;
            }
            if (owner.endsWith("<") || leaf.contains("<")) {
                return name;
            }
            int ownerLeaf = owner.lastIndexOf("::");
            String ownerSimple = ownerLeaf < 0 ? owner : owner.substring(ownerLeaf + 2);
            if (leaf.equals(ownerSimple) ||
                "vftable".equals(leaf) ||
                leaf.startsWith("~")) {
                name = owner;
            }
        }

        return isPlausibleTypeName(name) ? name : null;
    }

    private String normalizeQualifiedNetworkTypeName(String value) {
        String result = normalizeNativeType(value);
        if (result == null) {
            return null;
        }
        result = result
            .replace("class ", "")
            .replace("struct ", "")
            .replace("enum ", "")
            .replace("const ", "")
            .replace(" volatile", "")
            .replace("&", "")
            .replace("*", "")
            .trim();
        return normalizeNativeType(result);
    }

    private boolean isNetworkTemplateOwnerName(String name) {
        return name != null &&
            (name.contains("ReplicatedFieldHandler") ||
                name.contains("DeltaCompressedReplicatedFieldHandler") ||
                name.contains("DynamicDeltaReplicatedFieldHandler") ||
                name.contains("ReplicatedContainer") ||
                name.contains("ReplicatedMapFieldHandler") ||
                name.contains("ReplicatedVectorFieldHandler") ||
                name.contains("ReplicatedSetFieldHandler") ||
                name.contains("FixedReplicatedState") ||
                name.contains("ReplicatedStateBundle") ||
                name.contains("IFragment") ||
                name.contains("IMessage"));
    }

    private NetworkTemplateType parseNetworkTemplateType(String qualifiedName) {
        String normalized = normalizeQualifiedNetworkTypeName(qualifiedName);
        if (normalized == null) {
            return null;
        }

        int end = normalized.lastIndexOf('>');
        if (end < 0) {
            return null;
        }
        int start = matchingTemplateStart(normalized, end);
        if (start < 0) {
            return null;
        }

        String owner = normalized.substring(0, start);
        String simple = simpleTypeName(owner);
        if (!isNetworkTemplateSimpleName(simple)) {
            return null;
        }

        String argumentText = normalized.substring(start + 1, end);
        List<String> args = splitTopLevel(argumentText);
        if (args.isEmpty()) {
            return null;
        }

        String qualifiedOwner = owner;
        return new NetworkTemplateType(
            qualifiedOwner + "<" + String.join(",", args) + ">",
            qualifiedOwner,
            simple,
            args);
    }

    private GenericType parseGenericTemplateType(String qualifiedName) {
        String normalized = normalizeQualifiedNetworkTypeName(qualifiedName);
        if (normalized == null) {
            return null;
        }

        int end = normalized.lastIndexOf('>');
        if (end < 0) {
            return null;
        }
        int start = matchingTemplateStart(normalized, end);
        if (start < 0) {
            return null;
        }

        String owner = normalized.substring(0, start);
        String argumentText = normalized.substring(start + 1, end);
        List<String> args = splitTopLevel(argumentText);
        if (args.isEmpty()) {
            return null;
        }
        return new GenericType(
            owner + "<" + String.join(",", args) + ">",
            owner,
            simpleTypeName(owner),
            args);
    }

    private JsonObject foldEvidenceForCandidates(
        String expectedTypeId,
        String expectedKind,
        String... typeNames) {

        LinkedHashSet<String> seen = new LinkedHashSet<>();
        for (String typeName : typeNames) {
            if (typeName == null || !seen.add(typeName)) {
                continue;
            }
            JsonObject evidence = foldEvidenceForTypeName(typeName, expectedTypeId, expectedKind);
            if (evidence != null) {
                return evidence;
            }
        }
        return null;
    }

    private JsonObject foldEvidenceForTypeName(
        String typeName,
        String expectedTypeId,
        String expectedKind) {

        FoldedTypeId folded = foldedTypeIdForTypeName(typeName);
        if (folded == null) {
            return null;
        }

        JsonObject object = new JsonObject();
        add(object, "sourceTypeName", folded.sourceTypeName);
        add(object, "formula", folded.formula);
        add(object, "computedTypeId", folded.typeId);
        JsonArray operands = new JsonArray();
        for (int i = 0; i < folded.operandTypeIds.size(); i++) {
            JsonObject operand = new JsonObject();
            add(operand, "typeName", folded.operandTypeNames.get(i));
            add(operand, "typeId", folded.operandTypeIds.get(i));
            operands.add(operand);
        }
        if (operands.size() != 0) {
            object.add("operands", operands);
        }

        String expected = canonicalUuidFromString(expectedTypeId);
        if (expected != null) {
            add(object, expectedKind + "TypeId", expected);
            object.addProperty(
                "matches" + Character.toUpperCase(expectedKind.charAt(0)) + expectedKind.substring(1),
                uuidEquals(folded.typeId, expected));
        }
        return object;
    }

    private FoldedTypeId foldedTypeIdForTypeName(String typeName) {
        String normalized = normalizeQualifiedNetworkTypeName(typeName);
        if (normalized == null) {
            return null;
        }

        GenericType type = parseGenericTemplateType(normalized);
        if (type == null) {
            return null;
        }

        String simple = type.simpleName;
        List<String> args = type.args;
        if ("less".equals(simple) && args.size() == 1) {
            return postfixFold(type, "AZStd::less<T>", AZSTD_LESS_TYPE_ID, typeIds(args));
        }
        if ("hash".equals(simple) && args.size() == 1) {
            return postfixFold(type, "AZStd::hash<T>", AZSTD_HASH_TYPE_ID, typeIds(args));
        }
        if ("equal_to".equals(simple) && args.size() == 1) {
            return postfixFold(type, "AZStd::equal_to<T>", AZSTD_EQUAL_TO_TYPE_ID, typeIds(args));
        }
        if ("char_traits".equals(simple) && args.size() == 1) {
            return postfixFold(type, "AZStd::char_traits<T>", AZSTD_CHAR_TRAITS_TYPE_ID, typeIds(args));
        }
        if ("basic_string".equals(simple) && args.size() >= 3) {
            return postfixFold(
                type,
                "AZStd::basic_string<C,Traits,Allocator>",
                AZSTD_BASIC_STRING_TYPE_ID,
                typeIds(args.subList(0, 3)));
        }
        if ("pair".equals(simple) && args.size() >= 2) {
            return postfixFold(type, "AZStd::pair<K,V>", AZSTD_PAIR_TYPE_ID, typeIds(args.subList(0, 2)));
        }
        if ("vector".equals(simple) && !args.isEmpty()) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.size() >= 2 ? args.get(1) : AZSTD_ALLOCATOR_TYPE_NAME);
            return postfixFold(type, "AZStd::vector<T,Allocator>", AZSTD_VECTOR_TYPE_ID, typeIds(operands));
        }
        if ("list".equals(simple) && !args.isEmpty()) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.size() >= 2 ? args.get(1) : AZSTD_ALLOCATOR_TYPE_NAME);
            return postfixFold(type, "AZStd::list<T,Allocator>", AZSTD_LIST_TYPE_ID, typeIds(operands));
        }
        if ("forward_list".equals(simple) && !args.isEmpty()) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.size() >= 2 ? args.get(1) : AZSTD_ALLOCATOR_TYPE_NAME);
            return postfixFold(
                type,
                "AZStd::forward_list<T,Allocator>",
                AZSTD_FORWARD_LIST_TYPE_ID,
                typeIds(operands));
        }
        if ("set".equals(simple) && !args.isEmpty()) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.size() >= 2 ? args.get(1) : "AZStd::less<" + args.get(0) + ">");
            operands.add(args.size() >= 3 ? args.get(2) : AZSTD_ALLOCATOR_TYPE_NAME);
            return postfixFold(type, "AZStd::set<K,Compare,Allocator>", AZSTD_SET_TYPE_ID, typeIds(operands));
        }
        if ("unordered_set".equals(simple) && !args.isEmpty()) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.size() >= 2 ? args.get(1) : "AZStd::hash<" + args.get(0) + ">");
            operands.add(args.size() >= 3 ? args.get(2) : "AZStd::equal_to<" + args.get(0) + ">");
            operands.add(args.size() >= 4 ? args.get(3) : AZSTD_ALLOCATOR_TYPE_NAME);
            return postfixFold(
                type,
                "AZStd::unordered_set<K,Hash,Equal,Allocator>",
                AZSTD_UNORDERED_SET_TYPE_ID,
                typeIds(operands));
        }
        if ("map".equals(simple) && args.size() >= 2) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.get(1));
            operands.add(args.size() >= 3 ? args.get(2) : "AZStd::less<" + args.get(0) + ">");
            operands.add(args.size() >= 4 ? args.get(3) : AZSTD_ALLOCATOR_TYPE_NAME);
            return postfixFold(type, "AZStd::map<K,V,Compare,Allocator>", AZSTD_MAP_TYPE_ID, typeIds(operands));
        }
        if (("unordered_map".equals(simple) || "unordered_flat_map".equals(simple)) && args.size() >= 2) {
            ArrayList<String> operands = new ArrayList<>();
            operands.add(args.get(0));
            operands.add(args.get(1));
            operands.add(args.size() >= 3 ? args.get(2) : "AZStd::hash<" + args.get(0) + ">");
            operands.add(args.size() >= 4 ? args.get(3) : "AZStd::equal_to<" + args.get(0) + ">");
            operands.add(args.size() >= 5 ? args.get(4) : AZSTD_ALLOCATOR_TYPE_NAME);
            String baseTypeId = "unordered_flat_map".equals(simple)
                ? AZSTD_UNORDERED_FLAT_MAP_TYPE_ID
                : AZSTD_UNORDERED_MAP_TYPE_ID;
            return postfixFold(
                type,
                "AZStd::" + simple + "<K,V,Hash,Equal,Allocator>",
                baseTypeId,
                typeIds(operands));
        }
        if ("shared_ptr".equals(simple) && !args.isEmpty()) {
            return postfixFold(type, "AZStd::shared_ptr<T>", AZSTD_SHARED_PTR_TYPE_ID, typeIds(args.subList(0, 1)));
        }
        if ("intrusive_ptr".equals(simple) && !args.isEmpty()) {
            return postfixFold(
                type,
                "AZStd::intrusive_ptr<T>",
                AZSTD_INTRUSIVE_PTR_TYPE_ID,
                typeIds(args.subList(0, 1)));
        }
        if ("unique_ptr".equals(simple) && !args.isEmpty()) {
            return postfixFold(type, "AZStd::unique_ptr<T>", AZSTD_UNIQUE_PTR_TYPE_ID, typeIds(args.subList(0, 1)));
        }
        if ("optional".equals(simple) && !args.isEmpty()) {
            return postfixFold(type, "AZStd::optional<T>", AZSTD_OPTIONAL_TYPE_ID, typeIds(args.subList(0, 1)));
        }
        if ("fixed_vector".equals(simple) && args.size() >= 2) {
            String capacity = templateAutoTypeId(args.get(1));
            if (capacity == null) {
                return null;
            }
            return postfixFold(
                type,
                "AZStd::fixed_vector<T,Capacity>",
                AZSTD_FIXED_VECTOR_TYPE_ID,
                typeIdsWithAuto(args.get(0), args.get(1), capacity));
        }
        if ("array".equals(simple) && args.size() >= 2) {
            String size = templateAutoTypeId(args.get(1));
            if (size == null) {
                return null;
            }
            return postfixFold(
                type,
                "AZStd::array<T,Size>",
                AZSTD_ARRAY_TYPE_ID,
                typeIdsWithAuto(args.get(0), args.get(1), size));
        }
        if ("bitset".equals(simple) && !args.isEmpty()) {
            String bits = templateAutoTypeId(args.get(0));
            if (bits == null) {
                return null;
            }
            return postfixFold(
                type,
                "AZStd::bitset<Bits>",
                AZSTD_BITSET_TYPE_ID,
                new TypeIdOperands(
                    List.of(args.get(0)),
                    List.of(bits)));
        }
        if ("tuple".equals(simple)) {
            TypeIdOperands operands = typeIds(args);
            if (operands == null || operands.typeIds.isEmpty()) {
                return null;
            }
            String aggregate = aggregateTypeIdsRight(operands.typeIds);
            if (aggregate == null) {
                return null;
            }
            return new FoldedTypeId(
                type.qualifiedName,
                "AZStd::tuple<T...>",
                combineTypeIds(AZSTD_TUPLE_TYPE_ID, aggregate),
                operands.typeNames,
                operands.typeIds);
        }
        if ("Asset".equals(simple) && type.ownerName.endsWith("Data::Asset") && !args.isEmpty()) {
            return prefixFold(type, "AZ::Data::Asset<T>", AZ_DATA_ASSET_TYPE_ID, typeIds(args.subList(0, 1)));
        }
        if ("RValueToLValueWrapper".equals(simple) && !args.isEmpty()) {
            return postfixFold(
                type,
                "AZ::Internal::RValueToLValueWrapper<T>",
                AZ_INTERNAL_RVALUE_TO_LVALUE_WRAPPER_TYPE_ID,
                typeIds(args.subList(0, 1)));
        }
        if ("ReplicatedField".equals(simple) && !args.isEmpty()) {
            return prefixFold(type, "MB::ReplicatedField<T>", MB_REPLICATED_FIELD_TYPE_ID, typeIds(args.subList(0, 1)));
        }
        if ("UID".equals(simple) && type.ownerName.endsWith("Pervasives::UID") && !args.isEmpty()) {
            String bits = templateAutoTypeId(args.get(0));
            if (bits == null) {
                return null;
            }
            return new FoldedTypeId(
                type.qualifiedName,
                "Amazon::Pervasives::UID<Bits>",
                combineTypeIds(bits, AMAZON_PERVASIVES_UID_TYPE_ID),
                List.of(args.get(0)),
                List.of(bits));
        }

        return null;
    }

    private FoldedTypeId prefixFold(
        GenericType type,
        String formula,
        String templateBase,
        TypeIdOperands operands) {

        if (operands == null || operands.typeIds.isEmpty()) {
            return null;
        }
        String aggregate = aggregateTypeIds(operands.typeIds);
        if (aggregate == null) {
            return null;
        }
        return new FoldedTypeId(
            type.qualifiedName,
            formula,
            combineTypeIds(templateBase, aggregate),
            operands.typeNames,
            operands.typeIds);
    }

    private FoldedTypeId postfixFold(
        GenericType type,
        String formula,
        String templateBase,
        TypeIdOperands operands) {

        if (operands == null || operands.typeIds.isEmpty()) {
            return null;
        }
        String aggregate = aggregateTypeIds(operands.typeIds);
        if (aggregate == null) {
            return null;
        }
        return new FoldedTypeId(
            type.qualifiedName,
            formula,
            combineTypeIds(aggregate, templateBase),
            operands.typeNames,
            operands.typeIds);
    }

    private TypeIdOperands typeIds(List<String> typeNames) {
        ArrayList<String> names = new ArrayList<>();
        ArrayList<String> ids = new ArrayList<>();
        for (String typeName : typeNames) {
            String typeId = typeIdForTypeName(typeName);
            if (typeId == null) {
                return null;
            }
            names.add(typeName);
            ids.add(typeId);
        }
        return new TypeIdOperands(names, ids);
    }

    private TypeIdOperands typeIdsWithAuto(String valueTypeName, String autoName, String autoTypeId) {
        String valueTypeId = typeIdForTypeName(valueTypeName);
        if (valueTypeId == null) {
            return null;
        }
        return new TypeIdOperands(
            List.of(valueTypeName, autoName),
            List.of(valueTypeId, autoTypeId));
    }

    private String typeIdForTypeName(String typeName) {
        String direct = directTypeIdForTypeName(typeName);
        if (direct != null) {
            return direct;
        }
        FoldedTypeId folded = foldedTypeIdForTypeName(typeName);
        return folded == null ? null : folded.typeId;
    }

    private String directTypeIdForTypeName(String typeName) {
        String normalized = normalizeQualifiedNetworkTypeName(typeName);
        if (normalized == null) {
            return null;
        }
        normalized = normalized
            .replace("std::string", "AZStd::string")
            .replace("std::allocator", "AZStd::allocator")
            .replace("AZStd::_", "AZStd::");
        SerializeTypeInfo reflected = serializeTypeForTypeName(normalized);
        if (reflected != null && reflected.typeId != null) {
            return reflected.typeId;
        }
        return switch (normalized) {
            case "char" -> CHAR_TYPE_ID;
            case "signed char", "i8", "int8_t", "std::int8_t", "AZ::s8", "AZ::sbyte" -> S8_TYPE_ID;
            case "unsigned char", "u8", "uint8_t", "std::uint8_t", "AZ::u8", "AZ::byte" -> U8_TYPE_ID;
            case "short", "i16", "int16_t", "std::int16_t", "AZ::s16" -> SHORT_TYPE_ID;
            case "unsigned short", "u16", "uint16_t", "std::uint16_t", "AZ::u16" -> U16_TYPE_ID;
            case "int", "i32", "int32_t", "std::int32_t", "AZ::s32" -> INT_TYPE_ID;
            case "unsigned int", "u32", "uint32_t", "std::uint32_t", "AZ::u32" -> U32_TYPE_ID;
            case "long", "AZ::long" -> LONG_TYPE_ID;
            case "unsigned long", "AZ::ulong" -> ULONG_TYPE_ID;
            case "i64", "int64_t", "std::int64_t", "AZ::s64" -> S64_TYPE_ID;
            case "u64", "uint64_t", "std::uint64_t", "AZ::u64" -> U64_TYPE_ID;
            case "float", "f32" -> FLOAT_TYPE_ID;
            case "double", "f64" -> DOUBLE_TYPE_ID;
            case "bool" -> BOOL_TYPE_ID;
            case "AZ::Uuid", "AZ::TypeId" -> AZ_UUID_TYPE_ID;
            case "AZ::EntityId" -> ENTITY_ID_TYPE_ID;
            case "AZ::Crc32" -> CRC32_TYPE_ID;
            case "AZ::Vector2" -> VECTOR2_TYPE_ID;
            case "AZ::Vector3" -> VECTOR3_TYPE_ID;
            case "AZ::Vector4" -> VECTOR4_TYPE_ID;
            case "AZ::Transform" -> TRANSFORM_TYPE_ID;
            case "AZ::Quaternion" -> QUATERNION_TYPE_ID;
            case "AZ::Color" -> COLOR_TYPE_ID;
            case "AZ::ColorF" -> COLORF_TYPE_ID;
            case "AZ::ColorB" -> COLORB_TYPE_ID;
            case "AZ::Aabb" -> AABB_TYPE_ID;
            case "AZStd::allocator" -> AZSTD_ALLOCATOR_TYPE_ID;
            case "AZStd::string", "string" -> AZSTD_STRING_TYPE_ID;
            case "AZ::Data::AssetId" -> AZ_DATA_ASSET_ID_TYPE_ID;
            default -> canonicalUuidFromString(normalized);
        };
    }

    private SerializeTypeInfo serializeTypeForTypeName(String typeName) {
        if (typeName == null || typeName.trim().isEmpty()) {
            return null;
        }
        String normalized = typeName.trim();
        List<SerializeTypeInfo> exact = serializeTypesByName.get(normalized);
        if (exact != null && exact.size() == 1) {
            return exact.get(0);
        }
        List<SerializeTypeInfo> leaf = serializeTypesByLeafName.get(sourceTypeLeaf(normalized));
        if (leaf != null && leaf.size() == 1) {
            return leaf.get(0);
        }
        return null;
    }

    private String templateAutoTypeId(String value) {
        if (value == null) {
            return null;
        }
        try {
            long parsed = Long.parseUnsignedLong(value.trim());
            return createData(Long.toUnsignedString(parsed).getBytes(StandardCharsets.US_ASCII));
        }
        catch (NumberFormatException ignored) {
            return null;
        }
    }

    private String aggregateTypeIds(List<String> typeIds) {
        if (typeIds == null || typeIds.isEmpty()) {
            return null;
        }
        String acc = typeIds.get(0);
        for (int i = 1; i < typeIds.size(); i++) {
            acc = combineTypeIds(acc, typeIds.get(i));
        }
        return acc;
    }

    private String aggregateTypeIdsRight(List<String> typeIds) {
        if (typeIds == null || typeIds.isEmpty()) {
            return null;
        }
        String acc = typeIds.get(typeIds.size() - 1);
        for (int i = typeIds.size() - 2; i >= 0; i--) {
            acc = combineTypeIds(typeIds.get(i), acc);
        }
        return acc;
    }

    private String combineTypeIds(String lhs, String rhs) {
        byte[] lhsBytes = uuidBytes(lhs);
        byte[] rhsBytes = uuidBytes(rhs);
        if (lhsBytes == null || rhsBytes == null) {
            return null;
        }
        byte[] bytes = new byte[32];
        System.arraycopy(lhsBytes, 0, bytes, 0, 16);
        System.arraycopy(rhsBytes, 0, bytes, 16, 16);
        return createData(bytes);
    }

    private String createData(byte[] bytes) {
        if (bytes == null || bytes.length == 0) {
            return NIL_TYPE_ID;
        }
        try {
            byte[] digest = MessageDigest.getInstance("SHA-1").digest(bytes);
            byte[] data = new byte[16];
            System.arraycopy(digest, 0, data, 0, data.length);
            data[8] = (byte)((data[8] & 0xbf) | 0x80);
            data[6] = (byte)((data[6] & 0x5f) | 0x50);
            return uuidFromBytes(data);
        }
        catch (Exception exception) {
            throw new IllegalStateException("SHA-1 is unavailable", exception);
        }
    }

    private byte[] uuidBytes(String uuid) {
        String normalized = normalizeUuid(uuid);
        if (normalized == null) {
            return null;
        }
        String hex = normalized.replace("-", "");
        if (hex.length() != 32) {
            return null;
        }
        byte[] bytes = new byte[16];
        for (int i = 0; i < bytes.length; i++) {
            int value = Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
            bytes[i] = (byte)value;
        }
        return bytes;
    }

    private String uuidFromBytes(byte[] bytes) {
        StringBuilder builder = new StringBuilder(36);
        for (int i = 0; i < bytes.length; i++) {
            if (i == 4 || i == 6 || i == 8 || i == 10) {
                builder.append('-');
            }
            int value = bytes[i] & 0xff;
            builder.append(Character.toUpperCase(Character.forDigit(value >>> 4, 16)));
            builder.append(Character.toUpperCase(Character.forDigit(value & 0xf, 16)));
        }
        return builder.toString();
    }

    private int matchingTemplateStart(String value, int end) {
        int depth = 0;
        for (int i = end; i >= 0; i--) {
            char c = value.charAt(i);
            if (c == '>') {
                depth++;
            }
            else if (c == '<') {
                depth--;
                if (depth == 0) {
                    return i;
                }
            }
        }
        return -1;
    }

    private boolean isNetworkTemplateSimpleName(String simple) {
        return simple != null &&
            (simple.equals("ReplicatedFieldHandler") ||
                simple.equals("DeltaCompressedReplicatedFieldHandler") ||
                simple.equals("DeltaCompressedReplicatedFieldHandlerBase") ||
                simple.equals("DynamicDeltaReplicatedFieldHandler") ||
                simple.equals("ReplicatedContainer") ||
                simple.equals("ReplicatedMapFieldHandler") ||
                simple.equals("ReplicatedVectorFieldHandler") ||
                simple.equals("ReplicatedSetFieldHandler") ||
                simple.equals("FixedReplicatedState"));
    }

    private List<Address> unmarshalCallTargets(RegistryEntry entry) {
        ArrayList<Address> result = new ArrayList<>();
        Address unmarshalAddress = parseCapturedAddress(entry.unmarshal);
        Function unmarshal = functionAtOrContaining(unmarshalAddress);
        if (unmarshal == null) {
            return result;
        }
        result.add(unmarshal.getEntryPoint());
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
        recoverPcodeMessageFields(plan, wrapper);
        recoverInlineMessageFields(plan, wrapper.getEntryPoint(), wrapperText);
        recoverHelperArgumentMessageFields(plan, wrapper, wrapperText);
        recoverDirectObjectStoreMessageFields(plan, wrapper.getEntryPoint(), wrapperText);
        if (plan.fields.isEmpty()) {
            return null;
        }
        sortMessageFieldsByRecoveryOrder(plan);
        enrichMessageFieldsFromSourceSignatures(entry, plan);
        return plan;
    }

    private void recoverPcodeMessageFields(MessageUnmarshalPlan plan, Function wrapper) {
        HighFunction high = highFunction(wrapper);
        if (high == null) {
            return;
        }

        Set<String> messageBases = pcodeMessageStorageBases(high);
        HashMap<String, String> tempNativeTypes = new HashMap<>();
        int recoveryOrder = 0;
        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOpAST op = ops.next();
            if (op.getOpcode() != PcodeOp.CALL) {
                if (op.getOpcode() == PcodeOp.STORE) {
                    recoverPcodeMessageStore(plan, op, messageBases, tempNativeTypes, recoveryOrder++);
                }
                continue;
            }

            PcodeCallTargetInfo targetInfo = pcodeCallTargetInfo(op);
            Function target = targetInfo == null ? null : targetInfo.target;
            if (target == null) {
                recordPcodeMessageReject(
                    plan,
                    "pcode-call",
                    "call-target-unresolved",
                    op,
                    targetInfo,
                    null,
                    null);
                continue;
            }
            String nativeType = unmarshalNativeTypeFromTarget(target);
            if (nativeType == null) {
                recordPcodeMessageReject(
                    plan,
                    "pcode-call",
                    "target-not-unmarshal-named",
                    op,
                    targetInfo,
                    null,
                    null);
                continue;
            }
            observePcodeTempUnmarshal(tempNativeTypes, op, nativeType);

            PcodeArgStorageSelection storageSelection =
                pcodeStorageArgumentEvidence(op, messageBases);
            PcodeStorage storage = storageSelection == null ? null : storageSelection.storage;
            if (storage == null) {
                recordPcodeMessageReject(
                    plan,
                    "pcode-call",
                    "no-storage-arg-matched",
                    op,
                    targetInfo,
                    null,
                    storageSelection == null ? null : storageSelection.argStorageEvidence);
                continue;
            }
            String storageExpression = storage.expression();
            if (!isLikelyMessageStorage(storageExpression)) {
                recordPcodeMessageReject(
                    plan,
                    "pcode-call",
                    "storage-not-message-shaped",
                    op,
                    targetInfo,
                    storage,
                    storageSelection.argStorageEvidence);
                continue;
            }

            pcodeMessageFieldCandidateCount++;
            int before = plan.fields.size();
            FieldCall field = addPcodeMessageField(
                plan,
                op.getSeqnum().getTarget(),
                storage,
                nativeType,
                "message-unmarshal-pcode-call",
                recoveryOrder++);
            applyPcodeUnmarshalEvidence(
                field,
                pcodeUnmarshalEvidence(
                    op,
                    targetInfo,
                    "direct-unmarshal",
                    storage,
                    storageSelection.storageArgSlot,
                    "message-unmarshal-pcode-call",
                    storageSelection.argStorageEvidence),
                plan.fields.size() > before);
            attachNestedDirectTypeShape(field, target, nativeType);
            if (field != null && plan.fields.size() > before) {
                pcodeMessageFieldAcceptedCount++;
            }
        }
    }

    private void recoverPcodeMessageStore(
        MessageUnmarshalPlan plan,
        PcodeOp op,
        Set<String> messageBases,
        Map<String, String> tempNativeTypes,
        int recoveryOrder) {

        if (op == null || op.getOpcode() != PcodeOp.STORE || op.getNumInputs() < 3) {
            recordPcodeMessageReject(
                plan,
                "pcode-store",
                "store-malformed",
                op,
                null,
                null,
                null);
            return;
        }
        PcodeStorage storage = pcodeStorageExpression(op.getInput(1));
        if (!isPcodeMessageStorage(storage, messageBases)) {
            recordPcodeMessageReject(
                plan,
                "pcode-store",
                "store-dest-not-message-base",
                op,
                null,
                storage,
                null);
            return;
        }
        String nativeType = pcodeValueNativeType(op.getInput(2), tempNativeTypes);
        if (nativeType == null) {
            recordPcodeMessageReject(
                plan,
                "pcode-store",
                "store-value-type-unknown",
                op,
                pcodeValueCallTargetInfo(op.getInput(2)),
                storage,
                null);
            return;
        }

        pcodeMessageFieldCandidateCount++;
        int before = plan.fields.size();
        FieldCall field = addPcodeMessageField(
            plan,
            op.getSeqnum().getTarget(),
            storage,
                nativeType,
                "message-unmarshal-pcode-store",
                recoveryOrder);
        applyPcodeUnmarshalEvidence(
            field,
            pcodeUnmarshalEvidence(
                op,
                pcodeValueCallTargetInfo(op.getInput(2)),
                "store-value-call",
                storage,
                null,
                "message-unmarshal-pcode-store",
                null),
            plan.fields.size() > before);
        if (field != null && plan.fields.size() > before) {
            pcodeMessageFieldAcceptedCount++;
        }
    }

    private void observePcodeTempUnmarshal(
        Map<String, String> tempNativeTypes,
        PcodeOp op,
        String nativeType) {

        observePcodeTempUnmarshal(tempNativeTypes, null, op, nativeType, null);
    }

    private void observePcodeTempUnmarshal(
        Map<String, String> tempNativeTypes,
        Map<String, String> tempWireShapes,
        PcodeOp op,
        String nativeType,
        String wireShape) {

        if (tempNativeTypes == null || nativeType == null || nativeType.isEmpty() ||
            op == null || op.getOpcode() != PcodeOp.CALL || op.getNumInputs() < 3) {
            return;
        }
        PcodeStorage storage = pcodePreferredUnmarshalOutputStorage(op);
        if (!isPcodeLocalTempStorage(storage)) {
            return;
        }
        String key = storageKey(storage);
        tempNativeTypes.put(key, nativeType);
        if (tempWireShapes != null && wireShape != null && !wireShape.isEmpty()) {
            tempWireShapes.put(key, wireShape);
        }
    }

    private PcodeStorage pcodePreferredUnmarshalOutputStorage(PcodeOp op) {
        if (op == null || op.getNumInputs() < 3) {
            return null;
        }
        return pcodeStorageExpression(op.getInput(op.getNumInputs() - 2));
    }

    private PcodeArgStorageSelection pcodeStorageArgumentEvidence(
        PcodeOp op,
        Set<String> messageBases) {

        if (op == null || op.getNumInputs() < 2) {
            return null;
        }

        PcodeArgStorageSelection selection = new PcodeArgStorageSelection();
        int preferred = op.getNumInputs() >= 3 ? op.getNumInputs() - 2 : -1;
        for (int i = 1; i < op.getNumInputs(); i++) {
            PcodeStorage storage = pcodeStorageExpression(op.getInput(i));
            JsonObject evidence = new JsonObject();
            evidence.addProperty("slot", i);
            if (storage != null) {
                add(evidence, "base", storage.base);
                evidence.addProperty("offset", "0x" + Long.toHexString(storage.offset));
                add(evidence, "expression", storage.expression());
            }
            boolean isMessageStorage = isPcodeMessageStorage(storage, messageBases);
            evidence.addProperty("isMessageStorage", isMessageStorage);
            evidence.addProperty("preferredOutputSlot", i == preferred);
            selection.argStorageEvidence.add(evidence);
            if (isMessageStorage && i == preferred) {
                selection.storage = storage;
                selection.storageArgSlot = i;
                selection.selectionRule = "preferred-output-slot";
            }
            else if (isMessageStorage && selection.fallbackStorage == null) {
                selection.fallbackStorage = storage;
                selection.fallbackStorageArgSlot = i;
            }
        }
        if (selection.storage == null && selection.fallbackStorage != null) {
            selection.storage = selection.fallbackStorage;
            selection.storageArgSlot = selection.fallbackStorageArgSlot;
            selection.selectionRule = "first-message-storage-fallback";
        }
        if (selection.selectionRule != null) {
            JsonObject selected = new JsonObject();
            selected.addProperty("selectionRule", selection.selectionRule);
            selected.addProperty("slot", selection.storageArgSlot);
            selection.argStorageEvidence.add(selected);
        }
        return selection;
    }

    private PcodeStorage pcodeStorageArgument(
        PcodeOp op,
        Function target,
        Set<String> messageBases) {

        if (op == null || target == null || op.getNumInputs() < 3) {
            return null;
        }

        int preferred = op.getNumInputs() - 2;
        PcodeStorage storage = pcodeStorageExpression(op.getInput(preferred));
        if (isPcodeMessageStorage(storage, messageBases)) {
            return storage;
        }

        for (int i = 1; i < op.getNumInputs(); i++) {
            storage = pcodeStorageExpression(op.getInput(i));
            if (isPcodeMessageStorage(storage, messageBases)) {
                return storage;
            }
        }
        return null;
    }

    private void attachNestedDirectTypeShape(
        FieldCall field,
        Function target,
        String nativeType) {

        if (field == null || target == null || nativeType == null) {
            return;
        }
        NestedTypeShape shape = recoverNestedDirectTypeShape(target, nativeType);
        if (shape != null) {
            field.nestedTypeShape = shape;
        }
    }

    private NestedTypeShape recoverNestedDirectTypeShape(
        Function target,
        String nativeType) {

        return recoverNestedDirectTypeShape(target, nativeType, false);
    }

    private NestedTypeShape recoverNestedDirectTypeShape(
        Function target,
        String nativeType,
        boolean allowAnonymousHelper) {

        if (target == null || (!allowAnonymousHelper && nativeType == null)) {
            return null;
        }
        String key = functionCacheKey(
            "nested-direct-type-shape:" + allowAnonymousHelper + ":" + nativeType,
            target);
        if (nestedTypeShapeCache.containsKey(key)) {
            return nestedTypeShapeCache.get(key);
        }

        NestedTypeShape shape = recoverNestedDirectTypeShapeUncached(
            target,
            nativeType,
            new LinkedHashSet<>(),
            0,
            allowAnonymousHelper);
        nestedTypeShapeCache.put(key, shape);
        if (shape == null) {
            nestedTypeShapeFailures++;
        }
        else {
            nestedTypeShapesRecovered++;
        }
        return shape;
    }

    private NestedTypeShape recoverNestedDirectTypeShapeUncached(
        Function target,
        String nativeType,
        Set<String> seen,
        int depth,
        boolean allowAnonymousHelper) {

        if (target == null || depth > NESTED_DIRECT_TYPE_DEPTH_LIMIT) {
            recordNestedTypeShapeReject("depth-limit");
            return null;
        }
        String seenKey = target.getEntryPoint() + "|" + nativeType;
        if (!seen.add(seenKey)) {
            recordNestedTypeShapeReject("cycle");
            return null;
        }

        String targetName = fullFunctionName(target);
        String owner = directUnmarshalOwnerFullName(targetName);
        if (owner == null) {
            owner = directUnmarshalOwnerFullNameFromPrototype(target);
        }
        if (owner == null) {
            owner = nativeType;
        }
        if (owner == null && !allowAnonymousHelper) {
            recordNestedTypeShapeReject("target-not-direct-unmarshal");
            return null;
        }
        HighFunction high = highFunction(target);
        if (high == null) {
            recordNestedTypeShapeReject("decompile-failed");
            return null;
        }

        LinkedHashMap<String, LinkedHashMap<Long, NestedTypeMember>> byBase =
            new LinkedHashMap<>();
        HashMap<String, String> tempNativeTypes = new HashMap<>();
        HashMap<String, String> tempWireShapes = new HashMap<>();
        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOpAST op = ops.next();
            if (op.getOpcode() == PcodeOp.CALL) {
                Function callTarget = pcodeCallTarget(op);
                String memberType = unmarshalNativeTypeFromTarget(callTarget);
                String memberWireShape = wireShapeFromNativeType(memberType);
                String evidenceSource = "pcode-call";
                if (memberWireShape == null) {
                    String scalarType = scalarOutputStoreNativeType(callTarget);
                    if (memberType == null) {
                        memberType = scalarType;
                    }
                    memberWireShape = wireShapeFromNativeType(scalarType);
                    evidenceSource = "pcode-call-scalar-output-store";
                }
                observePcodeTempUnmarshal(
                    tempNativeTypes,
                    tempWireShapes,
                    op,
                    memberType,
                    memberWireShape);
                PcodeStorage storage = pcodePreferredUnmarshalOutputStorage(op);
                addNestedTypeMemberCandidate(
                    byBase,
                    storage,
                    memberType,
                    memberWireShape,
                    op,
                    callTarget,
                    evidenceSource);
                continue;
            }
            if (op.getOpcode() != PcodeOp.STORE || op.getNumInputs() < 3) {
                continue;
            }
            PcodeStorage storage = pcodeStorageExpression(op.getInput(1));
            String memberType = pcodeValueNativeType(op.getInput(2), tempNativeTypes);
            String memberWireShape =
                pcodeValueWireShape(op.getInput(2), tempWireShapes, tempNativeTypes);
            addNestedTypeMemberCandidate(
                byBase,
                storage,
                memberType,
                memberWireShape,
                op,
                pcodeValueCallTargetInfo(op.getInput(2)) == null
                    ? null
                    : pcodeValueCallTargetInfo(op.getInput(2)).target,
                "pcode-store");
        }

        String leaf = sourceTypeLeaf(owner);
        if ("ActorRequestId".equals(leaf)) {
            return actorRequestIdShapeFromCandidates(owner, target, targetName, byBase);
        }
        return directTypeShapeFromCandidates(
            owner,
            target,
            targetName,
            byBase,
            allowAnonymousHelper && owner == null);
    }

    private void addNestedTypeMemberCandidate(
        Map<String, LinkedHashMap<Long, NestedTypeMember>> byBase,
        PcodeStorage storage,
        String nativeType,
        String wireShape,
        PcodeOp op,
        Function target,
        String source) {

        if (byBase == null || storage == null || nativeType == null ||
            wireShape == null || isPcodeLocalTempStorage(storage)) {
            return;
        }
        LinkedHashMap<Long, NestedTypeMember> members =
            byBase.computeIfAbsent(storage.base, ignored -> new LinkedHashMap<>());
        if (members.size() >= NESTED_DIRECT_TYPE_MEMBER_LIMIT) {
            return;
        }
        NestedTypeMember existing = members.get(storage.offset);
        if (existing != null) {
            if (!existing.nativeType.equals(nativeType)) {
                existing.typeConflict = true;
            }
            return;
        }

        NestedTypeMember member = new NestedTypeMember();
        member.offset = storage.offset;
        member.nativeType = nativeType;
        member.wireShape = wireShape;
        member.byteWidth = nativeTypeByteWidth(nativeType);
        if (member.byteWidth == null) {
            member.byteWidth = wireShapeByteWidth(wireShape);
        }
        member.name = "_" + members.size();
        member.nameSource = "synthetic-offset";
        member.nameProven = false;
        member.evidenceSource = source;
        member.callsite = op == null ? null : op.getSeqnum().getTarget();
        member.target = target == null ? null : target.getEntryPoint();
        member.targetName = fullFunctionName(target);
        members.put(storage.offset, member);
    }

    private NestedTypeShape directTypeShapeFromCandidates(
        String owner,
        Function target,
        String targetName,
        Map<String, LinkedHashMap<Long, NestedTypeMember>> byBase,
        boolean anonymousHelper) {

        NestedTypeShape selected = null;
        int matches = 0;
        for (Map.Entry<String, LinkedHashMap<Long, NestedTypeMember>> entry : byBase.entrySet()) {
            ArrayList<NestedTypeMember> members =
                new ArrayList<>(entry.getValue().values());
            members.sort((left, right) -> Long.compare(left.offset, right.offset));
            if (!isGenericDirectTypeMemberShape(members)) {
                continue;
            }
            matches++;

            for (int i = 0; i < members.size(); i++) {
                NestedTypeMember member = members.get(i);
                member.index = i;
                member.name = "_" + i;
            }

            NestedTypeShape shape = new NestedTypeShape();
            shape.typeName = sourceTypeLeaf(owner);
            shape.typeNameFull = owner;
            shape.typeNameSource = anonymousHelper ? "anonymous-helper" : "ghidra-symbol";
            shape.function = target.getEntryPoint();
            shape.functionName = targetName;
            applySerializeIdentity(shape, owner);
            shape.memberBase = entry.getKey();
            shape.memberNameSource = "synthetic-offset";
            shape.memberNamesProven = false;
            shape.validation = "layout-consistent-direct-type";
            shape.members.addAll(members);
            applyNestedTypeMemberNames(shape);
            selected = shape;
        }
        if (matches > 1) {
            recordNestedTypeShapeReject("ambiguous-member-base");
            return null;
        }
        if (selected == null) {
            recordNestedTypeShapeReject("direct-type-shape-mismatch");
        }
        return selected;
    }

    private boolean isGenericDirectTypeMemberShape(List<NestedTypeMember> members) {
        if (members == null || members.size() < 2 ||
            members.size() > NESTED_DIRECT_TYPE_MEMBER_LIMIT) {
            return false;
        }
        long end = 0;
        for (int i = 0; i < members.size(); i++) {
            NestedTypeMember member = members.get(i);
            if (member == null ||
                Boolean.TRUE.equals(member.typeConflict) ||
                member.byteWidth == null ||
                member.byteWidth <= 0 ||
                member.offset < 0 ||
                member.offset < end) {
                return false;
            }
            if (i == 0 && member.offset != 0L) {
                return false;
            }
            end = member.offset + member.byteWidth;
            if (end > 0x400) {
                return false;
            }
        }
        return true;
    }

    private NestedTypeShape actorRequestIdShapeFromCandidates(
        String owner,
        Function target,
        String targetName,
        Map<String, LinkedHashMap<Long, NestedTypeMember>> byBase) {

        NestedTypeShape selected = null;
        int matches = 0;
        for (Map.Entry<String, LinkedHashMap<Long, NestedTypeMember>> entry : byBase.entrySet()) {
            ArrayList<NestedTypeMember> members =
                new ArrayList<>(entry.getValue().values());
            members.sort((left, right) -> Long.compare(left.offset, right.offset));
            if (!isActorRequestIdMemberShape(members)) {
                continue;
            }
            matches++;
            normalizeActorRequestIdMemberOffsets(members);

            for (int i = 0; i < members.size(); i++) {
                members.get(i).index = i;
                members.get(i).name = "_" + i;
            }

            NestedTypeShape shape = new NestedTypeShape();
            shape.typeName = sourceTypeLeaf(owner);
            shape.typeNameFull = owner;
            shape.typeNameSource = "ghidra-symbol";
            shape.function = target.getEntryPoint();
            shape.functionName = targetName;
            applySerializeIdentity(shape, owner);
            shape.memberBase = entry.getKey();
            shape.memberNameSource = "synthetic-offset";
            shape.memberNamesProven = false;
            shape.validation = "layout-consistent-two-u64";
            shape.members.addAll(members);
            applyNestedTypeMemberNames(shape);
            applyActorRequestIdNativeMemberNames(shape);
            selected = shape;
        }
        if (matches > 1) {
            recordNestedTypeShapeReject("ambiguous-member-base");
            return null;
        }
        if (selected == null) {
            recordNestedTypeShapeReject("actor-request-id-shape-mismatch");
        }
        return selected;
    }

    private void applySerializeIdentity(NestedTypeShape shape, String typeName) {
        if (shape == null || typeName == null) {
            return;
        }
        if (isWireNativeSourceTypeName(typeName)) {
            return;
        }
        SerializeTypeInfo reflected = serializeTypeForTypeName(typeName);
        if (reflected == null) {
            String typeId = typeIdForTypeName(typeName);
            if (typeId != null && !isBuiltinTypeId(typeId)) {
                shape.typeId = typeId;
                shape.typeIdSource = "az-type-id-fold";
            }
            return;
        }
        shape.typeId = reflected.typeId;
        shape.typeIdSource = "serialize-context-name";
        shape.factory = reflected.factory;
        shape.azRttiAddress = reflected.azRttiAddress;
    }

    private boolean isBuiltinTypeId(String typeId) {
        String normalized = normalizeUuid(typeId);
        if (normalized == null) {
            return false;
        }
        return uuidEquals(normalized, CHAR_TYPE_ID) ||
            uuidEquals(normalized, S8_TYPE_ID) ||
            uuidEquals(normalized, SHORT_TYPE_ID) ||
            uuidEquals(normalized, INT_TYPE_ID) ||
            uuidEquals(normalized, LONG_TYPE_ID) ||
            uuidEquals(normalized, S64_TYPE_ID) ||
            uuidEquals(normalized, U8_TYPE_ID) ||
            uuidEquals(normalized, U16_TYPE_ID) ||
            uuidEquals(normalized, U32_TYPE_ID) ||
            uuidEquals(normalized, ULONG_TYPE_ID) ||
            uuidEquals(normalized, U64_TYPE_ID) ||
            uuidEquals(normalized, FLOAT_TYPE_ID) ||
            uuidEquals(normalized, DOUBLE_TYPE_ID) ||
            uuidEquals(normalized, BOOL_TYPE_ID);
    }

    private boolean isWireNativeSourceTypeName(String typeName) {
        String normalized = normalizeNativeType(typeName);
        if (normalized == null || normalized.isEmpty()) {
            return false;
        }
        if (wireShapeFromNativeType(normalized) != null) {
            return true;
        }
        if (normalized.startsWith("AZStd::fixed_vector<")
            || normalized.startsWith("fixed_vector<")
            || normalized.startsWith("AZStd::vector<")
            || normalized.startsWith("vector<")
            || normalized.startsWith("AZStd::array<")
            || normalized.startsWith("array<")) {
            return true;
        }
        String leaf = sourceTypeLeaf(normalized);
        if (leaf == null) {
            return false;
        }
        return switch (leaf) {
            case "ActorId",
                "ActorRef",
                "AssetId",
                "BaselineableFragment",
                "BaselineableFragmentRef",
                "ClientActorHash",
                "ClientContextId",
                "ComponentId",
                "ConnTicket",
                "Duration",
                "FieldGroup",
                "FieldVector",
                "Fragment",
                "HubAddress",
                "HubId",
                "InterestId",
                "LoginToken",
                "MovementInteractionId",
                "ProxyAddress",
                "SequenceNumber",
                "SyncedTimestamp",
                "Timestamp",
                "TypeIndex",
                "TypeIndexCrc" -> true;
            default -> false;
        };
    }

    private boolean isActorRequestIdMemberShape(List<NestedTypeMember> members) {
        if (members == null || members.size() != 2) {
            return false;
        }
        NestedTypeMember first = members.get(0);
        NestedTypeMember second = members.get(1);
        boolean wireRelative = first.offset == 0L && second.offset == 8L;
        boolean nativeBaseObject = first.offset == 8L && second.offset == 16L;
        return (wireRelative || nativeBaseObject) &&
            "u64".equals(wireShapeFromNativeType(first.nativeType)) &&
            "u64".equals(wireShapeFromNativeType(second.nativeType)) &&
            !Boolean.TRUE.equals(first.typeConflict) &&
            !Boolean.TRUE.equals(second.typeConflict);
    }

    private void normalizeActorRequestIdMemberOffsets(List<NestedTypeMember> members) {
        if (members == null || members.size() != 2) {
            return;
        }
        NestedTypeMember first = members.get(0);
        NestedTypeMember second = members.get(1);
        if (first.offset == 8L && second.offset == 16L) {
            first.nativeOffset = first.offset;
            second.nativeOffset = second.offset;
            first.offset = 0L;
            second.offset = 8L;
        }
    }

    private void applyNestedTypeMemberNames(NestedTypeShape shape) {
        if (shape == null || shape.members.isEmpty()) {
            return;
        }

        List<Structure> structures = nestedTypeStructures(shape);
        if (structures.isEmpty()) {
            recordNestedTypeShapeReject("datatype-member-structure-missing");
            return;
        }

        Structure selected = null;
        ArrayList<String> selectedNames = null;
        int matches = 0;
        for (Structure structure : structures) {
            ArrayList<String> names = datatypeMemberNamesForShape(structure, shape);
            if (names == null) {
                continue;
            }
            matches++;
            if (selected == null) {
                selected = structure;
                selectedNames = names;
                continue;
            }
            if (!selectedNames.equals(names)) {
                recordNestedTypeShapeReject("ambiguous-datatype-member-names");
                return;
            }
        }

        if (selected == null || selectedNames == null) {
            recordNestedTypeShapeReject("datatype-member-names-missing");
            return;
        }

        for (int i = 0; i < shape.members.size(); i++) {
            NestedTypeMember member = shape.members.get(i);
            member.name = selectedNames.get(i);
            member.nameSource = "ghidra-datatype";
            member.nameProven = true;
        }
        shape.memberNameSource = "ghidra-datatype";
        shape.memberNamesProven = true;
        shape.datatypePath = selected.getPathName();
    }

    private void applyActorRequestIdNativeMemberNames(NestedTypeShape shape) {
        if (shape == null ||
            !"ActorRequestId".equals(shape.typeName) ||
            !isActorRequestIdMemberShape(shape.members)) {
            return;
        }
        if (!hasActorRequestIdTargetLocalIdProof()) {
            return;
        }

        NestedTypeMember targetLocalId = shape.members.get(0);
        targetLocalId.name = "targetLocalId";
        targetLocalId.nameSource = "rmidispatch-targetLocalId-xref";
        targetLocalId.nameProven = true;
        targetLocalId.nameEvidence =
            "Amazon::Hub::FindActorFacetTarget logs qword [message+0x08] with native string targetLocalId";

        if (!Boolean.TRUE.equals(shape.memberNamesProven)) {
            shape.memberNameSource = "partial-rmidispatch-xref";
            shape.memberNamesProven = false;
        }
    }

    private boolean hasActorRequestIdTargetLocalIdProof() {
        Address functionAddress = currentProgram.getImageBase().add(FIND_ACTOR_FACET_TARGET_RVA);
        Function function = functionAtOrContaining(functionAddress);
        if (function == null || !function.getEntryPoint().equals(functionAddress)) {
            recordNestedTypeShapeReject("targetLocalId-proof-missing-dispatch-function");
            return false;
        }

        if (!instructionContains(
                FIND_ACTOR_FACET_TARGET_MESSAGE_COPY_RVA,
                "mov",
                "rsi",
                "rcx") ||
            !instructionContains(
                FIND_ACTOR_FACET_TARGET_TARGET_OFFSET_RVA,
                "mov",
                "r14d",
                "8") ||
            !instructionContains(
                FIND_ACTOR_FACET_TARGET_TARGET_ADD_RVA,
                "add",
                "r14",
                "rsi") ||
            !instructionContains(
                FIND_ACTOR_FACET_TARGET_TARGET_LOAD_RVA,
                "mov",
                "r8",
                "[r14]")) {
            recordNestedTypeShapeReject("targetLocalId-proof-instruction-mismatch");
            return false;
        }

        Address targetLocalId = exactAsciiStringAddress("targetLocalId");
        if (targetLocalId == null) {
            recordNestedTypeShapeReject("targetLocalId-proof-string-missing");
            return false;
        }
        Instruction nameInstruction =
            instructionAtRva(FIND_ACTOR_FACET_TARGET_TARGET_NAME_RVA);
        if (nameInstruction == null ||
            !instructionReferences(nameInstruction, targetLocalId) ||
            !instructionContains(
                FIND_ACTOR_FACET_TARGET_TARGET_NAME_RVA,
                "lea",
                "rdx")) {
            recordNestedTypeShapeReject("targetLocalId-proof-xref-missing");
            return false;
        }
        return true;
    }

    private Address exactAsciiStringAddress(String value) {
        Address selected = null;
        for (Address address : asciiStringsContaining(value)) {
            if (!value.equals(readPrintableString(address))) {
                continue;
            }
            if (selected != null && !selected.equals(address)) {
                return null;
            }
            selected = address;
        }
        return selected;
    }

    private Instruction instructionAtRva(long rva) {
        Address address = currentProgram.getImageBase().add(rva);
        Instruction instruction = currentProgram.getListing().getInstructionAt(address);
        if (instruction != null) {
            return instruction;
        }
        try {
            disassemble(address);
        }
        catch (Exception ignored) {
        }
        return currentProgram.getListing().getInstructionAt(address);
    }

    private boolean instructionReferences(Instruction instruction, Address target) {
        if (instruction == null || target == null) {
            return false;
        }
        for (Reference reference : instruction.getReferencesFrom()) {
            if (target.equals(reference.getToAddress())) {
                return true;
            }
        }
        return false;
    }

    private boolean instructionContains(long rva, String mnemonic, String... tokens) {
        Instruction instruction = instructionAtRva(rva);
        if (instruction == null) {
            return false;
        }
        String text = instructionText(instruction);
        if (!text.startsWith(mnemonic.toLowerCase(Locale.ROOT) + " ")) {
            return false;
        }
        for (String token : tokens) {
            if (!text.contains(token.toLowerCase(Locale.ROOT))) {
                return false;
            }
        }
        return true;
    }

    private String instructionText(Instruction instruction) {
        StringBuilder text = new StringBuilder(
            instruction.getMnemonicString().toLowerCase(Locale.ROOT));
        for (int i = 0; i < instruction.getNumOperands(); i++) {
            text.append(i == 0 ? ' ' : ',');
            text.append(
                instruction.getDefaultOperandRepresentation(i).toLowerCase(Locale.ROOT));
        }
        return text.toString()
            .replace("qword ptr ", "")
            .replace("byte ptr ", "")
            .replace("dword ptr ", "")
            .replace("0x08", "8")
            .replace("0x8", "8");
    }

    private List<Structure> nestedTypeStructures(NestedTypeShape shape) {
        if (shape == null || currentProgram == null) {
            return Collections.emptyList();
        }
        String qualifiedName = shape.typeNameFull != null ? shape.typeNameFull : shape.typeName;
        String leafName = shape.typeName != null ? shape.typeName : sourceTypeLeaf(qualifiedName);
        if (leafName == null || leafName.isEmpty()) {
            return Collections.emptyList();
        }

        String key = qualifiedName + "|" + leafName;
        if (nestedTypeStructureCache.containsKey(key)) {
            return nestedTypeStructureCache.get(key);
        }

        ArrayList<Structure> exact = new ArrayList<>();
        ArrayList<Structure> leaf = new ArrayList<>();
        Iterator<DataType> iterator = currentProgram.getDataTypeManager().getAllDataTypes();
        while (iterator.hasNext()) {
            DataType dataType = iterator.next();
            if (!(dataType instanceof Structure)) {
                continue;
            }
            Structure structure = (Structure) dataType;
            if (qualifiedName != null && datatypeMatchesQualifiedName(dataType, qualifiedName)) {
                exact.add(structure);
                continue;
            }
            if (leafName.equals(dataType.getName())) {
                leaf.add(structure);
            }
        }

        List<Structure> result = exact.isEmpty() ? leaf : exact;
        nestedTypeStructureCache.put(key, result);
        return result;
    }

    private boolean datatypeMatchesQualifiedName(DataType dataType, String qualifiedName) {
        if (dataType == null || qualifiedName == null || qualifiedName.isBlank()) {
            return false;
        }
        String leaf = sourceTypeLeaf(qualifiedName);
        String path = dataType.getPathName();
        String categoryPath = dataType.getCategoryPath() == null
            ? ""
            : dataType.getCategoryPath().getPath();
        if (qualifiedName.equals(dataType.getName()) || qualifiedName.equals(path)) {
            return true;
        }

        String qualifiedPath = "/" + qualifiedName.replace("::", "/");
        String normalizedPath = path == null ? "" : path.replace('\\', '/');
        String normalizedCategory = categoryPath.replace('\\', '/');
        if (normalizedPath.equals(qualifiedPath) || normalizedPath.endsWith(qualifiedPath)) {
            return true;
        }
        if (normalizedCategory.equals(qualifiedPath) || normalizedCategory.endsWith(qualifiedPath)) {
            return true;
        }
        return leaf != null &&
            leaf.equals(dataType.getName()) &&
            normalizedCategory.endsWith("/" + qualifiedName.replace("::", "/"));
    }

    private ArrayList<String> datatypeMemberNamesForShape(
        Structure structure,
        NestedTypeShape shape) {

        if (structure == null || shape == null || shape.members.isEmpty()) {
            return null;
        }
        int requiredLength = nestedTypeRequiredLength(shape);
        if (requiredLength <= 0 || structure.getLength() != requiredLength) {
            return null;
        }

        ArrayList<String> names = new ArrayList<>();
        for (NestedTypeMember member : shape.members) {
            DataTypeComponent component = datatypeComponentAtOffset(structure, member.offset);
            if (component == null) {
                return null;
            }
            if (member.byteWidth != null && component.getLength() != member.byteWidth) {
                return null;
            }
            String name = component.getFieldName();
            if (!isProvenDatatypeMemberName(name)) {
                return null;
            }
            names.add(name);
        }
        return names;
    }

    private int nestedTypeRequiredLength(NestedTypeShape shape) {
        long length = 0;
        for (NestedTypeMember member : shape.members) {
            if (member.byteWidth == null || member.byteWidth <= 0) {
                return -1;
            }
            length = Math.max(length, member.offset + member.byteWidth);
        }
        return length > Integer.MAX_VALUE ? -1 : (int) length;
    }

    private DataTypeComponent datatypeComponentAtOffset(Structure structure, long offset) {
        if (structure == null || offset < 0 || offset > Integer.MAX_VALUE) {
            return null;
        }
        for (DataTypeComponent component : structure.getDefinedComponents()) {
            if (component.getOffset() == (int) offset) {
                return component;
            }
        }
        return null;
    }

    private boolean isProvenDatatypeMemberName(String name) {
        if (name == null) {
            return false;
        }
        String trimmed = name.trim();
        if (trimmed.isEmpty()) {
            return false;
        }
        String lower = trimmed.toLowerCase(Locale.ROOT);
        return !lower.equals("field") &&
            !lower.matches("field_?\\d+") &&
            !lower.matches("undefined\\d*") &&
            !lower.matches("padding_?\\d*") &&
            !lower.startsWith("unnamed");
    }

    private Integer nativeTypeByteWidth(String nativeType) {
        String shape = wireShapeFromNativeType(nativeType);
        return wireShapeByteWidth(shape);
    }

    private Integer wireShapeByteWidth(String shape) {
        if (shape == null) {
            return null;
        }
        return switch (shape) {
            case "bool", "u8" -> 1;
            case "u16" -> 2;
            case "u32", "f32" -> 4;
            case "u64", "f64", "vec2" -> 8;
            case "uuid", "vec4" -> 16;
            case "vec3" -> 12;
            case "transform" -> 48;
            default -> null;
        };
    }

    private String scalarOutputStoreNativeType(Function target) {
        if (target == null) {
            return null;
        }

        String pcodeType = scalarOutputStoreNativeTypeFromPcode(target);
        if (pcodeType != null) {
            return pcodeType;
        }
        return scalarOutputStoreNativeTypeFromInstructionFlow(target);
    }

    private String scalarOutputStoreNativeTypeFromPcode(Function target) {
        HighFunction high = highFunction(target);
        if (high == null) {
            return null;
        }

        LinkedHashMap<String, String> candidateTypes = new LinkedHashMap<>();
        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOpAST op = ops.next();
            if (op.getOpcode() != PcodeOp.STORE || op.getNumInputs() < 3) {
                continue;
            }
            PcodeStorage storage = pcodeStorageExpression(op.getInput(1));
            if (!isScalarOutputPointerStorage(storage)) {
                continue;
            }
            String nativeType = scalarNativeTypeFromByteWidth(op.getInput(2).getSize());
            if (nativeType == null) {
                continue;
            }
            String previous = candidateTypes.putIfAbsent(storage.base, nativeType);
            if (previous != null && !previous.equals(nativeType)) {
                return null;
            }
        }
        return selectedScalarOutputCandidate(candidateTypes);
    }

    private String scalarOutputStoreNativeTypeFromInstructionFlow(Function target) {
        ForwardArgState state = new ForwardArgState();
        state.registers.put("RCX", TrackedValue.baseOffset("arg:RCX", 0));
        state.registers.put("RDX", TrackedValue.baseOffset("arg:RDX", 0));
        state.registers.put("R8", TrackedValue.baseOffset("arg:R8", 0));
        state.registers.put("R9", TrackedValue.baseOffset("arg:R9", 0));
        state.registers.put("RSP", TrackedValue.stackOffset(0));

        LinkedHashMap<String, String> candidateTypes = new LinkedHashMap<>();
        for (Instruction instruction : functionInstructions(target)) {
            String mnemonic = upperMnemonic(instruction);
            if ("MOV".equals(mnemonic) && isMemoryWriteMnemonic(mnemonic)) {
                TrackedValue storage =
                    trackedBaseOffsetForMemoryOperand(instruction, 0, state.registers);
                if (isInstructionScalarOutputPointerStorage(storage)) {
                    String nativeType =
                        scalarNativeTypeFromByteWidth(instructionMemoryWriteByteWidth(instruction));
                    if (nativeType != null) {
                        String previous = candidateTypes.putIfAbsent(storage.baseKey, nativeType);
                        if (previous != null && !previous.equals(nativeType)) {
                            return null;
                        }
                    }
                }
            }
            observeForwardInstruction(instruction, state, false);
        }
        return selectedScalarOutputCandidate(candidateTypes);
    }

    private String selectedScalarOutputCandidate(Map<String, String> candidateTypes) {
        if (candidateTypes == null || candidateTypes.isEmpty()) {
            return null;
        }
        for (String preferred : List.of("param_3", "arg:R8", "R8")) {
            String nativeType = candidateTypes.get(preferred);
            if (nativeType != null) {
                return nativeType;
            }
        }
        if (candidateTypes.size() == 1) {
            return candidateTypes.values().iterator().next();
        }
        return null;
    }

    private boolean isScalarOutputPointerStorage(PcodeStorage storage) {
        return storage != null &&
            storage.offset == 0L &&
            (storage.base.matches("param_[3-9]") ||
                storage.base.matches("p[lub]?Var\\d*"));
    }

    private boolean isInstructionScalarOutputPointerStorage(TrackedValue storage) {
        return storage != null &&
            storage.baseKey != null &&
            storage.baseOffset != null &&
            storage.baseOffset == 0 &&
            storage.baseKey.matches("arg:(R8|R9)");
    }

    private int instructionMemoryWriteByteWidth(Instruction instruction) {
        String text = operandText(instruction, 0);
        if (text == null) {
            return -1;
        }
        String lower = text.toLowerCase(Locale.ROOT);
        if (lower.contains("byte ptr")) {
            return 1;
        }
        if (lower.contains("word ptr")) {
            return 2;
        }
        if (lower.contains("dword ptr")) {
            return 4;
        }
        if (lower.contains("qword ptr")) {
            return 8;
        }
        return -1;
    }

    private String scalarNativeTypeFromByteWidth(int byteWidth) {
        return switch (byteWidth) {
            case 1 -> "u8";
            case 2 -> "u16";
            case 4 -> "u32";
            case 8 -> "u64";
            default -> null;
        };
    }

    private void recordNestedTypeShapeReject(String reason) {
        nestedTypeShapeRejectCounts.merge(reason, 1, Integer::sum);
    }

    private Set<String> pcodeMessageStorageBases(HighFunction high) {
        LinkedHashSet<String> bases = new LinkedHashSet<>();
        bases.add("this");
        bases.add("_Dst");
        bases.add("param_3");

        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOpAST op = ops.next();
            if (op.getOpcode() != PcodeOp.CALL || op.getOutput() == null) {
                continue;
            }
            Function target = pcodeCallTarget(op);
            Integer returnedCallInputSlot = returnedCallInputSlot(target);
            if (returnedCallInputSlot != null && returnedCallInputSlot < op.getNumInputs()) {
                PcodeStorage inputStorage = pcodeStorageExpression(op.getInput(returnedCallInputSlot));
                if (isPcodeMessageStorage(inputStorage, bases)) {
                    addPcodeAliasStorageBases(bases, op.getOutput());
                    continue;
                }
            }
            if (isLikelyConstructorTarget(target)) {
                String name = pcodeHighName(op.getOutput());
                if (name != null) {
                    bases.add(name);
                }
            }
        }
        return bases;
    }

    private boolean isPcodeMessageStorage(PcodeStorage storage, Set<String> messageBases) {
        return storage != null &&
            messageBases != null &&
            messageBases.contains(storage.base) &&
            isLikelyMessageStorage(storage.expression());
    }

    private void addPcodeAliasStorageBases(Set<String> bases, Varnode seed) {
        if (bases == null || seed == null) {
            return;
        }
        ArrayDeque<Varnode> pending = new ArrayDeque<>();
        LinkedHashSet<String> aliases = new LinkedHashSet<>();
        pending.add(seed);
        int visited = 0;
        while (!pending.isEmpty() && visited++ < PCODE_ALIAS_DESCENDANT_LIMIT) {
            Varnode node = pending.removeFirst();
            if (node == null) {
                continue;
            }
            String key = pcodeNodeKey(node);
            if (!aliases.add(key)) {
                continue;
            }
            String name = pcodeHighName(node);
            if (isPcodeStorageBase(name)) {
                bases.add(name);
            }

            Iterator<PcodeOp> descendants = node.getDescendants();
            while (descendants != null && descendants.hasNext()) {
                PcodeOp descendant = descendants.next();
                Varnode output = descendant == null ? null : descendant.getOutput();
                if (output == null || !pcodeAliasDescendantCarriesBase(descendant, aliases)) {
                    continue;
                }
                pending.add(output);
            }
        }
    }

    private boolean pcodeAliasDescendantCarriesBase(
        PcodeOp op,
        Set<String> aliases) {

        if (op == null) {
            return false;
        }
        switch (op.getOpcode()) {
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.INT_ZEXT:
            case PcodeOp.INT_SEXT:
            case PcodeOp.SUBPIECE:
            case PcodeOp.INDIRECT:
                return op.getNumInputs() > 0 && aliases.contains(pcodeNodeKey(op.getInput(0)));
            case PcodeOp.MULTIEQUAL:
                return pcodePhiCarriesAliasOrNull(op, aliases);
            default:
                return false;
        }
    }

    private boolean pcodePhiCarriesAliasOrNull(PcodeOp op, Set<String> aliases) {
        if (op == null || op.getOpcode() != PcodeOp.MULTIEQUAL || op.getNumInputs() == 0) {
            return false;
        }
        boolean sawAlias = false;
        for (int i = 0; i < op.getNumInputs(); i++) {
            Varnode input = op.getInput(i);
            if (aliases.contains(pcodeNodeKey(input))) {
                sawAlias = true;
                continue;
            }
            Long constant = pcodeConstantValue(input, new LinkedHashSet<>(), 0);
            if (constant == null || constant != 0L) {
                return false;
            }
        }
        return sawAlias;
    }

    private boolean isPcodeLocalTempStorage(PcodeStorage storage) {
        return storage != null && "stack".equals(storage.base);
    }

    private String storageKey(PcodeStorage storage) {
        return storage == null ? null : storage.base + ":0x" + Long.toHexString(storage.offset);
    }

    private boolean isLikelyConstructorTarget(Function target) {
        String name = fullFunctionName(target);
        if (name == null || name.startsWith("FUN_") || !name.contains("::")) {
            return false;
        }
        String[] parts = name.split("::");
        if (parts.length < 2) {
            return false;
        }
        String functionName = parts[parts.length - 1];
        String ownerName = parts[parts.length - 2];
        return functionName.equals(ownerName);
    }

    private String unmarshalNativeTypeFromTarget(Function target) {
        if (target == null) {
            return null;
        }
        String name = fullFunctionName(target);
        if (name == null) {
            return null;
        }

        String templateType = unmarshalTemplateType(name);
        if (templateType != null) {
            return templateType;
        }

        templateType = marshalerTemplateType(name);
        if (templateType != null) {
            return templateType;
        }

        String owner = directUnmarshalOwnerType(name);
        if (owner != null) {
            return owner;
        }
        return null;
    }

    private String marshalerTemplateType(String functionName) {
        if (functionName == null) {
            return null;
        }
        String[] markers = {"Marshaler<", "Marshaller<"};
        for (String marker : markers) {
            int start = functionName.indexOf(marker);
            if (start < 0) {
                continue;
            }
            int templateStart = functionName.indexOf('<', start);
            int templateEnd = matchingIndex(functionName, templateStart, '<', '>');
            if (templateStart >= 0 && templateEnd > templateStart) {
                return functionName.substring(templateStart + 1, templateEnd).trim();
            }
        }
        return null;
    }

    private String directUnmarshalOwnerFullName(String functionName) {
        if (functionName == null || functionName.contains("Marshaler<") ||
            functionName.contains("Marshaller<") || functionName.contains("UnmarshalFields<")) {
            return null;
        }
        int unmarshal = functionName.lastIndexOf("::Unmarshal");
        if (unmarshal < 0) {
            return null;
        }
        String owner = functionName.substring(0, unmarshal).trim();
        if (owner.isEmpty() || owner.startsWith("FUN_") || owner.contains("::FUN_")) {
            return null;
        }
        return owner;
    }

    private String directUnmarshalOwnerFullNameFromPrototype(Function target) {
        if (target == null) {
            return null;
        }
        String selected = null;
        for (Parameter parameter : target.getParameters()) {
            String typeName = directTypeNameFromParameter(parameter);
            if (typeName == null) {
                continue;
            }
            if (selected != null && !selected.equals(typeName)) {
                recordNestedTypeShapeReject("ambiguous-prototype-direct-type");
                return null;
            }
            selected = typeName;
        }
        return selected;
    }

    private String directTypeNameFromParameter(Parameter parameter) {
        if (parameter == null) {
            return null;
        }
        DataType dataType = parameter.getDataType();
        if (dataType == null) {
            return null;
        }
        String name = normalizePrototypeTypeName(dataType.getDisplayName());
        if (!isPlausibleDirectPrototypeTypeName(name)) {
            return null;
        }
        return name;
    }

    private String normalizePrototypeTypeName(String value) {
        if (value == null) {
            return null;
        }
        String result = value
            .replace(" *", "*")
            .replace("*", "")
            .replace(" &", "&")
            .replace("&", "")
            .replace("const ", "")
            .trim();
        return normalizeNativeType(result);
    }

    private boolean isPlausibleDirectPrototypeTypeName(String value) {
        if (!isPlausibleTypeName(value)) {
            return false;
        }
        String normalized = normalizeNativeType(value);
        if (normalized == null ||
            wireShapeFromNativeType(normalized) != null ||
            normalized.startsWith("undefined") ||
            normalized.startsWith("param_") ||
            normalized.startsWith("FUN_")) {
            return false;
        }
        String lower = normalized.toLowerCase(Locale.ROOT);
        if (lower.contains("buffer") ||
            lower.contains("context") ||
            lower.contains("marshal") ||
            lower.contains("stream") ||
            lower.contains("allocator") ||
            lower.contains("memory")) {
            return false;
        }
        return !lower.equals("void") &&
            !lower.equals("char") &&
            !lower.equals("byte") &&
            !lower.equals("longlong") &&
            !lower.equals("ulonglong");
    }

    private String directUnmarshalOwnerType(String functionName) {
        return sourceTypeLeaf(directUnmarshalOwnerFullName(functionName));
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

    private void recoverDirectObjectStoreMessageFields(
        MessageUnmarshalPlan plan,
        Address callsite,
        String text) {

        Map<String, String> temporaryTypes = directUnmarshalTemporaryTypes(text);
        Matcher matcher = POINTER_STORE_RE.matcher(text);
        while (matcher.find()) {
            String storage = normalizedExpression(matcher.group("target"));
            if (!isLikelyMessageStorage(storage)) {
                continue;
            }

            String nativeType = nativeTypeFromPointerStore(
                matcher.group("type"),
                matcher.group("rhs"),
                temporaryTypes);
            if (nativeType == null) {
                continue;
            }
            FieldCall field = addMessageField(
                plan,
                callsite,
                storage,
                nativeType,
                "message-unmarshal-direct-object-store",
                matcher.start());
            if (field.nativeType == null && nativeType != null) {
                refineMessageFieldType(
                    field,
                    nativeType,
                    "message-unmarshal-direct-object-store");
            }
        }
    }

    private Map<String, String> directUnmarshalTemporaryTypes(String text) {
        LinkedHashMap<String, String> result = new LinkedHashMap<>();
        for (ParsedUnmarshalCall call : parseMarshalerUnmarshalCalls(text)) {
            String storage = normalizedExpression(storageArgumentForMarshalerCall(call));
            if (isLikelyTemporaryStorage(storage)) {
                result.put(storage, call.templateType);
            }
        }
        for (ParsedUnmarshalCall call : parseDirectTypeUnmarshalCalls(text)) {
            String storage = normalizedExpression(storageArgumentForDirectUnmarshalCall(call));
            if (isLikelyTemporaryStorage(storage)) {
                result.put(storage, call.templateType);
            }
        }
        return result;
    }

    private String nativeTypeFromPointerStore(
        String pointerType,
        String rhs,
        Map<String, String> temporaryTypes) {

        String rhsExpression = normalizedExpression(rhs);
        String rhsBase = firstIdentifier(rhsExpression);
        if (rhsBase != null) {
            String type = temporaryTypes.get(rhsBase);
            if (type != null) {
                return type;
            }
        }

        String pointerNativeType = nativeTypeFromPointerType(pointerType);
        if ("bool".equals(pointerNativeType)) {
            return "bool";
        }
        return null;
    }

    private String nativeTypeFromPointerType(String pointerType) {
        if (pointerType == null) {
            return null;
        }
        String value = pointerType.replace("*", "").trim();
        if (value.startsWith("bool")) {
            return "bool";
        }
        if (value.startsWith("byte") || value.startsWith("undefined1")) {
            return "u8";
        }
        if (value.startsWith("undefined2")) {
            return "u16";
        }
        if (value.startsWith("undefined4")) {
            return "u32";
        }
        if (value.startsWith("undefined8")) {
            return "u64";
        }
        return null;
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

            List<String> helperParams = parameterNamesFromDecompiledFunction(helperText);
            recoverWholeMessageHelperFields(
                plan,
                instruction.getMinAddress(),
                helper,
                helperText,
                wrapperArgs,
                helperParams,
                helperTextIndex);

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
            refineHelperArgumentFieldTypesFromPcode(helper, fieldsByHelperParam);
            attachSingleHelperArgumentNestedShape(helper, fieldsByHelperParam);
            refineDirectBoolWrites(helperText, fieldsByHelperParam);
            refineNestedBoolWrites(helper, helperText, fieldsByHelperParam);
        }
    }

    private void refineHelperArgumentFieldTypesFromPcode(
        Function helper,
        Map<String, FieldCall> fieldsByHelperParam) {

        if (helper == null || fieldsByHelperParam == null || fieldsByHelperParam.isEmpty()) {
            return;
        }
        HighFunction high = highFunction(helper);
        if (high == null) {
            return;
        }

        LinkedHashSet<String> helperOutputBases = new LinkedHashSet<>(fieldsByHelperParam.keySet());
        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOpAST op = ops.next();
            if (op.getOpcode() != PcodeOp.CALL) {
                continue;
            }

            PcodeCallTargetInfo targetInfo = pcodeCallTargetInfo(op);
            Function target = targetInfo == null ? null : targetInfo.target;
            if (target == null) {
                continue;
            }

            PcodeArgStorageSelection storageSelection =
                pcodeStorageArgumentEvidence(op, helperOutputBases);
            PcodeStorage storage = storageSelection == null ? null : storageSelection.storage;
            if (storage == null || storage.offset != 0L) {
                continue;
            }

            FieldCall field = fieldsByHelperParam.get(storage.base);
            if (field == null) {
                continue;
            }

            String nativeType = unmarshalNativeTypeFromTarget(target);
            if (nativeType == null) {
                nativeType = directUnmarshalOwnerFullNameFromPrototype(target);
            }
            if (nativeType != null) {
                refineMessageFieldType(
                    field,
                    nativeType,
                    "message-unmarshal-helper-pcode-call",
                    fullFunctionName(target),
                    "helper-pcode-call");
            }

            NestedTypeShape shape = recoverNestedDirectTypeShape(target, nativeType, true);
            if (shape != null && field.nestedTypeShape == null) {
                field.nestedTypeShape = shape;
                if (field.nativeType == null && shape.typeNameFull != null) {
                    refineMessageFieldType(
                        field,
                        shape.typeNameFull,
                        "message-unmarshal-helper-pcode-nested-shape");
                }
            }
        }
    }

    private void attachSingleHelperArgumentNestedShape(
        Function helper,
        Map<String, FieldCall> fieldsByHelperParam) {

        if (helper == null || fieldsByHelperParam == null ||
            fieldsByHelperParam.size() != 1) {
            return;
        }
        FieldCall field = fieldsByHelperParam.values().iterator().next();
        if (field == null || field.nestedTypeShape != null) {
            return;
        }
        String nativeType = directUnmarshalOwnerFullNameFromPrototype(helper);
        NestedTypeShape shape = recoverNestedDirectTypeShape(helper, nativeType, true);
        if (shape == null) {
            return;
        }
        field.nestedTypeShape = shape;
        if (field.nativeType == null && shape.typeNameFull != null) {
            refineMessageFieldType(
                field,
                shape.typeNameFull,
                "message-unmarshal-helper-nested-shape");
        }
    }

    private void recoverWholeMessageHelperFields(
        MessageUnmarshalPlan plan,
        Address callsite,
        Function helper,
        String helperText,
        List<String> callerArgs,
        List<String> helperParams,
        int recoveryOrder) {

        Map<String, String> baseExpressions =
            wholeMessageBaseExpressions(callerArgs, helperParams);
        if (baseExpressions.isEmpty()) {
            return;
        }
        recoverWholeMessageHelperFields(
            plan,
            callsite,
            helper,
            helperText,
            baseExpressions,
            recoveryOrder);
    }

    private void recoverWholeMessageHelperFields(
        MessageUnmarshalPlan plan,
        Address callsite,
        Function helper,
        String helperText,
        Map<String, String> baseExpressions,
        int recoveryOrder) {

        if (plan == null || helper == null || helperText == null ||
            baseExpressions.isEmpty()) {
            return;
        }

        Deque<WholeMessageHelperFrame> stack = new ArrayDeque<>();
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        stack.push(new WholeMessageHelperFrame(
            callsite,
            helper,
            helperText,
            baseExpressions,
            recoveryOrder));

        while (!stack.isEmpty()) {
            WholeMessageHelperFrame frame = stack.pop();
            if (frame.helper == null || frame.helperText == null ||
                frame.baseExpressions.isEmpty()) {
                continue;
            }

            String seenKey = frame.helper.getEntryPoint() + "|" + frame.baseExpressions;
            if (!seen.add(seenKey)) {
                continue;
            }

            recoverWholeMessageDirectFields(
                plan,
                frame.callsite,
                frame.helperText,
                frame.baseExpressions,
                frame.recoveryOrder);
            recoverWholeMessageTempStoreFields(
                plan,
                frame.callsite,
                frame.helperText,
                frame.baseExpressions,
                frame.recoveryOrder);
            pushNestedWholeMessageHelperFrames(stack, frame);
        }
    }

    private Map<String, String> wholeMessageBaseExpressions(
        List<String> callerArgs,
        List<String> helperParams) {

        LinkedHashMap<String, String> result = new LinkedHashMap<>();
        int count = Math.min(callerArgs.size(), helperParams.size());
        for (int i = 0; i < count; i++) {
            String callerArg = canonicalStorageExpression(callerArgs.get(i));
            if (!isWrapperMessageStorageBase(callerArg)) {
                continue;
            }
            result.put(helperParams.get(i), storageExpressionWithOffset(callerArg, 0));
        }
        return result;
    }

    private boolean isWrapperMessageStorageBase(String expression) {
        if (expression == null) {
            return false;
        }
        return "param_3".equals(expression) ||
            expression.startsWith("_Dst");
    }

    private void recoverWholeMessageDirectFields(
        MessageUnmarshalPlan plan,
        Address callsite,
        String helperText,
        Map<String, String> baseExpressions,
        int recoveryOrder) {

        for (ParsedUnmarshalCall call : parseMarshalerUnmarshalCalls(helperText)) {
            String storage = storageArgumentForMarshalerCall(call);
            String translated = translatedStorageExpression(storage, baseExpressions);
            if (translated == null) {
                continue;
            }
            FieldCall field = addMessageField(
                plan,
                callsite,
                translated,
                call.templateType,
                "message-unmarshal-whole-helper-marshaler",
                relativeRecoveryOrder(recoveryOrder, call.textIndex));
            applyTextUnmarshalEvidence(
                field,
                callsite,
                call.functionName,
                "whole-helper-marshaler",
                "message-unmarshal-whole-helper-marshaler");
        }

        for (ParsedUnmarshalCall call : parseDirectTypeUnmarshalCalls(helperText)) {
            for (String arg : call.args) {
                String translated = translatedStorageExpression(arg, baseExpressions);
                if (translated == null) {
                    continue;
                }
                FieldCall field = addMessageField(
                    plan,
                    callsite,
                    translated,
                    call.templateType,
                    "message-unmarshal-whole-helper-direct-type",
                    relativeRecoveryOrder(recoveryOrder, call.textIndex));
                applyTextUnmarshalEvidence(
                    field,
                    callsite,
                    call.functionName,
                    "whole-helper-direct-type",
                    "message-unmarshal-whole-helper-direct-type");
                attachNestedDirectTypeShape(
                    field,
                    directTypeUnmarshalFunction(call.templateType, call.functionName),
                    call.templateType);
                break;
            }
        }

        for (ParsedReadRawCall call : parseReadRawCalls(helperText)) {
            String translated =
                translatedStorageExpression(call.storageExpression, baseExpressions);
            if (translated == null) {
                continue;
            }
            addRawMessageField(
                plan,
                callsite,
                translated,
                call.byteLength,
                "message-unmarshal-whole-helper-read-raw",
                relativeRecoveryOrder(recoveryOrder, call.textIndex));
        }
    }

    private void recoverWholeMessageTempStoreFields(
        MessageUnmarshalPlan plan,
        Address callsite,
        String helperText,
        Map<String, String> baseExpressions,
        int recoveryOrder) {

        LinkedHashMap<String, ParsedUnmarshalCall> tempUnmarshalCalls =
            new LinkedHashMap<>();
        for (ParsedUnmarshalCall call : parseMarshalerUnmarshalCalls(helperText)) {
            String storage = storageArgumentForMarshalerCall(call);
            if (translatedStorageExpression(storage, baseExpressions) != null) {
                continue;
            }
            String temp = localTempName(storage);
            if (temp != null) {
                tempUnmarshalCalls.put(temp, call);
            }
        }

        LinkedHashMap<String, ParsedReadRawCall> tempReadRawCalls = new LinkedHashMap<>();
        for (ParsedReadRawCall call : parseReadRawCalls(helperText)) {
            if (translatedStorageExpression(call.storageExpression, baseExpressions) != null) {
                continue;
            }
            String temp = localTempName(call.storageExpression);
            if (temp != null) {
                tempReadRawCalls.put(temp, call);
            }
        }

        for (Map.Entry<String, ParsedUnmarshalCall> entry : tempUnmarshalCalls.entrySet()) {
            for (WholeMessageStore store : wholeMessageStoresForTemp(
                helperText,
                baseExpressions,
                entry.getKey())) {

                String nativeType = store.nativeType == null
                    ? entry.getValue().templateType
                    : store.nativeType;
                addMessageField(
                    plan,
                    callsite,
                    store.storageExpression,
                    nativeType,
                    "message-unmarshal-whole-helper-temp-store",
                    relativeRecoveryOrder(recoveryOrder, entry.getValue().textIndex));
            }
        }

        for (Map.Entry<String, ParsedReadRawCall> entry : tempReadRawCalls.entrySet()) {
            for (WholeMessageStore store : wholeMessageStoresForTemp(
                helperText,
                baseExpressions,
                entry.getKey())) {

                if ("bool".equals(store.nativeType)) {
                    addMessageField(
                        plan,
                        callsite,
                        store.storageExpression,
                        "bool",
                        "message-unmarshal-whole-helper-raw-bool-store",
                        relativeRecoveryOrder(recoveryOrder, entry.getValue().textIndex));
                }
                else {
                    addRawMessageField(
                        plan,
                        callsite,
                        store.storageExpression,
                        entry.getValue().byteLength,
                        "message-unmarshal-whole-helper-raw-temp-store",
                        relativeRecoveryOrder(recoveryOrder, entry.getValue().textIndex));
                }
            }
        }
    }

    private void pushNestedWholeMessageHelperFrames(
        Deque<WholeMessageHelperFrame> stack,
        WholeMessageHelperFrame frame) {

        for (Instruction instruction : functionInstructions(frame.helper)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }

            Function target = functionAtOrContaining(callTarget(instruction));
            if (target == null || target.getEntryPoint().equals(frame.helper.getEntryPoint())) {
                continue;
            }
            if (isLeafUnmarshalTarget(target)) {
                continue;
            }

            String targetText = decompileC(target);
            if (!looksLikeMessageUnmarshalHelper(targetText)) {
                continue;
            }

            int occurrence = callOccurrenceIndex(frame.helper, instruction, target);
            List<String> targetArgs =
                callArgumentsForTarget(frame.helperText, target, occurrence);
            if (targetArgs.isEmpty()) {
                continue;
            }

            List<String> targetParams = parameterNamesFromDecompiledFunction(targetText);
            LinkedHashMap<String, String> targetBaseExpressions = new LinkedHashMap<>();
            int count = Math.min(targetArgs.size(), targetParams.size());
            for (int i = 0; i < count; i++) {
                String translated =
                    translatedStorageExpression(targetArgs.get(i), frame.baseExpressions);
                if (translated != null) {
                    targetBaseExpressions.put(targetParams.get(i), translated);
                }
            }
            if (targetBaseExpressions.isEmpty()) {
                continue;
            }

            stack.push(new WholeMessageHelperFrame(
                instruction.getMinAddress(),
                target,
                targetText,
                targetBaseExpressions,
                relativeRecoveryOrder(
                    frame.recoveryOrder,
                    callTextIndexForTarget(frame.helperText, target, occurrence))));
        }
    }

    private List<WholeMessageStore> wholeMessageStoresForTemp(
        String text,
        Map<String, String> baseExpressions,
        String tempName) {

        ArrayList<WholeMessageStore> result = new ArrayList<>();
        if (text == null || tempName == null) {
            return result;
        }

        int search = 0;
        for (String rawStatement : text.split(";")) {
            String statement = rawStatement.trim();
            int statementIndex = text.indexOf(rawStatement, search);
            if (statementIndex >= 0) {
                search = statementIndex + rawStatement.length();
            }

            int equals = assignmentEqualsIndex(statement);
            if (equals <= 0) {
                continue;
            }
            String left = storeTargetExpression(statement.substring(0, equals));
            String right = statement.substring(equals + 1).trim();
            if (!referencesTemp(right, tempName)) {
                continue;
            }

            String storage = translatedStoreTarget(left, baseExpressions);
            if (storage == null) {
                continue;
            }

            WholeMessageStore store = new WholeMessageStore();
            store.storageExpression = storage;
            store.nativeType = storeNativeType(left, right);
            store.textIndex = statementIndex < 0 ? Integer.MAX_VALUE : statementIndex;
            result.add(store);
        }
        return result;
    }

    private String storeTargetExpression(String leftHandSide) {
        if (leftHandSide == null) {
            return null;
        }
        String value = leftHandSide.trim();
        int brace = value.lastIndexOf('{');
        if (brace >= 0) {
            value = value.substring(brace + 1).trim();
        }
        int newline = Math.max(value.lastIndexOf('\n'), value.lastIndexOf('\r'));
        if (newline >= 0) {
            value = value.substring(newline + 1).trim();
        }
        int comma = value.lastIndexOf(',');
        if (comma >= 0) {
            value = value.substring(comma + 1).trim();
        }
        return value;
    }

    private int assignmentEqualsIndex(String statement) {
        if (statement == null) {
            return -1;
        }
        for (int i = statement.length() - 1; i >= 0; i--) {
            if (statement.charAt(i) != '=') {
                continue;
            }
            char previous = i == 0 ? '\0' : statement.charAt(i - 1);
            char next = i + 1 >= statement.length() ? '\0' : statement.charAt(i + 1);
            if (previous == '!' || previous == '<' || previous == '>' ||
                previous == '=' || next == '=') {
                continue;
            }
            return i;
        }
        return -1;
    }

    private boolean referencesTemp(String expression, String tempName) {
        if (expression == null || tempName == null) {
            return false;
        }
        return Pattern.compile("\\b" + Pattern.quote(tempName) + "\\b")
            .matcher(expression)
            .find();
    }

    private String translatedStoreTarget(
        String leftHandSide,
        Map<String, String> baseExpressions) {

        if (leftHandSide == null) {
            return null;
        }
        for (Map.Entry<String, String> entry : baseExpressions.entrySet()) {
            Long offset = relativeByteOffsetFromBase(leftHandSide, entry.getKey());
            if (offset != null) {
                return storageExpressionWithOffset(entry.getValue(), offset);
            }
        }
        return null;
    }

    private String storeNativeType(String leftHandSide, String rightHandSide) {
        if (leftHandSide != null && leftHandSide.contains("bool *")) {
            return "bool";
        }
        if (rightHandSide != null && rightHandSide.contains("!= 0")) {
            return "bool";
        }
        return null;
    }

    private String translatedStorageExpression(
        String expression,
        Map<String, String> baseExpressions) {

        if (expression == null || baseExpressions.isEmpty()) {
            return null;
        }
        for (Map.Entry<String, String> entry : baseExpressions.entrySet()) {
            Long offset = relativeByteOffsetFromBase(expression, entry.getKey());
            if (offset != null) {
                return storageExpressionWithOffset(entry.getValue(), offset);
            }
        }
        return null;
    }

    private Long relativeByteOffsetFromBase(String expression, String baseName) {
        String value = canonicalStorageExpression(expression);
        if (value == null || baseName == null) {
            return null;
        }
        Long direct = relativeByteOffsetFromCanonicalValue(value, baseName);
        if (direct != null) {
            return direct;
        }
        return relativeByteOffsetFromCanonicalValue(
            stripLeadingDereference(value),
            baseName);
    }

    private Long relativeByteOffsetFromCanonicalValue(String value, String baseName) {
        if (value == null || baseName == null) {
            return null;
        }
        if (value.equals(baseName)) {
            return 0L;
        }
        Matcher matcher = Pattern
            .compile("\\b" + Pattern.quote(baseName) +
                "\\s*\\+\\s*(?<offset>0x[0-9a-fA-F]+|\\d+)")
            .matcher(value);
        if (!matcher.find()) {
            return null;
        }
        String offsetText = matcher.group("offset");
        Long offset = parseIntegerLiteral(offsetText);
        return offset == null ? null : offset * storageOffsetUnit(baseName, offsetText);
    }

    private String stripLeadingDereference(String expression) {
        if (expression == null) {
            return null;
        }
        String value = expression.trim();
        while (value.startsWith("*")) {
            value = stripLeadingCastsAndParens(value.substring(1).trim());
        }
        return value;
    }

    private String stripLeadingCastsAndParens(String expression) {
        String value = expression == null ? null : expression.trim();
        boolean changed;
        do {
            changed = false;
            while (value != null && value.startsWith("(")) {
                int end = matchingIndex(value, 0, '(', ')');
                if (end < 0) {
                    return value;
                }
                String inner = value.substring(1, end).trim();
                if (isLikelyCastType(inner)) {
                    value = value.substring(end + 1).trim();
                    changed = true;
                    continue;
                }
                if (end == value.length() - 1) {
                    value = inner;
                    changed = true;
                    continue;
                }
                break;
            }
        } while (changed);
        return value;
    }

    private String storageExpressionWithOffset(String baseExpression, long offset) {
        String baseName = storageBaseName(baseExpression);
        if (baseName == null) {
            return baseExpression;
        }
        long total = storageOffsetOrZero(baseExpression) + offset;
        return baseName + " + 0x" + Long.toHexString(total);
    }

    private String storageBaseName(String expression) {
        String value = canonicalStorageExpression(expression);
        if (value == null) {
            return null;
        }
        Matcher matcher = Pattern.compile("^([A-Za-z_][A-Za-z0-9_]*)\\b").matcher(value);
        return matcher.find() ? matcher.group(1) : null;
    }

    private long storageOffsetOrZero(String expression) {
        String baseName = storageBaseName(expression);
        if (baseName == null) {
            return 0;
        }
        Long offset = relativeByteOffsetFromBase(expression, baseName);
        return offset == null ? 0 : offset;
    }

    private String canonicalStorageExpression(String expression) {
        String value = normalizedExpression(expression);
        if (value == null) {
            return null;
        }
        value = value
            .replace("(longlong)", "")
            .replace("(ulonglong)", "")
            .replace("(undefined8)", "")
            .trim();
        return stripWrappingParens(value);
    }

    private String stripWrappingParens(String expression) {
        String value = expression == null ? null : expression.trim();
        while (value != null && value.startsWith("(") && value.endsWith(")")) {
            int end = matchingIndex(value, 0, '(', ')');
            if (end != value.length() - 1) {
                break;
            }
            value = value.substring(1, value.length() - 1).trim();
        }
        return value;
    }

    private String localTempName(String expression) {
        String value = canonicalStorageExpression(expression);
        if (value == null) {
            return null;
        }
        int bracket = value.indexOf('[');
        if (bracket >= 0) {
            value = value.substring(0, bracket).trim();
        }
        if (!value.matches("(local|local_res)[A-Za-z0-9_]*")) {
            return null;
        }
        return value;
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
            String helperParam =
                exactHelperParameterFromExpression(storage, fieldsByHelperParam);
            if (helperParam == null) {
                continue;
            }
            FieldCall field = fieldsByHelperParam.get(helperParam);
            refineMessageFieldType(
                field,
                call.templateType,
                "message-unmarshal-helper-nested-call",
                call.functionName,
                "helper-nested-marshaler");
        }
        for (ParsedUnmarshalCall call : parseDirectTypeUnmarshalCalls(helperText)) {
            String storage = storageArgumentForDirectUnmarshalCall(call);
            String helperParam =
                exactHelperParameterFromExpression(storage, fieldsByHelperParam);
            if (helperParam == null) {
                continue;
            }
            FieldCall field = fieldsByHelperParam.get(helperParam);
            refineMessageFieldType(
                field,
                call.templateType,
                "message-unmarshal-helper-direct-type-call",
                call.functionName,
                "helper-nested-direct-type");
        }
        for (ParsedReadRawCall call : parseReadRawCalls(helperText)) {
            String helperParam =
                exactHelperParameterFromExpression(call.storageExpression, fieldsByHelperParam);
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

        return addMessageField(
            plan,
            callsite,
            storageExpression,
            null,
            nativeType,
            confidence,
            recoveryOrder);
    }

    private FieldCall addPcodeMessageField(
        MessageUnmarshalPlan plan,
        Address callsite,
        PcodeStorage storage,
        String nativeType,
        String confidence,
        int recoveryOrder) {

        return addMessageField(
            plan,
            callsite,
            storage == null ? null : storage.expression(),
            storage,
            nativeType,
            confidence,
            recoveryOrder);
    }

    private FieldCall addMessageField(
        MessageUnmarshalPlan plan,
        Address callsite,
        String storageExpression,
        PcodeStorage pcodeStorage,
        String nativeType,
        String confidence,
        int recoveryOrder) {

        String storage = normalizedExpression(storageExpression);
        String storageIdentity = messageStorageIdentity(storageExpression, pcodeStorage);
        for (FieldCall existing : plan.fields) {
            if (sameMessageStorageIdentity(storageIdentity, existing)) {
                refineMessageFieldType(existing, nativeType, confidence);
                existing.recoveryOrder = Math.min(existing.recoveryOrder, recoveryOrder);
                return existing;
            }
            if (storage != null && storage.equals(normalizedExpression(existing.storageExpression))) {
                refineMessageFieldType(existing, nativeType, confidence);
                existing.recoveryOrder = Math.min(existing.recoveryOrder, recoveryOrder);
                if (existing.storageIdentity == null) {
                    existing.storageIdentity = storageIdentity;
                }
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
        field.storageIdentity = storageIdentity;
        field.storageOffset = storageByteOffsetFromExpression(storageExpression);
        if (pcodeStorage != null) {
            field.storageBase = pcodeStorage.base;
            field.storageBaseOffset = pcodeStorage.offset;
            field.storageOffset = pcodeStorage.offset;
        }
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

    private PcodeUnmarshalEvidence pcodeUnmarshalEvidence(
        PcodeOp op,
        PcodeCallTargetInfo targetInfo,
        String targetKind,
        PcodeStorage storage,
        Integer storageArgSlot,
        String evidenceSource,
        JsonArray argStorageEvidence) {

        PcodeUnmarshalEvidence evidence = new PcodeUnmarshalEvidence();
        evidence.callsite = op == null ? null : op.getSeqnum().getTarget();
        evidence.targetRawAddress = targetInfo == null ? null : targetInfo.rawTarget;
        evidence.target = targetInfo == null || "store-value-call".equals(targetKind)
            ? null
            : targetInfo.targetAddress();
        evidence.valueCallTarget = targetInfo == null || !"store-value-call".equals(targetKind)
            ? null
            : targetInfo.targetAddress();
        evidence.targetName = targetInfo == null || targetInfo.target == null
            ? null
            : fullFunctionName(targetInfo.target);
        evidence.targetKind = targetKind;
        evidence.targetExactStart = targetInfo == null ? null : targetInfo.targetExactStart;
        evidence.containingTarget = targetInfo == null || targetInfo.containing == null
            ? null
            : targetInfo.containing.getEntryPoint();
        evidence.containingTargetName = targetInfo == null || targetInfo.containing == null
            ? null
            : fullFunctionName(targetInfo.containing);
        evidence.storage = storage;
        evidence.storageArgSlot = storageArgSlot;
        evidence.evidenceSource = evidenceSource;
        evidence.argStorageEvidence = argStorageEvidence;
        return evidence;
    }

    private void applyPcodeUnmarshalEvidence(
        FieldCall field,
        PcodeUnmarshalEvidence evidence,
        boolean newField) {

        if (field == null || evidence == null) {
            return;
        }
        if (evidence.storage != null) {
            field.storageBase = evidence.storage.base;
            field.storageBaseOffset = evidence.storage.offset;
        }
        if (newField && !field.typeConflict()) {
            field.unmarshalCallsite = evidence.callsite;
            field.unmarshalTargetRaw = evidence.targetRawAddress;
            field.unmarshalTarget = evidence.target;
            field.valueCallTarget = evidence.valueCallTarget;
            field.unmarshalTargetName = evidence.targetName;
            field.unmarshalTargetKind = evidence.targetKind;
            field.unmarshalTargetExactStart = evidence.targetExactStart;
            field.unmarshalTargetContaining = evidence.containingTarget;
            field.unmarshalTargetContainingName = evidence.containingTargetName;
            field.storageArgSlot = evidence.storageArgSlot;
            field.evidenceSource = evidence.evidenceSource;
            field.argStorageEvidence = evidence.argStorageEvidence;
            return;
        }
        field.multipleCallEvidence = true;
        if (field.mergedCallsites == null) {
            field.mergedCallsites = new JsonArray();
            JsonObject current = new JsonObject();
            add(current, "callsite", formatAddress(field.unmarshalCallsite));
            add(current, "target", formatAddress(field.unmarshalTarget));
            add(current, "targetName", field.unmarshalTargetName);
            add(current, "targetKind", field.unmarshalTargetKind);
            field.mergedCallsites.add(current);
        }
        JsonObject merged = new JsonObject();
        add(merged, "callsite", formatAddress(evidence.callsite));
        add(merged, "target", formatAddress(evidence.target));
        add(merged, "valueCallTarget", formatAddress(evidence.valueCallTarget));
        add(merged, "targetName", evidence.targetName);
        add(merged, "targetKind", evidence.targetKind);
        field.mergedCallsites.add(merged);
    }

    private void applyTextUnmarshalEvidence(
        FieldCall field,
        Address callsite,
        String targetName,
        String targetKind,
        String evidenceSource) {

        if (field == null || targetName == null || targetName.isEmpty()) {
            return;
        }
        if (field.unmarshalTargetName != null || field.valueCallTarget != null ||
            field.unmarshalTarget != null) {
            return;
        }
        field.unmarshalCallsite = callsite;
        field.unmarshalTargetName = targetName;
        field.unmarshalTargetKind = targetKind;
        field.evidenceSource = evidenceSource;
    }

    private void recordPcodeMessageReject(
        MessageUnmarshalPlan plan,
        String phase,
        String reason,
        PcodeOp op,
        PcodeCallTargetInfo targetInfo,
        PcodeStorage storage,
        JsonArray argStorageEvidence) {

        if (plan == null) {
            return;
        }
        pcodeMessageFieldRejectedCount++;
        pcodeMessageFieldRejectCounts.merge(reason, 1, Integer::sum);
        JsonObject reject = new JsonObject();
        add(reject, "phase", phase);
        add(reject, "reason", reason);
        add(reject, "callsite", formatAddress(op == null ? null : op.getSeqnum().getTarget()));
        if (targetInfo != null) {
            add(reject, "targetRaw", formatAddress(targetInfo.rawTarget));
            add(reject, "target", formatAddress(targetInfo.targetAddress()));
            if (targetInfo.target != null) {
                add(reject, "targetName", fullFunctionName(targetInfo.target));
            }
            if (targetInfo.containing != null) {
                add(reject, "containingTarget", formatAddress(targetInfo.containing.getEntryPoint()));
                add(reject, "containingTargetName", fullFunctionName(targetInfo.containing));
            }
        }
        if (storage != null) {
            add(reject, "storageBase", storage.base);
            reject.addProperty("storageBaseOffset", "0x" + Long.toHexString(storage.offset));
            add(reject, "storageExpression", storage.expression());
        }
        if (argStorageEvidence != null && argStorageEvidence.size() != 0) {
            reject.add("argStorage", argStorageEvidence);
        }
        plan.rejectedPcodeFields.add(reject);
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

        refineMessageFieldType(field, nativeType, source, null, null);
    }

    private void refineMessageFieldType(
        FieldCall field,
        String nativeType,
        String source,
        String targetName,
        String targetKind) {

        if (field == null || nativeType == null || nativeType.isEmpty()) {
            return;
        }
        if (field.nativeType == null) {
            field.nativeType = nativeType;
            field.wireShape = wireShapeFromNativeType(nativeType);
            field.wireShapeSource = field.wireShape == null ? null : source;
            addMessageFieldTypeEvidence(field, nativeType, source, targetName, targetKind);
            return;
        }
        if (!field.nativeType.equals(nativeType)) {
            if (shouldReplaceWeakMessageFieldType(field, nativeType, source)) {
                addMessageFieldTypeEvidence(
                    field,
                    field.nativeType,
                    field.confidence,
                    null,
                    null);
                addMessageFieldTypeEvidence(field, nativeType, source, targetName, targetKind);
                field.sourceTypeName = appendDistinctType(field.sourceTypeName, field.nativeType);
                field.nativeType = nativeType;
                field.wireShape = wireShapeFromNativeType(nativeType);
                field.wireShapeSource = field.wireShape == null ? null : source;
                field.confidence = source;
                return;
            }
            addMessageFieldTypeEvidence(
                field,
                field.nativeType,
                field.confidence,
                null,
                null);
            addMessageFieldTypeEvidence(field, nativeType, source, targetName, targetKind);
            field.sourceTypeName = appendDistinctType(field.sourceTypeName, field.nativeType);
            field.sourceTypeName = appendDistinctType(field.sourceTypeName, nativeType);
            field.nativeType = "composite";
            field.typeConflict = true;
            field.wireShape = null;
            field.wireShapeSource = null;
        }
    }

    private void addMessageFieldTypeEvidence(
        FieldCall field,
        String nativeType,
        String source,
        String targetName,
        String targetKind) {

        if (field == null || nativeType == null || nativeType.isEmpty()) {
            return;
        }
        if (field.typeEvidence == null) {
            field.typeEvidence = new JsonArray();
        }
        for (JsonElement element : field.typeEvidence) {
            if (!element.isJsonObject()) {
                continue;
            }
            JsonObject existing = element.getAsJsonObject();
            if (Objects.equals(jsonString(existing, "nativeType"), nativeType) &&
                Objects.equals(jsonString(existing, "source"), source) &&
                Objects.equals(jsonString(existing, "targetName"), targetName) &&
                Objects.equals(jsonString(existing, "targetKind"), targetKind)) {
                return;
            }
        }
        JsonObject evidence = new JsonObject();
        add(evidence, "nativeType", nativeType);
        add(evidence, "source", source);
        add(evidence, "targetName", targetName);
        add(evidence, "targetKind", targetKind);
        String wireShape = wireShapeFromNativeType(nativeType);
        add(evidence, "wireShape", wireShape);
        field.typeEvidence.add(evidence);
    }

    private String jsonString(JsonObject object, String name) {
        JsonElement value = object == null ? null : object.get(name);
        return value == null || value.isJsonNull() ? null : value.getAsString();
    }

    private boolean shouldReplaceWeakMessageFieldType(
        FieldCall field,
        String nativeType,
        String source) {

        if (field == null || nativeType == null || source == null) {
            return false;
        }
        if (!source.contains("marshaler") &&
            !source.contains("direct-type") &&
            !source.contains("nested-call")) {
            return false;
        }
        String confidence = field.confidence == null ? "" : field.confidence;
        if (!confidence.contains("temp-store") &&
            !confidence.contains("direct-object-store") &&
            !confidence.contains("raw-temp-store")) {
            return false;
        }
        return "u64".equals(field.nativeType) ||
            "u32".equals(field.nativeType) ||
            "u16".equals(field.nativeType) ||
            "u8".equals(field.nativeType);
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
                ecx = literalOperandValue(instruction, 1);
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
        long unit = storageOffsetUnit(base, matcher.group("offset"));
        return offset * unit;
    }

    private long storageOffsetUnit(String base, String offsetText) {
        if (base == null) {
            return 1L;
        }
        if (base.startsWith("plVar") || base.startsWith("puVar")) {
            return 8L;
        }
        if (!isHexIntegerLiteral(offsetText) && isMessageObjectPointerBase(base)) {
            return 8L;
        }
        return 1L;
    }

    private boolean isMessageObjectPointerBase(String base) {
        return "this".equals(base) || "param_3".equals(base) || base.startsWith("_Dst");
    }

    private boolean isHexIntegerLiteral(String value) {
        return value != null && value.matches("(?i)0x[0-9a-f]+");
    }

    private String messageStorageIdentity(String expression, PcodeStorage pcodeStorage) {
        if (pcodeStorage != null) {
            return pcodeStorage.base + ":0x" + Long.toHexString(pcodeStorage.offset);
        }
        String normalized = normalizedExpression(expression);
        if (normalized == null) {
            return null;
        }
        String base = firstIdentifier(normalized);
        Long byteOffset = storageByteOffsetFromExpression(normalized);
        if (base == null || byteOffset == null) {
            return normalized;
        }
        return base + ":0x" + Long.toHexString(byteOffset);
    }

    private boolean sameMessageStorageIdentity(String identity, FieldCall field) {
        return identity != null && field != null && identity.equals(field.storageIdentity);
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
                null,
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

            String templateType = text.substring(templateStart + 1, templateEnd).trim();
            result.add(new ParsedUnmarshalCall(
                templateType,
                "GridMate::Marshaler<" + templateType + ">::Unmarshal",
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
                owner + "::Unmarshal",
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
            value.matches(".*\\b(this|param_\\d+)\\s*\\+\\s*(0x[0-9a-fA-F]+|\\d+).*") ||
            value.matches("\\b(p[lub]?Var\\d+|p[lub]?Var)\\s*\\+\\s*(0x[0-9a-fA-F]+|\\d+).*");
    }

    private boolean isLikelyTemporaryStorage(String expression) {
        if (expression == null) {
            return false;
        }
        String value = normalizedExpression(expression);
        return value != null && value.matches("local_[A-Za-z0-9_]+(\\[[0-9]+\\])?");
    }

    private String firstIdentifier(String expression) {
        if (expression == null) {
            return null;
        }
        Matcher matcher = Pattern.compile("\\b[A-Za-z_][A-Za-z0-9_]*\\b").matcher(expression);
        return matcher.find() ? matcher.group() : null;
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

    private String exactHelperParameterFromExpression(
        String expression,
        Map<String, FieldCall> fieldsByHelperParam) {

        String value = normalizedExpression(expression);
        if (value == null || fieldsByHelperParam.isEmpty()) {
            return null;
        }
        String castStripped = stripSimpleCasts(value);
        for (String helperParam : fieldsByHelperParam.keySet()) {
            if (value.equals(helperParam) ||
                castStripped.equals(helperParam) ||
                value.matches("\\b" + Pattern.quote(helperParam) + "\\s*\\+\\s*0\\b") ||
                castStripped.matches("\\b" + Pattern.quote(helperParam) + "\\s*\\+\\s*0\\b")) {
                return helperParam;
            }
        }
        return null;
    }

    private String stripSimpleCasts(String expression) {
        if (expression == null) {
            return "";
        }
        return expression.replaceAll("\\([^()]*\\)", "").trim();
    }

    private List<String> callArgumentsForTarget(String text, Function target) {
        return callArgumentsForTarget(text, target, 0);
    }

    private List<String> callArgumentsForTarget(
        String text,
        Function target,
        int occurrence) {

        if (text == null || target == null) {
            return List.of();
        }
        for (String name : functionCallNameCandidates(target)) {
            List<String> args = callArgumentsForName(text, name, occurrence);
            if (!args.isEmpty()) {
                return args;
            }
        }
        return List.of();
    }

    private int callTextIndexForTarget(String text, Function target) {
        return callTextIndexForTarget(text, target, 0);
    }

    private int callTextIndexForTarget(String text, Function target, int occurrence) {
        if (text == null || target == null) {
            return Integer.MAX_VALUE;
        }
        for (String name : functionCallNameCandidates(target)) {
            int index = callTextIndexForName(text, name, occurrence);
            if (index >= 0) {
                return index;
            }
        }
        return Integer.MAX_VALUE;
    }

    private int callTextIndexForName(String text, String name) {
        return callTextIndexForName(text, name, 0);
    }

    private int callTextIndexForName(String text, String name, int occurrence) {
        if (text == null || name == null || name.isEmpty()) {
            return -1;
        }
        int matched = 0;
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
            if (matched++ == occurrence) {
                return nameIndex;
            }
            int argsEnd = matchingIndex(text, argsStart, '(', ')');
            search = argsEnd < 0 ? nameIndex + name.length() : argsEnd + 1;
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
        return callArgumentsForName(text, name, 0);
    }

    private List<String> callArgumentsForName(String text, String name, int occurrence) {
        if (text == null || name == null || name.isEmpty()) {
            return List.of();
        }
        int matched = 0;
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
            if (matched++ == occurrence) {
                return splitTopLevel(text.substring(argsStart + 1, argsEnd));
            }
            search = argsEnd + 1;
        }
        return List.of();
    }

    private int callOccurrenceIndex(
        Function owner,
        Instruction currentInstruction,
        Function target) {

        if (owner == null || currentInstruction == null || target == null) {
            return 0;
        }
        int occurrence = 0;
        Address currentAddress = currentInstruction.getMinAddress();
        Address targetAddress = target.getEntryPoint();
        for (Instruction instruction : functionInstructions(owner)) {
            if (!instruction.getFlowType().isCall()) {
                continue;
            }
            Function candidate = functionAtOrContaining(callTarget(instruction));
            if (candidate == null ||
                !candidate.getEntryPoint().equals(targetAddress)) {
                continue;
            }
            if (instruction.getMinAddress().equals(currentAddress)) {
                return occurrence;
            }
            occurrence++;
        }
        return 0;
    }

    private boolean isLeafUnmarshalTarget(Function target) {
        String name = fullFunctionName(target);
        if (name == null) {
            return false;
        }
        return name.contains("::Unmarshal") ||
            name.contains("Marshaler<") ||
            name.contains("Marshaller<");
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

    private HighFunction highFunction(Function function) {
        if (function == null || decompiler == null) {
            return null;
        }
        String key = functionCacheKey("high-function", function);
        if (highFunctionCache.containsKey(key)) {
            return highFunctionCache.get(key);
        }
        try {
            DecompileResults results = decompiler.decompileFunction(function, 30, monitor);
            if (!results.decompileCompleted() || results.getHighFunction() == null) {
                highFunctionCache.put(key, null);
                return null;
            }
            HighFunction high = results.getHighFunction();
            highFunctionCache.put(key, high);
            return high;
        }
        catch (Exception ignored) {
            highFunctionCache.put(key, null);
            return null;
        }
    }

    private Function pcodeCallTarget(PcodeOp op) {
        PcodeCallTargetInfo info = pcodeCallTargetInfo(op);
        return info == null ? null : info.target;
    }

    private PcodeCallTargetInfo pcodeCallTargetInfo(PcodeOp op) {
        if (op == null || op.getOpcode() != PcodeOp.CALL || op.getNumInputs() == 0) {
            return null;
        }
        Address rawAddress = pcodeAddress(op.getInput(0));
        Address resolvedAddress = resolvedCodeTarget(rawAddress);
        Function exact = functionExactlyAt(resolvedAddress);
        Function containing = exact == null ? functionContaining(resolvedAddress) : null;
        PcodeCallTargetInfo info = new PcodeCallTargetInfo();
        info.rawTarget = rawAddress;
        info.resolvedTarget = resolvedAddress;
        info.target = exact;
        info.targetExactStart = exact != null;
        info.containing = containing;
        return info;
    }

    private PcodeCallTargetInfo pcodeValueCallTargetInfo(Varnode node) {
        return pcodeValueCallTargetInfo(node, new LinkedHashSet<>(), 0);
    }

    private PcodeCallTargetInfo pcodeValueCallTargetInfo(
        Varnode node,
        Set<String> seen,
        int depth) {

        if (node == null || depth > PCODE_VALUE_DEPTH_LIMIT) {
            return null;
        }
        String key = pcodeNodeKey(node);
        if (!seen.add(key)) {
            return null;
        }
        PcodeOp def = node.getDef();
        if (def == null) {
            return null;
        }
        switch (def.getOpcode()) {
            case PcodeOp.CALL:
                return pcodeCallTargetInfo(def);
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.INT_ZEXT:
            case PcodeOp.INT_SEXT:
            case PcodeOp.SUBPIECE:
            case PcodeOp.INDIRECT:
                return def.getNumInputs() == 0
                    ? null
                    : pcodeValueCallTargetInfo(def.getInput(0), seen, depth + 1);
            case PcodeOp.MULTIEQUAL:
                PcodeCallTargetInfo merged = null;
                for (int i = 0; i < def.getNumInputs(); i++) {
                    PcodeCallTargetInfo value =
                        pcodeValueCallTargetInfo(def.getInput(i), seen, depth + 1);
                    if (value == null) {
                        continue;
                    }
                    if (merged == null) {
                        merged = value;
                    }
                    else if (!sameAddressValue(merged.resolvedTarget, value.resolvedTarget)) {
                        return null;
                    }
                }
                return merged;
            default:
                return null;
        }
    }

    private Address pcodeAddress(Varnode node) {
        if (node == null) {
            return null;
        }
        Address address = node.getAddress();
        if (isProgramAddress(address)) {
            return address;
        }
        if (node.isConstant()) {
            address = absoluteAddress(node.getOffset());
            if (isProgramAddress(address)) {
                return address;
            }
        }
        return null;
    }

    private Integer returnedCallInputSlot(Function target) {
        if (target == null) {
            return null;
        }
        String key = functionCacheKey("returned-call-input-slot", target);
        if (returnedCallInputSlotCache.containsKey(key)) {
            return returnedCallInputSlotCache.get(key);
        }

        HighFunction high = highFunction(target);
        if (high == null) {
            returnedCallInputSlotCache.put(key, null);
            return null;
        }

        Integer merged = null;
        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOpAST op = ops.next();
            if (op.getOpcode() != PcodeOp.RETURN || op.getNumInputs() < 2) {
                continue;
            }
            Integer slot =
                returnedCallInputSlotFromValue(op.getInput(1), new LinkedHashSet<>(), 0);
            if (slot == null) {
                returnedCallInputSlotCache.put(key, null);
                return null;
            }
            if (merged == null) {
                merged = slot;
            }
            else if (!merged.equals(slot)) {
                returnedCallInputSlotCache.put(key, null);
                return null;
            }
        }
        returnedCallInputSlotCache.put(key, merged);
        return merged;
    }

    private Integer returnedCallInputSlotFromValue(
        Varnode node,
        Set<String> seen,
        int depth) {

        if (node == null || depth > PCODE_VALUE_DEPTH_LIMIT) {
            return null;
        }
        String key = "return-slot:" + pcodeNodeKey(node);
        if (!seen.add(key)) {
            return null;
        }

        Integer direct = callInputSlotForRegister(node);
        if (direct != null) {
            return direct;
        }

        PcodeOp def = node.getDef();
        if (def == null) {
            return null;
        }
        switch (def.getOpcode()) {
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.INT_ZEXT:
            case PcodeOp.INT_SEXT:
            case PcodeOp.SUBPIECE:
            case PcodeOp.INDIRECT:
                return def.getNumInputs() == 0
                    ? null
                    : returnedCallInputSlotFromValue(def.getInput(0), seen, depth + 1);
            case PcodeOp.MULTIEQUAL:
                Integer merged = null;
                for (int i = 0; i < def.getNumInputs(); i++) {
                    Integer slot = returnedCallInputSlotFromValue(
                        def.getInput(i),
                        new LinkedHashSet<>(seen),
                        depth + 1);
                    if (slot == null) {
                        return null;
                    }
                    if (merged == null) {
                        merged = slot;
                    }
                    else if (!merged.equals(slot)) {
                        return null;
                    }
                }
                return merged;
            default:
                return null;
        }
    }

    private Integer callInputSlotForRegister(Varnode node) {
        String register = pcodeBaseRegisterName(node);
        if (register == null) {
            return null;
        }
        return switch (register) {
            case "RCX" -> 1;
            case "RDX" -> 2;
            case "R8" -> 3;
            case "R9" -> 4;
            default -> null;
        };
    }

    private String pcodeBaseRegisterName(Varnode node) {
        if (node == null || !node.isRegister() || node.getAddress() == null) {
            return null;
        }
        try {
            Register register =
                currentProgram.getLanguage().getRegister(node.getAddress(), node.getSize());
            if (register == null) {
                return null;
            }
            Register base = register.getBaseRegister();
            return (base == null ? register : base)
                .getName()
                .toUpperCase(Locale.ROOT);
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private boolean isStackPointerVarnode(Varnode node) {
        String registerName = pcodeBaseRegisterName(node);
        if (registerName == null) {
            return false;
        }
        try {
            Register stackPointer = currentProgram.getCompilerSpec().getStackPointer();
            if (stackPointer != null) {
                Register stackBase = stackPointer.getBaseRegister();
                String stackName = (stackBase == null ? stackPointer : stackBase)
                    .getName()
                    .toUpperCase(Locale.ROOT);
                return registerName.equals(stackName);
            }
        }
        catch (Exception ignored) {
        }
        return "RSP".equals(registerName) ||
            "ESP".equals(registerName) ||
            "SP".equals(registerName);
    }

    private boolean sameAddressValue(Address left, Address right) {
        if (left == null || right == null) {
            return left == right;
        }
        return left.equals(right);
    }

    private PcodeStorage pcodeStorageExpression(Varnode node) {
        return pcodeStorageExpression(node, new LinkedHashSet<>(), 0);
    }

    private PcodeStorage pcodeStorageExpression(
        Varnode node,
        Set<String> seen,
        int depth) {

        if (node == null || depth > PCODE_VALUE_DEPTH_LIMIT) {
            return null;
        }
        String key = pcodeNodeKey(node);
        if (!seen.add(key)) {
            return null;
        }

        PcodeOp def = node.getDef();
        if (def != null) {
            PcodeStorage storage = pcodeStorageFromDef(def, seen, depth + 1);
            if (storage != null) {
                return storage;
            }
        }

        if (node.getAddress() != null && node.getAddress().isStackAddress()) {
            return new PcodeStorage("stack", node.getOffset());
        }
        if (isStackPointerVarnode(node)) {
            return new PcodeStorage("stack", 0L);
        }

        String name = pcodeHighName(node);
        if (isPcodeStorageBase(name)) {
            return new PcodeStorage(name, 0L);
        }
        return null;
    }

    private PcodeStorage pcodeStorageFromDef(
        PcodeOp def,
        Set<String> seen,
        int depth) {

        switch (def.getOpcode()) {
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.SUBPIECE:
            case PcodeOp.INDIRECT:
                return def.getNumInputs() == 0
                    ? null
                    : pcodeStorageExpression(def.getInput(0), seen, depth);
            case PcodeOp.PTRADD:
                return pcodePointerAddExpression(def, seen, depth);
            case PcodeOp.PTRSUB:
                return pcodePointerSubExpression(def, seen, depth);
            case PcodeOp.INT_ADD:
                return pcodeAddExpression(def, seen, depth, true);
            case PcodeOp.INT_SUB:
                return pcodeAddExpression(def, seen, depth, false);
            case PcodeOp.MULTIEQUAL:
                return pcodeMergedStorageExpression(def, seen, depth);
            default:
                return null;
        }
    }

    private PcodeStorage pcodePointerAddExpression(
        PcodeOp def,
        Set<String> seen,
        int depth) {

        if (def.getNumInputs() < 3) {
            return null;
        }
        PcodeStorage base =
            pcodeStorageExpression(def.getInput(0), new LinkedHashSet<>(seen), depth);
        Long index = pcodeConstantValue(def.getInput(1), new LinkedHashSet<>(seen), depth);
        Long elementSize =
            pcodeConstantValue(def.getInput(2), new LinkedHashSet<>(seen), depth);
        if (base == null || index == null || elementSize == null) {
            return null;
        }
        return base.plus(index * elementSize);
    }

    private PcodeStorage pcodePointerSubExpression(
        PcodeOp def,
        Set<String> seen,
        int depth) {

        if (def.getNumInputs() < 2) {
            return null;
        }
        PcodeStorage base =
            pcodeStorageExpression(def.getInput(0), new LinkedHashSet<>(seen), depth);
        Long offset = pcodeConstantValue(def.getInput(1), new LinkedHashSet<>(seen), depth);
        if (base == null || offset == null) {
            return null;
        }
        return base.plus(offset);
    }

    private PcodeStorage pcodeAddExpression(
        PcodeOp def,
        Set<String> seen,
        int depth,
        boolean add) {

        if (def.getNumInputs() < 2) {
            return null;
        }
        PcodeStorage left =
            pcodeStorageExpression(def.getInput(0), new LinkedHashSet<>(seen), depth);
        Long right = pcodeConstantValue(def.getInput(1), new LinkedHashSet<>(seen), depth);
        if (left != null && right != null) {
            return left.plus(add ? right : -right);
        }
        if (add) {
            PcodeStorage rightStorage =
                pcodeStorageExpression(def.getInput(1), new LinkedHashSet<>(seen), depth);
            Long leftConstant =
                pcodeConstantValue(def.getInput(0), new LinkedHashSet<>(seen), depth);
            if (rightStorage != null && leftConstant != null) {
                return rightStorage.plus(leftConstant);
            }
        }
        return null;
    }

    private PcodeStorage pcodeMergedStorageExpression(
        PcodeOp def,
        Set<String> seen,
        int depth) {

        PcodeStorage merged = null;
        for (int i = 0; i < def.getNumInputs(); i++) {
            PcodeStorage storage =
                pcodeStorageExpression(def.getInput(i), new LinkedHashSet<>(seen), depth);
            if (storage == null) {
                return null;
            }
            if (merged == null) {
                merged = storage;
            }
            else if (!merged.sameLocation(storage)) {
                return null;
            }
        }
        return merged;
    }

    private String pcodeValueNativeType(
        Varnode node,
        Map<String, String> tempNativeTypes) {
        return pcodeValueNativeType(node, tempNativeTypes, new LinkedHashSet<>(), 0);
    }

    private String pcodeValueWireShape(
        Varnode node,
        Map<String, String> tempWireShapes,
        Map<String, String> tempNativeTypes) {
        return pcodeValueWireShape(
            node,
            tempWireShapes,
            tempNativeTypes,
            new LinkedHashSet<>(),
            0);
    }

    private String pcodeValueWireShape(
        Varnode node,
        Map<String, String> tempWireShapes,
        Map<String, String> tempNativeTypes,
        Set<String> seen,
        int depth) {

        if (node == null || depth > PCODE_VALUE_DEPTH_LIMIT) {
            return null;
        }
        String tempShape = pcodeTempWireShape(node, tempWireShapes);
        if (tempShape != null) {
            return tempShape;
        }
        String nativeShape = wireShapeFromNativeType(pcodeTempNativeType(node, tempNativeTypes));
        if (nativeShape != null) {
            return nativeShape;
        }
        String key = "shape:" + pcodeNodeKey(node);
        if (!seen.add(key)) {
            return null;
        }

        PcodeOp def = node.getDef();
        if (def == null) {
            return wireShapeFromNativeType(pcodeValueNativeType(node, tempNativeTypes));
        }

        switch (def.getOpcode()) {
            case PcodeOp.CALL: {
                Function target = pcodeCallTarget(def);
                String nativeType = unmarshalNativeTypeFromTarget(target);
                String shape = wireShapeFromNativeType(nativeType);
                if (shape != null) {
                    return shape;
                }
                return wireShapeFromNativeType(scalarOutputStoreNativeType(target));
            }
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.SUBPIECE:
            case PcodeOp.INDIRECT:
                return def.getNumInputs() == 0
                    ? null
                    : pcodeValueWireShape(
                        def.getInput(0),
                        tempWireShapes,
                        tempNativeTypes,
                        seen,
                        depth + 1);
            case PcodeOp.INT_EQUAL:
            case PcodeOp.INT_NOTEQUAL:
            case PcodeOp.INT_LESS:
            case PcodeOp.INT_LESSEQUAL:
            case PcodeOp.INT_SLESS:
            case PcodeOp.INT_SLESSEQUAL:
            case PcodeOp.BOOL_AND:
            case PcodeOp.BOOL_OR:
            case PcodeOp.BOOL_XOR:
            case PcodeOp.BOOL_NEGATE:
                return node.getSize() == 1 ? "bool" : null;
            case PcodeOp.MULTIEQUAL:
                return pcodeMergedValueWireShape(
                    def,
                    tempWireShapes,
                    tempNativeTypes,
                    seen,
                    depth + 1);
            default:
                return wireShapeFromNativeType(pcodeValueNativeType(node, tempNativeTypes));
        }
    }

    private String pcodeMergedValueWireShape(
        PcodeOp def,
        Map<String, String> tempWireShapes,
        Map<String, String> tempNativeTypes,
        Set<String> seen,
        int depth) {

        String merged = null;
        for (int i = 0; i < def.getNumInputs(); i++) {
            String valueShape =
                pcodeValueWireShape(
                    def.getInput(i),
                    tempWireShapes,
                    tempNativeTypes,
                    new LinkedHashSet<>(seen),
                    depth);
            if (valueShape == null) {
                return null;
            }
            if (merged == null) {
                merged = valueShape;
            }
            else if (!merged.equals(valueShape)) {
                return null;
            }
        }
        return merged;
    }

    private String pcodeValueNativeType(
        Varnode node,
        Map<String, String> tempNativeTypes,
        Set<String> seen,
        int depth) {

        if (node == null || depth > PCODE_VALUE_DEPTH_LIMIT) {
            return null;
        }
        String tempType = pcodeTempNativeType(node, tempNativeTypes);
        if (tempType != null) {
            return tempType;
        }
        String key = "type:" + pcodeNodeKey(node);
        if (!seen.add(key)) {
            return null;
        }

        PcodeOp def = node.getDef();
        if (def == null) {
            return null;
        }

        switch (def.getOpcode()) {
            case PcodeOp.CALL:
                return unmarshalNativeTypeFromTarget(pcodeCallTarget(def));
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.SUBPIECE:
            case PcodeOp.INDIRECT:
                return def.getNumInputs() == 0
                    ? null
                    : pcodeValueNativeType(
                        def.getInput(0),
                        tempNativeTypes,
                        seen,
                        depth + 1);
            case PcodeOp.INT_EQUAL:
            case PcodeOp.INT_NOTEQUAL:
            case PcodeOp.INT_LESS:
            case PcodeOp.INT_LESSEQUAL:
            case PcodeOp.INT_SLESS:
            case PcodeOp.INT_SLESSEQUAL:
            case PcodeOp.BOOL_AND:
            case PcodeOp.BOOL_OR:
            case PcodeOp.BOOL_XOR:
            case PcodeOp.BOOL_NEGATE:
                return node.getSize() == 1 ? "bool" : null;
            case PcodeOp.MULTIEQUAL:
                return pcodeMergedValueNativeType(
                    def,
                    tempNativeTypes,
                    seen,
                    depth + 1);
            default:
                return null;
        }
    }

    private String pcodeMergedValueNativeType(
        PcodeOp def,
        Map<String, String> tempNativeTypes,
        Set<String> seen,
        int depth) {

        String merged = null;
        for (int i = 0; i < def.getNumInputs(); i++) {
            String valueType =
                pcodeValueNativeType(
                    def.getInput(i),
                    tempNativeTypes,
                    new LinkedHashSet<>(seen),
                    depth);
            if (valueType == null) {
                return null;
            }
            if (merged == null) {
                merged = valueType;
            }
            else if (!merged.equals(valueType)) {
                return null;
            }
        }
        return merged;
    }

    private String pcodeTempNativeType(
        Varnode node,
        Map<String, String> tempNativeTypes) {

        if (tempNativeTypes == null || tempNativeTypes.isEmpty()) {
            return null;
        }
        PcodeStorage storage = pcodeStorageExpression(node);
        if (!isPcodeLocalTempStorage(storage)) {
            return null;
        }
        return tempNativeTypes.get(storageKey(storage));
    }

    private String pcodeTempWireShape(
        Varnode node,
        Map<String, String> tempWireShapes) {

        if (tempWireShapes == null || tempWireShapes.isEmpty()) {
            return null;
        }
        PcodeStorage storage = pcodeStorageExpression(node);
        if (!isPcodeLocalTempStorage(storage)) {
            return null;
        }
        return tempWireShapes.get(storageKey(storage));
    }

    private Long pcodeConstantValue(Varnode node, Set<String> seen, int depth) {
        if (node == null || depth > PCODE_VALUE_DEPTH_LIMIT) {
            return null;
        }
        if (node.isConstant()) {
            return node.getOffset();
        }
        String key = "constant:" + pcodeNodeKey(node);
        if (!seen.add(key)) {
            return null;
        }
        PcodeOp def = node.getDef();
        if (def == null || def.getNumInputs() == 0) {
            return null;
        }
        switch (def.getOpcode()) {
            case PcodeOp.COPY:
            case PcodeOp.CAST:
            case PcodeOp.INT_ZEXT:
            case PcodeOp.INT_SEXT:
            case PcodeOp.SUBPIECE:
                return pcodeConstantValue(def.getInput(0), seen, depth + 1);
            case PcodeOp.INT_ADD: {
                Long left =
                    pcodeConstantValue(def.getInput(0), new LinkedHashSet<>(seen), depth + 1);
                Long right =
                    pcodeConstantValue(def.getInput(1), new LinkedHashSet<>(seen), depth + 1);
                return left == null || right == null ? null : left + right;
            }
            case PcodeOp.INT_SUB: {
                Long left =
                    pcodeConstantValue(def.getInput(0), new LinkedHashSet<>(seen), depth + 1);
                Long right =
                    pcodeConstantValue(def.getInput(1), new LinkedHashSet<>(seen), depth + 1);
                return left == null || right == null ? null : left - right;
            }
            default:
                return null;
        }
    }

    private String pcodeHighName(Varnode node) {
        if (!(node instanceof VarnodeAST ast) || ast.getHigh() == null) {
            return null;
        }
        try {
            String name = ast.getHigh().getName();
            if (name == null || name.isBlank() || name.startsWith("UNNAMED")) {
                return null;
            }
            return name;
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private boolean isPcodeStorageBase(String name) {
        return name != null &&
            (name.equals("this") ||
                name.equals("_Dst") ||
                name.matches("param_\\d+") ||
                name.matches("p[lub]?Var\\d*"));
    }

    private String pcodeNodeKey(Varnode node) {
        if (node == null) {
            return "<null>";
        }
        return node.getAddress() + ":" + node.getOffset() + ":" + node.getSize() +
            ":" + node.isConstant();
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
        boolean changed;
        do {
            changed = false;
            while (result.startsWith("&")) {
                result = result.substring(1).trim();
                changed = true;
            }
            while (result.startsWith("(")) {
                int end = matchingIndex(result, 0, '(', ')');
                if (end <= 0) {
                    break;
                }
                String inner = result.substring(1, end).trim();
                if (isLikelyCastType(inner)) {
                    result = result.substring(end + 1).trim();
                    changed = true;
                    continue;
                }
                if (end == result.length() - 1) {
                    result = inner;
                    changed = true;
                    continue;
                }
                break;
            }
        } while (changed);
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
        String normalized = normalizeNativeType(nativeType);
        if ("bool".equals(normalized)) {
            return "bool";
        }
        if ("u8".equals(normalized) ||
            "uint8_t".equals(normalized) ||
            "std::uint8_t".equals(normalized) ||
            "AZ::u8".equals(normalized) ||
            "i8".equals(normalized) ||
            "int8_t".equals(normalized) ||
            "std::int8_t".equals(normalized) ||
            "AZ::s8".equals(normalized) ||
            "unsigned char".equals(normalized) ||
            "char".equals(normalized)) {
            return "u8";
        }
        if ("u16".equals(normalized) ||
            "uint16_t".equals(normalized) ||
            "std::uint16_t".equals(normalized) ||
            "AZ::u16".equals(normalized) ||
            "i16".equals(normalized) ||
            "int16_t".equals(normalized) ||
            "std::int16_t".equals(normalized) ||
            "AZ::s16".equals(normalized)) {
            return "u16";
        }
        if ("u32".equals(normalized) ||
            "uint32_t".equals(normalized) ||
            "std::uint32_t".equals(normalized) ||
            "AZ::u32".equals(normalized) ||
            "i32".equals(normalized) ||
            "int32_t".equals(normalized) ||
            "std::int32_t".equals(normalized) ||
            "AZ::s32".equals(normalized) ||
            "AZ::Crc32".equals(normalized) ||
            "FragmentKey".equals(normalized) ||
            "Amazon::Hub::FragmentKey".equals(normalized)) {
            return "u32";
        }
        if ("u64".equals(normalized) ||
            "uint64_t".equals(normalized) ||
            "std::uint64_t".equals(normalized) ||
            "AZ::u64".equals(normalized) ||
            "i64".equals(normalized) ||
            "int64_t".equals(normalized) ||
            "std::int64_t".equals(normalized) ||
            "AZ::s64".equals(normalized) ||
            "AZ::EntityId".equals(normalized) ||
            "TimePoint".equals(normalized) ||
            "MB::TimePoint".equals(normalized) ||
            "WallClockTimePoint".equals(normalized) ||
            "MB::WallClockTimePoint".equals(normalized)) {
            return "u64";
        }
        if ("f32".equals(normalized) || "float".equals(normalized)) {
            return "f32";
        }
        if ("f64".equals(normalized) || "double".equals(normalized)) {
            return "f64";
        }
        if ("AZ::Vector2".equals(normalized)) {
            return "vec2";
        }
        if ("AZ::Vector3".equals(normalized)) {
            return "vec3";
        }
        if ("AZ::Vector4".equals(normalized)) {
            return "vec4";
        }
        if ("AZ::Quaternion".equals(normalized)) {
            return "quat";
        }
        if ("AZ::Matrix3x3".equals(normalized)) {
            return "mat3";
        }
        if ("AZ::Transform".equals(normalized)) {
            return "affine3";
        }
        if ("AZ::Bounds".equals(normalized)) {
            return "aabb2d";
        }
        if ("AZ::Aabb".equals(normalized)) {
            return "aabb3d";
        }
        if ("EntityRef".equals(normalized)) {
            return "entity-ref";
        }
        if ("AZStd::string".equals(normalized) ||
            "std::string".equals(normalized) ||
            "string".equals(normalized)) {
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

    private String normalizeNativeType(String value) {
        if (value == null) {
            return null;
        }
        String result = value
            .replace('\r', ' ')
            .replace('\n', ' ')
            .replace('\t', ' ')
            .replaceAll("\\s+", " ")
            .replace(" ,", ",")
            .replace(", ", ",")
            .replace(" <", "<")
            .replace("< ", "<")
            .replace(" >", ">")
            .trim();
        return result.isEmpty() ? null : result;
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
        field.handlerConstruction = state.handlerConstruction;
        field.constructorWrites = state.handlerConstructorWrites;
        field.registrationKind = "field";
        FieldHandlerShape shape = fieldHandlerShape(field.handlerVtable);
        if (shape != null) {
            field.handlerKind = shape.kind;
            field.handlerVtableSlots = shape.vtableSlots;
            if (shape.wireShape != null) {
                field.wireShape = shape.wireShape.shape;
                field.wireShapeSource = shape.wireShape.source;
            }
        }
        field.handlerTypeName = fieldHandlerTypeName(field.handlerConstruction, field.handlerVtable);
        enrichFieldFromHandlerType(field);
        field.confidence = field.name == null
            ? "register-field-call-unresolved-name"
            : "register-field-call";
        return field;
    }

    private void enrichFieldFromHandlerType(FieldCall field) {
        if (field == null || field.handlerTypeName == null) {
            return;
        }

        if (field.nativeType == null) {
            field.nativeType = field.handlerTypeName;
        }
        if (field.sourceTypeName == null) {
            field.sourceTypeName = field.handlerTypeName;
        }
        if (field.wireShape == null) {
            WireShape wireShape = wireShapeFromHandlerTypeName(field.handlerTypeName);
            if (wireShape != null) {
                field.wireShape = wireShape.shape;
                field.wireShapeSource = wireShape.source;
            }
        }
    }

    private WireShape wireShapeFromHandlerTypeName(String handlerTypeName) {
        NetworkTemplateType type = parseNetworkTemplateType(handlerTypeName);
        if (type == null) {
            return null;
        }

        if (type.simpleName.equals("ReplicatedFieldHandler") && !type.args.isEmpty()) {
            String shape = wireShapeFromReplicatedFieldHandlerArgs(type.args);
            return shape == null ? null : new WireShape(shape, "handler-template-type");
        }

        if (type.simpleName.equals("ReplicatedMapFieldHandler") ||
            type.simpleName.equals("ReplicatedContainer")) {
            String shape = wireShapeFromKeyValueHandlerArgs(type.args);
            return shape == null ? null : new WireShape(shape, "handler-template-type");
        }

        if (type.simpleName.equals("ReplicatedVectorFieldHandler") && !type.args.isEmpty()) {
            String valueShape = wireShapeFromNativeTypeOrMarshaller(type.args.get(0), null);
            return valueShape == null
                ? null
                : new WireShape(containerShape("u32", valueShape), "handler-template-type");
        }

        if (type.simpleName.equals("ReplicatedSetFieldHandler") && !type.args.isEmpty()) {
            String keyShape = wireShapeFromNativeTypeOrMarshaller(type.args.get(0), null);
            return keyShape == null
                ? null
                : new WireShape(containerShape(keyShape, keyShape), "handler-template-type");
        }

        if ((type.simpleName.equals("DeltaCompressedReplicatedFieldHandler") ||
            type.simpleName.equals("DeltaCompressedReplicatedFieldHandlerBase") ||
            type.simpleName.equals("DynamicDeltaReplicatedFieldHandler")) &&
            !type.args.isEmpty()) {
            String shape = wireShapeFromNativeTypeOrMarshaller(type.args.get(0), null);
            return shape == null ? null : new WireShape(shape, "handler-template-type");
        }
        return null;
    }

    private String wireShapeFromReplicatedFieldHandlerArgs(List<String> args) {
        String nativeType = args.isEmpty() ? null : args.get(0);
        String marshallerType = args.size() > 1 ? args.get(1) : null;
        return wireShapeFromNativeTypeOrMarshaller(nativeType, marshallerType);
    }

    private String wireShapeFromKeyValueHandlerArgs(List<String> args) {
        if (args.size() < 2) {
            return null;
        }

        String keyMarshaller = args.size() >= 4 ? args.get(args.size() - 2) : null;
        String valueMarshaller = args.size() >= 4 ? args.get(args.size() - 1) : null;
        String keyShape = wireShapeFromNativeTypeOrMarshaller(args.get(0), keyMarshaller);
        String valueShape = wireShapeFromNativeTypeOrMarshaller(args.get(1), valueMarshaller);
        return containerShape(keyShape, valueShape);
    }

    private String wireShapeFromNativeTypeOrMarshaller(String nativeType, String marshallerType) {
        String fromMarshaller = wireShapeFromMarshallerType(marshallerType);
        if (fromMarshaller != null) {
            return fromMarshaller;
        }
        return wireShapeFromNativeType(nativeType);
    }

    private String wireShapeFromMarshallerType(String marshallerType) {
        String normalized = normalizeNativeType(marshallerType);
        if (normalized == null) {
            return null;
        }
        if (normalized.contains("HalfMarshaler")) {
            return "half-f32";
        }
        if (normalized.contains("VlqU32Marshaler")) {
            return "vlq-u32";
        }
        if (normalized.contains("VlqU64Marshaler")) {
            return "vlq-u64";
        }
        if (normalized.contains("PackedQuaternionMarshaller") ||
            normalized.contains("QuatCompNormMarshaler") ||
            normalized.contains("QuatCompressSmallestThree")) {
            return "quat-comp-norm";
        }
        return null;
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
        ForwardArgState state = initialForwardArgState(owner, new LinkedHashSet<>());
        for (Instruction instruction : functionInstructions(owner)) {
            if (instruction.getMinAddress().compareTo(callsite) >= 0) {
                break;
            }
            observeForwardInstruction(instruction, state);
        }

        ArgState result = new ArgState();
        TrackedValue name = state.registers.get("RDX");
        if (name != null && name.fieldName != null) {
            result.nameAddress = name.fieldNameAddress;
            result.name = name.fieldName;
        }
        else if (name != null && name.address != null) {
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
                result.handlerConstruction =
                    state.handlerConstructionsByThisOffset.get(handler.thisOffset);
                result.handlerConstructorWrites =
                    state.constructorWritesByHandlerOffset.get(handler.thisOffset);
            }
            else if (handler.baseKey != null && handler.baseOffset != null) {
                String key = baseOffsetKey(handler.baseKey, handler.baseOffset);
                result.handlerExpression = handler.expression;
                result.handlerVtable = state.vtablesByBaseOffset.get(key);
                result.handlerConstruction = state.handlerConstructionsByBaseOffset.get(key);
                result.handlerConstructorWrites =
                    state.constructorWritesByBaseOffset.get(key);
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

    private ForwardArgState initialForwardArgState(
        Function owner,
        Set<String> activeFunctions) {

        ForwardArgState inherited = inheritedForwardArgState(owner, activeFunctions);
        return inherited == null ? newForwardArgState() : inherited.copy();
    }

    private ForwardArgState inheritedForwardArgState(
        Function owner,
        Set<String> activeFunctions) {

        if (owner == null) {
            return null;
        }
        String key = functionCacheKey("inherited-forward-state", owner);
        if (inheritedForwardStateCache.containsKey(key)) {
            ForwardArgState cached = inheritedForwardStateCache.get(key);
            return cached == null ? null : cached.copy();
        }
        if (activeFunctions.size() >= INHERITED_FORWARD_STATE_RECURSION_LIMIT) {
            return null;
        }

        String activeKey = owner.getEntryPoint().toString();
        if (!activeFunctions.add(activeKey)) {
            return null;
        }
        try {
            ForwardArgState result = recoverInheritedForwardArgState(owner, activeFunctions);
            inheritedForwardStateCache.put(key, result == null ? null : result.copy());
            return result;
        }
        finally {
            activeFunctions.remove(activeKey);
        }
    }

    private ForwardArgState recoverInheritedForwardArgState(
        Function callee,
        Set<String> activeFunctions) {

        ForwardArgState mergedState = null;
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(callee.getEntryPoint());
        while (references.hasNext()) {
            Reference reference = references.next();
            if (!reference.getReferenceType().isCall()) {
                continue;
            }
            Address callsite = reference.getFromAddress();
            Instruction callInstruction =
                currentProgram.getListing().getInstructionAt(callsite);
            if (callInstruction == null ||
                !callee.getEntryPoint().equals(resolvedCodeTarget(callTarget(callInstruction)))) {
                continue;
            }

            Function caller = functionAtOrContaining(callsite);
            if (caller == null || caller.getEntryPoint().equals(callee.getEntryPoint())) {
                continue;
            }

            ForwardArgState callerState =
                initialForwardArgState(caller, activeFunctions);
            for (Instruction instruction : functionInstructions(caller)) {
                if (instruction.getMinAddress().compareTo(callsite) >= 0) {
                    break;
                }
                observeForwardInstruction(instruction, callerState);
            }

            TrackedValue receiver = callerState.registers.get("RCX");
            if (receiver == null || receiver.thisOffset == null) {
                continue;
            }

            ForwardArgState calleeState = newForwardArgState();
            calleeState.copyObjectEvidenceFrom(callerState);
            calleeState.registers.put("RCX", receiver.copy());
            copyCallArgument(calleeState, callerState, "RDX");
            copyCallArgument(calleeState, callerState, "R8");
            copyCallArgument(calleeState, callerState, "R9");
            if (mergedState == null) {
                mergedState = calleeState;
            }
            else if (!mergedState.mergeCompatibleObjectEvidenceFrom(calleeState)) {
                return null;
            }
        }
        return mergedState;
    }

    private void copyCallArgument(
        ForwardArgState calleeState,
        ForwardArgState callerState,
        String register) {

        TrackedValue value = callerState.registers.get(register);
        if (value != null) {
            calleeState.registers.put(register, value.copy());
        }
    }

    private ForwardArgState newForwardArgState() {
        ForwardArgState state = new ForwardArgState();
        state.registers.put("RCX", TrackedValue.thisOffset(0));
        state.registers.put("RDX", TrackedValue.baseOffset("arg:RDX", 0));
        state.registers.put("R8", TrackedValue.baseOffset("arg:R8", 0));
        state.registers.put("R9", TrackedValue.baseOffset("arg:R9", 0));
        state.registers.put("RSP", TrackedValue.stackOffset(0));
        return state;
    }

    private void observeForwardInstruction(Instruction instruction, ForwardArgState state) {
        observeForwardInstruction(instruction, state, true);
    }

    private void observeForwardInstruction(
        Instruction instruction,
        ForwardArgState state,
        boolean observeConstructors) {

        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return;
        }

        if ("PUSH".equals(mnemonic)) {
            TrackedValue source = trackedOperandValue(instruction, 0, state);
            adjustTrackedRegister(state, "RSP", -8);
            Integer stackSlot = stackRegisterOffset(state, "RSP");
            if (stackSlot != null) {
                writeStackValue(state, stackSlot, 64, source);
            }
            return;
        }

        if ("POP".equals(mnemonic)) {
            Integer stackSlot = stackRegisterOffset(state, "RSP");
            TrackedValue value = stackSlot == null ? null : state.valuesByStackSlot.get(stackSlot);
            String destination = registerOperand(instruction, 0);
            if (destination != null) {
                putOrRemove(state.registers, destination, value == null ? null : value.copy());
            }
            if (stackSlot != null) {
                invalidateStackRange(state, stackSlot, 8);
            }
            adjustTrackedRegister(state, "RSP", 8);
            return;
        }

        if (instruction.getFlowType().isCall()) {
            if (isStackProbeCall(instruction)) {
                state.registers.remove("R10");
                state.registers.remove("R11");
                return;
            }

            TrackedValue formattedFieldName = formattedFieldNameCallValue(instruction, state);

            if (isAllocatorCall(instruction, state)) {
                clearVolatileRegisters(state.registers);
                clearVolatileAllocatorDispatchRegisters(state);
                state.registers.put(
                    "RAX",
                    TrackedValue.baseOffset(
                        allocationBaseKey(instruction.getMinAddress()),
                        0));
                return;
            }

            boolean addFilterGroupCall = isAddFilterGroupCall(instruction, state);
            promoteReplicatedStateConstructorReceiver(instruction, state);
            if (observeConstructors) {
                observeHandlerConstructorMemsetCall(instruction, state);
                observeVectorFieldHandlerConstructorCall(instruction, state);
                observeFieldHandlerConstructorCall(instruction, state);
            }
            clearVolatileRegisters(state.registers);
            clearVolatileAllocatorDispatchRegisters(state);
            if (formattedFieldName != null) {
                state.registers.put("RAX", formattedFieldName);
            }
            if (addFilterGroupCall) {
                state.registers.put(
                    "RAX",
                    TrackedValue.immediate(state.nextFilterGroupIndex++));
            }
            return;
        }

        if ("MUL".equals(mnemonic) || "DIV".equals(mnemonic) || "IDIV".equals(mnemonic) ||
            ("IMUL".equals(mnemonic) && operandCount(instruction) == 1)) {
            state.registers.remove("RAX");
            state.registers.remove("RDX");
            state.allocatorDispatchRegisters.remove("RAX");
            state.allocatorDispatchRegisters.remove("RDX");
            return;
        }

        if ("CDQ".equals(mnemonic) || "CQO".equals(mnemonic) || "CWD".equals(mnemonic)) {
            state.registers.remove("RDX");
            state.allocatorDispatchRegisters.remove("RDX");
            return;
        }

        String destination = registerOperand(instruction, 0);
        if (destination != null) {
            if (("XOR".equals(mnemonic) || "SUB".equals(mnemonic)) &&
                destination.equals(registerOperand(instruction, 1))) {
                state.registers.put(destination, TrackedValue.immediate(0));
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("XCHG".equals(mnemonic)) {
                String source = registerOperand(instruction, 1);
                if (source == null) {
                    state.registers.remove(destination);
                    return;
                }
                TrackedValue left = state.registers.get(destination);
                TrackedValue right = state.registers.get(source);
                putOrRemove(state.registers, destination, right == null ? null : right.copy());
                putOrRemove(state.registers, source, left == null ? null : left.copy());
                swapAllocatorDispatchRegisters(state, destination, source);
                return;
            }

            if ("LEA".equals(mnemonic)) {
                TrackedValue value = trackedLeaValue(instruction, state);
                putOrRemove(state.registers, destination, value);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("MOV".equals(mnemonic)) {
                TrackedValue value = trackedOperandValue(instruction, 1, state);
                putOrRemove(state.registers, destination, value);
                if (isAllocatorVtableSlotLoad(instruction)) {
                    state.allocatorDispatchRegisters.add(destination);
                }
                else {
                    state.allocatorDispatchRegisters.remove(destination);
                }
                return;
            }

            if ("MOVZX".equals(mnemonic) ||
                "MOVSX".equals(mnemonic) ||
                "MOVSXD".equals(mnemonic)) {
                TrackedValue value = trackedOperandValue(instruction, 1, state);
                putOrRemove(state.registers, destination,
                    value != null && value.immediate != null ? value : null);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("IMUL".equals(mnemonic)) {
                TrackedValue value = trackedMultiplyValue(instruction, state);
                putOrRemove(state.registers, destination, value);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("INC".equals(mnemonic) || "DEC".equals(mnemonic)) {
                TrackedValue current = state.registers.get(destination);
                TrackedValue adjusted = current == null
                    ? null
                    : current.addOffset("INC".equals(mnemonic) ? 1 : -1);
                putOrRemove(state.registers, destination, adjusted);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("NEG".equals(mnemonic) || "NOT".equals(mnemonic) || "BSWAP".equals(mnemonic)) {
                TrackedValue current = state.registers.get(destination);
                TrackedValue adjusted = trackedUnaryIntegerValue(mnemonic, current);
                putOrRemove(state.registers, destination, adjusted);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("ADD".equals(mnemonic) || "SUB".equals(mnemonic)) {
                Long immediate = literalOperandValue(instruction, 1);
                TrackedValue trackedDelta = immediate == null
                    ? trackedOperandValue(instruction, 1, state)
                    : TrackedValue.immediate(immediate);
                TrackedValue current = state.registers.get(destination);
                TrackedValue adjusted = "ADD".equals(mnemonic)
                    ? addTrackedValues(current, trackedDelta)
                    : subtractTrackedValues(current, trackedDelta);
                putOrRemove(state.registers, destination, adjusted);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if ("AND".equals(mnemonic) ||
                "OR".equals(mnemonic) ||
                "XOR".equals(mnemonic) ||
                "SHL".equals(mnemonic) ||
                "SAL".equals(mnemonic) ||
                "SHR".equals(mnemonic) ||
                "SAR".equals(mnemonic) ||
                "ROL".equals(mnemonic) ||
                "ROR".equals(mnemonic)) {
                Long immediate = literalOperandValue(instruction, 1);
                TrackedValue right = immediate == null
                    ? trackedOperandValue(instruction, 1, state)
                    : TrackedValue.immediate(immediate);
                TrackedValue adjusted =
                    trackedBinaryIntegerValue(mnemonic, state.registers.get(destination), right);
                putOrRemove(state.registers, destination, adjusted);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }

            if (writesRegister(mnemonic)) {
                state.registers.remove(destination);
                state.allocatorDispatchRegisters.remove(destination);
                return;
            }
        }

        if (isMemoryWriteMnemonic(mnemonic)) {
            Integer offset = trackedThisOffsetForMemoryOperand(instruction, 0, state.registers);
            TrackedValue baseOffset = trackedBaseOffsetForMemoryOperand(instruction, 0, state.registers);
            TrackedValue source = "MOV".equals(mnemonic)
                ? trackedOperandValue(instruction, 1, state)
                : null;
            observeThisValueWrite(instruction, state, offset, source);
            observeStackValueWrite(instruction, state, source);
            if (offset != null && source != null &&
                source.address != null && isVtableLike(source.address)) {
                state.vtablesByThisOffset.put(offset, source.address);
                state.handlerConstructionsByThisOffset.put(
                    offset,
                    new HandlerConstruction(
                        "inline-vtable-write",
                        instruction.getMinAddress(),
                        null,
                        null,
                        source.address));
            }
            if (baseOffset != null &&
                baseOffset.baseKey != null &&
                baseOffset.baseOffset != null &&
                source != null &&
                source.address != null &&
                isVtableLike(source.address)) {
                recordBaseVtableWrite(
                    state,
                    baseOffset.baseKey,
                    baseOffset.baseOffset,
                    source.address,
                    instruction,
                    "dynamic-base-vtable-write");
            }
            if ("MOV".equals(mnemonic)) {
                observeHandlerConstructorWrite(instruction, state);
            }
        }
    }

    private void promoteReplicatedStateConstructorReceiver(
        Instruction instruction,
        ForwardArgState state) {

        Address target = resolvedCodeTarget(callTarget(instruction));
        Address replicatedStateConstructor =
            currentProgram.getImageBase().add(REPLICATED_STATE_CONSTRUCTOR_RVA);
        if (!replicatedStateConstructor.equals(target)) {
            String targetName = fullFunctionName(functionAtOrContaining(target));
            if (targetName == null ||
                !targetName.contains("MB::ReplicatedState::ReplicatedState")) {
                return;
            }
        }

        TrackedValue receiver = state.registers.get("RCX");
        if (receiver == null || receiver.baseKey == null || receiver.baseOffset == null) {
            return;
        }
        promoteBaseToThis(state, receiver.baseKey, receiver.baseOffset);
    }

    private void promoteBaseToThis(ForwardArgState state, String baseKey, int baseOffset) {
        for (Map.Entry<String, TrackedValue> entry : new ArrayList<>(state.registers.entrySet())) {
            TrackedValue value = entry.getValue();
            TrackedValue promoted = promoteValueBaseToThis(value, baseKey, baseOffset);
            if (promoted != value) {
                state.registers.put(entry.getKey(), promoted);
            }
        }

        for (Map.Entry<Integer, TrackedValue> entry : new ArrayList<>(state.valuesByStackSlot.entrySet())) {
            TrackedValue value = entry.getValue();
            TrackedValue promoted = promoteValueBaseToThis(value, baseKey, baseOffset);
            if (promoted != value) {
                state.valuesByStackSlot.put(entry.getKey(), promoted);
            }
        }
    }

    private TrackedValue promoteValueBaseToThis(
        TrackedValue value,
        String baseKey,
        int baseOffset) {

        if (value == null ||
            value.thisOffset != null ||
            value.baseKey == null ||
            value.baseOffset == null ||
            !value.baseKey.equals(baseKey)) {
            return value;
        }
        Integer promotedOffset = addOffsets(value.baseOffset, -(long)baseOffset);
        return promotedOffset == null ? value : TrackedValue.thisOffset(promotedOffset);
    }

    private boolean isAddFilterGroupCall(Instruction instruction, ForwardArgState state) {
        Address target = resolvedCodeTarget(callTarget(instruction));
        Address addFilterGroup = currentProgram.getImageBase().add(ADD_FILTER_GROUP_RVA);
        if (!addFilterGroup.equals(target)) {
            return false;
        }

        TrackedValue receiver = state.registers.get("RCX");
        return receiver != null && receiver.thisOffset != null && receiver.thisOffset == 0;
    }

    private boolean isStackProbeCall(Instruction instruction) {
        String targetName = fullFunctionName(functionAtOrContaining(callTarget(instruction)));
        if (targetName == null) {
            return false;
        }
        String normalized = targetName.toLowerCase(Locale.ROOT);
        return normalized.contains("__chkstk") ||
            normalized.contains("_chkstk") ||
            normalized.contains("_alloca_probe");
    }

    private TrackedValue formattedFieldNameCallValue(
        Instruction instruction,
        ForwardArgState state) {

        if (!instruction.getFlowType().isCall()) {
            return null;
        }

        TrackedValue destination = state.registers.get("RCX");
        TrackedValue formatValue = state.registers.get("RDX");
        TrackedValue indexValue = state.registers.get("R8");
        if (destination == null || destination.stackOffset == null ||
            formatValue == null || formatValue.address == null ||
            indexValue == null || indexValue.immediate == null) {
            return null;
        }

        String format = readPrintableString(formatValue.address);
        String fieldName = renderFormattedFieldName(format, indexValue.immediate);
        return fieldName == null ? null : TrackedValue.fieldName(formatValue.address, fieldName);
    }

    private String renderFormattedFieldName(String format, long index) {
        if (!isLikelyFieldNameFormat(format) || index < 0 || index > 4096) {
            return null;
        }

        Matcher matcher = Pattern.compile("%(?:0?\\d+)?[du]").matcher(format);
        if (!matcher.find() || matcher.find()) {
            return null;
        }

        String rendered = format.replaceFirst(
            "%(?:0?\\d+)?[du]",
            Long.toUnsignedString(index));
        return isLikelyFieldName(rendered) ? rendered : null;
    }

    private boolean isLikelyFieldNameFormat(String value) {
        if (value == null || value.isBlank() || value.length() > 96) {
            return false;
        }
        if (!value.contains("%")) {
            return false;
        }
        if (value.indexOf(' ') >= 0 || value.indexOf('\\') >= 0 || value.indexOf('/') >= 0) {
            return false;
        }
        String withoutPlaceholder = value.replaceAll("%(?:0?\\d+)?[du]", "0");
        return isLikelyFieldName(withoutPlaceholder);
    }

    private boolean isLikelyFieldName(String value) {
        return value != null &&
            value.matches("[A-Za-z_][A-Za-z0-9_]*");
    }

    private boolean isAllocatorCall(Instruction instruction, ForwardArgState state) {
        Function target = functionAtOrContaining(callTarget(instruction));
        String targetName = fullFunctionName(target);
        if (isNamedAllocatorFunction(targetName)) {
            return true;
        }
        if (!hasAllocatorArgumentShape(state)) {
            return false;
        }
        return isAllocatorVtableDispatch(instruction, state) || isAllocatorReturnFunction(target);
    }

    private boolean isNamedAllocatorFunction(String targetName) {
        if (targetName == null) {
            return false;
        }
        String normalized = targetName.toLowerCase(Locale.ROOT);
        return normalized.equals("operator_new") ||
            normalized.contains("operator new") ||
            normalized.contains("operator_new") ||
            normalized.contains("allocatefromnamedallocator");
    }

    private boolean hasAllocatorArgumentShape(ForwardArgState state) {
        TrackedValue byteSize = state.registers.get("RDX");
        TrackedValue alignment = state.registers.get("R8");
        return positiveBoundedImmediate(byteSize, 0x100000L) &&
            powerOfTwoImmediate(alignment, 0x1000L);
    }

    private boolean positiveBoundedImmediate(TrackedValue value, long maxInclusive) {
        return value != null &&
            value.immediate != null &&
            value.immediate > 0 &&
            value.immediate <= maxInclusive;
    }

    private boolean powerOfTwoImmediate(TrackedValue value, long maxInclusive) {
        if (value == null || value.immediate == null) {
            return false;
        }
        long immediate = value.immediate;
        return immediate > 0 &&
            immediate <= maxInclusive &&
            (immediate & (immediate - 1L)) == 0L;
    }

    private boolean isAllocatorReturnFunction(Function function) {
        if (function == null) {
            return false;
        }
        String key = functionCacheKey("allocator-return", function);
        Boolean cached = allocatorReturnFunctionCache.get(key);
        if (cached != null) {
            return cached;
        }

        boolean result = false;
        int count = 0;
        boolean sawAllocatorDispatch = false;
        LinkedHashSet<String> allocatorDispatchRegisters = new LinkedHashSet<>();
        for (Instruction instruction : functionInstructions(function)) {
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            String mnemonic = upperMnemonic(instruction);
            if (mnemonic == null) {
                continue;
            }
            if (instruction.getFlowType().isCall()) {
                String callRegister = registerOperand(instruction, 0);
                if (isAllocatorVtableDispatch(instruction)) {
                    sawAllocatorDispatch = true;
                }
                else if (callRegister != null &&
                    allocatorDispatchRegisters.contains(callRegister)) {
                    sawAllocatorDispatch = true;
                }
                else if (isNamedAllocatorFunction(
                    fullFunctionName(functionAtOrContaining(callTarget(instruction))))) {
                    sawAllocatorDispatch = true;
                }
                continue;
            }
            if (sawAllocatorDispatch && mnemonic.startsWith("RET")) {
                result = true;
                break;
            }
            if (sawAllocatorDispatch && writesReturnRegister(instruction)) {
                break;
            }
            String destination = registerOperand(instruction, 0);
            if (destination != null) {
                if ("MOV".equals(mnemonic) && isAllocatorVtableSlotLoad(instruction)) {
                    allocatorDispatchRegisters.add(destination);
                }
                else {
                    allocatorDispatchRegisters.remove(destination);
                }
            }
        }

        allocatorReturnFunctionCache.put(key, result);
        return result;
    }

    private boolean isAllocatorVtableDispatch(
        Instruction instruction,
        ForwardArgState state) {

        if (isAllocatorVtableDispatch(instruction)) {
            return true;
        }
        String callRegister = registerOperand(instruction, 0);
        return callRegister != null && state.allocatorDispatchRegisters.contains(callRegister);
    }

    private boolean isAllocatorVtableDispatch(Instruction instruction) {
        if (!instruction.getFlowType().isCall()) {
            return false;
        }
        MemoryReference memory = memoryReference(instruction, 0);
        if (memory != null) {
            return memory.displacement == 8;
        }
        MemoryAddress address = memoryAddress(instruction, 0);
        return address != null &&
            address.displacement == 8 &&
            !address.terms.isEmpty();
    }

    private boolean isAllocatorVtableSlotLoad(Instruction instruction) {
        if (!"MOV".equals(upperMnemonic(instruction)) ||
            registerOperand(instruction, 0) == null) {
            return false;
        }
        MemoryReference memory = memoryReference(instruction, 1);
        if (memory != null) {
            return memory.displacement == 8;
        }
        MemoryAddress address = memoryAddress(instruction, 1);
        return address != null &&
            address.displacement == 8 &&
            !address.terms.isEmpty();
    }

    private boolean writesReturnRegister(Instruction instruction) {
        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null || !writesRegister(mnemonic)) {
            return false;
        }
        return isReturnRegister(registerOperand(instruction, 0));
    }

    private String allocationBaseKey(Address callsite) {
        return "alloc:" + formatAddress(callsite);
    }

    private TrackedValue trackedThisLoadValue(
        Instruction instruction,
        int operandIndex,
        ForwardArgState state) {

        Integer offset = trackedThisOffsetForMemoryOperand(
            instruction,
            operandIndex,
            state.registers);
        if (offset == null) {
            return null;
        }
        TrackedValue value = state.valuesByThisOffset.get(offset);
        if (value != null) {
            return value.copy();
        }

        return isQwordMemoryOperand(instruction, operandIndex)
            ? TrackedValue.baseOffset(thisSlotBaseKey(offset), 0)
            : null;
    }

    private TrackedValue trackedStackLoadValue(
        Instruction instruction,
        int operandIndex,
        ForwardArgState state) {

        MemoryReference memory = memoryReference(instruction, operandIndex);
        if (memory == null || !isStackRegister(memory.baseRegister)) {
            return null;
        }

        Integer stackSlot = stackSlotOffset(memory, state);
        TrackedValue value = stackSlot == null ? null : state.valuesByStackSlot.get(stackSlot);
        return value == null ? null : value.copy();
    }

    private TrackedValue trackedFormattedFieldNamePointerLoadValue(
        Instruction instruction,
        int operandIndex,
        ForwardArgState state) {

        MemoryAddress memory = memoryAddress(instruction, operandIndex);
        if (memory == null || memory.displacement != 0 || memory.terms.size() != 1) {
            return null;
        }

        MemoryTerm term = memory.terms.get(0);
        if (term.scale != 1) {
            return null;
        }

        TrackedValue base = state.registers.get(term.register);
        return base != null && base.fieldName != null ? base.copy() : null;
    }

    private void observeThisValueWrite(
        Instruction instruction,
        ForwardArgState state,
        Integer offset,
        TrackedValue source) {

        if (offset == null) {
            return;
        }

        Integer widthBits = memoryWriteWidthBits(instruction);
        if (widthBits == null || widthBits > 64) {
            state.valuesByThisOffset.remove(offset);
            return;
        }

        if (source == null) {
            state.valuesByThisOffset.remove(offset);
        }
        else {
            state.valuesByThisOffset.put(offset, source.copy());
        }
    }

    private void observeStackValueWrite(
        Instruction instruction,
        ForwardArgState state,
        TrackedValue source) {

        MemoryReference memory = memoryReference(instruction, 0);
        if (memory == null || !isStackRegister(memory.baseRegister)) {
            return;
        }

        Integer widthBits = memoryWriteWidthBits(instruction);
        Integer stackSlot = stackSlotOffset(memory, state);
        if (stackSlot == null) {
            return;
        }
        writeStackValue(state, stackSlot, widthBits, source);
    }

    private void writeStackValue(
        ForwardArgState state,
        int stackSlot,
        Integer widthBits,
        TrackedValue source) {

        int byteWidth = byteWidth(widthBits);
        invalidateStackRange(state, stackSlot, byteWidth);
        if (byteWidth == 8 && source != null) {
            state.valuesByStackSlot.put(stackSlot, source.copy());
        }
    }

    private int byteWidth(Integer widthBits) {
        if (widthBits == null || widthBits <= 0) {
            return 8;
        }
        return Math.max(1, (widthBits + 7) / 8);
    }

    private void invalidateStackRange(ForwardArgState state, int start, int length) {
        long end = (long)start + Math.max(length, 1);
        ArrayList<Integer> stale = new ArrayList<>();
        for (Integer slot : state.valuesByStackSlot.keySet()) {
            long slotEnd = (long)slot + 8;
            if (slot < end && slotEnd > start) {
                stale.add(slot);
            }
        }
        for (Integer slot : stale) {
            state.valuesByStackSlot.remove(slot);
        }
    }

    private Integer stackSlotOffset(MemoryReference memory, ForwardArgState state) {
        if (memory == null || memory.baseRegister == null) {
            return null;
        }
        TrackedValue base = state.registers.get(memory.baseRegister);
        if (base == null || base.stackOffset == null) {
            return null;
        }
        return addOffsets(base.stackOffset, memory.displacement);
    }

    private Integer stackRegisterOffset(ForwardArgState state, String register) {
        TrackedValue value = state.registers.get(register);
        return value == null ? null : value.stackOffset;
    }

    private boolean isQwordMemoryOperand(Instruction instruction, int operandIndex) {
        String text = operandText(instruction, operandIndex);
        return text != null && text.toLowerCase(Locale.ROOT).contains("qword ptr");
    }

    private void recordBaseVtableWrite(
        ForwardArgState state,
        String baseKey,
        int baseOffset,
        Address vtable,
        Instruction instruction,
        String pattern) {

        recordSingleBaseVtableWrite(
            state,
            baseKey,
            baseOffset,
            vtable,
            instruction,
            pattern);

        VectorSlotAlias alias = vectorSlotAlias(baseKey);
        if (alias != null && alias.slotOffset == VECTOR_CURRENT_POINTER_OFFSET) {
            recordSingleBaseVtableWrite(
                state,
                thisSlotBaseKey(alias.ownerOffset),
                baseOffset,
                vtable,
                instruction,
                prefixPattern("vector-begin-alias", pattern));
        }
    }

    private void recordSingleBaseVtableWrite(
        ForwardArgState state,
        String baseKey,
        int baseOffset,
        Address vtable,
        Instruction instruction,
        String pattern) {

        String key = baseOffsetKey(baseKey, baseOffset);
        state.vtablesByBaseOffset.put(key, vtable);
        state.handlerConstructionsByBaseOffset.put(
            key,
            new HandlerConstruction(
                pattern,
                instruction.getMinAddress(),
                null,
                null,
                vtable));
    }

    private VectorSlotAlias vectorSlotAlias(String baseKey) {
        if (baseKey == null || !baseKey.startsWith("this-slot:0x")) {
            return null;
        }
        Long offset = parseIntegerLiteral(baseKey.substring("this-slot:".length()));
        if (offset == null ||
            offset < Integer.MIN_VALUE ||
            offset > Integer.MAX_VALUE ||
            (offset % 8L) != 0L) {
            return null;
        }
        long ownerOffset = offset - VECTOR_CURRENT_POINTER_OFFSET;
        if (ownerOffset < Integer.MIN_VALUE || ownerOffset > Integer.MAX_VALUE) {
            return null;
        }
        return new VectorSlotAlias((int)ownerOffset, VECTOR_CURRENT_POINTER_OFFSET);
    }

    private String thisSlotBaseKey(int offset) {
        return "this-slot:0x" + Integer.toHexString(offset);
    }

    private String baseOffsetKey(String baseKey, int offset) {
        return baseKey + "@0x" + Integer.toHexString(offset);
    }

    private void adjustTrackedRegister(ForwardArgState state, String register, int delta) {
        TrackedValue current = state.registers.get(register);
        if (current == null) {
            state.registers.remove(register);
            return;
        }
        Integer oldStackOffset = current.stackOffset;
        TrackedValue adjusted = current.addOffset(delta);
        if ("RSP".equals(register) && oldStackOffset != null && delta > 0) {
            invalidateStackRange(state, oldStackOffset, delta);
        }
        putOrRemove(state.registers, register, adjusted);
    }

    private TrackedValue trackedMultiplyValue(
        Instruction instruction,
        ForwardArgState state) {

        int operands = operandCount(instruction);
        TrackedValue left;
        TrackedValue right;
        if (operands >= 3) {
            left = trackedOperandValue(instruction, 1, state);
            Long immediate = literalOperandValue(instruction, 2);
            right = immediate == null
                ? trackedOperandValue(instruction, 2, state)
                : TrackedValue.immediate(immediate);
        }
        else if (operands >= 2) {
            left = state.registers.get(registerOperand(instruction, 0));
            Long immediate = literalOperandValue(instruction, 1);
            right = immediate == null
                ? trackedOperandValue(instruction, 1, state)
                : TrackedValue.immediate(immediate);
        }
        else {
            return null;
        }

        if (left == null || right == null ||
            left.immediate == null || right.immediate == null) {
            return null;
        }
        try {
            return TrackedValue.immediate(Math.multiplyExact(left.immediate, right.immediate));
        }
        catch (ArithmeticException ignored) {
            return null;
        }
    }

    private TrackedValue addTrackedValues(TrackedValue left, TrackedValue right) {
        if (left == null || right == null) {
            return null;
        }
        if (left.immediate != null && right.immediate != null) {
            try {
                return TrackedValue.immediate(Math.addExact(left.immediate, right.immediate));
            }
            catch (ArithmeticException ignored) {
                return null;
            }
        }
        if (right.immediate != null) {
            return addTrackedOffset(left, right.immediate);
        }
        if (left.immediate != null) {
            return addTrackedOffset(right, left.immediate);
        }
        return null;
    }

    private TrackedValue subtractTrackedValues(TrackedValue left, TrackedValue right) {
        if (left == null || right == null || right.immediate == null) {
            return null;
        }
        try {
            return addTrackedOffset(left, Math.negateExact(right.immediate));
        }
        catch (ArithmeticException ignored) {
            return null;
        }
    }

    private TrackedValue trackedUnaryIntegerValue(String mnemonic, TrackedValue value) {
        if (value == null || value.immediate == null) {
            return null;
        }
        try {
            return switch (mnemonic) {
                case "NEG" -> TrackedValue.immediate(Math.negateExact(value.immediate));
                case "NOT" -> TrackedValue.immediate(~value.immediate);
                case "BSWAP" -> TrackedValue.immediate(Long.reverseBytes(value.immediate));
                default -> null;
            };
        }
        catch (ArithmeticException ignored) {
            return null;
        }
    }

    private TrackedValue trackedBinaryIntegerValue(
        String mnemonic,
        TrackedValue left,
        TrackedValue right) {

        if (left == null || right == null || right.immediate == null) {
            return null;
        }

        if (left.immediate == null) {
            return switch (mnemonic) {
                case "AND" -> right.immediate == -1L ? left.copy() : null;
                case "OR", "XOR" -> right.immediate == 0L ? left.copy() : null;
                default -> null;
            };
        }

        long leftValue = left.immediate;
        long rightValue = right.immediate;
        return switch (mnemonic) {
            case "AND" -> TrackedValue.immediate(leftValue & rightValue);
            case "OR" -> TrackedValue.immediate(leftValue | rightValue);
            case "XOR" -> TrackedValue.immediate(leftValue ^ rightValue);
            case "SHL", "SAL" -> shiftLeftTrackedImmediate(leftValue, rightValue);
            case "SHR" -> shiftRightTrackedImmediate(leftValue, rightValue, false);
            case "SAR" -> shiftRightTrackedImmediate(leftValue, rightValue, true);
            case "ROL" -> rotateTrackedImmediate(leftValue, rightValue, true);
            case "ROR" -> rotateTrackedImmediate(leftValue, rightValue, false);
            default -> null;
        };
    }

    private TrackedValue shiftLeftTrackedImmediate(long value, long amount) {
        if (amount < 0 || amount > 63) {
            return null;
        }
        try {
            return TrackedValue.immediate(Math.multiplyExact(value, 1L << amount));
        }
        catch (ArithmeticException ignored) {
            return null;
        }
    }

    private TrackedValue shiftRightTrackedImmediate(long value, long amount, boolean signed) {
        if (amount < 0 || amount > 63) {
            return null;
        }
        int shift = (int)amount;
        return TrackedValue.immediate(signed ? value >> shift : value >>> shift);
    }

    private TrackedValue rotateTrackedImmediate(long value, long amount, boolean left) {
        if (amount < 0 || amount > 63) {
            return null;
        }
        int shift = (int)amount;
        return TrackedValue.immediate(left
            ? Long.rotateLeft(value, shift)
            : Long.rotateRight(value, shift));
    }

    private TrackedValue addTrackedOffset(TrackedValue value, long delta) {
        if (value == null || delta < Integer.MIN_VALUE || delta > Integer.MAX_VALUE) {
            return null;
        }
        return value.addOffset((int)delta);
    }

    private int operandCount(Instruction instruction) {
        try {
            return instruction.getNumOperands();
        }
        catch (Exception ignored) {
            return 0;
        }
    }

    private void observeHandlerConstructorWrite(
        Instruction instruction,
        ForwardArgState state) {

        Integer writeOffset = trackedThisOffsetForMemoryOperand(instruction, 0, state.registers);
        if (writeOffset == null) {
            return;
        }

        TrackedValue source = trackedOperandValue(instruction, 1, state);
        if (source != null && source.address != null && isVtableLike(source.address)) {
            return;
        }

        Integer handlerOffset = enclosingHandlerOffset(writeOffset, state);
        if (handlerOffset == null) {
            return;
        }

        int relativeOffset = writeOffset - handlerOffset;
        if (relativeOffset <= 0) {
            return;
        }

        HandlerConstructorWrite write = new HandlerConstructorWrite(
            instruction.getMinAddress(),
            handlerOffset,
            relativeOffset,
            memoryWriteWidthBits(instruction),
            null,
            trackedValueKind(source),
            trackedValueDisplay(source),
            trackedValueHex(source),
            operandText(instruction, 1),
            "constructor-memory-write");
        state.constructorWritesByHandlerOffset
            .computeIfAbsent(handlerOffset, ignored -> new ArrayList<>())
            .add(write);
    }

    private void observeHandlerConstructorMemsetCall(
        Instruction instruction,
        ForwardArgState state) {

        String targetName = fullFunctionName(functionAtOrContaining(callTarget(instruction)));
        if (targetName == null || !targetName.toLowerCase(Locale.ROOT).contains("memset")) {
            return;
        }

        TrackedValue destination = state.registers.get("RCX");
        TrackedValue fill = state.registers.get("RDX");
        TrackedValue length = state.registers.get("R8");
        if (destination == null || destination.thisOffset == null ||
            length == null || length.immediate == null) {
            return;
        }

        Integer handlerOffset = enclosingHandlerOffset(destination.thisOffset, state);
        if (handlerOffset == null) {
            return;
        }

        int relativeOffset = destination.thisOffset - handlerOffset;
        if (relativeOffset <= 0 || length.immediate > Integer.MAX_VALUE) {
            return;
        }

        HandlerConstructorWrite write = new HandlerConstructorWrite(
            instruction.getMinAddress(),
            handlerOffset,
            relativeOffset,
            8,
            length.immediate.intValue(),
            trackedValueKind(fill),
            trackedValueDisplay(fill),
            trackedValueHex(fill),
            "memset",
            "constructor-memset");
        state.constructorWritesByHandlerOffset
            .computeIfAbsent(handlerOffset, ignored -> new ArrayList<>())
            .add(write);
    }

    private Integer enclosingHandlerOffset(int writeOffset, ForwardArgState state) {
        Integer best = null;
        Integer next = null;
        for (Integer candidate : state.vtablesByThisOffset.keySet()) {
            if (candidate < writeOffset && (best == null || candidate > best)) {
                best = candidate;
            }
            else if (candidate > writeOffset && (next == null || candidate < next)) {
                next = candidate;
            }
        }
        if (best == null) {
            return null;
        }

        int relativeOffset = writeOffset - best;
        int maxSpan = FIELD_HANDLER_CONSTRUCTOR_WRITE_SPAN;
        if (next != null) {
            maxSpan = Math.min(maxSpan, next - best - 1);
        }
        return relativeOffset <= maxSpan ? best : null;
    }

    private Integer memoryWriteWidthBits(Instruction instruction) {
        String text = operandText(instruction, 0);
        if (text == null) {
            return null;
        }
        String normalized = text.toLowerCase(Locale.ROOT);
        if (normalized.contains("zmmword ptr")) {
            return 512;
        }
        if (normalized.contains("ymmword ptr")) {
            return 256;
        }
        if (normalized.contains("xmmword ptr")) {
            return 128;
        }
        if (normalized.contains("oword ptr") || normalized.contains("dqword ptr")) {
            return 128;
        }
        if (normalized.contains("qword ptr")) {
            return 64;
        }
        if (normalized.contains("dword ptr")) {
            return 32;
        }
        if (normalized.contains("word ptr")) {
            return 16;
        }
        if (normalized.contains("byte ptr")) {
            return 8;
        }
        return null;
    }

    private String trackedValueKind(TrackedValue value) {
        if (value == null) {
            return "unknown";
        }
        if (value.fieldName != null) {
            return "field-name";
        }
        if (value.immediate != null) {
            return "immediate";
        }
        if (value.address != null) {
            return "address";
        }
        if (value.thisOffset != null) {
            return "this-offset";
        }
        if (value.stackOffset != null) {
            return "stack-offset";
        }
        if (value.baseKey != null) {
            return "base-offset";
        }
        return "unknown";
    }

    private String trackedValueDisplay(TrackedValue value) {
        if (value == null) {
            return null;
        }
        if (value.fieldName != null) {
            return value.fieldName;
        }
        if (value.immediate != null) {
            return Long.toUnsignedString(value.immediate);
        }
        if (value.address != null) {
            return formatAddress(value.address);
        }
        if (value.thisOffset != null) {
            return value.expression;
        }
        if (value.stackOffset != null) {
            return value.expression;
        }
        if (value.baseKey != null) {
            return value.expression;
        }
        return value.expression;
    }

    private String trackedValueHex(TrackedValue value) {
        if (value == null || value.immediate == null) {
            return null;
        }
        return "0x" + Long.toHexString(value.immediate);
    }

    private void observeVectorFieldHandlerConstructorCall(
        Instruction instruction,
        ForwardArgState state) {

        Address call = resolvedCodeTarget(callTarget(instruction));
        if (!isVectorConstructorIteratorCall(call)) {
            return;
        }

        TrackedValue base = state.registers.get("RCX");
        TrackedValue elementSize = state.registers.get("RDX");
        TrackedValue elementCount = state.registers.get("R8");
        TrackedValue constructorValue = state.registers.get("R9");
        if (base == null || base.thisOffset == null ||
            elementSize == null || elementSize.immediate == null ||
            elementCount == null || elementCount.immediate == null ||
            constructorValue == null || constructorValue.address == null) {
            return;
        }

        long count = elementCount.immediate;
        long size = elementSize.immediate;
        if (count <= 0 || count > 256 || size <= 0 || size > 0x1000) {
            return;
        }

        Address constructor = resolvedCodeTarget(constructorValue.address);
        List<VtableWrite> writes = constructorVtableWrites(constructor);
        if (writes.isEmpty()) {
            return;
        }

        for (long i = 0; i < count; i++) {
            long elementOffset = base.thisOffset.longValue() + (i * size);
            if (elementOffset < Integer.MIN_VALUE || elementOffset > Integer.MAX_VALUE) {
                continue;
            }
            for (VtableWrite write : writes) {
                if (write.thisOffset == null) {
                    continue;
                }
                Integer fieldOffset = addOffsets((int)elementOffset, write.thisOffset.longValue());
                if (fieldOffset == null) {
                    continue;
                }
                state.vtablesByThisOffset.put(fieldOffset, write.vtable);
                Address writeConstructor = write.function == null ? constructor : write.function;
                state.handlerConstructionsByThisOffset.put(
                    fieldOffset,
                    new HandlerConstruction(
                        prefixPattern("vector-constructor", write.pattern),
                        instruction.getMinAddress(),
                        writeConstructor,
                        fullFunctionName(functionAtOrContaining(writeConstructor)),
                        write.vtable));
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

        Address constructor = resolvedCodeTarget(callTarget(instruction));
        List<VtableWrite> writes = constructorVtableWrites(constructor);
        if (writes.isEmpty()) {
            return;
        }

        for (VtableWrite write : writes) {
            if (write.thisOffset == null) {
                continue;
            }
            Integer fieldOffset = addOffsets(receiver.thisOffset, write.thisOffset.longValue());
            if (fieldOffset == null) {
                continue;
            }
            state.vtablesByThisOffset.put(fieldOffset, write.vtable);
            Address writeConstructor = write.function == null ? constructor : write.function;
            state.handlerConstructionsByThisOffset.put(
                fieldOffset,
                new HandlerConstruction(
                    prefixPattern("constructor-call", write.pattern),
                    instruction.getMinAddress(),
                    writeConstructor,
                    fullFunctionName(functionAtOrContaining(writeConstructor)),
                    write.vtable));
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
        for (VtableWrite write : constructorVtableWrites(target)) {
            if (write.thisOffset != null && write.thisOffset == 0) {
                return write.vtable;
            }
        }
        return null;
    }

    private Address vtableWrittenToTrackedThis(
        Instruction instruction,
        ForwardArgState state) {

        if (!"MOV".equals(upperMnemonic(instruction))) {
            return null;
        }

        Integer offset = trackedThisOffsetForMemoryOperand(instruction, 0, state.registers);
        TrackedValue source = trackedOperandValue(instruction, 1, state);
        if (offset != null && offset == 0 && source != null &&
            source.address != null && isVtableLike(source.address)) {
            return source.address;
        }
        return null;
    }

    private void observeArgumentAssignment(Instruction instruction, ArgState state) {
        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return;
        }

        String destination = registerOperand(instruction, 0);
        if (destination != null) {
            if ("RDX".equals(destination) && state.nameAddress == null) {
            Address address = referencedAddress(instruction, 1);
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
                    Address address = referencedAddress(instruction, 1);
                    if (address != null) {
                        state.handlerExpression = formatAddress(address);
                    }
                    else {
                        state.handlerExpression = operandText(instruction, 1);
                    }
                }
            }
            else if ("R9".equals(destination) && !state.groupKnown) {
                Long immediate = literalOperandValue(instruction, 1);
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
                Address address = referencedAddress(instruction, 1);
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
        for (int i = 0; i < I_FRAGMENT_VTABLE_SCAN_SLOTS; i++) {
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

    private JsonObject vtableDiagnostics(
        Address vtable,
        FieldHandlerShape handlerShape,
        int maxSlots,
        FragmentMetadataEvidence fragmentMetadata) {

        JsonObject object = new JsonObject();
        add(object, "symbol", primarySymbolName(vtable));
        object.addProperty("executableSlotCount", countExecutableVtableSlots(vtable, maxSlots));

        String role = null;
        String roleSource = null;
        if (handlerShape != null) {
            role = handlerShape.kind;
            roleSource = handlerShape.containerWireShape == null
                ? "field-handler-shape"
                : "replicated-container-shape";
        }
        else if (fragmentMetadata != null) {
            role = "fragment";
            roleSource = "fragment-metadata-vtable-slots";
        }
        else {
            role = vtableRoleFromSymbol(primarySymbolName(vtable));
            roleSource = role == null ? null : "vtable-symbol";
        }

        if (role == null) {
            role = "unknown";
            roleSource = "unclassified";
        }
        object.addProperty("role", role);
        object.addProperty("roleSource", roleSource);
        return object;
    }

    private int countExecutableVtableSlots(Address vtable, int maxSlots) {
        if (vtable == null || maxSlots <= 0) {
            return 0;
        }
        int count = 0;
        for (int slot = 0; slot < maxSlots; slot++) {
            Address target = readPointer(vtable.add(slot * 8L));
            if (target == null || !isExecutableAddress(target)) {
                break;
            }
            count++;
        }
        return count;
    }

    private String primarySymbolName(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        Symbol symbol = currentProgram.getSymbolTable().getPrimarySymbol(address);
        return symbol == null ? null : symbol.getName(true);
    }

    private String vtableRoleFromSymbol(String symbolName) {
        if (symbolName == null || symbolName.isBlank()) {
            return null;
        }
        String normalized = normalizeQualifiedNetworkTypeName(symbolName);
        if (normalized == null) {
            normalized = symbolName;
        }
        if (normalized.contains("ReplicatedContainer") ||
            normalized.contains("ReplicatedMapFieldHandler") ||
            normalized.contains("ReplicatedVectorFieldHandler") ||
            normalized.contains("ReplicatedSetFieldHandler")) {
            return "replicated-container";
        }
        if (normalized.contains("ReplicatedFieldHandler") ||
            normalized.contains("DeltaCompressedReplicatedFieldHandler") ||
            normalized.contains("DynamicDeltaReplicatedFieldHandler")) {
            return "replicated-field";
        }
        if (normalized.contains("FixedReplicatedState")) {
            return "fixed-replicated-state";
        }
        if (normalized.contains("IFragment")) {
            return "fragment";
        }
        if (normalized.contains("IMessage")) {
            return "message";
        }
        return null;
    }

    private FragmentMetadataEvidence fragmentMetadataEvidence(Address vtable) {
        if (vtable == null) {
            return null;
        }

        Address isMetadataTarget = readPointer(vtable.add(I_FRAGMENT_IS_METADATA_SLOT * 8L));
        Address categoryTarget = readPointer(vtable.add(I_FRAGMENT_GET_CATEGORY_SLOT * 8L));
        Address isMetadataFunction = resolvedCodeTarget(isMetadataTarget);
        Address categoryFunction = resolvedCodeTarget(categoryTarget);
        Long isMetadataValue = constantReturnValue(isMetadataFunction);
        Long categoryValue = constantReturnValue(categoryFunction);
        if (isMetadataValue == null && categoryValue == null) {
            return null;
        }

        FragmentMetadataEvidence evidence = new FragmentMetadataEvidence();
        evidence.isMetadataFunction = isMetadataFunction;
        evidence.isMetadata = isMetadataValue == null ? null : isMetadataValue != 0;
        evidence.categoryFunction = categoryFunction;
        if (categoryValue != null && categoryValue >= 0 && categoryValue <= 0xff) {
            evidence.categoryValue = categoryValue.intValue();
            evidence.category = fragmentCategoryName(evidence.categoryValue);
        }
        return evidence;
    }

    private Long constantReturnValue(Address address) {
        if (!isExecutableAddress(address)) {
            return null;
        }

        Long returnValue = null;
        int count = 0;
        for (Instruction instruction : linearInstructions(address, CONSTANT_RETURN_SCAN_LIMIT)) {
            if (count++ >= CONSTANT_RETURN_SCAN_LIMIT) {
                break;
            }

            String mnemonic = upperMnemonic(instruction);
            if (mnemonic == null) {
                return null;
            }
            if ("RET".equals(mnemonic)) {
                return returnValue;
            }
            if ("CCH".equals(mnemonic) || "NOP".equals(mnemonic)) {
                continue;
            }

            String destination = registerOperand(instruction, 0);
            if (("XOR".equals(mnemonic) || "SUB".equals(mnemonic)) &&
                isReturnRegister(destination) &&
                destination.equals(registerOperand(instruction, 1))) {
                returnValue = 0L;
                continue;
            }

            if ("MOV".equals(mnemonic) && isReturnRegister(destination)) {
            Long immediate = literalOperandValue(instruction, 1);
                if (immediate == null) {
                    return null;
                }
                returnValue = immediate;
                continue;
            }

            if ("MOVZX".equals(mnemonic) && isReturnRegister(destination)) {
                String source = registerOperand(instruction, 1);
                if (!isReturnRegister(source) || returnValue == null) {
                    return null;
                }
                continue;
            }

            if ("ADD".equals(mnemonic) || "SUB".equals(mnemonic)) {
                String stackRegister = registerOperand(instruction, 0);
                if ("RSP".equals(stackRegister) && literalOperandValue(instruction, 1) != null) {
                    continue;
                }
            }

            return null;
        }
        return null;
    }

    private boolean isReturnRegister(String register) {
        return "AL".equals(register) || "RAX".equals(register);
    }

    private String fragmentCategoryName(Integer value) {
        if (value == null) {
            return null;
        }
        return switch (value) {
            case 0 -> "Uncategorized";
            case 1 -> "PlayerCharacter";
            case 2 -> "NonPlayerCharacter";
            case 3 -> "ImportantNonPlayerCharacter";
            case 4 -> "Spell";
            case 5 -> "Projectile";
            case 6 -> "Buildable";
            case 7 -> "NumCategories";
            default -> null;
        };
    }

    private AzRttiEvidence decodeAzRttiFromVtable(Address vtable) {
        if (vtable == null) {
            return null;
        }

        AzRttiEvidence evidence = new AzRttiEvidence();
        evidence.source = "instance-vtable";
        evidence.address = formatAddress(vtable);

        for (int slot = 0; slot < AZ_RTTI_VTABLE_SCAN_SLOTS; slot++) {
            if (isVtableSlotBoundary(vtable, slot)) {
                break;
            }
            Address slotPointer = readPointer(vtable.add(slot * 8L));
            Address body = resolvedCodeTarget(slotPointer);
            if (!isExecutableAddress(body)) {
                break;
            }

            TypeIdDecode typeId = decodeAzRttiTypeIdProvider(slotPointer);
            if (typeId != null) {
                if (evidence.typeId == null) {
                    evidence.typeId = typeId.typeId;
                }
                evidence.providers.add(typeId.toJson(slot));
            }

            TypeNameDecode typeName = decodeAzRttiTypeNameProvider(slotPointer);
            if (typeName != null) {
                if (evidence.typeName == null) {
                    evidence.typeName = typeName.typeName;
                    evidence.typeNameSource = typeName.typeNameSource;
                }
                evidence.providers.add(typeName.toJson(slot));
            }
        }

        String preferredTypeId = preferredAzRttiTypeId(evidence);
        if (preferredTypeId != null) {
            evidence.typeId = preferredTypeId;
        }
        return evidence.hasIdentity() ? evidence : null;
    }

    private String preferredAzRttiTypeId(AzRttiEvidence evidence) {
        if (evidence == null) {
            return null;
        }

        String typeName = providerTypeNameAtSlot(evidence, 1);
        String isTypeOfTypeId = providerTypeIdAtSlot(evidence, 2);
        if (isLikelyRuntimeTypeName(typeName) && isTypeOfTypeId != null) {
            return isTypeOfTypeId;
        }

        String actualTypeId = providerTypeIdAtSlot(evidence, 0);
        if (actualTypeId != null) {
            return actualTypeId;
        }
        return evidence.typeId;
    }

    private boolean isVtableSlotBoundary(Address vtable, int slot) {
        if (slot <= 0 || vtable == null) {
            return false;
        }

        Address slotAddress = vtable.add(slot * 8L);
        ReferenceIterator references =
            currentProgram.getReferenceManager().getReferencesTo(slotAddress);
        return references.hasNext();
    }

    private TypeIdDecode decodeAzRttiTypeIdProvider(Address function) {
        Address provider = resolvedCodeTarget(function);
        return decodeTypeIdFromReferencedStrings(function, provider, true);
    }

    private TypeIdDecode decodeDirectTypeIdProvider(Address function) {
        Address provider = resolvedCodeTarget(function);
        return decodeTypeIdFromReferencedStrings(function, provider, true);
    }

    private TypeIdDecode decodeTypeIdFromReferencedStrings(
        Address function,
        Address provider,
        boolean followProviderCalls) {
        Deque<HandlerScanFrame> stack = new ArrayDeque<>();
        stack.addFirst(new HandlerScanFrame(provider, 0));
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        LinkedHashSet<String> seenTypeIds = new LinkedHashSet<>();
        TypeIdDecode result = null;

        while (!stack.isEmpty()) {
            HandlerScanFrame frame = stack.removeFirst();
            Address current = resolvedCodeTarget(frame.address);
            if (!isExecutableAddress(current) || !seen.add(current.toString())) {
                continue;
            }

            List<Instruction> instructions =
                reachableInstructions(current, REGISTRY_HANDLER_RTTI_SCAN_LIMIT);
            if (instructions.isEmpty()) {
                continue;
            }

            ArrayList<Address> callees = new ArrayList<>();
            boolean foundUuidInFrame = false;
            for (Instruction instruction : instructions) {
                for (Address target : referencedAddresses(instruction)) {
                    String sourceKind = "referencedString";
                    String uuid = canonicalUuidFromExactString(readPrintableString(target));
                    if (uuid == null && isNativeUuidLiteralReference(instruction, target)) {
                        uuid = canonicalUuidFromNativeUuidLiteral(target);
                        if (uuid == null) {
                            incrementCount(nativeUuidRejectCounts, "bad-layout");
                        }
                        else if (!registryTypeIds.contains(normalizeUuid(uuid))) {
                            incrementCount(nativeUuidRejectCounts, "not-in-registry");
                            uuid = null;
                        }
                        sourceKind = "nativeUuidLiteral";
                    }
                    if (uuid == null) {
                        continue;
                    }

                    foundUuidInFrame = true;
                    String source = frame.depth == 0
                        ? sourceKind
                        : "providerCall" + Character.toUpperCase(sourceKind.charAt(0)) +
                            sourceKind.substring(1);
                    incrementCount(typeIdSourceCounts, source);
                    if (result == null) {
                        result = new TypeIdDecode();
                        result.function = function;
                        result.provider = current;
                        result.typeId = uuid;
                        result.typeIdSource = source;
                        result.sourceAddress = target;
                    }
                    if (seenTypeIds.add(uuid)) {
                        int chainIndex = result.typeIdChain.size();
                        result.typeIdChain.add(typeIdChainEntry(
                            current,
                            uuid,
                            source,
                            target,
                            frame.depth,
                            chainIndex));
                    }
                }

                if (!followProviderCalls || frame.depth >= REGISTRY_HANDLER_RTTI_CALL_DEPTH ||
                    foundUuidInFrame ||
                    !instruction.getFlowType().isCall()) {
                    continue;
                }
                Address target = resolvedCodeTarget(callTarget(instruction));
                if (isExecutableAddress(target)) {
                    callees.add(target);
                }
            }
            for (int i = callees.size() - 1; i >= 0; i--) {
                stack.addFirst(new HandlerScanFrame(callees.get(i), frame.depth + 1));
            }
        }
        return result;
    }

    private JsonObject typeIdChainEntry(
        Address provider,
        String typeId,
        String source,
        Address sourceAddress,
        int depth,
        int chainIndex) {

        JsonObject object = new JsonObject();
        add(object, "provider", formatAddress(provider));
        add(object, "typeId", typeId);
        add(object, "typeIdSource", source);
        add(object, "sourceAddress", formatAddress(sourceAddress));
        object.addProperty("depth", depth);
        object.addProperty("chainIndex", chainIndex);
        object.addProperty("azRttiRole", chainIndex == 0 ? "self" : "baseOrInterface");
        return object;
    }

    private TypeNameDecode decodeTypeNameFromReferencedStrings(Address function, Address provider) {
        List<Instruction> instructions =
            linearInstructions(provider, REGISTRY_HANDLER_RTTI_SCAN_LIMIT);
        if (instructions.isEmpty()) {
            return null;
        }

        TypeNameDecode best = null;
        int bestScore = Integer.MIN_VALUE;
        for (Instruction instruction : instructions) {
            for (Address target : referencedAddresses(instruction)) {
                String name = readPrintableString(target);
                if (!isLikelyRuntimeTypeName(name)) {
                    continue;
                }
                int score = runtimeTypeNameScore(name);
                if (best != null && score < bestScore) {
                    continue;
                }

                TypeNameDecode decode = new TypeNameDecode();
                decode.function = function;
                decode.provider = provider;
                decode.typeName = name;
                decode.typeNameSource = "referencedString";
                decode.typeNameAddress = target;
                best = decode;
                bestScore = score;
            }
        }
        return best;
    }

    private TypeNameDecode decodeAzRttiTypeNameProvider(Address function) {
        Address provider = resolvedCodeTarget(function);
        TypeNameDecode functionName = typeNameDecodeFromFunctionName(function, provider);
        if (functionName != null) {
            return functionName;
        }

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

        return decodeTypeNameFromReferencedStrings(function, provider);
    }

    private Address stringAddressReturnedBySimpleFunction(Address function) {
        Address target = resolvedCodeTarget(function);
        List<Instruction> instructions = linearInstructions(target, 16);
        if (instructions.isEmpty()) {
            return null;
        }

        for (Instruction instruction : instructions) {
            String mnemonic = upperMnemonic(instruction);
            if ("RET".equals(mnemonic)) {
                break;
            }
            if (!("LEA".equals(mnemonic) || "MOV".equals(mnemonic))) {
                continue;
            }
            String destination = registerOperand(instruction, 0);
            if (!"RAX".equals(destination)) {
                continue;
            }
            for (Address address : referencedAddresses(instruction)) {
                if (isReturnedIdentityAddress(address)) {
                    return address;
                }
            }
        }
        return null;
    }

    private boolean isReturnedIdentityAddress(Address address) {
        if (!isProgramAddress(address)) {
            return false;
        }
        if (isPlausibleTypeName(readPrintableString(address))) {
            return true;
        }
        Symbol symbol = currentProgram.getSymbolTable().getPrimarySymbol(address);
        return symbol != null && parseInstallRegistrationHookTypeName(symbol.getName(true)) != null;
    }

    private Address resolvedCodeTarget(Address address) {
        Address current = address;
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        while (true) {
            if (!isExecutableAddress(current)) {
                return current;
            }
            if (!seen.add(current.toString())) {
                return current;
            }

            Address target = terminalJumpTarget(current);
            if (target != null && !target.equals(current)) {
                current = target;
                continue;
            }
            return current;
        }
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
                Address target = branchTarget(instruction);
                return isProgramAddress(target) ? target : null;
            }
            if (mnemonic != null && (mnemonic.startsWith("RET") || mnemonic.startsWith("INT"))) {
                return null;
            }
            if (!isThunkPassThroughInstruction(instruction)) {
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

    private boolean isThunkPassThroughInstruction(Instruction instruction) {
        String mnemonic = upperMnemonic(instruction);
        if (mnemonic == null) {
            return false;
        }
        if ("NOP".equals(mnemonic)) {
            return true;
        }
        if ("MOV".equals(mnemonic) || "LEA".equals(mnemonic)) {
            String destination = registerOperand(instruction, 0);
            return destination != null &&
                ("RAX".equals(destination) ||
                    "RCX".equals(destination) ||
                    "RDX".equals(destination) ||
                    "R8".equals(destination) ||
                    "R9".equals(destination));
        }
        return false;
    }

    private Address branchTarget(Instruction instruction) {
        Address target = callTarget(instruction);
        if (isProgramAddress(target)) {
            return target;
        }

        for (Address referenced : referencedAddresses(instruction)) {
            if (isExecutableAddress(referenced)) {
                return referenced;
            }

            Address indirect = readPointer(referenced);
            if (isProgramAddress(indirect)) {
                return indirect;
            }
        }

        return null;
    }

    private WireShape classifyWireShape(Address marshal, Address marshalTarget) {
        Address effectiveMarshal = marshalTarget == null ? marshal : marshalTarget;
        return classifyMarshalPath(effectiveMarshal);
    }

    private ContainerWireShape classifyReplicatedContainerWireShape(Address vtable) {
        Address marshal = readPointer(vtable.add(FIELD_HANDLER_MARSHAL_SLOT * 8L));
        Address marshalFull = readPointer(vtable.add(14L * 8L));
        Address unmarshalFull = readPointer(vtable.add(15L * 8L));
        if (!isExecutableAddress(marshal) ||
            !isExecutableAddress(marshalFull) ||
            !isExecutableAddress(unmarshalFull)) {
            return null;
        }

        ArrayList<String> deltaShapes = new ArrayList<>();
        collectOrderedMarshalShapes(marshal, 2, new LinkedHashSet<>(), deltaShapes);
        ArrayList<String> fullShapes = new ArrayList<>();
        collectOrderedMarshalShapes(marshalFull, 3, new LinkedHashSet<>(), fullShapes);
        if (!deltaShapes.contains("vlq-u32") && !fullShapes.contains("vlq-u32")) {
            return null;
        }
        if (!deltaShapes.contains("sequence-number") && !fullShapes.contains("sequence-number")) {
            return null;
        }

        String deltaShape = containerShapeFromDeltaMarshalShapes(deltaShapes);
        String fullShape = containerShapeFromFullMarshalShapes(fullShapes);
        String primaryShape = deltaShape == null ? fullShape : deltaShape;
        if (primaryShape == null) {
            return null;
        }
        return new ContainerWireShape(
            new WireShape(primaryShape, "replicated-container-marshal-calls"),
            deltaShape,
            fullShape,
            deltaShapes,
            fullShapes);
    }

    private String containerShapeFromDeltaMarshalShapes(List<String> shapes) {
        int sequenceIndex = shapes.indexOf("sequence-number");
        String keyShape = previousDataShape(shapes, sequenceIndex < 0 ? shapes.size() : sequenceIndex);
        String valueShape = nextDataShape(shapes, Math.max(0, sequenceIndex + 1));
        return containerShape(keyShape, valueShape);
    }

    private String containerShapeFromFullMarshalShapes(List<String> shapes) {
        String keyShape = firstDataShape(shapes);
        String valueShape = secondDataShape(shapes);
        return containerShape(keyShape, valueShape);
    }

    private String containerShape(String keyShape, String valueShape) {
        if (keyShape == null || valueShape == null) {
            return null;
        }
        return "replicated-container<" + keyShape + "," + valueShape + ">";
    }

    private void collectOrderedMarshalShapes(
        Address address,
        int depth,
        Set<String> seen,
        ArrayList<String> shapes) {

        Address targetAddress = resolvedCodeTarget(address);
        if (!isExecutableAddress(targetAddress) || !seen.add(targetAddress.toString())) {
            return;
        }

        WireShape named = wireShapeFromFunctionName(targetAddress);
        if (named != null) {
            shapes.add(named.shape);
            return;
        }

        Function function = functionAtOrContaining(targetAddress);
        if (function == null || depth <= 0) {
            return;
        }

        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (instruction.getMinAddress().compareTo(targetAddress) < 0) {
                continue;
            }
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }
            if (!instruction.getFlowType().isCall()) {
                if (instruction.getFlowType().isJump()) {
                    Address jump = branchTarget(instruction);
                    if (isExecutableAddress(jump)) {
                        collectOrderedMarshalShapes(jump, depth - 1, seen, shapes);
                    }
                }
                continue;
            }
            Address call = callTarget(instruction);
            if (isExecutableAddress(call)) {
                collectOrderedMarshalShapes(call, depth - 1, seen, shapes);
            }
        }
    }

    private String previousDataShape(List<String> shapes, int beforeIndex) {
        for (int i = beforeIndex - 1; i >= 0; i--) {
            String shape = shapes.get(i);
            if (isReplicatedContainerDataShape(shape)) {
                return shape;
            }
        }
        return null;
    }

    private String nextDataShape(List<String> shapes, int startIndex) {
        for (int i = startIndex; i < shapes.size(); i++) {
            String shape = shapes.get(i);
            if (isReplicatedContainerDataShape(shape)) {
                return shape;
            }
        }
        return null;
    }

    private String firstDataShape(List<String> shapes) {
        return nextDataShape(shapes, 0);
    }

    private String secondDataShape(List<String> shapes) {
        String first = firstDataShape(shapes);
        if (first == null) {
            return null;
        }
        boolean sawFirst = false;
        for (String shape : shapes) {
            if (!isReplicatedContainerDataShape(shape)) {
                continue;
            }
            if (sawFirst) {
                return shape;
            }
            if (shape.equals(first)) {
                sawFirst = true;
            }
        }
        return null;
    }

    private boolean isReplicatedContainerDataShape(String shape) {
        return shape != null &&
            !"vlq-u32".equals(shape) &&
            !"sequence-number".equals(shape);
    }

    private WireShape classifyMarshalPath(Address address) {
        if (!isExecutableAddress(address)) {
            return null;
        }

        Deque<MarshalPathFrame> stack = new ArrayDeque<>();
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        stack.push(new MarshalPathFrame(address, ""));

        while (!stack.isEmpty()) {
            MarshalPathFrame frame = stack.pop();
            Address targetAddress = resolvedCodeTarget(frame.address);
            if (!isExecutableAddress(targetAddress) || !seen.add(targetAddress.toString())) {
                continue;
            }

            WireShape named = wireShapeFromFunctionName(targetAddress);
            if (named != null) {
                return frame.wrap(named);
            }

            if (looksLikeBoolMarshal(targetAddress)) {
                return frame.wrap(new WireShape("bool", "marshal-bool-instructions"));
            }

            Integer fixedRawLength = fixedRawMarshalLength(targetAddress);
            if (fixedRawLength != null) {
                if (fixedRawLength == 1) {
                    return frame.wrap(new WireShape("u8", "marshal-raw-write-length"));
                }
                if (fixedRawLength > 1) {
                    return frame.wrap(new WireShape(
                        "fixed-bytes-" + fixedRawLength,
                        "marshal-raw-write-length"));
                }
            }

            Function function = functionAtOrContaining(targetAddress);
            if (function == null) {
                continue;
            }

            int count = 0;
            for (Instruction instruction : functionInstructions(function)) {
                if (instruction.getMinAddress().compareTo(targetAddress) < 0) {
                    continue;
                }
                if (count++ >= VTABLE_SCAN_LIMIT) {
                    break;
                }
                if (!instruction.getFlowType().isCall()) {
                    continue;
                }
                Address target = callTarget(instruction);
                if (isExecutableAddress(target)) {
                    stack.push(frame.nested(target));
                }
            }
        }
        return null;
    }

    private Integer fixedRawMarshalLength(Address address) {
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
                Long immediate = literalOperandValue(instruction, 1);
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
                Long immediate = literalOperandValue(instruction, 1);
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
        WireShape generic = wireShapeFromMarshalFunctionName(name);
        if (generic != null) {
            return generic;
        }
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
        if (name.contains("GridMate::VlqU64Marshaler::Marshal") ||
            name.contains("Amazon::Pervasives::VlqU64Marshaler::Marshal")) {
            return new WireShape("vlq-u64", "marshal-function-name");
        }
        if (name.contains("Amazon::Pervasives::Marshaller<Amazon::Hub::SequenceNumber>::Marshal")) {
            return new WireShape("sequence-number", "marshal-function-name");
        }
        if (name.contains("GridMate::QuatCompNormMarshaler::Marshal") ||
            name.contains("GridMate::QuatCompressSmallestThree")) {
            return new WireShape("quat-comp-norm", "marshal-function-name");
        }
        return null;
    }

    private WireShape wireShapeFromMarshalFunctionName(String name) {
        if (name == null || !name.contains("::Marshal")) {
            return null;
        }

        String custom = wireShapeFromMarshallerType(name);
        if (custom != null) {
            return new WireShape(custom, "marshal-function-name");
        }

        int marshaller = name.indexOf("Marshaller<");
        if (marshaller < 0) {
            marshaller = name.indexOf("Marshaler<");
        }
        if (marshaller < 0) {
            return null;
        }
        int templateStart = name.indexOf('<', marshaller);
        int templateEnd = matchingIndex(name, templateStart, '<', '>');
        if (templateStart < 0 || templateEnd < 0) {
            return null;
        }

        String nativeType = name.substring(templateStart + 1, templateEnd);
        String shape = wireShapeFromNativeType(nativeType);
        return shape == null ? null : new WireShape(shape, "marshal-function-name");
    }

    private boolean looksLikeBoolMarshal(Address address) {
        Function function = functionAtOrContaining(address);
        if (function == null) {
            return false;
        }

        boolean sawSetcc = false;
        boolean sawOneByteLength = false;
        int count = 0;
        for (Instruction instruction : functionInstructions(function)) {
            if (instruction.getMinAddress().compareTo(address) < 0) {
                continue;
            }
            if (count++ >= VTABLE_SCAN_LIMIT) {
                break;
            }

            String mnemonic = upperMnemonic(instruction);
            if (mnemonic != null && mnemonic.startsWith("SET")) {
                sawSetcc = true;
            }
            if ("MOV".equals(mnemonic)) {
                String destination = registerOperand(instruction, 0);
                Long immediate = literalOperandValue(instruction, 1);
                if (("R8".equals(destination) || "RDX".equals(destination)) &&
                    immediate != null && immediate == 1L) {
                    sawOneByteLength = true;
                }
            }
            if ("RET".equals(mnemonic) || "CCH".equals(mnemonic)) {
                break;
            }
        }
        return sawSetcc && sawOneByteLength;
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
        List<Address> addresses = referencedAddresses(instruction);
        return addresses.isEmpty() ? null : addresses.get(0);
    }

    private Address referencedAddress(Instruction instruction, int operandIndex) {
        List<Address> addresses = referencedAddresses(instruction, operandIndex);
        return addresses.isEmpty() ? null : addresses.get(0);
    }

    private List<Address> referencedAddresses(Instruction instruction) {
        LinkedHashSet<Address> result = new LinkedHashSet<>();
        for (Reference reference : instruction.getReferencesFrom()) {
            Address to = reference.getToAddress();
            if (isProgramAddress(to)) {
                result.add(to);
            }
        }
        for (int i = 0; i < instruction.getNumOperands(); i++) {
            for (Object object : operandObjects(instruction, i)) {
                if (object instanceof Address address && isProgramAddress(address)) {
                    result.add(address);
                }
            }
            Address computed = computedOperandAddress(instruction, i);
            if (isProgramAddress(computed)) {
                result.add(computed);
            }
        }
        for (Address address : pcodeConstantAddresses(instruction)) {
            result.add(address);
        }
        return new ArrayList<>(result);
    }

    private List<Address> referencedAddresses(Instruction instruction, int operandIndex) {
        LinkedHashSet<Address> result = new LinkedHashSet<>();
        for (Reference reference : instruction.getReferencesFrom()) {
            if (reference.getOperandIndex() != operandIndex) {
                continue;
            }
            Address to = reference.getToAddress();
            if (isProgramAddress(to)) {
                result.add(to);
            }
        }

        for (Object object : operandObjects(instruction, operandIndex)) {
            if (object instanceof Address address && isProgramAddress(address)) {
                result.add(address);
            }
        }

        Address computed = computedOperandAddress(instruction, operandIndex);
        if (isProgramAddress(computed)) {
            result.add(computed);
        }
        return new ArrayList<>(result);
    }

    private List<Address> pcodeConstantAddresses(Instruction instruction) {
        LinkedHashSet<Address> result = new LinkedHashSet<>();
        try {
            for (PcodeOp op : instruction.getPcode()) {
                collectPcodeAddress(result, op.getOutput());
                for (int i = 0; i < op.getNumInputs(); i++) {
                    collectPcodeAddress(result, op.getInput(i));
                }
            }
        }
        catch (Exception ignored) {
            return List.of();
        }
        return new ArrayList<>(result);
    }

    private void collectPcodeAddress(Set<Address> result, Varnode node) {
        if (node == null) {
            return;
        }
        Address nodeAddress = node.getAddress();
        if (isProgramAddress(nodeAddress)) {
            result.add(nodeAddress);
        }
        if (node.isConstant()) {
            Address address = absoluteAddress(node.getOffset());
            if (isProgramAddress(address)) {
                result.add(address);
            }
        }
    }

    private Address computedOperandAddress(Instruction instruction, int operandIndex) {
        Object[] objects = operandObjects(instruction, operandIndex);
        boolean ripRelative = false;
        boolean hasNonRipRegister = false;
        long displacement = 0;
        Address absoluteCandidate = null;

        for (Object object : objects) {
            if (object instanceof Register register) {
                String name = canonicalRegisterName(register.getName());
                if ("RIP".equals(name) || "EIP".equals(name)) {
                    ripRelative = true;
                }
                else {
                    hasNonRipRegister = true;
                }
            }
            else if (object instanceof Scalar scalar) {
                long signed = scalar.getSignedValue();
                displacement += signed;

                Address candidate = absoluteAddress(scalar.getUnsignedValue());
                if (isProgramAddress(candidate)) {
                    absoluteCandidate = candidate;
                }
            }
        }

        if (ripRelative) {
            long next = instruction.getMinAddress().getOffset() + instruction.getLength();
            Address computed = absoluteAddress(next + displacement);
            if (isProgramAddress(computed)) {
                return computed;
            }
        }

        return hasNonRipRegister ? null : absoluteCandidate;
    }

    private Integer memoryDisplacementForThisLikeOperand(Instruction instruction, int operandIndex) {
        Object[] objects = operandObjects(instruction, operandIndex);
        String baseRegister = null;
        int displacement = 0;
        for (Object object : objects) {
            if (object instanceof Register register) {
                String name = canonicalRegisterName(register.getName());
                if (!"RIP".equals(name) && !"RSP".equals(name)) {
                    if (baseRegister != null && !baseRegister.equals(name)) {
                        return null;
                    }
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
        return baseRegister != null && displacement != 0 ? displacement : null;
    }

    private TrackedValue trackedLeaValue(
        Instruction instruction,
        ForwardArgState state) {

        Integer offset = trackedThisOffsetForMemoryOperand(instruction, 1, state.registers);
        if (offset != null) {
            return TrackedValue.thisOffset(offset);
        }

        MemoryReference memory = memoryReference(instruction, 1);
        Integer stackOffset = stackSlotOffset(memory, state);
        if (stackOffset != null) {
            return TrackedValue.stackOffset(stackOffset);
        }

        TrackedValue baseOffset = trackedBaseOffsetForMemoryOperand(instruction, 1, state.registers);
        if (baseOffset != null) {
            return baseOffset;
        }

        Address address = referencedAddress(instruction, 1);
        if (address != null) {
            return TrackedValue.address(address);
        }
        return null;
    }

    private TrackedValue trackedOperandValue(
        Instruction instruction,
        int operandIndex,
        ForwardArgState state) {

        String sourceRegister = registerOperand(instruction, operandIndex);
        if (sourceRegister != null) {
            TrackedValue value = state.registers.get(sourceRegister);
            return value == null ? null : value.copy();
        }

        TrackedValue value = trackedThisLoadValue(instruction, operandIndex, state);
        if (value != null) {
            return value;
        }

        value = trackedStackLoadValue(instruction, operandIndex, state);
        if (value != null) {
            return value;
        }

        value = trackedFormattedFieldNamePointerLoadValue(instruction, operandIndex, state);
        if (value != null) {
            return value;
        }

        Address address = referencedAddress(instruction, operandIndex);
        if (address != null) {
            return TrackedValue.address(address);
        }

        String sourceText = operandText(instruction, operandIndex);
        if (sourceText != null && sourceText.contains("[")) {
            return null;
        }

        Long immediate = literalOperandValue(instruction, operandIndex);
        return immediate == null ? null : TrackedValue.immediate(immediate);
    }

    private Integer trackedThisOffsetForMemoryOperand(
        Instruction instruction,
        int operandIndex,
        Map<String, TrackedValue> registers) {

        MemoryAddress memory = memoryAddress(instruction, operandIndex);
        if (memory == null) {
            return null;
        }
        boolean sawThisBase = false;
        int displacement = 0;
        int thisBaseOffset = 0;
        for (MemoryTerm term : memory.terms) {
            if ("RIP".equals(term.register) || "RSP".equals(term.register)) {
                return null;
            }

            TrackedValue value = registers.get(term.register);
            if (value == null) {
                return null;
            }
            if (value.thisOffset != null) {
                if (sawThisBase || term.scale != 1) {
                    return null;
                }
                sawThisBase = true;
                thisBaseOffset = value.thisOffset;
            }
            else if (value.immediate != null) {
                Long scaled = scaledImmediate(value.immediate, term.scale);
                if (scaled == null || scaled < Integer.MIN_VALUE || scaled > Integer.MAX_VALUE) {
                    return null;
                }
                Integer nextDisplacement = addOffsets(displacement, scaled);
                if (nextDisplacement == null) {
                    return null;
                }
                displacement = nextDisplacement;
            }
            else {
                return null;
            }
        }

        if (!sawThisBase) {
            return null;
        }
        Integer withMemoryDisplacement = addOffsets(thisBaseOffset, memory.displacement);
        return withMemoryDisplacement == null
            ? null
            : addOffsets(withMemoryDisplacement, displacement);
    }

    private TrackedValue trackedBaseOffsetForMemoryOperand(
        Instruction instruction,
        int operandIndex,
        Map<String, TrackedValue> registers) {

        MemoryAddress memory = memoryAddress(instruction, operandIndex);
        if (memory == null) {
            return null;
        }
        String trackedBaseKey = null;
        int trackedBaseOffset = 0;
        int displacement = 0;
        for (MemoryTerm term : memory.terms) {
            if ("RIP".equals(term.register) || "RSP".equals(term.register)) {
                return null;
            }

            TrackedValue value = registers.get(term.register);
            if (value == null) {
                return null;
            }
            if (value.baseKey != null && value.baseOffset != null) {
                if (trackedBaseKey != null || term.scale != 1) {
                    return null;
                }
                trackedBaseKey = value.baseKey;
                trackedBaseOffset = value.baseOffset;
            }
            else if (value.immediate != null) {
                Long scaled = scaledImmediate(value.immediate, term.scale);
                if (scaled == null || scaled < Integer.MIN_VALUE || scaled > Integer.MAX_VALUE) {
                    return null;
                }
                Integer nextDisplacement = addOffsets(displacement, scaled);
                if (nextDisplacement == null) {
                    return null;
                }
                displacement = nextDisplacement;
            }
            else {
                return null;
            }
        }

        if (trackedBaseKey == null) {
            return null;
        }
        Integer withMemoryDisplacement = addOffsets(trackedBaseOffset, memory.displacement);
        Integer totalOffset = withMemoryDisplacement == null
            ? null
            : addOffsets(withMemoryDisplacement, displacement);
        return totalOffset == null
            ? null
            : TrackedValue.baseOffset(trackedBaseKey, totalOffset);
    }

    private Long scaledImmediate(long value, int scale) {
        try {
            return Math.multiplyExact(value, (long)scale);
        }
        catch (ArithmeticException ignored) {
            return null;
        }
    }

    private <K, V> void putOrRemove(
        Map<K, V> map,
        K key,
        V value) {

        if (key == null) {
            return;
        }
        if (value == null) {
            map.remove(key);
        }
        else {
            map.put(key, value);
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

    private void clearVolatileAllocatorDispatchRegisters(ForwardArgState state) {
        state.allocatorDispatchRegisters.remove("RAX");
        state.allocatorDispatchRegisters.remove("RCX");
        state.allocatorDispatchRegisters.remove("RDX");
        state.allocatorDispatchRegisters.remove("R8");
        state.allocatorDispatchRegisters.remove("R9");
        state.allocatorDispatchRegisters.remove("R10");
        state.allocatorDispatchRegisters.remove("R11");
    }

    private void swapAllocatorDispatchRegisters(
        ForwardArgState state,
        String left,
        String right) {

        boolean leftTracked = state.allocatorDispatchRegisters.contains(left);
        boolean rightTracked = state.allocatorDispatchRegisters.contains(right);
        if (rightTracked) {
            state.allocatorDispatchRegisters.add(left);
        }
        else {
            state.allocatorDispatchRegisters.remove(left);
        }
        if (leftTracked) {
            state.allocatorDispatchRegisters.add(right);
        }
        else {
            state.allocatorDispatchRegisters.remove(right);
        }
    }

    private Long literalOperandValue(Instruction instruction, int operandIndex) {
        String text = operandText(instruction, operandIndex);
        if (text != null && text.contains("[")) {
            return null;
        }
        return immediateValue(instruction, operandIndex);
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

    private Long parseSignedIntegerLiteral(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim().replace("_", "");
        if (trimmed.isEmpty()) {
            return null;
        }

        boolean negative = trimmed.startsWith("-");
        if (negative) {
            trimmed = trimmed.substring(1);
        }
        Long parsed = parseIntegerLiteral(trimmed);
        if (parsed == null) {
            return null;
        }
        if (!negative) {
            return parsed;
        }
        if (parsed == Long.MIN_VALUE) {
            return null;
        }
        return -parsed;
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

    private MemoryReference memoryReference(Instruction instruction, int operandIndex) {
        MemoryAddress memory = memoryAddress(instruction, operandIndex);
        if (memory == null || memory.terms.size() != 1) {
            return null;
        }
        MemoryTerm term = memory.terms.get(0);
        return term.scale == 1
            ? new MemoryReference(term.register, memory.displacement)
            : null;
    }

    private MemoryAddress memoryAddress(Instruction instruction, int operandIndex) {
        String text = operandText(instruction, operandIndex);
        if (text == null || !text.contains("[") || !text.contains("]")) {
            return null;
        }

        int start = text.indexOf('[');
        int end = text.lastIndexOf(']');
        if (end <= start) {
            return null;
        }

        ArrayList<MemoryTerm> terms = new ArrayList<>();
        int displacement = 0;
        String inside = text.substring(start + 1, end)
            .replace(" ", "")
            .replace("-", "+-");
        for (String token : inside.split("\\+")) {
            if (token == null || token.isEmpty()) {
                continue;
            }

            MemoryTerm term = parseMemoryTerm(token);
            if (term != null) {
                terms.add(term);
                continue;
            }

            Long value = parseSignedIntegerLiteral(token);
            if (value == null || value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) {
                return null;
            }
            Integer nextDisplacement = addOffsets(displacement, value);
            if (nextDisplacement == null) {
                return null;
            }
            displacement = nextDisplacement;
        }

        return terms.isEmpty() ? null : new MemoryAddress(terms, displacement);
    }

    private MemoryTerm parseMemoryTerm(String token) {
        String[] parts = token.split("\\*", -1);
        if (parts.length == 0 || parts.length > 2) {
            return null;
        }

        String register = canonicalRegisterName(parts[0]);
        if (!isKnownRegisterName(register)) {
            return null;
        }

        int scale = 1;
        if (parts.length == 2) {
            Long parsedScale = parseSignedIntegerLiteral(parts[1]);
            if (parsedScale == null ||
                (parsedScale != 1L &&
                    parsedScale != 2L &&
                    parsedScale != 4L &&
                    parsedScale != 8L)) {
                return null;
            }
            scale = parsedScale.intValue();
        }
        return new MemoryTerm(register, scale);
    }

    private boolean isKnownRegisterName(String register) {
        return register != null &&
            ("RAX".equals(register) ||
                "RBX".equals(register) ||
                "RCX".equals(register) ||
                "RDX".equals(register) ||
                "RSI".equals(register) ||
                "RDI".equals(register) ||
                "RBP".equals(register) ||
                "RSP".equals(register) ||
                "RIP".equals(register) ||
                register.matches("R(?:[8-9]|1[0-5])"));
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
        if (upper.matches("R(?:[8-9]|1[0-5])[BW]")) {
            return upper.substring(0, upper.length() - 1);
        }
        if (upper.matches("R(?:[8-9]|1[0-5])W")) {
            return upper.substring(0, upper.length() - 1);
        }
        if ("AL".equals(upper) || "AH".equals(upper) || "AX".equals(upper) || "EAX".equals(upper)) {
            return "RAX";
        }
        if ("BL".equals(upper) || "BH".equals(upper) || "BX".equals(upper) || "EBX".equals(upper)) {
            return "RBX";
        }
        if ("CL".equals(upper) || "CH".equals(upper) || "CX".equals(upper) || "ECX".equals(upper)) {
            return "RCX";
        }
        if ("DL".equals(upper) || "DH".equals(upper) || "DX".equals(upper) || "EDX".equals(upper)) {
            return "RDX";
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
        if (function == null) {
            function = createKnownFunction(address);
        }
        functionLookupCache.put(key, function);
        return function;
    }

    private Function functionExactlyAt(Address address) {
        if (address == null) {
            return null;
        }
        String key = addressCacheKey("function-exactly-at", address);
        if (functionLookupCache.containsKey(key)) {
            return functionLookupCache.get(key);
        }
        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function == null) {
            function = createKnownFunction(address);
        }
        functionLookupCache.put(key, function);
        return function;
    }

    private Function functionByFullName(String name) {
        if (name == null || name.isEmpty()) {
            return null;
        }
        ensureFunctionByFullNameCacheLoaded();
        return functionByFullNameCache.get(name);
    }

    private Function directTypeUnmarshalFunction(String nativeType, String functionName) {
        Function exact = functionByFullName(functionName);
        if (exact != null) {
            return exact;
        }

        String leaf = sourceTypeLeaf(nativeType);
        if (leaf == null || leaf.isEmpty()) {
            return null;
        }
        ensureFunctionByFullNameCacheLoaded();

        Function selected = null;
        for (Map.Entry<String, Function> entry : functionByFullNameCache.entrySet()) {
            String owner = directUnmarshalOwnerFullName(entry.getKey());
            if (!leaf.equals(sourceTypeLeaf(owner))) {
                continue;
            }
            if (selected != null && !selected.getEntryPoint().equals(entry.getValue().getEntryPoint())) {
                recordNestedTypeShapeReject("ambiguous-direct-type-unmarshal-function");
                return null;
            }
            selected = entry.getValue();
        }
        return selected;
    }

    private void ensureFunctionByFullNameCacheLoaded() {
        if (functionByFullNameCacheLoaded) {
            return;
        }
        Iterator<Function> functions = currentProgram.getFunctionManager().getFunctions(true);
        while (functions.hasNext()) {
            Function function = functions.next();
            String fullName = fullFunctionName(function);
            if (fullName != null && !functionByFullNameCache.containsKey(fullName)) {
                functionByFullNameCache.put(fullName, function);
            }
        }
        functionByFullNameCacheLoaded = true;
    }

    private Function createKnownFunction(Address address) {
        if (!isExecutableAddress(address)) {
            return null;
        }
        try {
            if (currentProgram.getListing().getInstructionAt(address) == null) {
                disassemble(address);
            }
            Function existing = currentProgram.getFunctionManager().getFunctionAt(address);
            if (existing != null) {
                return existing;
            }
            String name = "FUN_" + Long.toHexString(address.getOffset());
            Function created = createFunction(address, name);
            if (created != null) {
                recoveredFunctionCount++;
            }
            return created;
        }
        catch (Exception ignored) {
            recoveredFunctionFailureCount++;
            return null;
        }
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

    private List<Instruction> linearInstructions(Address address, int limit) {
        if (!isExecutableAddress(address) || limit <= 0) {
            return List.of();
        }

        ArrayList<Instruction> instructions = new ArrayList<>();
        Address cursor = address;
        for (int i = 0; i < limit; i++) {
            Instruction instruction = currentProgram.getListing().getInstructionAt(cursor);
            if (instruction == null) {
                break;
            }
            instructions.add(instruction);

            String mnemonic = upperMnemonic(instruction);
            if (mnemonic != null &&
                (mnemonic.startsWith("RET") ||
                    mnemonic.startsWith("INT") ||
                    "CCH".equals(mnemonic))) {
                break;
            }

            Address fallThrough = instruction.getFallThrough();
            if (fallThrough == null || !isProgramAddress(fallThrough)) {
                break;
            }
            cursor = fallThrough;
        }
        return instructions;
    }

    private List<Instruction> reachableInstructions(Address address, int limit) {
        if (!isExecutableAddress(address) || limit <= 0) {
            return List.of();
        }

        ArrayList<Instruction> instructions = new ArrayList<>();
        Deque<Address> pending = new ArrayDeque<>();
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        pending.add(address);

        while (!pending.isEmpty() && instructions.size() < limit) {
            Address cursor = pending.removeFirst();
            while (instructions.size() < limit) {
                if (!isExecutableAddress(cursor) || !seen.add(cursor.toString())) {
                    break;
                }

                Instruction instruction = currentProgram.getListing().getInstructionAt(cursor);
                if (instruction == null) {
                    break;
                }
                instructions.add(instruction);

                String mnemonic = upperMnemonic(instruction);
                if (mnemonic != null &&
                    (mnemonic.startsWith("RET") ||
                        mnemonic.startsWith("INT") ||
                        "CCH".equals(mnemonic))) {
                    break;
                }

                Address fallThrough = instruction.getFallThrough();
                boolean branch = false;
                boolean unconditionalJump = false;
                if (!instruction.getFlowType().isCall()) {
                    Address[] flows = instruction.getFlows();
                    if (flows != null) {
                        for (Address flow : flows) {
                            if (!isExecutableAddress(flow) || flow.equals(fallThrough)) {
                                continue;
                            }
                            branch = true;
                            if (!seen.contains(flow.toString())) {
                                pending.addFirst(flow);
                            }
                        }
                    }
                    unconditionalJump = mnemonic != null && mnemonic.startsWith("JMP");
                }

                if (fallThrough == null ||
                    !isProgramAddress(fallThrough) ||
                    unconditionalJump ||
                    (branch && fallThrough.equals(cursor))) {
                    break;
                }
                cursor = fallThrough;
            }
        }
        return instructions;
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
        if (isAllocatorTypeName(value)) {
            return false;
        }
        if (canonicalUuidFromString(value) != null) {
            return false;
        }
        if (isUuidLikeFragment(value)) {
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

    private boolean isUuidLikeFragment(String value) {
        if (value == null || value.length() < 32 || value.length() > 38) {
            return false;
        }

        int hyphens = 0;
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '-') {
                hyphens++;
                continue;
            }
            boolean hex =
                (c >= '0' && c <= '9') ||
                    (c >= 'a' && c <= 'f') ||
                    (c >= 'A' && c <= 'F');
            if (!hex) {
                return false;
            }
        }
        return hyphens >= 4;
    }

    private String canonicalUuidFromString(String value) {
        if (value == null) {
            return null;
        }
        Matcher matcher = UUID_RE.matcher(value);
        return matcher.find() ? matcher.group(1).toUpperCase(Locale.ROOT) : null;
    }

    private String canonicalUuidFromExactString(String value) {
        if (value == null) {
            return null;
        }
        Matcher matcher = UUID_RE.matcher(value.trim());
        return matcher.matches() ? matcher.group(1).toUpperCase(Locale.ROOT) : null;
    }

    private boolean isNativeUuidLiteralReference(Instruction instruction, Address target) {
        if (!"LEA".equals(upperMnemonic(instruction)) || target == null) {
            return false;
        }
        Address source = referencedAddress(instruction, 1);
        return target.equals(source);
    }

    private String canonicalUuidFromNativeUuidLiteral(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }

        byte[] uuid = new byte[16];
        try {
            boolean braced = (getByte(address) & 0xff) == '{';
            int start = braced ? 1 : 0;
            int nibble = 0;
            for (int i = 0; i < 36; i++) {
                int value = getByte(address.add(start + (long)i)) & 0xff;
                if (isUuidDashOffset(i)) {
                    if (value != '-') {
                        return null;
                    }
                    continue;
                }
                int parsed = nativeUuidNibble(value);
                if (parsed < 0) {
                    return null;
                }
                int index = nibble / 2;
                if ((nibble & 1) == 0) {
                    uuid[index] = (byte)(parsed << 4);
                }
                else {
                    uuid[index] |= (byte)parsed;
                }
                nibble++;
            }
            if (nibble != 32) {
                return null;
            }
            if (braced) {
                if ((getByte(address.add(37L)) & 0xff) != '}') {
                    return null;
                }
                int terminator = getByte(address.add(38L)) & 0xff;
                if (terminator != 0) {
                    return null;
                }
            }
            else {
                int terminator = getByte(address.add(36L)) & 0xff;
                if (terminator != 0) {
                    return null;
                }
            }
        }
        catch (Exception ignored) {
            return null;
        }

        return uuidToString(uuid).toUpperCase(Locale.ROOT);
    }

    private boolean isUuidDashOffset(int offset) {
        return offset == 8 || offset == 13 || offset == 18 || offset == 23;
    }

    private int nativeUuidNibble(int value) {
        if (value >= '0' && value <= '9') {
            return value - '0';
        }
        if (value >= 'A' && value <= 'F') {
            return value - 'A' + 10;
        }
        if (value >= 'a' && value <= 'f') {
            return value - 'a' + 10;
        }
        return -1;
    }

    private String uuidToString(byte[] uuid) {
        return String.format(
            Locale.ROOT,
            "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
            uuid[0] & 0xff,
            uuid[1] & 0xff,
            uuid[2] & 0xff,
            uuid[3] & 0xff,
            uuid[4] & 0xff,
            uuid[5] & 0xff,
            uuid[6] & 0xff,
            uuid[7] & 0xff,
            uuid[8] & 0xff,
            uuid[9] & 0xff,
            uuid[10] & 0xff,
            uuid[11] & 0xff,
            uuid[12] & 0xff,
            uuid[13] & 0xff,
            uuid[14] & 0xff,
            uuid[15] & 0xff);
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

        JsonObject toJson(
            AzRttiEvidence azRttiEvidence,
            AzRttiEvidence valueAzRttiEvidence,
            HookTypeEvidence hookTypeEvidence,
            TypeNameEvidence typeNameEvidence) {
            JsonObject object = new JsonObject();
            add(object, "uuid", uuid);
            add(object, "name", name);
            add(object, "index", index);
            add(object, "typeIndex", typeIndex);
            add(object, "storageAddress", storageAddress);
            add(object, "baseVtable", baseVtable);
            add(object, "vtable", vtable);

            String registryTypeId = canonicalUuidFromString(uuid);
            String selectedProviderTypeName =
                providerAnyTypeNameForTypeId(azRttiEvidence, registryTypeId);
            String concreteRegistryProviderTypeName =
                providerTypeNameForTypeId(azRttiEvidence, registryTypeId);
            String concreteValueProviderTypeName = valueAzRttiEvidence == null
                ? null
                : providerTypeNameForTypeId(valueAzRttiEvidence, valueAzRttiEvidence.typeId);
            String selectedValueProviderTypeName = valueAzRttiEvidence == null
                ? null
                : providerAnyTypeNameForTypeId(valueAzRttiEvidence, valueAzRttiEvidence.typeId);
            if (isBaseNetworkTypeName(selectedProviderTypeName)) {
                add(object, "identityKind", "base-or-interface");
                add(object, "baseTypeName", selectedProviderTypeName);
            }

            if (hookTypeEvidence != null &&
                isPlausibleTypeName(hookTypeEvidence.typeName)) {
                add(object, "typeName", hookTypeEvidence.typeName);
                add(object, "typeNameSource", hookTypeEvidence.typeNameSource());
            }
            else if (azRttiEvidence != null &&
                isConcreteNetworkTypeName(concreteRegistryProviderTypeName)) {
                add(object, "typeName", concreteRegistryProviderTypeName);
                add(object, "typeNameSource", "azRtti-provider");
            }
            else if (typeNameEvidence != null &&
                isPlausibleTypeName(typeNameEvidence.typeName)) {
                add(object, "typeName", typeNameEvidence.typeName);
                add(object, "typeNameSource", typeNameEvidence.source);
            }
            else if (isPlausibleTypeName(name)) {
                add(object, "typeName", name);
                add(object, "typeNameSource", "typeregistry-entry");
            }
            else if (isConcreteNetworkTypeName(concreteValueProviderTypeName)) {
                add(object, "typeName", concreteValueProviderTypeName);
                add(object, "typeNameSource", "valueAzRtti-provider");
            }
            else if (valueAzRttiEvidence != null &&
                isConcreteNetworkTypeName(valueAzRttiEvidence.typeName)) {
                add(object, "typeName", valueAzRttiEvidence.typeName);
                add(object, "typeNameSource", "valueAzRtti-observed-type-name");
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
            if (valueAzRttiEvidence != null) {
                add(object, "valueSelectedTypeName", selectedValueProviderTypeName);
                if (isPlausibleTypeName(concreteValueProviderTypeName)) {
                    add(object, "valueTypeName", concreteValueProviderTypeName);
                    add(object, "valueTypeNameSource", "valueAzRtti-provider");
                }
                else if (isPlausibleTypeName(valueAzRttiEvidence.typeName)) {
                    add(object, "valueTypeName", valueAzRttiEvidence.typeName);
                    add(object, "valueTypeNameSource", "valueAzRtti-observed-type-name");
                }
                add(object, "valueObservedTypeName", valueAzRttiEvidence.typeName);
                object.add("valueAzRtti", valueAzRttiEvidence.toJson());
            }
            if (hookTypeEvidence != null) {
                add(object, "registrationTypeName", hookTypeEvidence.typeName);
                object.add("registrationHook", hookTypeEvidence.toJson());
            }
            if (typeNameEvidence != null) {
                object.add("recoveredTypeName", typeNameEvidence.toJson());
            }
            JsonObject foldEvidence = foldEvidenceForCandidates(
                registryTypeId,
                "registry",
                hookTypeEvidence == null ? null : hookTypeEvidence.typeName,
                concreteRegistryProviderTypeName,
                typeNameEvidence == null ? null : typeNameEvidence.typeName,
                name,
                concreteValueProviderTypeName,
                valueAzRttiEvidence == null ? null : valueAzRttiEvidence.typeName);
            if (foldEvidence != null) {
                object.add("foldEvidence", foldEvidence);
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
                FragmentMetadataEvidence fragmentMetadata =
                    fragmentMetadataEvidence(instanceVtable);
                object.add("instanceVtableDiagnostics", vtableDiagnostics(
                    instanceVtable,
                    null,
                    I_FRAGMENT_VTABLE_SCAN_SLOTS,
                    fragmentMetadata));
                if (fragmentMetadata != null) {
                    object.add("fragmentMetadata", fragmentMetadata.toJson());
                }
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

    private final class FragmentMetadataEvidence {
        Address isMetadataFunction;
        Boolean isMetadata;
        Address categoryFunction;
        Integer categoryValue;
        String category;

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("source", "i-fragment-vtable");
            object.addProperty("isMetadataSlot", I_FRAGMENT_IS_METADATA_SLOT);
            add(object, "isMetadataFunction", formatAddress(isMetadataFunction));
            if (isMetadata != null) {
                object.addProperty("isMetadata", isMetadata);
            }
            object.addProperty("categorySlot", I_FRAGMENT_GET_CATEGORY_SLOT);
            add(object, "categoryFunction", formatAddress(categoryFunction));
            add(object, "categoryValue", categoryValue);
            add(object, "category", category);
            return object;
        }
    }

    private final class NestedTypeShape {
        String typeId;
        String typeIdSource;
        String typeName;
        String typeNameFull;
        String typeNameSource;
        Address function;
        String functionName;
        String factory;
        String azRttiAddress;
        Address constructor;
        Address vtable;
        String memberBase;
        String memberNameSource;
        Boolean memberNamesProven;
        String datatypePath;
        String validation;
        final ArrayList<NestedTypeMember> members = new ArrayList<>();

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "typeId", typeId);
            add(object, "typeIdSource", typeIdSource);
            add(object, "typeName", typeName);
            add(object, "typeNameFull", typeNameFull);
            add(object, "typeNameSource", typeNameSource);
            add(object, "function", formatAddress(function));
            add(object, "functionName", functionName);
            add(object, "factory", factory);
            add(object, "azRttiAddress", azRttiAddress);
            add(object, "constructor", formatAddress(constructor));
            add(object, "vtable", formatAddress(vtable));
            add(object, "memberBase", memberBase);
            add(object, "memberNameSource", memberNameSource);
            if (memberNamesProven != null) {
                object.addProperty("memberNamesProven", memberNamesProven);
            }
            add(object, "datatypePath", datatypePath);
            add(object, "validation", validation);
            JsonArray memberJson = new JsonArray();
            for (NestedTypeMember member : members) {
                memberJson.add(member.toJson());
            }
            object.add("members", memberJson);
            return object;
        }
    }

    private final class SerializeTypeInfo {
        String typeId;
        String name;
        String factory;
        String azRttiAddress;
    }

    private final class NestedTypeMember {
        int index;
        long offset;
        Long nativeOffset;
        String name;
        String nameSource;
        Boolean nameProven;
        String nameEvidence;
        String nativeType;
        String wireShape;
        Integer byteWidth;
        String evidenceSource;
        Address callsite;
        Address target;
        String targetName;
        Boolean typeConflict;

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            object.addProperty("index", index);
            object.addProperty("offset", "0x" + Long.toHexString(offset));
            if (nativeOffset != null) {
                object.addProperty("nativeOffset", "0x" + Long.toHexString(nativeOffset));
            }
            add(object, "name", name);
            add(object, "nameSource", nameSource);
            if (nameProven != null) {
                object.addProperty("nameProven", nameProven);
            }
            add(object, "nameEvidence", nameEvidence);
            add(object, "nativeType", nativeType);
            add(object, "wireShape", wireShape);
            if (byteWidth != null) {
                object.addProperty("byteWidth", byteWidth);
            }
            add(object, "evidenceSource", evidenceSource);
            add(object, "callsite", formatAddress(callsite));
            add(object, "target", formatAddress(target));
            add(object, "targetName", targetName);
            if (typeConflict != null) {
                object.addProperty("typeConflict", typeConflict);
            }
            return object;
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
        HandlerConstruction handlerConstruction;
        List<HandlerConstructorWrite> handlerConstructorWrites;
        String handlerKind;
        Integer handlerVtableSlots;
        String handlerTypeName;
        String registrationKind;
        Boolean filterGroupAttribute;
        Integer groupCursorOffset;
        String nativeType;
        String sourceTypeName;
        String storageExpression;
        String storageIdentity;
        Long storageOffset;
        String storageBase;
        Long storageBaseOffset;
        Integer rawByteLength;
        String wireShape;
        String wireShapeSource;
        List<HandlerConstructorWrite> constructorWrites;
        String confidence;
        Address unmarshalCallsite;
        Address unmarshalTargetRaw;
        Address unmarshalTarget;
        Address valueCallTarget;
        String unmarshalTargetName;
        String unmarshalTargetKind;
        Boolean unmarshalTargetExactStart;
        Address unmarshalTargetContaining;
        String unmarshalTargetContainingName;
        Integer storageArgSlot;
        String evidenceSource;
        JsonArray argStorageEvidence;
        JsonArray typeEvidence;
        NestedTypeShape nestedTypeShape;
        JsonArray mergedCallsites;
        Boolean multipleCallEvidence;
        Boolean typeConflict;

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
            add(object, "handlerKind", handlerKind);
            add(object, "handlerVtableSlots", handlerVtableSlots);
            add(object, "handlerTypeName", handlerTypeName);
            JsonObject handlerFoldEvidence =
                foldEvidenceForTypeName(handlerTypeName, null, "computed");
            if (handlerFoldEvidence != null) {
                object.add("handlerFoldEvidence", handlerFoldEvidence);
            }
            add(object, "registrationKind", registrationKind);
            if (filterGroupAttribute != null) {
                object.addProperty("filterGroupAttribute", filterGroupAttribute);
            }
            if (groupCursorOffset != null) {
                object.addProperty("groupCursorOffset", "0x" + Integer.toHexString(groupCursorOffset));
            }
            if (handlerConstruction != null) {
                object.add("handlerConstruction", handlerConstruction.toJson());
            }
            add(object, "nativeType", nativeType);
            add(object, "sourceTypeName", sourceTypeName);
            add(object, "sourceTypeId", fieldSourceTypeId());
            if (typeEvidence != null && typeEvidence.size() != 0) {
                object.add("typeEvidence", typeEvidence);
            }
            if (nestedTypeShape != null) {
                object.add("nestedTypeShape", nestedTypeShape.toJson());
            }
            JsonObject sourceTypeFoldEvidence =
                foldEvidenceForCandidates(null, "computed", sourceTypeName, nativeType);
            if (sourceTypeFoldEvidence != null) {
                object.add("sourceTypeFoldEvidence", sourceTypeFoldEvidence);
            }
            add(object, "storageExpression", storageExpression);
            add(object, "storageIdentity", storageIdentity);
            if (storageOffset != null) {
                object.addProperty("storageOffset", "0x" + Long.toHexString(storageOffset));
            }
            add(object, "storageBase", storageBase);
            if (storageBaseOffset != null) {
                object.addProperty(
                    "storageBaseOffset",
                    "0x" + Long.toHexString(storageBaseOffset));
            }
            if (rawByteLength != null) {
                object.addProperty("rawByteLength", rawByteLength);
            }
            add(object, "wireShape", wireShape);
            add(object, "wireShapeSource", wireShapeSource);
            if (constructorWrites != null && !constructorWrites.isEmpty()) {
                JsonArray writes = new JsonArray();
                for (HandlerConstructorWrite write : constructorWrites) {
                    writes.add(write.toJson());
                }
                object.add("constructorWrites", writes);
            }
            add(object, "confidence", confidence);
            JsonObject unmarshalEvidence = unmarshalEvidenceJson();
            if (unmarshalEvidence.size() != 0) {
                object.add("unmarshalEvidence", unmarshalEvidence);
            }
            return object;
        }

        private String fieldSourceTypeId() {
            if (nestedTypeShape != null && nestedTypeShape.typeId != null) {
                return nestedTypeShape.typeId;
            }
            String typeId = sourceTypeIdFromName(sourceTypeName);
            if (typeId != null) {
                return typeId;
            }
            return sourceTypeIdFromName(nativeType);
        }

        private String sourceTypeIdFromName(String typeName) {
            if (isWireNativeSourceTypeName(typeName)) {
                return null;
            }
            String typeId = typeIdForTypeName(typeName);
            if (typeId == null || isBuiltinTypeId(typeId)) {
                return null;
            }
            return typeId;
        }

        boolean typeConflict() {
            return Boolean.TRUE.equals(typeConflict);
        }

        boolean multipleCallEvidence() {
            return Boolean.TRUE.equals(multipleCallEvidence);
        }

        JsonObject unmarshalEvidenceJson() {
            JsonObject object = new JsonObject();
            add(object, "callsite", formatAddress(unmarshalCallsite));
            add(object, "targetRaw", formatAddress(unmarshalTargetRaw));
            add(object, "target", formatAddress(unmarshalTarget));
            add(object, "targetName", unmarshalTargetName);
            add(object, "valueCallTarget", formatAddress(valueCallTarget));
            add(object, "targetKind", unmarshalTargetKind);
            if (unmarshalTargetExactStart != null) {
                object.addProperty("targetExactStart", unmarshalTargetExactStart);
            }
            add(object, "containingTarget", formatAddress(unmarshalTargetContaining));
            add(object, "containingTargetName", unmarshalTargetContainingName);
            add(object, "storageBase", storageBase);
            if (storageBaseOffset != null) {
                object.addProperty(
                    "storageBaseOffset",
                    "0x" + Long.toHexString(storageBaseOffset));
            }
            if (storageArgSlot != null) {
                object.addProperty("storageArgSlot", storageArgSlot);
            }
            add(object, "evidenceSource", evidenceSource);
            if (argStorageEvidence != null && argStorageEvidence.size() != 0) {
                object.add("argStorage", argStorageEvidence);
            }
            if (mergedCallsites != null && mergedCallsites.size() != 0) {
                object.add("mergedCallsites", mergedCallsites);
            }
            if (typeConflict != null) {
                object.addProperty("typeConflict", typeConflict);
            }
            if (multipleCallEvidence != null) {
                object.addProperty("multipleCallEvidence", multipleCallEvidence);
            }
            return object;
        }
    }

    private final class HandlerConstructorWrite {
        final Address write;
        final int handlerOffset;
        final int relativeOffset;
        final Integer widthBits;
        final Integer byteLength;
        final String valueKind;
        final String value;
        final String valueHex;
        final String sourceOperand;
        final String source;

        HandlerConstructorWrite(
            Address write,
            int handlerOffset,
            int relativeOffset,
            Integer widthBits,
            Integer byteLength,
            String valueKind,
            String value,
            String valueHex,
            String sourceOperand,
            String source) {

            this.write = write;
            this.handlerOffset = handlerOffset;
            this.relativeOffset = relativeOffset;
            this.widthBits = widthBits;
            this.byteLength = byteLength;
            this.valueKind = valueKind;
            this.value = value;
            this.valueHex = valueHex;
            this.sourceOperand = sourceOperand;
            this.source = source;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "write", formatAddress(write));
            object.addProperty("handlerOffset", "0x" + Integer.toHexString(handlerOffset));
            object.addProperty("relativeOffset", "0x" + Integer.toHexString(relativeOffset));
            if (widthBits != null) {
                object.addProperty("widthBits", widthBits);
            }
            if (byteLength != null) {
                object.addProperty("byteLength", byteLength);
            }
            add(object, "valueKind", valueKind);
            add(object, "value", value);
            add(object, "valueHex", valueHex);
            add(object, "sourceOperand", sourceOperand);
            add(object, "source", source);
            return object;
        }
    }

    private final class HandlerConstruction {
        final String pattern;
        final Address callsite;
        final Address constructor;
        final String constructorName;
        final Address vtable;

        HandlerConstruction(
            String pattern,
            Address callsite,
            Address constructor,
            String constructorName,
            Address vtable) {

            this.pattern = pattern;
            this.callsite = callsite;
            this.constructor = constructor;
            this.constructorName = constructorName;
            this.vtable = vtable;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "pattern", pattern);
            add(object, "callsite", formatAddress(callsite));
            add(object, "constructor", formatAddress(constructor));
            add(object, "constructorName", constructorName);
            add(object, "vtable", formatAddress(vtable));
            return object;
        }
    }

    private final class DirectFieldAppend {
        final int cursorOffset;
        Address nameAddress;
        String name;
        Address nameWrite;
        Integer handlerOffset;
        String handlerExpression;
        Address handlerWrite;
        Boolean filterGroupAttribute;

        DirectFieldAppend(int cursorOffset) {
            this.cursorOffset = cursorOffset;
        }

        FieldCall toFieldCall(ForwardArgState state) {
            if (name == null || handlerOffset == null) {
                return null;
            }

            FieldCall field = new FieldCall();
            field.callsite = handlerWrite == null ? nameWrite : handlerWrite;
            field.nameAddress = nameAddress;
            field.name = name;
            field.nameSource = "fixed-field-table-append";
            field.nameSourceAddress = nameWrite;
            field.handlerOffset = handlerOffset;
            field.handlerExpression = handlerExpression;
            field.registrationKind = directRegistrationKind();
            field.filterGroupAttribute = filterGroupAttribute;
            if ("attribute".equals(field.registrationKind)) {
                field.nameSource = "fixed-attribute-table-append";
            }
            field.groupCursorOffset = cursorOffset;
            field.handlerVtable = state.vtablesByThisOffset.get(handlerOffset);
            field.handlerConstruction = state.handlerConstructionsByThisOffset.get(handlerOffset);
            field.constructorWrites = state.constructorWritesByHandlerOffset.get(handlerOffset);
            FieldHandlerShape shape = fieldHandlerShape(field.handlerVtable);
            if (shape != null) {
                field.handlerKind = shape.kind;
                field.handlerVtableSlots = shape.vtableSlots;
                if (shape.wireShape != null) {
                    field.wireShape = shape.wireShape.shape;
                    field.wireShapeSource = shape.wireShape.source;
                }
            }
            field.handlerTypeName =
                fieldHandlerTypeName(field.handlerConstruction, field.handlerVtable);
            enrichFieldFromHandlerType(field);
            String confidencePrefix = "attribute".equals(field.registrationKind)
                ? "fixed-attribute-table-append"
                : "fixed-field-table-append";
            field.confidence = field.handlerVtable == null
                ? confidencePrefix + "-unresolved-handler-vtable"
                : confidencePrefix;
            return field;
        }

        String directRegistrationKind() {
            return cursorOffset == REPLICATED_STATE_ATTRIBUTE_VECTOR_OFFSET ||
                "ClientWhitelist".equals(name)
                ? "attribute"
                : "field";
        }
    }

    private static final class FixedNamedFieldValue {
        Address nameAddress;
        String name;
        Address nameWrite;
        Integer handlerOffset;
        String handlerExpression;
    }

    private static final class FieldHandlerShape {
        final String kind;
        final int vtableSlots;
        final WireShape wireShape;
        final ContainerWireShape containerWireShape;

        FieldHandlerShape(
            String kind,
            int vtableSlots,
            WireShape wireShape,
            ContainerWireShape containerWireShape) {

            this.kind = kind;
            this.vtableSlots = vtableSlots;
            this.wireShape = wireShape;
            this.containerWireShape = containerWireShape;
        }
    }

    private static final class MemoryReference {
        final String baseRegister;
        final int displacement;

        MemoryReference(String baseRegister, int displacement) {
            this.baseRegister = baseRegister;
            this.displacement = displacement;
        }
    }

    private static final class MemoryAddress {
        final List<MemoryTerm> terms;
        final int displacement;

        MemoryAddress(List<MemoryTerm> terms, int displacement) {
            this.terms = Collections.unmodifiableList(new ArrayList<>(terms));
            this.displacement = displacement;
        }
    }

    private static final class MemoryTerm {
        final String register;
        final int scale;

        MemoryTerm(String register, int scale) {
            this.register = register;
            this.scale = scale;
        }
    }

    private static final class VectorSlotAlias {
        final int ownerOffset;
        final int slotOffset;

        VectorSlotAlias(int ownerOffset, int slotOffset) {
            this.ownerOffset = ownerOffset;
            this.slotOffset = slotOffset;
        }
    }

    private static final class NetworkTemplateType {
        final String qualifiedName;
        final String ownerName;
        final String simpleName;
        final List<String> args;

        NetworkTemplateType(
            String qualifiedName,
            String ownerName,
            String simpleName,
            List<String> args) {

            this.qualifiedName = qualifiedName;
            this.ownerName = ownerName;
            this.simpleName = simpleName;
            this.args = Collections.unmodifiableList(new ArrayList<>(args));
        }
    }

    private static final class GenericType {
        final String qualifiedName;
        final String ownerName;
        final String simpleName;
        final List<String> args;

        GenericType(
            String qualifiedName,
            String ownerName,
            String simpleName,
            List<String> args) {

            this.qualifiedName = qualifiedName;
            this.ownerName = ownerName;
            this.simpleName = simpleName;
            this.args = Collections.unmodifiableList(new ArrayList<>(args));
        }
    }

    private static final class TypeIdOperands {
        final List<String> typeNames;
        final List<String> typeIds;

        TypeIdOperands(List<String> typeNames, List<String> typeIds) {
            this.typeNames = Collections.unmodifiableList(new ArrayList<>(typeNames));
            this.typeIds = Collections.unmodifiableList(new ArrayList<>(typeIds));
        }
    }

    private static final class FoldedTypeId {
        final String sourceTypeName;
        final String formula;
        final String typeId;
        final List<String> operandTypeNames;
        final List<String> operandTypeIds;

        FoldedTypeId(
            String sourceTypeName,
            String formula,
            String typeId,
            List<String> operandTypeNames,
            List<String> operandTypeIds) {

            this.sourceTypeName = sourceTypeName;
            this.formula = formula;
            this.typeId = typeId;
            this.operandTypeNames = Collections.unmodifiableList(new ArrayList<>(operandTypeNames));
            this.operandTypeIds = Collections.unmodifiableList(new ArrayList<>(operandTypeIds));
        }
    }

    private static final class PcodeStorage {
        final String base;
        final long offset;

        PcodeStorage(String base, long offset) {
            this.base = base;
            this.offset = offset;
        }

        PcodeStorage plus(long delta) {
            return new PcodeStorage(base, offset + delta);
        }

        boolean sameLocation(PcodeStorage other) {
            return other != null && base.equals(other.base) && offset == other.offset;
        }

        String expression() {
            if (offset < 0) {
                return base + " - 0x" + Long.toHexString(-offset);
            }
            return base + " + 0x" + Long.toHexString(offset);
        }
    }

    private final class PcodeArgStorageSelection {
        PcodeStorage storage;
        Integer storageArgSlot;
        PcodeStorage fallbackStorage;
        Integer fallbackStorageArgSlot;
        String selectionRule;
        final JsonArray argStorageEvidence = new JsonArray();
    }

    private final class PcodeCallTargetInfo {
        Address rawTarget;
        Address resolvedTarget;
        Function target;
        boolean targetExactStart;
        Function containing;

        Address targetAddress() {
            return target == null ? resolvedTarget : target.getEntryPoint();
        }
    }

    private final class PcodeUnmarshalEvidence {
        Address callsite;
        Address targetRawAddress;
        Address target;
        Address valueCallTarget;
        String targetName;
        String targetKind;
        Boolean targetExactStart;
        Address containingTarget;
        String containingTargetName;
        PcodeStorage storage;
        Integer storageArgSlot;
        String evidenceSource;
        JsonArray argStorageEvidence;
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
        final JsonArray rejectedPcodeFields = new JsonArray();

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
            if (rejectedPcodeFields.size() != 0) {
                JsonObject diagnostics = new JsonObject();
                diagnostics.add("rejectedPcodeFields", rejectedPcodeFields);
                object.add("recoveryDiagnostics", diagnostics);
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
        final String functionName;
        final int textIndex;
        final List<String> args;

        ParsedUnmarshalCall(
            String templateType,
            String functionName,
            int textIndex,
            List<String> args) {

            this.templateType = templateType;
            this.functionName = functionName;
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

    private static final class WholeMessageStore {
        String storageExpression;
        String nativeType;
        int textIndex;
    }

    private static final class WholeMessageHelperFrame {
        final Address callsite;
        final Function helper;
        final String helperText;
        final Map<String, String> baseExpressions;
        final int recoveryOrder;

        WholeMessageHelperFrame(
            Address callsite,
            Function helper,
            String helperText,
            Map<String, String> baseExpressions,
            int recoveryOrder) {

            this.callsite = callsite;
            this.helper = helper;
            this.helperText = helperText;
            this.baseExpressions =
                Collections.unmodifiableMap(new LinkedHashMap<>(baseExpressions));
            this.recoveryOrder = recoveryOrder;
        }
    }

    private static final class MarshalPathFrame {
        final Address address;
        final String sourcePrefix;

        MarshalPathFrame(Address address, String sourcePrefix) {
            this.address = address;
            this.sourcePrefix = sourcePrefix == null ? "" : sourcePrefix;
        }

        MarshalPathFrame nested(Address address) {
            return new MarshalPathFrame(address, sourcePrefix + "marshal-call:");
        }

        WireShape wrap(WireShape shape) {
            if (shape == null || sourcePrefix.isEmpty()) {
                return shape;
            }
            return new WireShape(shape.shape, sourcePrefix + shape.source);
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

    private static final class ContainerWireShape {
        final WireShape primaryShape;
        final String deltaShape;
        final String fullShape;
        final List<String> deltaMarshalShapes;
        final List<String> fullMarshalShapes;

        ContainerWireShape(
            WireShape primaryShape,
            String deltaShape,
            String fullShape,
            List<String> deltaMarshalShapes,
            List<String> fullMarshalShapes) {

            this.primaryShape = primaryShape;
            this.deltaShape = deltaShape;
            this.fullShape = fullShape;
            this.deltaMarshalShapes = List.copyOf(deltaMarshalShapes);
            this.fullMarshalShapes = List.copyOf(fullMarshalShapes);
        }
    }

    private final class AzRttiScanResult {
        AzRttiEvidence exact;
        int exactScore = Integer.MIN_VALUE;
        AzRttiEvidence nameOnly;
        int nameOnlyScore = Integer.MIN_VALUE;
    }

    private static final class HandlerScanFrame {
        final Address address;
        final int depth;

        HandlerScanFrame(Address address, int depth) {
            this.address = address;
            this.depth = depth;
        }
    }

    private static final class TypeNameCandidate {
        final String typeName;
        final String source;
        final Address address;
        final int score;

        TypeNameCandidate(String typeName, String source, Address address, int score) {
            this.typeName = typeName;
            this.source = source;
            this.address = address;
            this.score = score;
        }
    }

    private static final class VtableWrite {
        final Address function;
        final Address instruction;
        final Address vtable;
        final int order;
        final Integer thisOffset;
        final String baseKey;
        final Integer baseOffset;
        final String pattern;

        VtableWrite(Address function, Address instruction, Address vtable, int order) {
            this(function, instruction, vtable, order, null, null, null, null);
        }

        VtableWrite(
            Address function,
            Address instruction,
            Address vtable,
            int order,
            Integer thisOffset,
            String pattern) {
            this(function, instruction, vtable, order, thisOffset, null, null, pattern);
        }

        VtableWrite(
            Address function,
            Address instruction,
            Address vtable,
            int order,
            Integer thisOffset,
            String baseKey,
            Integer baseOffset,
            String pattern) {

            this.function = function;
            this.instruction = instruction;
            this.vtable = vtable;
            this.order = order;
            this.thisOffset = thisOffset;
            this.baseKey = baseKey;
            this.baseOffset = baseOffset;
            this.pattern = pattern;
        }
    }

    private final class AzRttiEvidence {
        String source;
        String address;
        Address sourceInstruction;
        String typeId;
        String typeName;
        String typeNameSource;
        JsonArray constructorVptrWrites;
        final JsonArray providers = new JsonArray();

        boolean hasIdentity() {
            return typeId != null || typeName != null;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "source", source);
            add(object, "address", address);
            add(object, "sourceInstruction", formatAddress(sourceInstruction));
            add(object, "typeId", typeId);
            add(object, "typeName", typeName);
            add(object, "typeNameSource", typeNameSource);
            if (constructorVptrWrites != null && constructorVptrWrites.size() != 0) {
                object.add("constructorVptrWrites", constructorVptrWrites);
            }
            JsonArray providerTypeIds = providerValues("typeId");
            if (providerTypeIds.size() != 0) {
                object.add("providerTypeIds", providerTypeIds);
            }
            JsonArray providerTypeNames = providerValues("typeName");
            if (providerTypeNames.size() != 0) {
                object.add("providerTypeNames", providerTypeNames);
            }
            JsonObject graph = rttiGraph();
            if (graph != null) {
                object.add("rttiGraph", graph);
            }
            if (providers.size() != 0) {
                object.add("providers", providers);
            }
            return object;
        }

        private JsonArray providerValues(String property) {
            JsonArray array = new JsonArray();
            LinkedHashSet<String> seen = new LinkedHashSet<>();
            for (JsonElement element : providers) {
                if (!element.isJsonObject()) {
                    continue;
                }
                String value = string(element.getAsJsonObject(), property);
                if (value != null && seen.add(value)) {
                    array.add(value);
                }
            }
            return array;
        }

        private JsonObject rttiGraph() {
            LinkedHashMap<String, JsonObject> nodes = new LinkedHashMap<>();

            for (JsonElement element : providers) {
                if (!element.isJsonObject()) {
                    continue;
                }
                JsonObject provider = element.getAsJsonObject();
                String providerTypeId = string(provider, "typeId");
                if (providerTypeId != null) {
                    JsonObject node = providerGraphNode(nodes, providerTypeIdKey(providerTypeId));
                    add(node, "typeId", providerTypeId);
                    addProviderSlot(node, provider);
                }
            }

            for (JsonElement element : providers) {
                if (!element.isJsonObject()) {
                    continue;
                }
                JsonObject provider = element.getAsJsonObject();
                String providerTypeName = string(provider, "typeName");
                if (providerTypeName == null) {
                    continue;
                }

                String providerTypeId = pairedProviderTypeId(provider);
                JsonObject node = providerTypeId == null
                    ? providerGraphNode(nodes, "typeName:" + providerTypeName)
                    : providerGraphNode(nodes, providerTypeIdKey(providerTypeId));
                if (providerTypeId != null) {
                    add(node, "typeId", providerTypeId);
                }
                addProviderName(node, providerTypeName);
                addProviderSlot(node, provider);
                JsonObject foldEvidence =
                    foldEvidenceForTypeName(providerTypeName, providerTypeId, "provider");
                if (foldEvidence != null && !node.has("foldEvidence")) {
                    node.add("foldEvidence", foldEvidence);
                }
            }

            if (nodes.isEmpty()) {
                return null;
            }

            ArrayList<Map.Entry<String, JsonObject>> orderedNodes =
                orderedGraphNodes(nodes);
            JsonArray nodeJson = new JsonArray();
            String selectedKey = providerTypeIdKey(typeId);
            int order = 0;
            JsonArray providerOrder = new JsonArray();
            for (Map.Entry<String, JsonObject> entry : orderedNodes) {
                JsonObject node = entry.getValue();
                node.addProperty("key", entry.getKey());
                node.addProperty("order", order++);
                Integer firstSlot = firstProviderSlot(node);
                if (firstSlot != null) {
                    node.addProperty("firstSlot", firstSlot);
                }
                String role = entry.getKey().equals(selectedKey)
                    ? "selected"
                    : providerGraphNodeRole(node);
                node.addProperty("role", role);
                nodeJson.add(node);
                providerOrder.add(entry.getKey());
            }

            JsonArray edges = new JsonArray();
            if (selectedKey != null && nodes.containsKey(selectedKey)) {
                int edgeOrder = 0;
                for (Map.Entry<String, JsonObject> entry : orderedNodes) {
                    if (entry.getKey().equals(selectedKey)) {
                        continue;
                    }
                    JsonObject edge = new JsonObject();
                    edge.addProperty("order", edgeOrder++);
                    edge.addProperty("from", selectedKey);
                    edge.addProperty("to", entry.getKey());
                    edge.addProperty("kind", "same-vtable-provider");
                    edge.addProperty("targetRole", providerGraphNodeRole(entry.getValue()));
                    edges.add(edge);
                }
            }

            JsonObject graph = new JsonObject();
            add(graph, "selectedTypeId", typeId);
            add(graph, "selectedTypeName", selectedProviderTypeName());
            add(graph, "observedTypeName", typeName);
            graph.add("providerOrder", providerOrder);
            graph.add("nodes", nodeJson);
            if (edges.size() != 0) {
                graph.add("edges", edges);
            }
            return graph;
        }

        private String pairedProviderTypeId(JsonObject typeNameProvider) {
            Integer slot = integer(typeNameProvider, "slot");
            if (slot == null) {
                return null;
            }

            String sameSlot = providerTypeIdAtSlot(slot);
            if (sameSlot != null) {
                return sameSlot;
            }
            if (slot == 1) {
                String actualTypeId = providerTypeIdAtSlot(2);
                return actualTypeId == null ? providerTypeIdAtSlot(0) : actualTypeId;
            }
            return null;
        }

        private String selectedProviderTypeName() {
            if (typeId == null) {
                return null;
            }

            LinkedHashSet<Integer> matchingSlots = new LinkedHashSet<>();
            for (JsonElement element : providers) {
                if (!element.isJsonObject()) {
                    continue;
                }
                JsonObject provider = element.getAsJsonObject();
                if (uuidEquals(typeId, string(provider, "typeId"))) {
                    Integer slot = integer(provider, "slot");
                    if (slot != null) {
                        matchingSlots.add(slot);
                    }
                }
            }

            for (Integer slot : matchingSlots) {
                String sameSlotName = providerTypeNameAtSlot(AzRttiEvidence.this, slot);
                if (isLikelyRuntimeTypeName(sameSlotName)) {
                    return sameSlotName;
                }
            }

            if (matchingSlots.contains(2)) {
                String nextSlotName = providerTypeNameAtSlot(AzRttiEvidence.this, 1);
                if (isLikelyRuntimeTypeName(nextSlotName)) {
                    return nextSlotName;
                }
            }

            if (matchingSlots.contains(0)) {
                String nextSlotName = providerTypeNameAtSlot(AzRttiEvidence.this, 1);
                if (isLikelyRuntimeTypeName(nextSlotName)) {
                    return nextSlotName;
                }
            }
            return null;
        }

        private String providerTypeIdAtSlot(int slot) {
            return providerTypeIdAtSlot(AzRttiEvidence.this, slot);
        }

        private String providerTypeIdAtSlot(AzRttiEvidence evidence, int slot) {
            if (evidence == null) {
                return null;
            }
            for (JsonElement element : evidence.providers) {
                if (!element.isJsonObject()) {
                    continue;
                }
                JsonObject provider = element.getAsJsonObject();
                Integer providerSlot = integer(provider, "slot");
                if (providerSlot != null && providerSlot == slot) {
                    String providerTypeId = string(provider, "typeId");
                    if (providerTypeId != null) {
                        return providerTypeId;
                    }
                }
            }
            return null;
        }

        private String providerTypeIdKey(String providerTypeId) {
            if (providerTypeId == null) {
                return null;
            }
            String normalized = normalizeUuid(providerTypeId);
            return normalized == null ? "typeId:" + providerTypeId : "typeId:" + normalized;
        }

        private ArrayList<Map.Entry<String, JsonObject>> orderedGraphNodes(
            LinkedHashMap<String, JsonObject> nodes) {

            ArrayList<Map.Entry<String, JsonObject>> entries =
                new ArrayList<>(nodes.entrySet());
            entries.sort((left, right) -> {
                Integer leftSlot = firstProviderSlot(left.getValue());
                Integer rightSlot = firstProviderSlot(right.getValue());
                if (leftSlot == null && rightSlot == null) {
                    return 0;
                }
                if (leftSlot == null) {
                    return 1;
                }
                if (rightSlot == null) {
                    return -1;
                }
                return Integer.compare(leftSlot, rightSlot);
            });
            return entries;
        }

        private Integer firstProviderSlot(JsonObject node) {
            JsonArray slots = array(node, "slots");
            if (slots == null || slots.size() == 0) {
                return null;
            }

            Integer result = null;
            for (JsonElement element : slots) {
                Integer slot = intStringValue(element);
                if (slot != null && (result == null || slot < result)) {
                    result = slot;
                }
            }
            return result;
        }

        private Integer intStringValue(JsonElement element) {
            String value = stringValue(element);
            if (value == null) {
                return null;
            }
            try {
                return Integer.parseInt(value);
            }
            catch (NumberFormatException ignored) {
                return null;
            }
        }

        private JsonObject providerGraphNode(
            Map<String, JsonObject> nodes,
            String key) {

            JsonObject node = nodes.get(key);
            if (node == null) {
                node = new JsonObject();
                node.add("names", new JsonArray());
                node.add("slots", new JsonArray());
                nodes.put(key, node);
            }
            return node;
        }

        private void addProviderName(JsonObject node, String name) {
            if (name == null) {
                return;
            }
            JsonArray names = array(node, "names");
            if (!jsonArrayContains(names, name)) {
                names.add(name);
            }
        }

        private void addProviderSlot(JsonObject node, JsonObject provider) {
            Integer slot = integer(provider, "slot");
            if (slot == null) {
                return;
            }
            JsonArray slots = array(node, "slots");
            String value = Integer.toString(slot);
            if (!jsonArrayContains(slots, value)) {
                slots.add(value);
            }
        }

        private boolean jsonArrayContains(JsonArray array, String value) {
            if (array == null || value == null) {
                return false;
            }
            for (JsonElement element : array) {
                if (value.equals(stringValue(element))) {
                    return true;
                }
            }
            return false;
        }

        private String providerGraphNodeRole(JsonObject node) {
            JsonArray names = array(node, "names");
            if (names != null) {
                for (JsonElement element : names) {
                    if (isBaseNetworkTypeName(stringValue(element))) {
                        return "base-or-interface";
                    }
                }
            }
            return "provider";
        }
    }

    private final class TypeNameEvidence {
        String source;
        Address function;
        Address typeNameAddress;
        String typeName;

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "source", source);
            add(object, "function", formatAddress(function));
            add(object, "typeName", typeName);
            add(object, "typeNameAddress", formatAddress(typeNameAddress));
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
        String source;

        String typeNameSource() {
            return source == null ? "registrationHook" : source;
        }

        JsonObject toJson() {
            JsonObject object = new JsonObject();
            add(object, "source", source == null ? "install-registration-hook" : source);
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
            object.addProperty("source", source == null ? "install-registration-hook" : source);
            return object;
        }
    }

    private final class TypeIdDecode {
        Address function;
        Address provider;
        Address sourceAddress;
        String typeId;
        String typeIdSource;
        final JsonArray typeIdChain = new JsonArray();

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
            if (typeIdChain.size() != 0) {
                object.add("typeIdChain", typeIdChain);
            }
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
            if (slot >= 0) {
                object.addProperty("slot", slot);
                object.addProperty("slotOffset", "0x" + Integer.toHexString(slot * 8));
            }
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
        HandlerConstruction handlerConstruction;
        List<HandlerConstructorWrite> handlerConstructorWrites;

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
                handlerConstruction = fallback.handlerConstruction;
                handlerConstructorWrites = fallback.handlerConstructorWrites;
            }
            else if (handlerVtable == null) {
                handlerVtable = fallback.handlerVtable;
            }
            if (handlerConstruction == null) {
                handlerConstruction = fallback.handlerConstruction;
            }
            if (handlerConstructorWrites == null) {
                handlerConstructorWrites = fallback.handlerConstructorWrites;
            }
        }
    }

    private static final class ForwardArgState {
        final Map<String, TrackedValue> registers = new HashMap<>();
        final Map<Integer, Address> vtablesByThisOffset = new HashMap<>();
        final Map<Integer, HandlerConstruction> handlerConstructionsByThisOffset =
            new HashMap<>();
        final Map<Integer, List<HandlerConstructorWrite>> constructorWritesByHandlerOffset =
            new HashMap<>();
        final Map<String, Address> vtablesByBaseOffset = new HashMap<>();
        final Map<String, HandlerConstruction> handlerConstructionsByBaseOffset =
            new HashMap<>();
        final Map<String, List<HandlerConstructorWrite>> constructorWritesByBaseOffset =
            new HashMap<>();
        final Set<String> allocatorDispatchRegisters = new LinkedHashSet<>();
        final Map<Integer, TrackedValue> valuesByThisOffset = new HashMap<>();
        final Map<Integer, TrackedValue> valuesByStackSlot = new HashMap<>();
        int nextFilterGroupIndex = 1;

        ForwardArgState copy() {
            ForwardArgState state = new ForwardArgState();
            for (Map.Entry<String, TrackedValue> entry : registers.entrySet()) {
                state.registers.put(entry.getKey(), entry.getValue().copy());
            }
            state.allocatorDispatchRegisters.addAll(allocatorDispatchRegisters);
            state.copyObjectEvidenceFrom(this);
            for (Map.Entry<Integer, TrackedValue> entry : valuesByStackSlot.entrySet()) {
                state.valuesByStackSlot.put(entry.getKey(), entry.getValue().copy());
            }
            state.nextFilterGroupIndex = nextFilterGroupIndex;
            return state;
        }

        void copyObjectEvidenceFrom(ForwardArgState other) {
            vtablesByThisOffset.putAll(other.vtablesByThisOffset);
            handlerConstructionsByThisOffset.putAll(other.handlerConstructionsByThisOffset);
            for (Map.Entry<Integer, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByHandlerOffset.entrySet()) {
                constructorWritesByHandlerOffset.put(
                    entry.getKey(),
                    new ArrayList<>(entry.getValue()));
            }
            vtablesByBaseOffset.putAll(other.vtablesByBaseOffset);
            handlerConstructionsByBaseOffset.putAll(other.handlerConstructionsByBaseOffset);
            for (Map.Entry<String, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByBaseOffset.entrySet()) {
                constructorWritesByBaseOffset.put(
                    entry.getKey(),
                    new ArrayList<>(entry.getValue()));
            }
            for (Map.Entry<Integer, TrackedValue> entry : other.valuesByThisOffset.entrySet()) {
                valuesByThisOffset.put(entry.getKey(), entry.getValue().copy());
            }
        }

        boolean mergeCompatibleObjectEvidenceFrom(ForwardArgState other) {
            if (!compatibleAddressMap(vtablesByThisOffset, other.vtablesByThisOffset)) {
                return false;
            }
            if (!compatibleAddressMapByString(vtablesByBaseOffset, other.vtablesByBaseOffset)) {
                return false;
            }
            if (!compatibleRegister("RCX", other)) {
                return false;
            }
            mergeOptionalRegister("RDX", other);
            mergeOptionalRegister("R8", other);
            mergeOptionalRegister("R9", other);
            mergeOptionalRegister("RSP", other);
            allocatorDispatchRegisters.retainAll(other.allocatorDispatchRegisters);

            vtablesByThisOffset.putAll(other.vtablesByThisOffset);
            vtablesByBaseOffset.putAll(other.vtablesByBaseOffset);
            for (Map.Entry<Integer, HandlerConstruction> entry :
                other.handlerConstructionsByThisOffset.entrySet()) {
                handlerConstructionsByThisOffset.putIfAbsent(entry.getKey(), entry.getValue());
            }
            for (Map.Entry<String, HandlerConstruction> entry :
                other.handlerConstructionsByBaseOffset.entrySet()) {
                handlerConstructionsByBaseOffset.putIfAbsent(entry.getKey(), entry.getValue());
            }
            for (Map.Entry<Integer, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByHandlerOffset.entrySet()) {
                constructorWritesByHandlerOffset
                    .computeIfAbsent(entry.getKey(), ignored -> new ArrayList<>())
                    .addAll(entry.getValue());
            }
            for (Map.Entry<String, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByBaseOffset.entrySet()) {
                constructorWritesByBaseOffset
                    .computeIfAbsent(entry.getKey(), ignored -> new ArrayList<>())
                    .addAll(entry.getValue());
            }
            for (Map.Entry<Integer, TrackedValue> entry : other.valuesByThisOffset.entrySet()) {
                TrackedValue existing = valuesByThisOffset.get(entry.getKey());
                if (existing != null && !existing.sameValue(entry.getValue())) {
                    return false;
                }
                valuesByThisOffset.putIfAbsent(entry.getKey(), entry.getValue().copy());
            }
            return true;
        }

        private boolean compatibleRegister(String register, ForwardArgState other) {
            TrackedValue left = registers.get(register);
            TrackedValue right = other.registers.get(register);
            if (left == null || right == null) {
                return left == right;
            }
            return left.sameValue(right);
        }

        private void mergeOptionalRegister(String register, ForwardArgState other) {
            TrackedValue left = registers.get(register);
            TrackedValue right = other.registers.get(register);
            if (left == null || right == null) {
                registers.remove(register);
                return;
            }
            if (!left.sameValue(right)) {
                registers.remove(register);
            }
        }

        private static boolean compatibleAddressMap(
            Map<Integer, Address> left,
            Map<Integer, Address> right) {

            for (Map.Entry<Integer, Address> entry : right.entrySet()) {
                Address existing = left.get(entry.getKey());
                if (existing != null && !existing.equals(entry.getValue())) {
                    return false;
                }
            }
            return true;
        }

        private static boolean compatibleAddressMapByString(
            Map<String, Address> left,
            Map<String, Address> right) {

            for (Map.Entry<String, Address> entry : right.entrySet()) {
                Address existing = left.get(entry.getKey());
                if (existing != null && !existing.equals(entry.getValue())) {
                    return false;
                }
            }
            return true;
        }
    }

    private static final class TrackedValue {
        final Address address;
        final Integer thisOffset;
        final Integer stackOffset;
        final String baseKey;
        final Integer baseOffset;
        final Long immediate;
        final Address fieldNameAddress;
        final String fieldName;
        final String expression;

        private TrackedValue(
            Address address,
            Integer thisOffset,
            Integer stackOffset,
            String baseKey,
            Integer baseOffset,
            Long immediate,
            Address fieldNameAddress,
            String fieldName,
            String expression) {

            this.address = address;
            this.thisOffset = thisOffset;
            this.stackOffset = stackOffset;
            this.baseKey = baseKey;
            this.baseOffset = baseOffset;
            this.immediate = immediate;
            this.fieldNameAddress = fieldNameAddress;
            this.fieldName = fieldName;
            this.expression = expression;
        }

        static TrackedValue address(Address address) {
            return new TrackedValue(address, null, null, null, null, null, null, null, null);
        }

        static TrackedValue thisOffset(int offset) {
            return new TrackedValue(
                null,
                offset,
                null,
                null,
                null,
                null,
                null,
                null,
                thisExpression(offset));
        }

        static TrackedValue stackOffset(int offset) {
            return new TrackedValue(
                null,
                null,
                offset,
                null,
                null,
                null,
                null,
                null,
                stackExpression(offset));
        }

        static TrackedValue baseOffset(String baseKey, int offset) {
            return new TrackedValue(
                null,
                null,
                null,
                baseKey,
                offset,
                null,
                null,
                null,
                baseExpression(baseKey, offset));
        }

        static TrackedValue immediate(long value) {
            return new TrackedValue(
                null,
                null,
                null,
                null,
                null,
                value,
                null,
                null,
                Long.toUnsignedString(value));
        }

        static TrackedValue fieldName(Address formatAddress, String fieldName) {
            return new TrackedValue(
                null,
                null,
                null,
                null,
                null,
                null,
                formatAddress,
                fieldName,
                fieldName);
        }

        TrackedValue addOffset(int delta) {
            try {
                if (thisOffset != null) {
                    return thisOffset(Math.addExact(thisOffset, delta));
                }
                if (stackOffset != null) {
                    return stackOffset(Math.addExact(stackOffset, delta));
                }
                if (baseKey != null && baseOffset != null) {
                    return baseOffset(baseKey, Math.addExact(baseOffset, delta));
                }
                if (immediate != null) {
                    return immediate(Math.addExact(immediate, (long)delta));
                }
            }
            catch (ArithmeticException ignored) {
                return null;
            }
            return this;
        }

        TrackedValue copy() {
            return new TrackedValue(
                address,
                thisOffset,
                stackOffset,
                baseKey,
                baseOffset,
                immediate,
                fieldNameAddress,
                fieldName,
                expression);
        }

        boolean sameValue(TrackedValue other) {
            if (other == null) {
                return false;
            }
            return sameAddress(address, other.address) &&
                sameObject(thisOffset, other.thisOffset) &&
                sameObject(stackOffset, other.stackOffset) &&
                sameObject(baseKey, other.baseKey) &&
                sameObject(baseOffset, other.baseOffset) &&
                sameObject(immediate, other.immediate) &&
                sameAddress(fieldNameAddress, other.fieldNameAddress) &&
                sameObject(fieldName, other.fieldName);
        }

        private static boolean sameAddress(Address left, Address right) {
            if (left == null || right == null) {
                return left == right;
            }
            return left.equals(right);
        }

        private static boolean sameObject(Object left, Object right) {
            if (left == null || right == null) {
                return left == right;
            }
            return left.equals(right);
        }

        private static String thisExpression(int offset) {
            if (offset == 0) {
                return "this";
            }
            if (offset > 0) {
                return "this+0x" + Integer.toHexString(offset);
            }
            return "this-0x" + Long.toHexString(-(long)offset);
        }

        private static String stackExpression(int offset) {
            if (offset == 0) {
                return "stack";
            }
            if (offset > 0) {
                return "stack+0x" + Integer.toHexString(offset);
            }
            return "stack-0x" + Long.toHexString(-(long)offset);
        }

        private static String baseExpression(String baseKey, int offset) {
            if (offset == 0) {
                return baseKey;
            }
            if (offset > 0) {
                return baseKey + "+0x" + Integer.toHexString(offset);
            }
            return baseKey + "-0x" + Long.toHexString(-(long)offset);
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

    private JsonArray stringArray(List<String> values) {
        JsonArray array = new JsonArray();
        if (values == null) {
            return array;
        }
        for (String value : values) {
            array.add(value);
        }
        return array;
    }
}
