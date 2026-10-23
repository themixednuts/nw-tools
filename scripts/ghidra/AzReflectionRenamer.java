// Import names, datatypes and layouts from AZ reflection resources.
//@category NewWorld

import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
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
import com.google.gson.stream.JsonWriter;

import org.apache.commons.compress.archivers.sevenz.SevenZArchiveEntry;
import org.apache.commons.compress.archivers.sevenz.SevenZFile;

import docking.widgets.filechooser.GhidraFileChooser;
import docking.widgets.filechooser.GhidraFileChooserMode;
import ghidra.app.script.GhidraScript;
import ghidra.app.services.ProgramManager;
import ghidra.framework.model.DomainFile;
import ghidra.framework.model.DomainFolder;
import ghidra.program.model.listing.Program;
import ghidra.app.util.SymbolPathParser;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.ArrayDataType;
import ghidra.program.model.data.BooleanDataType;
import ghidra.program.model.data.ByteDataType;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.CharDataType;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeComponent;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.DoubleDataType;
import ghidra.program.model.data.FloatDataType;
import ghidra.program.model.data.EnumDataType;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.LongLongDataType;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.ShortDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.data.TypedefDataType;
import ghidra.program.model.data.UnionDataType;
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
import ghidra.program.model.listing.AutoParameterType;
import ghidra.program.model.listing.ReturnParameterImpl;
import ghidra.program.model.listing.Variable;
import ghidra.program.model.listing.VariableStorage;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.lang.Register;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.Varnode;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.program.model.symbol.SymbolType;
import ghidra.util.exception.CancelledException;

public class AzReflectionRenamer extends GhidraScript {
    private static final String SCRIPT_VERSION = "1.6.9";
    private static final Pattern MODULE_ADDR_RE =
        Pattern.compile("^(?<module>[^+]+)\\+0x(?<offset>[0-9a-fA-F]+)$");
    private static final Pattern HEX_ADDR_RE =
        Pattern.compile("^0x(?<addr>[0-9a-fA-F]+)$");
    private static final Pattern BAD_NAME_CHARS_RE =
        Pattern.compile("[^A-Za-z0-9_:<>,~*&()\\[\\]=!+\\-/%|^.?]");
    private static final Pattern ANGLE_OPERATOR_RE =
        Pattern.compile("\\boperator\\s*(<=>|<<=?|>>=?|<=|>=|<|>)");
    private static final Pattern UNDERSCORES_RE = Pattern.compile("_+");
    private static final Pattern UUID_RE = Pattern.compile(
        "(?i)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}");
    private static final Pattern MB_GET_TYPE_NAME_SIGNATURE_RE = Pattern.compile(
        "const char \\*__cdecl MB::GetTypeName<(?:class |struct |enum )?(?<type>[^\\r\\n]+)>\\(void\\)");
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

    private static final String[] INFERRED_STATIC_RTTI_SLOT_NAMES = {
        "GetTypeId",
        "GetTypeName",
        "IsTypeOf",
        "EnumHierarchy",
        "CastConst",
        "Cast",
    };

    private static final String[] AZ_RTTI_INSTANCE_SLOT_NAMES = {
        "GetType",
        "GetTypeName",
        "IsTypeOf",
        "EnumTypes",
        "AddressOfConst",
        "AddressOf",
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
    private final Map<String, Structure> previewCoreDatatypes = new HashMap<>();
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
    private ReflectionStrings reflectionStrings;
    private Set<String> labelSeen;
    private Map<String, String> targetNameOwners;
    private Map<String, Address> pointerReadCache;
    private Map<String, Boolean> executableAddressCache;
    private Map<String, Namespace> scopeCache;
    private Map<String, List<Function>> calledFunctionsCache;
    private Map<String, List<InstanceVtableCandidate>> instanceVtableCandidatesCache;
    private Set<String> moduleInstanceVtableAddresses;
    private String currentModuleSource;
    private String currentModuleName;
    private Set<String> ambiguousModuleComponentNames;
    private final Set<String> reflectedClassScopePaths = new HashSet<>();
    private final Set<String> reflectedNamespaceScopePaths = new HashSet<>();
    private final Set<String> unparsedTypeNames = new TreeSet<>();
    private Map<String, JsonObject> datatypeGenerics = Collections.emptyMap();
    private Map<String, String> datatypeNames = Collections.emptyMap();
    private final Map<String, String> enumUnderlyingTypes = new LinkedHashMap<>();
    private boolean nativeLayoutProfile;
    private final Set<String> datatypeConflictingPaths = new HashSet<>();
    private final Set<Namespace> templateScopeCandidates = new LinkedHashSet<>();
    private static final Set<String> ENGINE_NAMESPACE_ROOTS = Set.of("AZ", "AZStd", "std", "MB", "Javelin");
    private GhidraCli cli;
    private boolean forceOutput;
    private boolean dryRun;
    private final ArrayList<JsonArraySpool> temporarySpools = new ArrayList<>();
    private java.nio.file.Path spoolDirectory;
    private java.nio.file.Path programProjectDirectory;
    private long programProjectReserve;
    private static final long INITIAL_STORAGE_RESERVE = 4L * 1024 * 1024 * 1024;
    private static final long RUNNING_STORAGE_RESERVE = 512L * 1024 * 1024;
    private Map<String, PendingComment> pendingComments;
    private JsonObject validationReport;
    private File validationFile;
    private boolean validating;
    private int validationErrors;
    private String currentPhase;
    private static final int MAX_VALIDATION_ERROR_SAMPLES = 100;
    private final Map<String, Integer> rejectedCodePointers = new LinkedHashMap<>();
    private static final Map<String, String> MODULE_NAMES_BY_VTABLE = moduleNamesByVtable();

    @Override
    protected void run() throws Exception {
        try {
            runImport();
        }
        catch (ReportStorageException error) {
            end(false);
            recordRunFailure("storage-failed", error);
            println("Reflection import stopped; this run's program edits were rolled back.");
            println(error.getMessage());
            println("The previous report was preserved. Correct the storage problem before rerunning.");
        }
        catch (CancelledException error) {
            end(false);
            recordRunFailure("cancelled", error);
            throw error;
        }
        catch (Exception error) {
            end(false);
            recordRunFailure("failed", error);
            throw error;
        }
        finally {
            pendingComments = null;
            closeTemporarySpools();
        }
    }

    private void runImport() throws Exception {
        cli = GhidraCli.parse(
            getScriptArgs(),
            Set.of(
                "resources",
                "serialize-json",
                "out",
                "module-evidence",
                "class-registration",
                "field-registration",
                "behavior-context"),
            Set.of("apply-renames", "full-actions", "force", "dry-run"),
            0);
        if (cli.helpRequested()) {
            printHelp();
            return;
        }
        if (cli.versionRequested()) {
            println("AzReflectionRenamer " + SCRIPT_VERSION);
            return;
        }
        forceOutput = cli.flag("force", null, true);
        dryRun = cli.flag("dry-run", null, false);
        if (!requireOpenProgram()) return;

        symbols = currentProgram.getSymbolTable();
        dataTypeManager = currentProgram.getDataTypeManager();
        gson = new GsonBuilder().setPrettyPrinting().create();
        pendingComments = new LinkedHashMap<>();
        labelSeen = new HashSet<>();
        targetNameOwners = new HashMap<>();
        pointerReadCache = new HashMap<>();
        executableAddressCache = new HashMap<>();
        scopeCache = new HashMap<>();
        calledFunctionsCache = new HashMap<>();
        instanceVtableCandidatesCache = new HashMap<>();
        moduleInstanceVtableAddresses = new HashSet<>();
        reflectionStrings = null;
        ambiguousModuleComponentNames = new TreeSet<>();
        validationReport = null;

        inputFile = chooseInputFile();
        boolean requestedApply = chooseApplyRenames();
        applyRenames = false;
        outputFile = chooseOutputFile(inputFile);
        if (outputFile.exists() && !forceOutput && requestedApply) {
            throw new IllegalArgumentException(
                "output already exists: " + outputFile + "; pass --force to replace it");
        }
        initializeValidationReport(requestedApply);
        prepareReportStorage();
        currentPhase = "reading-modules-and-registration";
        monitor.setMessage("Reading module reflection");
        moduleEvidence = loadModuleEvidence(inputFile);
        classRegistrationEvidence = loadClassRegistrationEvidence(inputFile);
        fieldRegistrationEvidence = loadFieldRegistrationEvidence(inputFile);
        currentPhase = "reading-behavior-context";
        monitor.setMessage("Reading BehaviorContext reflection");
        behaviorContextEvidence = loadBehaviorContextEvidence(inputFile);
        monitor.checkCancelled();
        currentPhase = "reading-serialize-context";
        JsonElement root = readSerializeContext(inputFile);
        if (!inputFile.isFile() && behaviorContextEvidence.input == null && moduleEvidence.inputs.isEmpty()) {
            throw new IllegalArgumentException("No supported reflection captures found in " + inputFile.getParent());
        }
        if (behaviorContextEvidence.skippedInputs > 0 || moduleEvidence.skippedInputs > 0 ||
            classRegistrationEvidence.skippedInputs > 0 || classRegistrationEvidence.malformedRecords > 0 ||
            fieldRegistrationEvidence.skippedInputs > 0 || fieldRegistrationEvidence.malformedRecords > 0) {
            throw new IOException("Reflection inputs could not be read: " +
                behaviorContextEvidence.skippedReasons + " " + moduleEvidence.skippedReasons + " " +
                classRegistrationEvidence.skippedReasons + " " + fieldRegistrationEvidence.skippedReasons);
        }
        monitor.setMessage("Indexing reflection type identities");
        currentPhase = "indexing-type-identities";
        ScanResult scan = scanSerializeContext(root);
        reconcileBehaviorTypeScopes(scan.classData);
        enrichFieldRegistrationEvidence();
        println("Validating reflection and proposed layouts before program edits...");
        ActionSink previewActions = new ActionSink(false);
        validating = true;
        try {
            runReflectionPass(root, scan, previewActions, true);
        }
        finally { validating = false; }
        finishValidationReport(scan, previewActions);
        writeValidationReport();
        if (validationErrors > 0) {
            println("Validation found " + validationErrors + " errors. No program changes were applied. " +
                "See " + validationFile.getAbsolutePath());
            return;
        }
        if (!requestedApply) {
            println("Dry run complete. No program changes were applied. Report: " + validationFile.getAbsolutePath());
            return;
        }
        resetReflectionPassState();
        applyRenames = true;
        boolean includeFullActions = includeFullActionReport();
        ActionSink actions = new ActionSink(includeFullActions);
        JsonObject containerReport = runReflectionPass(root, scan, actions, false);

        ActionStats stats = actions.stats;

        JsonObject report = new JsonObject();
        report.addProperty("input", inputFile.getAbsolutePath());
        report.addProperty("mode", "unified-reflection");
        report.addProperty("scriptVersion", SCRIPT_VERSION);
        report.addProperty("reportFormatVersion", 2);
        report.addProperty("validationReport", validationFile.getAbsolutePath());
        report.addProperty("resources", inputFile.getParentFile().getAbsolutePath());
        report.addProperty("serializeContextLoaded", inputFile.isFile());
        report.addProperty("typeRegistry", "excluded");
        report.addProperty("nativeLayoutProfile", nativeLayoutProfile ? "NewWorld 3-26 x64" : "unmatched; reflection fields only");
        report.add("reflectedTypes", reflectedTypesReport(scan.classData));
        report.addProperty("requestedRenames", applyRenames);
        report.addProperty("applyRenames", applyRenames);
        report.addProperty("dryRun", dryRun);
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
        report.add("unparsedTypeNames", stringArray(unparsedTypeNames));
        report.add("containers", containerReport);
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
        currentPhase = "writing-applied-report";
        writeRenameReport(report, actions);

        String summary = buildSummary(scan, actions, stats);
        println(summary);
    }

    private JsonObject runReflectionPass(JsonElement root, ScanResult scan, ActionSink actions, boolean preview)
        throws Exception {
        Map<String, SlotGroup> rttiGroups = rttiSlotGroups(scan.rttiTypes.values(), SERIALIZE_RTTI_SLOT_NAMES);
        Map<String, SlotGroup> callbacks = callbackGroups(scan.classData);
        Set<String> functionSeen = new HashSet<>();
        Set<String> aliasSeen = new HashSet<>();
        runReflectionPhase("core-reflection", preview, () -> {
            ensureCoreReflectionDatatypes(actions);
        });
        runReflectionPhase("datatypes-and-layouts", preview, () ->
            declaredStructuresByTypeId = processReflectedDatatypes(scan.classData, actions));
        runReflectionPhase("serialize-rtti", preview, () -> {
            processCoreRttiCastHelpers(functionSeen, actions);
            monitor.initialize(scan.rttiTypes.size());
            for (RttiType type : scan.rttiTypes.values()) {
                monitor.setMessage("RTTI names: " + type.targetTypeName);
                processRttiType(type, SERIALIZE_RTTI_SLOT_NAMES, rttiGroups, functionSeen, aliasSeen, actions);
                monitor.incrementProgress(1);
                monitor.checkCancelled();
            }
        });
        runReflectionPhase("serialize-callbacks", preview, () -> {
            monitor.initialize(scan.classData.size());
            for (ClassData data : scan.classData) {
                monitor.setMessage("Serialized callbacks: " + data.targetTypeName);
                processClassData(data, callbacks, functionSeen, aliasSeen, actions);
                monitor.incrementProgress(1);
                monitor.checkCancelled();
            }
            processSerializeFactoryVtables(scan.classData, actions);
            processMemberFunctionThisParameters(scan.classData, declaredStructuresByTypeId, actions);
        });
        JsonObject containerReport = new JsonObject();
        runReflectionPhase("serialization-containers", preview, () -> {
            JsonObject result = processSerializeContainers(root, actions);
            for (var entry : result.entrySet()) containerReport.add(entry.getKey(), entry.getValue());
        });
        runReflectionPhase("registration-traces", preview, () -> {
            processClassRegistrationTraces(classRegistrationFunctionGroups(),
                classRegistrationFunctionIndex(classRegistrationEvidence), functionSeen, aliasSeen, actions);
            processFieldRegistrationTraces(functionSeen, actions);
        });
        runReflectionPhase("module-descriptors", preview, () -> processModuleDescriptorRenames(actions));
        runReflectionPhase("inferred-static-rtti", preview,
            () -> processInferredStaticRttiHelperVtables(functionSeen, aliasSeen, actions));
        runReflectionPhase("behavior-context-apis-and-buses", preview,
            () -> processBehaviorContextEvidence(behaviorContextEvidence, functionSeen, aliasSeen, actions));
        runReflectionPhase("mb-names-and-reflection-signatures", preview, () -> {
            processMbGetTypeNameFunctions(functionSeen, actions);
            processStaticReflectFunctionParameters(scan.classData, actions);
        });
        runReflectionPhase("namespace-cleanup-and-comments", preview, () -> {
            cleanupRepeatedFunctionNamespaces(actions);
            flushPendingComments();
        });
        return containerReport;
    }

    @FunctionalInterface
    private interface ReflectionPhase { void run() throws Exception; }

    private void runReflectionPhase(String name, boolean preview, ReflectionPhase operation) throws Exception {
        currentPhase = name;
        monitor.setMessage((preview ? "Validating: " : "Applying: ") + name);
        monitor.checkCancelled();
        JsonObject phase = new JsonObject();
        phase.addProperty("name", name);
        int errorsBefore = validationErrors;
        long started = System.nanoTime();
        try {
            operation.run();
            monitor.checkCancelled();
        }
        catch (CancelledException | ReportStorageException error) { throw error; }
        catch (Exception error) {
            if (!preview) throw error;
            recordValidationError(error, null);
        }
        finally {
            if (preview) {
                phase.addProperty("status", validationErrors == errorsBefore ? "passed" : "failed");
                phase.addProperty("errorCount", validationErrors - errorsBefore);
                phase.addProperty("elapsedMillis", (System.nanoTime() - started) / 1_000_000);
                validationReport.getAsJsonArray("phases").add(phase);
            }
        }
    }

    private void resetReflectionPassState() {
        pendingComments.clear();
        labelSeen.clear();
        targetNameOwners.clear();
        scopeCache.clear();
        calledFunctionsCache.clear();
        instanceVtableCandidatesCache.clear();
        moduleInstanceVtableAddresses.clear();
        ambiguousModuleComponentNames.clear();
        datatypeConflictingPaths.clear();
        templateScopeCandidates.clear();
        declaredStructuresByTypeId = Collections.emptyMap();
        previewCoreDatatypes.clear();
    }

    private void initializeValidationReport(boolean requestedApply) {
        String name = outputFile.getName();
        int extension = name.lastIndexOf('.');
        String stem = extension > 0 ? name.substring(0, extension) : name;
        if (stem.equals("reflection-report")) stem = "reflection";
        validationFile = new File(outputFile.getAbsoluteFile().getParentFile(), stem + "-dry-run.json");
        validationErrors = 0;
        rejectedCodePointers.clear();
        validationReport = new JsonObject();
        validationReport.addProperty("scriptVersion", SCRIPT_VERSION);
        validationReport.addProperty("mode", "reflection-validation");
        validationReport.addProperty("program", currentProgram.getName());
        validationReport.addProperty("imageBase", currentProgram.getImageBase().toString());
        validationReport.addProperty("input", inputFile.getAbsolutePath());
        validationReport.addProperty("typeRegistry", "excluded");
        validationReport.addProperty("requestedApply", requestedApply);
        validationReport.addProperty("programEditsDuringValidation", false);
        validationReport.addProperty("coverage", "Read-only evidence analysis, proposed layouts and function signature/storage plans. " +
            "Database writes and apply-only operations run after validation and can still fail.");
        validationReport.addProperty("status", "reading-inputs");
        validationReport.add("phases", new JsonArray());
        validationReport.add("errors", new JsonArray());
    }

    private void finishValidationReport(ScanResult scan, ActionSink actions) {
        validationReport.addProperty("status", validationErrors == 0 ? "passed" : "failed");
        validationReport.addProperty("errorCount", validationErrors);
        validationReport.addProperty("errorSamplesOmitted",
            Math.max(0, validationErrors - MAX_VALIDATION_ERROR_SAMPLES));
        validationReport.addProperty("nativeLayoutProfile", nativeLayoutProfile);
        validationReport.addProperty("rttiTypeCount", scan.rttiTypes.size());
        validationReport.addProperty("classDataCount", scan.classData.size());
        validationReport.addProperty("moduleDescriptorCount", moduleEvidence.descriptorCount);
        validationReport.addProperty("behaviorClassCount", behaviorContextEvidence.classCount);
        validationReport.addProperty("behaviorEbusCount", behaviorContextEvidence.ebusCount);
        validationReport.addProperty("behaviorEbusEventCount", behaviorContextEvidence.ebusEventCount);
        validationReport.addProperty("actionCount", actions.size());
        validationReport.addProperty("wouldApplyCount", actions.stats.wouldApplyCount);
        validationReport.addProperty("appliedCount", actions.stats.appliedCount);
        validationReport.add("actionKindCounts", mapToJson(actions.stats.kindCounts));
        validationReport.add("actionReasonCounts", mapToJson(actions.stats.reasonCounts));
        validationReport.add("actionSamples", actions.actionSamples());
        validationReport.addProperty("checkedSignatureCount", actions.stats.checkedSignatureCount);
        validationReport.add("signatureReasonCounts", mapToJson(actions.stats.signatureReasonCounts));
        validationReport.add("signaturePlanSamples", actions.signatureSamples);
        validationReport.add("rejectedNativeCodePointers", mapToJson(rejectedCodePointers));
        validationReport.add("unparsedTypeNames", stringArray(unparsedTypeNames));
    }

    private void recordValidationError(Exception error, JsonObject evidence) {
        validationErrors++;
        if (validationReport == null || validationReport.getAsJsonArray("errors").size() >= MAX_VALIDATION_ERROR_SAMPLES) return;
        JsonObject item = new JsonObject();
        item.addProperty("phase", currentPhase);
        item.addProperty("exception", error.getClass().getName());
        item.addProperty("message", error.getMessage());
        JsonArray stack = new JsonArray();
        for (StackTraceElement frame : error.getStackTrace()) {
            if (stack.size() == 40) break;
            stack.add(frame.toString());
        }
        item.add("stackTrace", stack);
        if (error.getCause() != null) item.addProperty("cause", error.getCause().toString());
        if (evidence != null) item.add("action", actionSample(evidence));
        validationReport.getAsJsonArray("errors").add(item);
    }

    private void recordRunFailure(String status, Exception error) {
        if (validationReport == null) return;
        if (!(error instanceof ReflectionActionException)) recordValidationError(error, null);
        validationReport.addProperty("status", status);
        validationReport.addProperty("failurePass", applyRenames ? "apply" : "validation");
        validationReport.addProperty("errorCount", validationErrors);
        try { writeValidationReport(); }
        catch (Exception reportError) { println("Cannot save validation diagnostics: " + reportError.getMessage()); }
    }

    private void recordValidationActionError(JsonObject action, JsonObject detail) {
        validationErrors++;
        JsonArray errors = validationReport.getAsJsonArray("errors");
        if (errors.size() >= MAX_VALIDATION_ERROR_SAMPLES) return;
        JsonObject item = new JsonObject();
        item.addProperty("phase", currentPhase);
        item.addProperty("kind", "action-error");
        item.addProperty("message", stringMember(detail, "error"));
        copyActionProperty(detail, item, "errorType");
        item.add("action", actionSample(action));
        if (detail != action) item.add("field", detail);
        errors.add(item);
    }

    private static class ReflectionActionException extends IllegalStateException {
        ReflectionActionException(JsonObject action) {
            super("Reflection action failed: " + action.get("kind") + " " + action.get("name") +
                " at " + action.get("address") + ": " + action.get("error"));
        }
    }

    private void writeValidationReport() {
        java.nio.file.Path temporary = null;
        try {
            java.nio.file.Path parent = validationFile.toPath().toAbsolutePath().getParent();
            Files.createDirectories(parent);
            temporary = Files.createTempFile(parent, ".reflection-validation-", ".tmp");
            try (JsonWriter writer = new JsonWriter(openReportWriter(temporary))) {
                writer.setIndent("  ");
                writer.setSerializeNulls(true);
                writeJsonValue(validationReport, writer, false);
            }
            try {
                Files.move(temporary, validationFile.toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            }
            catch (java.nio.file.AtomicMoveNotSupportedException error) {
                Files.move(temporary, validationFile.toPath(), StandardCopyOption.REPLACE_EXISTING);
            }
        }
        catch (IOException error) {
            throw new ReportStorageException("Cannot save reflection validation at " + validationFile, error);
        }
        finally {
            if (temporary != null) {
                try { Files.deleteIfExists(temporary); }
                catch (IOException error) { println("Unable to remove temporary validation report " + temporary); }
            }
        }
        println("Dry-run validation report: " + validationFile.getAbsolutePath());
    }

    private boolean requireOpenProgram() throws Exception {
        if (currentProgram != null) return true;
        ProgramManager manager = startupProgramManager();
        if (manager == null) {
            println("No CodeBrowser program manager is attached. Open NewWorld 3-26 in CodeBrowser " +
                "and run the script from that window's Script Manager.");
            return false;
        }

        Program selected = manager.getCurrentProgram();
        boolean ambiguous = false;
        if (selected == null) {
            ArrayList<Program> matches = new ArrayList<>();
            for (Program program : manager.getAllOpenPrograms()) {
                if (isStartupTarget(program.getName()) ||
                    (program.getDomainFile() != null && isStartupTarget(program.getDomainFile().getName()))) matches.add(program);
            }
            if (matches.size() == 1) selected = matches.get(0);
            ambiguous = matches.size() > 1;
        }
        if (selected == null && !ambiguous && state != null && state.getProject() != null) {
            ArrayList<DomainFile> matches = new ArrayList<>();
            collectStartupTargets(state.getProject().getProjectData().getRootFolder(), matches);
            if (matches.size() == 1) selected = manager.openProgram(matches.get(0));
        }
        if (selected != null) {
            openProgram(selected);
        }
        else {
            Program choice = chooseStartupProgram();
            if (choice == null) {
                println("Reflection import stopped before edits. Open or select NewWorld 3-26 in CodeBrowser to continue.");
                return false;
            }
            try { openProgram(choice); }
            finally { choice.release(this); }
        }
        if (currentProgram == null) {
            println("The selected program could not be activated in this CodeBrowser. Import stopped before edits.");
            return false;
        }
        println("Reflection target: " + currentProgram.getName());
        return true;
    }

    protected ProgramManager startupProgramManager() {
        return state == null || state.getTool() == null ? null : state.getTool().getService(ProgramManager.class);
    }

    protected Program chooseStartupProgram() throws Exception {
        if (state == null || state.getTool() == null || isRunningHeadless()) return null;
        String[] arguments = getScriptArgs();
        setScriptArgs(new String[0]);
        try { return askProgram("Select NewWorld 3-26"); }
        finally { setScriptArgs(arguments); }
    }

    private boolean isStartupTarget(String name) {
        return name != null && name.toLowerCase(Locale.ROOT).replaceAll("\\.exe$", "")
            .replaceAll("[\\s_-]", "").equals("newworld326");
    }

    private void collectStartupTargets(DomainFolder folder, List<DomainFile> matches) throws CancelledException {
        monitor.checkCancelled();
        for (DomainFile file : folder.getFiles()) {
            if (isStartupTarget(file.getName()) && Program.class.isAssignableFrom(file.getDomainObjectClass())) matches.add(file);
        }
        for (DomainFolder child : folder.getFolders()) collectStartupTargets(child, matches);
    }

    private JsonElement readSerializeContext(File file) throws IOException {
        if (!file.isFile()) return new JsonObject();
        try (Reader reader = Files.newBufferedReader(file.toPath(), StandardCharsets.UTF_8)) {
            JsonElement root = JsonParser.parseReader(reader);
            if (root == null || !root.isJsonObject() ||
                !(root.getAsJsonObject().has("uuidMap") || root.getAsJsonObject().has("classData"))) {
                throw new IOException("Not a SerializeContext capture: " + file);
            }
            return root;
        }
    }

    private void writeRenameReport(JsonObject report, ActionSink actions) throws Exception {
        File parent = outputFile.getAbsoluteFile().getParentFile();
        Files.createDirectories(parent.toPath());
        java.nio.file.Path temporary = null;
        try {
            temporary = Files.createTempFile(parent.toPath(), ".reflection-report-", ".tmp");
            try (JsonWriter writer = new JsonWriter(openReportWriter(temporary))) {
                writer.setIndent("  ");
                writer.setSerializeNulls(true);
                writer.beginObject();
                for (Map.Entry<String, JsonElement> entry : report.entrySet()) {
                    monitor.checkCancelled();
                    writer.name(entry.getKey());
                    if (entry.getKey().equals("behaviorContextEvidence")) {
                        writer.beginObject();
                        for (Map.Entry<String, JsonElement> field : entry.getValue().getAsJsonObject().entrySet()) {
                            writer.name(field.getKey());
                            writeJsonValue(field.getValue(), writer, false);
                        }
                        writer.name("inventory");
                        writeBehaviorInventory(writer, behaviorContextEvidence);
                        writer.endObject();
                    }
                    else writeJsonValue(entry.getValue(), writer, false);
                }
                if (actions.keepsFullActions()) {
                    writer.name("symbolNameSnapshots");
                    actions.symbolNameSnapshots.writeTo(writer);
                    writer.name("actions");
                    actions.fullActionSpool.writeTo(writer);
                }
                writer.endObject();
            }
            try {
                Files.move(temporary, outputFile.toPath(), StandardCopyOption.REPLACE_EXISTING,
                    StandardCopyOption.ATOMIC_MOVE);
            }
            catch (java.nio.file.AtomicMoveNotSupportedException error) {
                Files.move(temporary, outputFile.toPath(), StandardCopyOption.REPLACE_EXISTING);
            }
        }
        catch (IOException error) {
            throw new ReportStorageException("Cannot write reflection report at " + outputFile.getAbsolutePath(), error);
        }
        finally {
            if (temporary != null) {
                try { Files.deleteIfExists(temporary); }
                catch (IOException error) { println("Unable to remove temporary report " + temporary + ": " + error.getMessage()); }
            }
        }
        println("Reflection report: " + outputFile.getAbsolutePath());
    }

    private void writeBehaviorInventory(JsonWriter writer, BehaviorContextEvidence index)
        throws IOException, CancelledException {
        writer.beginObject();
        writer.name("context"); writeJsonValue(index.contextMetadata, writer, false);
        writer.name("classes"); index.classInventory.writeTo(writer);
        writer.name("globalMethods"); index.globalMethodInventory.writeTo(writer);
        writer.name("globalProperties"); index.globalPropertyInventory.writeTo(writer);
        writer.name("ebuses"); index.ebusInventory.writeTo(writer);
        writer.name("typeToClassMap"); writeJsonValue(index.typeToClassInventory, writer, false);
        writer.name("onDemandReflection"); writeJsonValue(index.onDemandInventory, writer, false);
        writer.name("constants"); writeJsonValue(index.constantInventory, writer, false);
        writer.name("functionUses").beginArray();
        for (BehaviorFunctionCandidate candidate : index.functionCandidates) {
            monitor.checkCancelled();
            String name = fullName(candidate.scope, candidate.localName);
            writer.beginObject().name("name").value(name).name("address").value(candidate.address)
                .name("kind").value(candidate.kind);
            if (candidate.ownerTypeId != null) writer.name("ownerTypeId").value(candidate.ownerTypeId);
            if (candidate.slot != null) writer.name("virtualDispatchSlotOffset").value(candidate.slot);
            String offset = captureOffset(candidate.address);
            if (offset != null) writer.name("implementationAlias")
                .value("AZ::BehaviorContext::Method<" + name + ">::Implementation<0x" + offset + ">");
            writer.endObject();
        }
        writer.endArray().endObject();
    }

    private void writeJsonValue(JsonElement value, JsonWriter writer, boolean compact) throws IOException {
        if (value == null || value.isJsonNull()) {
            writer.nullValue();
        }
        else if (value.isJsonArray()) {
            writer.beginArray();
            for (JsonElement item : value.getAsJsonArray()) writeJsonValue(item, writer, compact);
            writer.endArray();
        }
        else if (value.isJsonObject()) {
            writer.beginObject();
            JsonObject object = value.getAsJsonObject();
            for (Map.Entry<String, JsonElement> entry : object.entrySet()) {
                if (compact && isBehaviorAlias(object, entry.getKey(), entry.getValue())) continue;
                writer.name(entry.getKey());
                writeJsonValue(entry.getValue(), writer, compact);
            }
            writer.endObject();
        }
        else if (value.getAsJsonPrimitive().isBoolean()) writer.value(value.getAsBoolean());
        else if (value.getAsJsonPrimitive().isNumber()) writer.jsonValue(value.getAsString());
        else writer.value(value.getAsString());
    }

    private void copyJsonValue(JsonReader reader, JsonWriter writer) throws IOException, CancelledException {
        monitor.checkCancelled();
        switch (reader.peek()) {
            case BEGIN_OBJECT:
                reader.beginObject(); writer.beginObject();
                while (reader.hasNext()) {
                    writer.name(reader.nextName());
                    copyJsonValue(reader, writer);
                }
                reader.endObject(); writer.endObject();
                break;
            case BEGIN_ARRAY:
                reader.beginArray(); writer.beginArray();
                while (reader.hasNext()) copyJsonValue(reader, writer);
                reader.endArray(); writer.endArray();
                break;
            case NULL:
                reader.nextNull(); writer.nullValue();
                break;
            case NUMBER:
                writer.jsonValue(reader.nextString());
                break;
            case BOOLEAN:
                writer.value(reader.nextBoolean());
                break;
            case STRING:
                writer.value(reader.nextString());
                break;
            default:
                throw new IOException("Unexpected report token: " + reader.peek());
        }
    }

    private class JsonArraySpool implements AutoCloseable {
        private final boolean compact;
        private final boolean enabled;
        private final String purpose;
        private java.nio.file.Path path;
        private JsonWriter writer;
        private boolean finished;
        private int count;

        JsonArraySpool(boolean compact, boolean enabled) {
            this(compact, enabled, "inventory");
        }

        JsonArraySpool(boolean compact, boolean enabled, String purpose) {
            this.compact = compact;
            this.enabled = enabled;
            this.purpose = purpose;
        }

        void add(JsonElement value) {
            count++;
            if (!enabled) return;
            try {
                if (finished) throw new IOException("Report spool already finalized");
                if (writer == null) {
                    path = Files.createTempFile(reportSpoolDirectory(), purpose + "-", ".json");
                    path.toFile().deleteOnExit();
                    temporarySpools.add(this);
                    writer = new JsonWriter(openReportWriter(path));
                    writer.setSerializeNulls(true);
                    writer.beginArray();
                }
                writeJsonValue(value, writer, compact);
            }
            catch (IOException error) {
                throw new ReportStorageException("Cannot write reflection evidence at " +
                    (path == null ? reportStorageParent() : path), error);
            }
        }

        int size() { return count; }

        void finish() throws IOException {
            if (finished) return;
            finished = true;
            if (writer != null) {
                writer.endArray();
                writer.close();
                writer = null;
            }
        }

        void writeTo(JsonWriter output) throws IOException, CancelledException {
            finish();
            if (path == null) { output.beginArray().endArray(); return; }
            try (JsonReader input = new JsonReader(Files.newBufferedReader(path, StandardCharsets.UTF_8))) {
                copyJsonValue(input, output);
            }
        }

        JsonArray readArray() {
            try {
                finish();
                if (path == null) return new JsonArray();
                try (Reader input = Files.newBufferedReader(path, StandardCharsets.UTF_8)) {
                    return JsonParser.parseReader(input).getAsJsonArray();
                }
            }
            catch (IOException error) {
                throw new java.io.UncheckedIOException(error);
            }
        }

        @Override
        public void close() throws IOException {
            try {
                if (writer != null) {
                    try { writer.close(); }
                    catch (IOException ignored) { /* Unfinished scratch JSON is discarded below. */ }
                }
            }
            finally {
                writer = null;
                if (path != null) Files.deleteIfExists(path);
            }
        }
    }

    private void closeTemporarySpools() {
        for (JsonArraySpool spool : temporarySpools) {
            try { spool.close(); }
            catch (IOException error) { println("Reflection scratch cleanup at " + spool.path + ": " + error.getMessage()); }
        }
        temporarySpools.clear();
        if (spoolDirectory != null) {
            try { Files.deleteIfExists(spoolDirectory); }
            catch (IOException error) { println("Unable to remove reflection scratch directory " + spoolDirectory + ": " + error.getMessage()); }
            spoolDirectory = null;
        }
    }

    private java.nio.file.Path reportStorageParent() {
        File anchor = outputFile != null ? outputFile : inputFile;
        return anchor == null ? ghidra.framework.Application.getUserTempDirectory().toPath() :
            anchor.getAbsoluteFile().getParentFile().toPath();
    }

    private java.nio.file.Path reportSpoolDirectory() throws IOException {
        if (spoolDirectory == null) {
            java.nio.file.Path parent = reportStorageParent();
            Files.createDirectories(parent);
            spoolDirectory = Files.createTempDirectory(parent, ".az-reflection-");
            spoolDirectory.toFile().deleteOnExit();
        }
        return spoolDirectory;
    }

    private void prepareReportStorage() {
        try {
            java.nio.file.Path parent = reportStorageParent();
            Files.createDirectories(parent);
            if (outputFile.isDirectory()) throw new IOException("Report destination is a directory: " + outputFile);
            requireStorageSpace(parent, INITIAL_STORAGE_RESERVE);
            requireStorageSpace(ghidra.framework.Application.getUserTempDirectory().toPath(), INITIAL_STORAGE_RESERVE);
            var domainFile = currentProgram.getDomainFile();
            var project = domainFile == null ? null : domainFile.getProjectLocator();
            if (project != null && project.getProjectDir().isDirectory()) {
                programProjectDirectory = project.getProjectDir().toPath();
                programProjectReserve = Math.max(INITIAL_STORAGE_RESERVE, domainFile.length() + RUNNING_STORAGE_RESERVE);
                requireStorageSpace(programProjectDirectory, programProjectReserve);
            }
            java.nio.file.Path probe = Files.createTempFile(parent, ".reflection-write-check-", ".tmp");
            Files.delete(probe);
        }
        catch (IOException error) {
            throw new ReportStorageException("Reflection storage preflight failed", error);
        }
    }

    protected long availableSpace(java.nio.file.Path directory) throws IOException {
        return Files.getFileStore(directory).getUsableSpace();
    }

    private void requireStorageSpace(java.nio.file.Path directory, long minimum) throws IOException {
        long available = availableSpace(directory);
        if (available < minimum) {
            throw new IOException(directory + " has " + available / (1024 * 1024) +
                " MiB free; at least " + minimum / (1024 * 1024) + " MiB is required. " +
                "Ghidra database temporary directory: " + ghidra.framework.Application.getUserTempDirectory());
        }
    }

    protected Writer openReportWriter(java.nio.file.Path path) throws IOException {
        return new java.io.FilterWriter(Files.newBufferedWriter(path, StandardCharsets.UTF_8)) {
            private long charactersSinceCheck = 262144;

            private void checkSpace(int length) throws IOException {
                charactersSinceCheck += length;
                if (charactersSinceCheck < 262144) return;
                charactersSinceCheck = 0;
                requireStorageSpace(path.getParent(), RUNNING_STORAGE_RESERVE);
                requireStorageSpace(ghidra.framework.Application.getUserTempDirectory().toPath(), RUNNING_STORAGE_RESERVE);
                if (programProjectDirectory != null) requireStorageSpace(programProjectDirectory, programProjectReserve);
            }

            @Override public void write(int value) throws IOException {
                checkSpace(1);
                out.write(value);
            }

            @Override public void write(char[] value, int offset, int length) throws IOException {
                checkSpace(length);
                out.write(value, offset, length);
            }

            @Override public void write(String value, int offset, int length) throws IOException {
                checkSpace(length);
                out.write(value, offset, length);
            }
        };
    }

    private static class ReportStorageException extends java.io.UncheckedIOException {
        ReportStorageException(String context, IOException cause) {
            super(context + ": " + cause.getMessage(), cause);
        }
    }

    private File chooseInputFile() throws Exception {
        String resources = optionValue("resources", "AZOTH_REFLECTION_RESOURCES");
        String serialize = optionValue("serialize-json", "AZOTH_SERIALIZE_JSON");
        if (resources != null && serialize != null) {
            throw new IllegalArgumentException("Choose --resources or --serialize-json.");
        }
        if (serialize != null) {
            File file = new File(serialize).getAbsoluteFile();
            if (!file.isFile()) throw new IOException("SerializeContext file not found: " + file);
            rememberInputDirectory(file);
            return file;
        }
        File selected = resources == null ? chooseInputFileViaPicker() : new File(resources);
        File directory = reflectionResourcesDirectory(selected);
        rememberInputDirectory(directory);
        return new File(directory, "serialize.json");
    }

    private File reflectionResourcesDirectory(File selected) {
        File absolute = selected.getAbsoluteFile();
        File directory = absolute.isFile() ? absolute.getParentFile() : absolute;
        if (!directory.isDirectory()) {
            throw new IllegalArgumentException("Reflection resources directory not found: " + directory);
        }
        File child = new File(directory, "resources");
        if (child.isDirectory()) return child;
        for (File parent = directory; parent != null; parent = parent.getParentFile()) {
            if (parent.getName().equalsIgnoreCase("resources") ||
                new File(parent, "serialize.json").isFile() ||
                new File(parent, "behavior-context.json").isFile() ||
                new File(parent, "behavior-context.json.zip").isFile() ||
                new File(parent, "behavior-context.7z").isFile() ||
                new File(parent, "modules").isDirectory()) return parent;
        }
        return directory;
    }

    // NewWorld 3-26's IDataContainer ends before the neighboring GenericClassInfo table.
    // Unknown flag/operation names stay comments rather than borrowing a different engine ABI.
    private static final String[] CONTAINER_SLOT_NAMES = {
        null, "GetElement", "GetElement", "EnumElements", "Size", "Capacity",
        "IsStableElements", "IsFixedSize", "IsFixedCapacity", "IsSmartPointer",
        null, null, null, "GetAssociativeContainerInterface", "ReserveElement",
        "GetElementByIndex", "StoreElement", null, null, "RemoveElement",
        "RemoveElements", "ClearElements", "ElementsUpdated"
    };

    private static final String[] CONTAINER_SLOT_DESCRIPTIONS = {
        "scalar deleting destructor", "GetElement(ClassElement&, const DataElement&)",
        "GetElement(u32 elementNameCrc)", "EnumElements", "Size", "Capacity",
        "IsStableElements", "IsFixedSize", "IsFixedCapacity", "IsSmartPointer",
        "flag; exact source name unresolved", "flag; exact source name unresolved",
        "flag; exact source name unresolved", "GetAssociativeContainerInterface",
        "ReserveElement", "GetElementByIndex", "StoreElement",
        "insertion/movement callback; exact source name unresolved",
        "element-swap callback; exact source name unresolved", "RemoveElement",
        "RemoveElements", "ClearElements", "ElementsUpdated"
    };

    private static class SerializeContainer {
        String typeId;
        String typeName;
        String family;
        String vtable;
        JsonObject genericInfo;
        JsonArray slots;
        ArrayList<String> scope;
    }

    private List<SerializeContainer> serializeContainers(JsonElement root) {
        ArrayList<JsonObject> objects = collectObjects(root);
        jsonObjectsById = indexObjectsById(objects);
        Map<String, JsonObject> generics = indexGenericInfo(root, objects, jsonObjectsById);
        Map<String, String> names = indexRawTypeNames(objects);
        ArrayList<SerializeContainer> result = new ArrayList<>();
        for (Map.Entry<String, JsonObject> entry : generics.entrySet()) {
            JsonObject generic = entry.getValue();
            JsonObject classData = objectMember(generic, "classData");
            String family = classData == null ? null : stringMember(classData, "name");
            if (!Set.of("AZStd::vector", "AZStd::unordered_map", "AZStd::unordered_set").contains(
                    family == null ? "" : family)) {
                continue;
            }
            JsonObject info = objectMember(classData, "containerInfo");
            SerializeContainer container = new SerializeContainer();
            container.typeId = entry.getKey();
            container.family = family;
            container.typeName = displayTypeName(entry.getKey(), family, generics, names, new HashSet<>());
            container.genericInfo = generic;
            container.vtable = info == null ? null : stringMember(info, "vtable");
            container.slots = info == null ? null : arrayMember(info, "slots");
            // ClassData<T> is the script's analysis namespace, not a claim that AZStd objects are virtual.
            if (container.typeName != null) {
                container.scope = new ArrayList<>(List.of("AZ", "SerializeContext",
                    "ClassData<" + safeTypeName(container.typeName) + ">", "container"));
            }
            result.add(container);
        }
        result.sort((a, b) -> a.typeId.compareTo(b.typeId));
        return result;
    }

    private JsonObject processSerializeContainers(JsonElement root, ActionSink actions) throws Exception {
        JsonObject report = new JsonObject();
        JsonObject summary = new JsonObject();
        JsonArray skipped = new JsonArray();
        List<SerializeContainer> containers = serializeContainers(root);
        summary.addProperty("specializations", containers.size());
        report.add("summary", summary);
        report.add("skipped", skipped);
        report.addProperty("programName", currentProgram.getName());
        if (!supportsContainerPointerLayout()) {
            summary.addProperty("reason", "unsupported-container-ABI; profile requires little-endian 64-bit pointers");
            return report;
        }
        Map<String, ArrayList<String>> functionOwners = new LinkedHashMap<>();
        Map<String, ArrayList<SerializeContainer>> functionContainers = new LinkedHashMap<>();
        Map<String, LinkedHashSet<Integer>> functionSlots = new LinkedHashMap<>();
        Set<String> tables = new HashSet<>();
        int validated = 0;
        int resolved = 0;
        for (SerializeContainer container : containers) {
            monitor.checkCancelled();
            String failure = validateContainerTable(container);
            if (failure != null) {
                JsonObject skip = new JsonObject();
                skip.addProperty("typeId", container.typeId);
                skip.addProperty("vtable", container.vtable);
                skip.addProperty("reason", failure);
                skipped.add(skip);
                continue;
            }
            validated++;
            tables.add(container.vtable);
            String owner = container.typeName == null ? container.family + " typeId=" + container.typeId :
                container.typeName;
            Address table = parseCaptureAddress(container.vtable);
            if (container.scope != null) {
                resolved++;
                JsonObject action = containerAction("container_vtable", container.vtable, owner);
                boolean applied = false;
                if (applyRenames) {
                    Symbol primary = symbols.getPrimarySymbol(table);
                    if (primary == null || primary.getSource() == SourceType.DEFAULT) {
                        applied = ensurePrimaryLabel(table, container.scope, "vftable", action);
                    }
                    else {
                        applied = ensureLabel(table, container.scope, "vftable", action);
                    }
                }
                finishContainerAction(action, applied, actions);
            }
            containerComment(table, CommentType.PLATE,
                "Serialize container adapter for " + owner + " [" + container.typeId + "]. " +
                "NewWorld 3-26 IDataContainer: 23 slots / 0xb8 bytes. Dump slots 23+ are adjacent tables. " +
                "AZStd objects themselves have no vptr. Shared table/body aliases list every reflected owner.", actions);
            JsonObject element = genericMember(arrayMember(container.genericInfo, "elements"), "element");
            String stride = element == null ? null : stringMember(element, "dataSize");
            for (int slot = 0; slot < CONTAINER_SLOT_NAMES.length; slot++) {
                JsonObject entry = resolveObject(container.slots.get(slot), jsonObjectsById);
                String address = stringMember(entry, "address");
                String description = CONTAINER_SLOT_DESCRIPTIONS[slot];
                containerComment(table.add(slot * 8L), CommentType.EOL,
                    "IDataContainer slot " + slot + ": " + description, actions);
                functionOwners.computeIfAbsent(address, key -> new ArrayList<>()).add(
                    owner + " :: slot " + slot + " " + description +
                    (stride == null ? "" : " ; serialized element bytes=" + stride));
                functionContainers.computeIfAbsent(address, key -> new ArrayList<>()).add(container);
                functionSlots.computeIfAbsent(address, key -> new LinkedHashSet<>()).add(slot);
                if (container.scope != null && CONTAINER_SLOT_NAMES[slot] != null) {
                    JsonObject action = containerAction("container_function_alias", address,
                        fullName(container.scope, CONTAINER_SLOT_NAMES[slot]));
                    boolean applied = applyRenames && ensureLabel(parseCaptureAddress(address),
                        container.scope, CONTAINER_SLOT_NAMES[slot], action);
                    finishContainerAction(action, applied, actions);
                }
            }
        }
        for (Map.Entry<String, ArrayList<String>> entry : functionOwners.entrySet()) {
            monitor.checkCancelled();
            String capturedAddress = entry.getKey();
            Address address = parseCaptureAddress(capturedAddress);
            ArrayList<SerializeContainer> owners = functionContainers.get(capturedAddress);
            Set<Integer> slots = functionSlots.get(capturedAddress);
            String localName = slots.size() == 1 ? CONTAINER_SLOT_NAMES[slots.iterator().next()] : null;
            JsonObject action = containerAction("container_function", capturedAddress,
                localName == null ? "unnamed callback" : localName);
            Function function = currentProgram.getFunctionManager().getFunctionAt(address);
            boolean changed = false;
            boolean created = false;
            if (applyRenames && function == null) {
                function = createMissingFunction(address, localName, action);
                created = function != null;
                changed = created;
            }
            // Keep folded bodies and existing user names. Aliases carry their specialization identities.
            if (applyRenames && function != null && owners.size() == 1 && localName != null &&
                    owners.get(0).scope != null &&
                    (created || function.getSymbol().getSource() == SourceType.DEFAULT)) {
                changed |= applyFunctionRename(function, owners.get(0).scope, localName, action);
            }
            finishContainerAction(action, changed, actions);
            containerComment(address, CommentType.PLATE,
                "Serialize container callback owners, from serialize.json and validated live vtables:\n" +
                String.join("\n", new LinkedHashSet<>(entry.getValue())), actions);
        }
        summary.addProperty("validatedSpecializations", validated);
        summary.addProperty("namedSpecializations", resolved);
        summary.addProperty("uniqueVtables", tables.size());
        summary.addProperty("uniqueCallbacks", functionOwners.size());
        summary.addProperty("unresolvedTypeNames", validated - resolved);
        summary.addProperty("skipped", skipped.size());
        return report;
    }

    private boolean supportsContainerPointerLayout() {
        // Program/project names are editable. The pointer layout and live table checks identify the profile.
        return currentProgram != null && currentProgram.getDefaultPointerSize() == 8 &&
            !currentProgram.getLanguage().isBigEndian();
    }

    private String validateContainerTable(SerializeContainer container) {
        Address table = parseCaptureAddress(container.vtable);
        if (!isProgramAddress(table) || !isProgramAddress(table.add(0xb7))) {
            return "vtable-not-in-program";
        }
        if (container.slots == null || container.slots.size() < CONTAINER_SLOT_NAMES.length) {
            return "incomplete-container-slots";
        }
        for (int slot = 0; slot < CONTAINER_SLOT_NAMES.length; slot++) {
            JsonObject entry = resolveObject(container.slots.get(slot), jsonObjectsById);
            if (entry == null || !Integer.toString(slot).equals(stringMember(entry, "index"))) {
                return "invalid-slot-index-" + slot;
            }
            Address expected = parseCaptureAddress(stringMember(entry, "address"));
            if (!isExecutableAddress(expected) || !expected.equals(readPointer(table.add(slot * 8L)))) {
                return "live-slot-mismatch-" + slot;
            }
        }
        return null;
    }

    private JsonObject containerAction(String kind, String address, String target) {
        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        action.addProperty("address", address);
        action.addProperty("target", target);
        return action;
    }

    private void finishContainerAction(JsonObject action, boolean applied, ActionSink actions) {
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
        }
        actions.add(action);
    }

    private void containerComment(Address address, CommentType type, String text, ActionSink actions) {
        JsonObject action = containerAction("container_comment", address.toString(), text);
        String existing = currentProgram.getListing().getComment(type, address);
        boolean changed = existing == null || !existing.contains(text);
        if (changed && applyRenames) {
            CodeUnit unit = currentProgram.getListing().getCodeUnitAt(address);
            if (pendingComments != null && unit != null) {
                changed = ensureCommentLine(unit, type, text, action);
            }
            else {
                currentProgram.getListing().setComment(address, type, existing == null ? text : existing + "\n" + text);
            }
        }
        action.addProperty("applied", changed && applyRenames);
        action.addProperty("wouldApply", changed && !applyRenames);
        actions.add(action);
    }

    private boolean includeFullActionReport() {
        return optionBool(
            "full-actions",
            "AZOTH_SERIALIZE_RENAME_FULL_ACTIONS",
            true);
    }

    private File chooseInputFileViaPicker() throws Exception {
        GhidraFileChooser chooser = createReflectionResourcesChooser();
        try {
            File selected = chooser.getSelectedFile();
            if (selected == null) throw new CancelledException();
            return selected;
        }
        finally { chooser.dispose(); }
    }

    private GhidraFileChooser createReflectionResourcesChooser() {
        GhidraFileChooser chooser = new GhidraFileChooser(null);
        chooser.setTitle("Select reflection resources folder");
        chooser.setApproveButtonText("Import reflection");
        chooser.setFileSelectionMode(GhidraFileChooserMode.DIRECTORIES_ONLY);
        File lastDirectory = lastInputDirectory();
        if (lastDirectory != null) chooser.setCurrentDirectory(lastDirectory);
        return chooser;
    }

    private File lastInputDirectory() {
        String path;
        try {
            path = Preferences.userRoot()
                .node(PREFS_NODE)
                .get(PREF_LAST_INPUT_DIR, null);
        }
        catch (SecurityException e) {
            println("Unable to read last reflection resources directory: " +
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
            println("Unable to persist last reflection resources directory: " +
                e.getMessage());
        }
    }

    private boolean chooseApplyRenames() {
        return !dryRun && optionBool("apply-renames", "AZOTH_SERIALIZE_RENAME", true);
    }

    private File chooseOutputFile(File input) {
        String explicit = optionValue("out", "AZOTH_SERIALIZE_RENAME_OUT");
        return explicit == null ? new File(input.getParentFile(), "reflection-report.json") : new File(explicit);
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
            try (FileReader reader = new FileReader(file, StandardCharsets.UTF_8)) {
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

    private File requireExplicitInput(String path, String label) {
        File file = new File(path);
        if (!file.exists()) throw new IllegalArgumentException(label + " not found: " + file);
        return file;
    }

    private File moduleEvidenceInput(File serializeInput) {
        String explicit = optionValue("module-evidence", "AZOTH_SERIALIZE_MODULE_EVIDENCE");
        if (explicit != null) {
            return requireExplicitInput(explicit, "Module evidence");
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
        Address stringAddress = stringAddressReturnedBySimpleFunction(address);
        if (stringAddress == null) {
            return null;
        }
        try {
            return readCString(stringAddress);
        }
        catch (Exception ignored) {
            return null;
        }
    }

    private Address stringAddressReturnedBySimpleFunction(Address address) {
        Address target = resolvedThunkTarget(address);
        if (!isProgramAddress(target)) {
            return null;
        }
        try {
            int b0 = unsignedByte(target, 0);
            int b1 = unsignedByte(target, 1);
            int b2 = unsignedByte(target, 2);
            if (b0 == 0x48 && b1 == 0x8d && b2 == 0x05) {
                int displacement = int32(target, 3);
                return absoluteAddress(target.getOffset() + 7L + displacement);
            }
            if (b0 == 0x48 && b1 == 0xb8) {
                return absoluteAddress(int64(target, 2));
            }
            if (b0 == 0xb8) {
                return absoluteAddress(uint32(target, 1));
            }
        }
        catch (Exception ignored) {
            return null;
        }
        return null;
    }

    private Address resolvedThunkTarget(Address address) {
        if (!isExecutableAddress(address)) {
            increment(rejectedCodePointers, address == null ? "null" :
                isProgramAddress(address) ? "non-executable" : "unmapped");
            return null;
        }
        ThisAdjustThunk thunk = thisAdjustThunk(address);
        if (thunk != null) {
            return thunk.targetAddress;
        }

        Address directTarget = directJumpTarget(address);
        return directTarget != null ? directTarget : address;
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

        try (BufferedReader reader = new BufferedReader(new FileReader(input, StandardCharsets.UTF_8))) {
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
                    index.malformedRecords++;
                    index.skippedReasons.add(lineNumber + ":parse-failed");
                    continue;
                }
                if (value == null || !value.isJsonObject()) {
                    index.skippedRecords++;
                    index.malformedRecords++;
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
        String explicit = optionValue(
            "class-registration",
            "AZOTH_SERIALIZE_CLASS_REGISTRATION");
        if (explicit != null) {
            return requireExplicitInput(explicit, "Class registration trace");
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

        try (BufferedReader reader = new BufferedReader(new FileReader(input, StandardCharsets.UTF_8))) {
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
                    index.malformedRecords++;
                    index.skippedReasons.add(lineNumber + ":parse-failed");
                    continue;
                }
                if (value == null || !value.isJsonObject()) {
                    index.skippedRecords++;
                    index.malformedRecords++;
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
        String explicit = optionValue(
            "field-registration",
            "AZOTH_SERIALIZE_FIELD_REGISTRATION");
        if (explicit != null) {
            return requireExplicitInput(explicit, "Field registration trace");
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

    private BehaviorContextEvidence loadBehaviorContextEvidence(File serializeInput) throws Exception {
        if (inputFile == null) inputFile = serializeInput;
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
        boolean readOnDemand = false;
        try (
            Reader source = input.openReader();
            JsonReader reader = new JsonReader(new BufferedReader(source))
        ) {
            reader.beginObject();
            while (reader.hasNext()) {
                monitor.checkCancelled();
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
                    scanBehaviorTypeToClassMap(reader, index);
                }
                else if ((name.equals("onDemandReflection") || name.equals("m_onDemandReflection")) &&
                    !readOnDemand) {
                    readOnDemand = true;
                    scanBehaviorOnDemand(reader, index);
                }
                else if (!Set.of("classes", "m_classes", "methods", "m_methods", "properties",
                    "m_properties", "ebuses", "m_ebuses", "typeToClassMap", "m_typeToClassMap",
                    "onDemandReflection", "m_onDemandReflection").contains(name)) {
                    index.contextMetadata.add(name, compactBehaviorMetadata(JsonParser.parseReader(reader)));
                }
                else {
                    reader.skipValue();
                }
            }
            reader.endObject();
        }
        catch (CancelledException error) {
            throw error;
        }
        catch (ReportStorageException error) {
            throw error;
        }
        catch (Exception error) {
            index.skippedInputs++;
            index.skippedReasons.add(input.description() + ":" + error.getMessage());
        }

        finalizeBehaviorContextEvidence(index);
        return index;
    }

    private BehaviorContextInput findBehaviorContextInput(File serializeInput) {
        String explicit = optionValue("behavior-context", "AZOTH_BEHAVIOR_CONTEXT_JSON");
        if (explicit != null) {
            File file = new File(explicit);
            if (!file.isFile()) throw new IllegalArgumentException("BehaviorContext file not found: " + file);
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
        if (sibling.isFile()) {
            return behaviorContextFileInput(sibling);
        }
        File zipSibling = new File(parent, "behavior-context.json.zip");
        if (zipSibling.isFile()) return behaviorContextFileInput(zipSibling);
        File compressedSibling = new File(parent, "behavior-context.7z");
        return behaviorContextFileInput(compressedSibling);
    }

    private BehaviorContextInput behaviorContextFileInput(File file) {
        if (file == null || !file.exists() || !file.isFile()) {
            return null;
        }
        if (file.getName().toLowerCase(Locale.ROOT).endsWith(".zip")) {
            try (ZipFile archive = new ZipFile(file)) {
                List<? extends ZipEntry> entries = Collections.list(archive.entries());
                for (ZipEntry entry : entries) {
                    String name = entry.getName().replace('\\', '/');
                    if (!entry.isDirectory() && (name.equalsIgnoreCase("behavior-context.json") ||
                            name.toLowerCase(Locale.ROOT).endsWith("/behavior-context.json"))) {
                        return new BehaviorContextInput(file, entry.getName());
                    }
                }
                throw new IllegalArgumentException("behavior-context.json missing from " + file);
            }
            catch (IOException error) {
                throw new IllegalArgumentException("Cannot read BehaviorContext archive: " + file, error);
            }
        }
        if (file.getName().toLowerCase(Locale.ROOT).endsWith(".7z")) {
            String entryName = behaviorContextArchiveEntry(file);
            if (entryName == null) {
                throw new IllegalArgumentException("behavior-context.json missing from " + file);
            }
            return new BehaviorContextInput(file, entryName);
        }
        return new BehaviorContextInput(file);
    }

    private String behaviorContextArchiveEntry(File archiveFile) {
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
            }
        }
        catch (IOException error) {
            throw new IllegalArgumentException("Cannot read BehaviorContext archive: " + archiveFile, error);
        }
        return null;
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
            monitor.checkCancelled();
            JsonObject record = JsonParser.parseReader(reader).getAsJsonObject();
            JsonObject second = objectMemberDirect(record, "second");
            if (second != null) record = second;
            scanBehaviorClass(record, index);
        }
        reader.endArray();
    }

    private JsonElement behaviorMember(JsonObject object, String field) {
        if (object == null) return null;
        JsonElement value = object.get(field);
        if (value == null || value.isJsonNull()) {
            value = object.get(field.startsWith("m_") ? field.substring(2) : "m_" + field);
        }
        return value;
    }

    private JsonObject behaviorObject(JsonObject object, String field) {
        return resolveBehaviorObject(behaviorMember(object, field));
    }

    private JsonArray behaviorArray(JsonObject object, String field) {
        JsonElement value = behaviorMember(object, field);
        return value != null && value.isJsonArray() ? value.getAsJsonArray() : null;
    }

    // Keep native members and all unique decoded evidence without storing alias copies.
    private JsonElement compactBehaviorMetadata(JsonElement value) {
        if (value == null || value.isJsonNull()) return com.google.gson.JsonNull.INSTANCE;
        if (value.isJsonArray()) {
            JsonArray result = new JsonArray();
            for (JsonElement item : value.getAsJsonArray()) result.add(compactBehaviorMetadata(item));
            return result;
        }
        if (!value.isJsonObject()) return value.deepCopy();
        JsonObject object = value.getAsJsonObject();
        JsonObject result = new JsonObject();
        for (Map.Entry<String, JsonElement> entry : object.entrySet()) {
            String key = entry.getKey();
            if (isBehaviorAlias(object, key, entry.getValue())) continue;
            result.add(key, compactBehaviorMetadata(entry.getValue()));
        }
        return result;
    }

    private boolean isBehaviorAlias(JsonObject object, String key, JsonElement value) {
        if (!key.startsWith("m_") && value.equals(object.get("m_" + key))) return true;
        String nativeKey = switch (key) {
            case "implementationFunction" -> "m_functionPtr";
            case "implementationSecondWord" -> "m_functionPtrSecondWord";
            case "implementationWords" -> "m_functionPtrWords";
            default -> null;
        };
        if (nativeKey != null && value.equals(object.get(nativeKey))) return true;
        if (key.equals("second") && (value.equals(object.get("method")) ||
            value.equals(object.get("property")))) return true;
        if (key.equals("functions") && value.isJsonObject()) {
            for (Map.Entry<String, JsonElement> callback : value.getAsJsonObject().entrySet()) {
                if (!callback.getValue().equals(object.get("m_" + callback.getKey()))) return false;
            }
            return true;
        }
        return false;
    }

    private JsonObject behaviorOwnerFacts(JsonObject owner, BehaviorContextEvidence index) {
        JsonObject facts = new JsonObject();
        for (String field : List.of("name", "typeId", "baseClasses", "requestBuses",
            "notificationBuses", "wrappedTypeId", "unwrapperUserData", "idParam",
            "deprecatedName", "toolTip", "attributes", "valueHasher")) {
            JsonElement value = behaviorMember(owner, field);
            if (value != null) facts.add(field, compactBehaviorMetadata(value));
        }
        countBehaviorAttributes(behaviorArray(owner, "m_attributes"), index);
        return facts;
    }

    private void countBehaviorAttributes(JsonArray attributes, BehaviorContextEvidence index) {
        if (attributes == null) return;
        for (JsonElement item : attributes) {
            if (!item.isJsonObject()) continue;
            JsonObject attribute = item.getAsJsonObject();
            String name = stringMemberDirect(attribute, "attributeName");
            if (name == null) name = stringMemberDirect(attribute, "name");
            if (name == null) name = stringMemberDirect(attribute, "attributeId");
            if (name == null) name = stringMemberDirect(attribute, "crc");
            increment(index.attributeCounts, name == null ? "unnamed" : name);
            JsonObject value = objectMemberDirect(attribute, "value");
            if (value != null && !value.has("value") && !value.has("flags")) {
                increment(index.unresolvedCounts, "attribute-value-not-captured");
            }
        }
    }

    private void scanBehaviorTypeToClassMap(JsonReader reader, BehaviorContextEvidence index)
        throws Exception {
        reader.beginArray();
        while (reader.hasNext()) {
            monitor.checkCancelled();
            JsonObject record = JsonParser.parseReader(reader).getAsJsonObject();
            index.typeToClassMapCount++;
            JsonObject link = new JsonObject();
            for (String key : List.of("typeId", "first", "className", "classAddress")) {
                if (record.has(key)) link.add(key, compactBehaviorMetadata(record.get(key)));
            }
            index.typeToClassInventory.add(link);
            JsonObject klass = objectMemberDirect(record, "second");
            if (klass == null) klass = objectMemberDirect(record, "class");
            if (klass == null) {
                increment(index.unresolvedCounts, "type-map-class-not-captured");
                continue;
            }
            String keyId = normalizeTypeId(stringMemberDirect(record, "typeId"));
            if (keyId == null) keyId = normalizeTypeId(stringMemberDirect(record, "first"));
            String classId = normalizeTypeId(behaviorTypeId(klass));
            if (keyId != null && classId != null && !keyId.equals(classId)) {
                index.layoutConflicts.add("BehaviorContext type map UUID mismatch: " + keyId + " vs " + classId);
                continue;
            }
            if (classId == null && keyId != null) klass.addProperty("m_typeId", keyId);
            scanBehaviorClass(klass, index);
        }
        reader.endArray();
    }

    private void scanBehaviorOnDemand(JsonReader reader, BehaviorContextEvidence index)
        throws Exception {
        reader.beginArray();
        while (reader.hasNext()) {
            monitor.checkCancelled();
            JsonObject record = JsonParser.parseReader(reader).getAsJsonObject();
            index.onDemandInventory.add(compactBehaviorMetadata(record));
            String typeId = normalizeTypeId(stringMemberDirect(record, "key"));
            if (typeId == null) typeId = normalizeTypeId(stringMemberDirect(record, "first"));
            String address = stringMemberDirect(record, "value");
            if (address == null) address = stringMemberDirect(record, "second");
            if (typeId == null || !isAddressLike(address)) {
                increment(index.unresolvedCounts, "on-demand-reflector-not-captured");
                continue;
            }
            JsonObject method = new JsonObject();
            method.addProperty("m_functionPtr", address);
            method.addProperty("reflectedTypeId", typeId);
            addBehaviorMethodCandidate(index,
                behaviorScope("AZ", "BehaviorContext", "OnDemandReflection<" + typeId + ">"),
                "behavior.onDemandReflection", "Reflect", method);
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
            index.globalMethodInventory.add(method);
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
            index.globalPropertyInventory.add(property);
            BehaviorConstantProperty constant = new BehaviorConstantProperty();
            constant.name = behaviorName(property, null);
            constant.getter = behaviorFunctionAddress(behaviorObject(property, "m_getter"));
            constant.writable = behaviorObject(property, "m_setter") != null;
            index.globalProperties.add(constant);
            countBehaviorAttributes(behaviorArray(property, "m_attributes"), index);
            String propertyName = safeTypeName(behaviorName(property, "Property"));
            int firstCandidate = index.functionCandidates.size();
            addBehaviorMethodCandidate(
                index,
                behaviorScope("AZ", "BehaviorContext"),
                "global.property.getter",
                behaviorAccessorName("Get", propertyName),
                behaviorObject(property, "m_getter"));
            addBehaviorMethodCandidate(
                index,
                behaviorScope("AZ", "BehaviorContext"),
                "global.property.setter",
                behaviorAccessorName("Set", propertyName),
                behaviorObject(property, "m_setter"));
            attachBehaviorPropertyEvidence(index, firstCandidate, property);
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
        String className = behaviorName(klass, null);
        String identity = normalizeTypeId(behaviorTypeId(klass));
        if (identity == null) identity = stringMemberDirect(klass, "address");
        if (identity != null && !index.seenClasses.add(identity)) return;
        index.classCount++;
        index.classInventory.add(klass);
        String typeId = behaviorTypeId(klass);
        if (typeId != null) {
            String normalizedTypeId = normalizeTypeId(typeId);
            if (normalizedTypeId != null) {
                index.classTypeIds.add(normalizedTypeId);
                if (knownSourceName(className) != null) {
                    index.typeNamesByTypeId.put(normalizedTypeId, className);
                }
                BehaviorClassMetadata metadata = new BehaviorClassMetadata();
                metadata.typeId = normalizedTypeId;
                metadata.name = className;
                metadata.size = parseLayoutInteger(stringMemberDirect(klass, "m_size"));
                if (metadata.size == null) metadata.size = parseLayoutInteger(stringMemberDirect(klass, "size"));
                metadata.alignment = parseLayoutInteger(stringMemberDirect(klass, "m_alignment"));
                if (metadata.alignment == null) metadata.alignment = parseLayoutInteger(stringMemberDirect(klass, "alignment"));
                index.classesByTypeId.put(normalizedTypeId, metadata);
            }
        }
        int firstCandidate = index.functionCandidates.size();
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

        JsonArray constructors = behaviorArray(klass, "m_constructors");
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

        JsonArray methods = behaviorArray(klass, "m_methods");
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

        JsonArray properties = behaviorArray(klass, "m_properties");
        if (properties != null) {
            for (JsonElement value : properties) {
                JsonObject property = behaviorProperty(value);
                if (property == null) {
                    continue;
                }
                index.classPropertyCount++;
                countBehaviorAttributes(behaviorArray(property, "m_attributes"), index);
                String propertyName = safeTypeName(behaviorName(property, "Property"));
                int firstPropertyCandidate = index.functionCandidates.size();
                addBehaviorMethodCandidate(
                    index,
                    scope,
                    "class.property.getter",
                    behaviorAccessorName("Get", propertyName),
                    behaviorObject(property, "m_getter"));
                addBehaviorMethodCandidate(
                    index,
                    scope,
                    "class.property.setter",
                    behaviorAccessorName("Set", propertyName),
                    behaviorObject(property, "m_setter"));
                attachBehaviorPropertyEvidence(index, firstPropertyCandidate, property);
            }
        }

        JsonArray baseClasses = behaviorArray(klass, "m_baseClasses");
        if (baseClasses != null) {
            index.classBaseEdgeCount += baseClasses.size();
        }
        JsonArray requestBuses = behaviorArray(klass, "m_requestBuses");
        if (requestBuses != null) {
            index.classRequestBusEdgeCount += requestBuses.size();
        }
        JsonArray notificationBuses = behaviorArray(klass, "m_notificationBuses");
        if (notificationBuses != null) {
            index.classNotificationBusEdgeCount += notificationBuses.size();
        }
        for (int i = firstCandidate; i < index.functionCandidates.size(); i++) {
            index.functionCandidates.get(i).ownerTypeId = normalizeTypeId(typeId);
        }
        JsonObject facts = behaviorOwnerFacts(klass, index);
        if (normalizeTypeId(typeId) != null) index.classFactsByTypeId.put(normalizeTypeId(typeId), facts);
        for (int i = firstCandidate; i < index.functionCandidates.size(); i++) {
            index.functionCandidates.get(i).ownerMetadata = facts;
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
        index.ebusInventory.add(bus);
        int firstCandidate = index.functionCandidates.size();
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

        JsonArray events = behaviorArray(bus, "m_events");
        Map<String, String> eventNamesByAddress = new HashMap<>();
        if (events != null) {
            for (JsonElement value : events) {
                JsonObject event = behaviorEvent(value);
                if (event == null) {
                    continue;
                }
                index.ebusEventCount++;
                countBehaviorAttributes(behaviorArray(event, "m_attributes"), index);
                String eventName = safeTypeName(behaviorName(event, "Event"));
                int firstEventCandidate = index.functionCandidates.size();
                rememberBehaviorEventName(eventNamesByAddress, event, eventName);
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.broadcast",
                    "Broadcast",
                    behaviorEventFunctionName(eventName),
                    behaviorObject(event, "m_broadcast"));
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.event",
                    "Event",
                    behaviorEventFunctionName(eventName),
                    behaviorObject(event, "m_event"));
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.queueBroadcast",
                    "QueueBroadcast",
                    behaviorEventFunctionName(eventName),
                    behaviorObject(event, "m_queueBroadcast"));
                addBehaviorScopedEventCandidate(
                    index,
                    scope,
                    "ebus.event.queueEvent",
                    "QueueEvent",
                    behaviorEventFunctionName(eventName),
                    behaviorObject(event, "m_queueEvent"));
                attachBehaviorPropertyEvidence(index, firstEventCandidate, event);
            }
        }

        JsonArray virtualProperties = behaviorArray(bus, "m_virtualProperties");
        if (virtualProperties != null) {
            for (JsonElement value : virtualProperties) {
                JsonObject property = behaviorEvent(value);
                if (property == null) {
                    continue;
                }
                index.ebusVirtualPropertyCount++;
                countBehaviorAttributes(behaviorArray(property, "m_attributes"), index);
                String propertyName = safeTypeName(behaviorName(property, "Property"));
                scanBehaviorVirtualProperty(
                    index,
                    scope,
                    busName,
                    propertyName,
                    eventNamesByAddress,
                    behaviorObject(property, "m_getter"),
                    "getter");
                scanBehaviorVirtualProperty(
                    index,
                    scope,
                    busName,
                    propertyName,
                    eventNamesByAddress,
                    behaviorObject(property, "m_setter"),
                    "setter");
            }
        }
        JsonObject facts = behaviorOwnerFacts(bus, index);
        JsonObject idParameter = behaviorObject(bus, "m_idParam");
        if (idParameter != null && behaviorTypeId(idParameter) == null) {
            increment(index.unresolvedCounts, "ebus-id-type-uuid-not-captured");
            facts.addProperty("idTypeResolution", "unresolved-capture-reference");
        }
        JsonArray propertyLinks = new JsonArray();
        if (virtualProperties != null) {
            for (JsonElement value : virtualProperties) {
                JsonObject property = behaviorEvent(value);
                if (property == null) continue;
                JsonObject link = new JsonObject();
                link.addProperty("name", behaviorName(property, null));
                JsonArray attributes = behaviorArray(property, "m_attributes");
                if (attributes != null) link.add("attributes", compactBehaviorMetadata(attributes));
                for (String side : List.of("getter", "setter")) {
                    JsonObject event = behaviorObject(property, "m_" + side);
                    if (event != null) {
                        link.addProperty(side + "Event", behaviorEventNameFor(event, eventNamesByAddress));
                        link.addProperty(side + "Address", stringMemberDirect(event, "address"));
                    }
                }
                propertyLinks.add(link);
            }
        }
        facts.add("virtualProperties", propertyLinks);
        for (int i = firstCandidate; i < index.functionCandidates.size(); i++) {
            index.functionCandidates.get(i).ownerMetadata = facts;
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
            behaviorObject(property, "m_broadcast"));
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".event",
            "Event",
            localName,
            behaviorObject(property, "m_event"));
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".queueBroadcast",
            "QueueBroadcast",
            localName,
            behaviorObject(property, "m_queueBroadcast"));
        addBehaviorScopedEventCandidate(
            index,
            scope,
            "ebus.virtualProperty." + side + ".queueEvent",
            "QueueEvent",
            localName,
            behaviorObject(property, "m_queueEvent"));
    }

    private void attachBehaviorPropertyEvidence(
        BehaviorContextEvidence index, int firstCandidate, JsonObject property) {
        JsonObject facts = new JsonObject();
        facts.addProperty("name", behaviorName(property, null));
        for (String key : List.of("attributes", "deprecatedName", "toolTip", "typeId")) {
            JsonElement value = behaviorMember(property, key);
            if (value != null) facts.add(key, compactBehaviorMetadata(value));
        }
        for (int i = firstCandidate; i < index.functionCandidates.size(); i++) {
            index.functionCandidates.get(i).metadata.add("propertyOrEvent", facts);
        }
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
            behaviorMethodAddress(behaviorObject(event, "m_broadcast")),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(behaviorObject(event, "m_event")),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(behaviorObject(event, "m_queueBroadcast")),
            eventName);
        rememberBehaviorEventAddress(
            eventNamesByAddress,
            behaviorMethodAddress(behaviorObject(event, "m_queueEvent")),
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
            behaviorMethodAddress(behaviorObject(event, "m_broadcast")));
        if (name != null) {
            return name;
        }
        name = behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(behaviorObject(event, "m_event")));
        if (name != null) {
            return name;
        }
        name = behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(behaviorObject(event, "m_queueBroadcast")));
        if (name != null) {
            return name;
        }
        return behaviorEventNameByAddress(
            eventNamesByAddress,
            behaviorMethodAddress(behaviorObject(event, "m_queueEvent")));
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

        JsonObject method = behaviorObject(owner, field);
        JsonObject functions = objectMemberDirect(owner, "functions");
        String normalizedField = field.startsWith("m_") ? field.substring(2) : field;
        if (field.equals("m_ebusHandlerOnDemandReflector")) normalizedField = "handlerReflector";
        if (method == null) method = objectMemberDirect(owner, normalizedField);
        if (method == null && functions != null) method = resolveBehaviorObject(functions.get(normalizedField));
        if (method == null) {
            String address = stringMemberDirect(owner, field);
            if (address == null) address = stringMemberDirect(owner, normalizedField);
            if (address == null && functions != null) address = stringMemberDirect(functions, normalizedField);
            if (isAddressLike(address)) {
                method = new JsonObject();
                method.addProperty("m_functionPtr", address);
            }
        }
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
            increment(index.unresolvedCounts, "method-without-native-function");
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
        candidate.metadata = compactBehaviorMetadata(method).getAsJsonObject();
        countBehaviorAttributes(behaviorArray(method, "m_attributes"), index);
        if (!method.has("m_arguments") && !method.has("arguments") && !method.has("parameters")) {
            increment(index.unresolvedCounts, "method-signature-not-captured");
        }
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
            monitor.checkCancelled();
            if (!group.slots.isEmpty()) {
                processBehaviorVirtualDispatchGroup(group, functionSeen, aliasSeen, actions);
            }
            else if (group.candidates.size() == 1 || group.targetNames.size() == 1) {
                BehaviorFunctionCandidate candidate = group.candidates.get(0);
                renameBehaviorFunction(group, candidate, functionSeen, actions);
                addBehaviorFunctionAliases(group, "behavior_function_alias", null, aliasSeen, actions);
            }
            else {
                addBehaviorSharedFunction(group, actions);
                addBehaviorFunctionAliases(group, "behavior_shared_function_alias", null, aliasSeen, actions);
            }
            applyBehaviorFunctionMetadata(group, aliasSeen, actions);
            if (monitor.isCancelled()) {
                return;
            }
        }
        applyBehaviorClassFacts(index, actions);
        processBehaviorConstants(index, actions);
    }

    private void applyBehaviorClassFacts(BehaviorContextEvidence index, ActionSink actions) {
        for (Map.Entry<String, JsonObject> entry : index.classFactsByTypeId.entrySet()) {
            enrichBehaviorTypeReferences(entry.getValue());
            String name = datatypeNames.get(entry.getKey());
            if (name == null) continue;
            List<String> parts = cppScopeParts(name);
            if (parts.isEmpty()) continue;
            String typeName = parts.get(parts.size() - 1);
            CategoryPath category = new CategoryPath("/" + String.join("/", parts.subList(0, parts.size() - 1)));
            DataType type = dataTypeManager.getDataType(category, typeName);
            if (type == null || !isGeneratedReflectedDatatype(type)) continue;
            String facts = "\nBehaviorContext facts: " + entry.getValue();
            String existing = type.getDescription();
            int oldFacts = existing.indexOf("\nBehaviorContext facts: ");
            String updated = (oldFacts < 0 ? existing : existing.substring(0, oldFacts)) + facts;
            if (existing.equals(updated)) continue;
            JsonObject action = new JsonObject();
            action.addProperty("kind", "behavior_class_metadata");
            action.addProperty("typeId", entry.getKey());
            action.addProperty("datatypePath", type.getPathName());
            action.addProperty("applied", applyRenames);
            if (applyRenames) type.setDescription(updated);
            else {
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
            actions.add(action);
        }
    }

    private void applyBehaviorFunctionMetadata(
        BehaviorFunctionGroup group, Set<String> seen, ActionSink actions) throws Exception {
        for (BehaviorFunctionCandidate candidate : group.candidates) {
            monitor.checkCancelled();
            enrichBehaviorTypeReferences(candidate.ownerMetadata);
            enrichBehaviorTypeReferences(candidate.metadata);
            String name = fullName(candidate.scope, candidate.localName);
            JsonObject evidence = new JsonObject();
            evidence.addProperty("name", name);
            evidence.addProperty("kind", candidate.kind);
            if (candidate.ownerTypeId != null) evidence.addProperty("ownerTypeId", candidate.ownerTypeId);
            if (candidate.ownerMetadata != null) evidence.add("owner", candidate.ownerMetadata);
            if (candidate.metadata != null) evidence.add("method", candidate.metadata);
            String comment = "BehaviorContext evidence: " + evidence;
            String digest = behaviorCommentDigest(comment);
            if (seen.add("metadata|" + addressKey(candidate.address) + "|" + digest)) {
                addBehaviorComment(candidate.address, comment, "behavior-metadata", actions);
            }
            if (candidate.metadata == null) continue;
            ArrayList<String> wrapperScope = behaviorScope("AZ", "BehaviorContext", "Method<" + name + ">");
            String implementationOffset = captureOffset(candidate.address);
            if (implementationOffset != null) {
                addBehaviorEvidenceLabel(candidate.address, wrapperScope,
                    "Implementation<0x" + implementationOffset + ">", "behavior_method_implementation",
                    comment, seen, actions);
            }
            String vtable = stringMemberDirect(candidate.metadata, "vtable");
            addBehaviorEvidenceLabel(vtable, wrapperScope, "vftable", "behavior_method_adapter_vtable",
                comment, seen, actions);
            JsonArray reflectFunctions = behaviorArray(candidate.metadata, "m_reflectFunction");
            if (reflectFunctions != null) {
                for (JsonElement value : reflectFunctions) {
                    if (!value.isJsonPrimitive()) continue;
                    String address = value.getAsString();
                    String offset = captureOffset(address);
                    if (offset == null) continue;
                    addBehaviorEvidenceLabel(address, wrapperScope,
                        "Reflect<0x" + offset + ">",
                        "behavior_method_reflector", comment, seen, actions);
                }
            }
        }
    }

    private void enrichBehaviorTypeReferences(JsonObject object) {
        if (object == null || object.has("resolvedTypes")) return;
        JsonObject resolved = new JsonObject();
        collectBehaviorTypeReferences(object, resolved);
        if (resolved.size() > 0) object.add("resolvedTypes", resolved);
    }

    private void collectBehaviorTypeReferences(JsonElement value, JsonObject resolved) {
        if (value == null || value.isJsonNull()) return;
        if (value.isJsonArray()) {
            for (JsonElement item : value.getAsJsonArray()) collectBehaviorTypeReferences(item, resolved);
        }
        else if (value.isJsonObject()) {
            for (Map.Entry<String, JsonElement> entry : value.getAsJsonObject().entrySet()) {
                if (!entry.getKey().equals("resolvedTypes")) collectBehaviorTypeReferences(entry.getValue(), resolved);
            }
        }
        else if (value.isJsonPrimitive() && value.getAsJsonPrimitive().isString()) {
            String typeId = normalizeTypeId(value.getAsString());
            if ("00000000-0000-0000-0000-000000000000".equals(typeId)) return;
            String name = typeId == null ? null : datatypeNames.get(typeId);
            if (name != null) resolved.addProperty(typeId, name);
        }
    }

    private void addBehaviorEvidenceLabel(
        String jsonAddress, ArrayList<String> scope, String localName, String kind,
        String comment, Set<String> seen, ActionSink actions) throws Exception {
        if (!isAddressLike(jsonAddress)) return;
        String name = fullName(scope, localName);
        if (!seen.add(kind + "|" + addressKey(jsonAddress) + "|" + name)) return;
        Address address = parseCaptureAddress(jsonAddress);
        JsonObject action = new JsonObject();
        action.addProperty("kind", kind);
        action.addProperty("address", jsonAddress);
        action.addProperty("name", name);
        action.addProperty("applied", false);
        if (!isProgramAddress(address) ||
            ((kind.equals("behavior_method_reflector") || kind.equals("behavior_method_implementation")) &&
                !isExecutableAddress(address))) {
            action.addProperty("reason", "address-not-in-program");
        }
        else {
            recordTargetNameAddress(name, jsonAddress, action);
            if (applyRenames) action.addProperty("applied", ensureLabel(address, scope, localName, action));
            else {
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
        }
        actions.add(action);
        if (seen.add("evidence-comment|" + addressKey(jsonAddress) + "|" + behaviorCommentDigest(comment))) {
            addBehaviorComment(jsonAddress, comment, "behavior-metadata", actions);
        }
    }

    private String behaviorCommentDigest(String text) throws Exception {
        byte[] digest = MessageDigest.getInstance("SHA-256").digest(text.getBytes(StandardCharsets.UTF_8));
        return java.util.HexFormat.of().formatHex(digest);
    }

    private void processBehaviorConstants(BehaviorContextEvidence index, ActionSink actions) throws Exception {
        while (index.constantInventory.size() > 0) index.constantInventory.remove(index.constantInventory.size() - 1);
        index.unresolvedCounts.remove("constant-value-not-recovered");
        Map<String, EnumDataType> families = new LinkedHashMap<>();
        for (BehaviorConstantProperty property : index.globalProperties) {
            monitor.checkCancelled();
            if (property.writable) continue;
            String address = property.getter;
            JsonObject entry = new JsonObject();
            String name = property.name;
            entry.addProperty("name", name);
            entry.addProperty("getter", address);
            long[] value = readBehaviorIntegerConstant(address);
            if (name == null || value == null) {
                entry.addProperty("reason", "getter-is-not-a-proven-integer-constant");
                increment(index.unresolvedCounts, "constant-value-not-recovered");
                index.constantInventory.add(entry);
                continue;
            }
            // Property prefixes identify script constant families, not native C++ enum identities.
            int separator = name.lastIndexOf('_');
            String family = safeTypeName(separator > 0 ? name.substring(0, separator) : "GlobalConstants");
            if (family == null) family = "GlobalConstants";
            int width = (int) value[1];
            String familyKey = family + "_" + (width * 8) + "bit";
            EnumDataType type = families.get(familyKey);
            if (type == null) {
                type = new EnumDataType(new CategoryPath("/AZ/BehaviorContext/Constants"), familyKey, width);
                type.setDescription("AZ BehaviorContext recovered constant family; values are verified return-register bits. " +
                    "Property prefix grouping does not establish native enum UUID or storage size.");
                families.put(familyKey, type);
            }
            String member = safeTypeName(name);
            if (member == null) {
                entry.addProperty("reason", "invalid-constant-name");
                index.constantInventory.add(entry);
                continue;
            }
            boolean conflict = false;
            boolean present = false;
            for (String existing : type.getNames()) {
                if (existing.equals(member)) {
                    present = true;
                    conflict = type.getValue(member) != value[0];
                    break;
                }
            }
            if (conflict) {
                entry.addProperty("reason", "constant-name-value-conflict");
                index.constantInventory.add(entry);
                continue;
            }
            if (!present) type.add(member, value[0], "BehaviorContext property " + name + "; getter " + address);
            entry.addProperty("returnBits", "0x" + Long.toUnsignedString(value[0], 16));
            entry.addProperty("returnRegisterBytes", width);
            entry.addProperty("datatypePath", type.getPathName());
            entry.addProperty("identity", "script-property-family; native-enum-identity-unavailable");
            index.constantInventory.add(entry);
            addBehaviorComment(address, "BehaviorContext constant " + name + " = " +
                entry.get("returnBits").getAsString() + "; " + type.getPathName() +
                "; native enum UUID/storage size not captured", "behavior-constant", actions);
        }
        for (EnumDataType type : families.values()) {
            monitor.checkCancelled();
            JsonObject action = new JsonObject();
            action.addProperty("kind", "behavior_constant_enum");
            action.addProperty("datatypePath", type.getPathName());
            action.addProperty("values", type.getCount());
            DataType existing = dataTypeManager.getDataType(type.getCategoryPath(), type.getName());
            boolean applied = false;
            if (existing != null && (!(existing instanceof ghidra.program.model.data.Enum) ||
                existing.getDescription() == null ||
                !existing.getDescription().startsWith("AZ BehaviorContext recovered constant family"))) {
                action.addProperty("reason", "manual-datatype-preserved");
            }
            else if (existing != null && sameBehaviorEnum((ghidra.program.model.data.Enum) existing, type)) {
                action.addProperty("reason", "already-current");
            }
            else if (applyRenames) {
                if (existing instanceof ghidra.program.model.data.Enum) {
                    ((ghidra.program.model.data.Enum) existing).replaceWith(type);
                }
                else dataTypeManager.addDataType(type, DataTypeConflictHandler.DEFAULT_HANDLER);
                applied = true;
            }
            else {
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
            action.addProperty("applied", applied);
            actions.add(action);
        }
    }

    private boolean sameBehaviorEnum(ghidra.program.model.data.Enum left, EnumDataType right) {
        if (left.getLength() != right.getLength() || left.getCount() != right.getCount()) return false;
        for (String name : right.getNames()) {
            boolean found = false;
            for (String other : left.getNames()) if (name.equals(other)) found = true;
            if (!found || left.getValue(name) != right.getValue(name) ||
                !sameText(left.getComment(name), right.getComment(name))) return false;
        }
        return true;
    }

    private long[] readBehaviorIntegerConstant(String jsonAddress) {
        if (jsonAddress == null || currentProgram.getDefaultPointerSize() != 8 ||
            currentProgram.getLanguage().isBigEndian() ||
            !currentProgram.getLanguage().getProcessor().toString().equalsIgnoreCase("x86")) return null;
        try {
            Address address = parseCaptureAddress(jsonAddress);
            if (!isProgramAddress(address) || !isExecutableAddress(address)) return null;
            int opcode = Byte.toUnsignedInt(currentProgram.getMemory().getByte(address));
            if (opcode == 0xb8 && currentProgram.getMemory().getByte(address.add(5)) == (byte) 0xc3) {
                return new long[]{Integer.toUnsignedLong(currentProgram.getMemory().getInt(address.add(1))), 4};
            }
            if ((opcode == 0x31 || opcode == 0x33) &&
                currentProgram.getMemory().getByte(address.add(1)) == (byte) 0xc0 &&
                currentProgram.getMemory().getByte(address.add(2)) == (byte) 0xc3) {
                return new long[]{0, 4};
            }
            if (opcode == 0x48 && currentProgram.getMemory().getByte(address.add(1)) == (byte) 0xb8 &&
                currentProgram.getMemory().getByte(address.add(10)) == (byte) 0xc3) {
                return new long[]{currentProgram.getMemory().getLong(address.add(2)), 8};
            }
            if (opcode == 0x48 && currentProgram.getMemory().getByte(address.add(1)) == (byte) 0xc7 &&
                currentProgram.getMemory().getByte(address.add(2)) == (byte) 0xc0 &&
                currentProgram.getMemory().getByte(address.add(7)) == (byte) 0xc3) {
                return new long[]{currentProgram.getMemory().getInt(address.add(3)), 8};
            }
        }
        catch (Exception ignored) {
            // Dynamic getters and unavailable bytes remain explicit unresolved evidence.
        }
        return null;
    }

    private void processBehaviorVirtualDispatchGroup(
        BehaviorFunctionGroup group,
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        if (group.slots.size() != 1) {
            addBehaviorSharedFunction(group, actions);
            addBehaviorFunctionAliases(group, "behavior_shared_function_alias", null, aliasSeen, actions);
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
        addBehaviorFunctionAliases(group, "behavior_virtual_dispatch_alias", slot, aliasSeen, actions);
    }

    private void addBehaviorFunctionAliases(
        BehaviorFunctionGroup group, String kind, String slot,
        Set<String> aliasSeen, ActionSink actions) throws Exception {
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
            action.addProperty("kind", kind);
            action.addProperty("address", candidate.address);
            action.addProperty("name", targetName);
            action.addProperty("behaviorKind", candidate.kind);
            if (slot != null) action.addProperty("virtualDispatchSlotOffset", slot);
            if (candidate.methodName != null) {
                action.addProperty("methodName", candidate.methodName);
            }
            if (!isProgramAddress(address)) {
                action.addProperty("applied", false);
                action.addProperty("reason", "address-not-in-program");
                actions.add(action);
                continue;
            }
            recordTargetNameAddress(targetName, candidate.address, action);

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

    private ArrayList<Address> behaviorNameStringAddresses(BehaviorFunctionCandidate candidate) throws CancelledException {
        ArrayList<Address> result = new ArrayList<>();
        addStringAddresses(result, candidate.localName);
        addStringAddresses(result, candidate.methodName);
        return result;
    }

    private void addStringAddresses(ArrayList<Address> result, String text) throws CancelledException {
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

        if (owner == null || !isProgramAddress(center) || target == null) return false;
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
        if (owner == null || !isProgramAddress(center)) return null;
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
        if (!isProgramAddress(callsite)) return null;
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
        recordTargetNameAddress(targetName, jsonAddress, action);

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
        recordTargetNameAddress(targetName, candidate.address, action);

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
        if (applyRenames && !functionHasTargetName(function, candidate.scope, candidate.localName)) {
            applied = applyFunctionRename(function, candidate.scope, candidate.localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !functionHasTargetName(function, candidate.scope, candidate.localName)) {
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

        if (pendingComments != null) {
            String key = codeUnit.getAddress() + "|" + commentType;
            PendingComment pending = pendingComments.get(key);
            if (pending == null) {
                pending = new PendingComment(codeUnit.getAddress(), commentType, codeUnit.getComment(commentType));
                pendingComments.put(key, pending);
            }
            if (!pending.lines.add(comment)) {
                action.addProperty("reason", "already-current");
                return false;
            }
            pending.additions.add(comment);
            return true;
        }
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

    private void flushPendingComments() throws CancelledException {
        for (PendingComment pending : pendingComments.values()) {
            monitor.checkCancelled();
            if (pending.additions.isEmpty()) continue;
            CodeUnit unit = currentProgram.getListing().getCodeUnitAt(pending.address);
            if (unit == null) continue;
            String existing = unit.getComment(pending.type);
            LinkedHashSet<String> lines = new LinkedHashSet<>();
            if (existing != null) Collections.addAll(lines, existing.split("\\R"));
            StringBuilder text = new StringBuilder(existing == null ? "" : existing);
            for (String addition : pending.additions) {
                if (!lines.add(addition)) continue;
                if (text.length() > 0) text.append('\n');
                text.append(addition);
            }
            String merged = text.toString();
            if (!sameText(existing, merged)) unit.setComment(pending.type, merged);
        }
        pendingComments.clear();
    }

    private static class PendingComment {
        final Address address;
        final CommentType type;
        final Set<String> lines = new HashSet<>();
        final ArrayList<String> additions = new ArrayList<>();

        PendingComment(Address address, CommentType type, String existing) {
            this.address = address;
            this.type = type;
            if (existing != null) Collections.addAll(lines, existing.split("\\R"));
        }
    }

    private JsonObject behaviorMethod(JsonElement value) {
        JsonObject object = resolveBehaviorObject(value);
        if (object == null) {
            return null;
        }
        JsonObject method = objectMemberDirect(object, "method");
        if (method != null) {
            return behaviorNamedRecord(method, object);
        }
        JsonObject second = objectMemberDirect(object, "second");
        if (second != null) {
            return behaviorNamedRecord(second, object);
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
            return behaviorNamedRecord(property, object);
        }
        JsonObject second = objectMemberDirect(object, "second");
        if (second != null) {
            return behaviorNamedRecord(second, object);
        }
        return object;
    }

    private JsonObject behaviorEvent(JsonElement value) {
        JsonObject object = resolveBehaviorObject(value);
        if (object == null) {
            return null;
        }
        // "event" is also the normalized per-ID dispatch method, not always a map wrapper.
        if (behaviorObject(object, "m_broadcast") != null ||
            behaviorObject(object, "m_queueBroadcast") != null ||
            behaviorObject(object, "m_queueEvent") != null ||
            object.has("m_event")) return object;
        JsonObject event = objectMemberDirect(object, "event");
        if (event != null && behaviorFunctionAddress(event) != null) return object;
        if (event == null) event = objectMemberDirect(object, "second");
        return event == null ? object : behaviorNamedRecord(event, object);
    }

    private JsonObject behaviorNamedRecord(JsonObject record, JsonObject wrapper) {
        if (behaviorName(record, null) == null && behaviorName(wrapper, null) != null) {
            record.addProperty("name", behaviorName(wrapper, null));
        }
        return record;
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
        scope.addAll(cppScopeParts(value));
        if (scope.isEmpty()) {
            scope.add("BehaviorContext");
        }
        registerReflectedClassScope(scope);
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
        List<String> parts = cppScopeParts(name);
        if (!parts.isEmpty()) name = parts.get(parts.size() - 1);
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
        report.addProperty("onDemandReflectors", index.onDemandInventory.size());
        report.add("unresolvedMetadata", mapToJson(index.unresolvedCounts));
        report.add("attributes", mapToJson(index.attributeCounts));
        report.add("layoutConflicts", stringArray(index.layoutConflicts));
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
        int addressOutsideProgramCount = countReason(stats, "address-not-in-program");
        int functionMissingCount = countReason(stats, "function-missing");
        int functionCreateFailedCount = countReason(stats, "function-create-failed");
        int dryRunCount = countReason(stats, "dry-run");

        StringBuilder summary = new StringBuilder();
        summary.append("AZ reflection import ");
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
        summary.append("\n  Unresolved source-name identities: ")
            .append(scan.unresolvedCollidingTypeNames.size());
        summary.append("\n  Datatype paths separated by UUID: ").append(datatypeConflictingPaths.size());
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
        appendActionExamples(summary, actions, "address-not-in-program", "Outside-program addresses skipped");
        appendActionExamples(
            summary,
            actions,
            "shared-behavior-function",
            "Shared BehaviorContext implementations with owner aliases");
        appendActionExamples(
            summary,
            actions,
            "behavior-virtual-dispatch",
            "BehaviorContext virtual dispatch comments");

        summary.append("\n\nRead:");
        if (!applyRenames) {
            summary.append("\n  This did not mutate the program.");
            summary.append("\n  Apply is OK for non-conflicting actions.");
            summary.append("\n  Identical qualified symbol names are allowed at different addresses.");
            summary.append("\n  ambiguous source type names are reported with their UUID sets.");
            summary.append("\n  Datatypes sharing a source name use separate __typeid__/UUID categories.");
            summary.append("\n  function-missing entries are expected in dry run; apply mode tries to create them first.");
        }
        else {
            summary.append("\n  Non-conflicting actions were applied.");
            summary.append("\n  Identical qualified symbol names were retained at different addresses.");
            summary.append("\n  address-not-in-program entries were skipped.");
            summary.append("\n  function-create-failed entries include clear/disassemble diagnostics.");
            summary.append("\n  ambiguous source type names were not given synthetic suffixes.");
            summary.append("\n  Datatypes sharing a source name use separate __typeid__/UUID categories.");
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
        copyActionProperty(action, sample, "detail");
        copyActionProperty(action, sample, "error");
        copyActionProperty(action, sample, "errorType");
        copyActionProperty(action, sample, "signatureChecked");
        copyActionProperty(action, sample, "callingConvention");
        copyActionProperty(action, sample, "automaticParameters");
        copyActionProperty(action, sample, "plannedFormalParameters");
        copyActionProperty(action, sample, "plannedParameterStorage");
        copyActionProperty(action, sample, "parameterNameAdjustments");
        copyActionProperty(action, sample, "customStoragePreserved");
        copyActionProperty(action, sample, "materializedAutomaticThis");
        copyActionProperty(action, sample, "thisTypeManagedByClassNamespace");
        copyActionProperty(action, sample, "requestedThisType");
        copyActionProperty(action, sample, "storageRepair");
        copyActionProperty(action, sample, "storageDetail");
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

    private String optionValue(String option, String environment) {
        return cli.value(option, environment);
    }

    private boolean optionBool(String option, String environment, boolean defaultValue) {
        return cli.flag(option, environment, defaultValue);
    }

    private void printHelp() {
        println("AzReflectionRenamer " + SCRIPT_VERSION);
        println("Import SerializeContext, BehaviorContext and modules; TypeRegistry is excluded.");
        println("");
        println("Options:");
        println("  --resources <DIR>             Discover reflection captures [env: AZOTH_REFLECTION_RESOURCES]");
        println("  --serialize-json <FILE>       Alternate SerializeContext file; discover siblings [env: AZOTH_SERIALIZE_JSON]");
        println("  --module-evidence <PATH>      Module evidence file or directory [env: AZOTH_SERIALIZE_MODULE_EVIDENCE]");
        println("  --class-registration <FILE>   Class-registration trace [env: AZOTH_SERIALIZE_CLASS_REGISTRATION]");
        println("  --field-registration <FILE>   Field-registration trace [env: AZOTH_SERIALIZE_FIELD_REGISTRATION]");
        println("  --behavior-context <FILE>     BehaviorContext JSON or archive [env: AZOTH_BEHAVIOR_CONTEXT_JSON]");
        println("  --out <FILE>                  Default: resources/reflection-report.json [env: AZOTH_SERIALIZE_RENAME_OUT]");
        println("  --apply-renames[=<BOOL>]      Default: true [env: AZOTH_SERIALIZE_RENAME]");
        println("  --full-actions[=<BOOL>]       Include every action [env: AZOTH_SERIALIZE_RENAME_FULL_ACTIONS]");
        println("  Capture files are optional individually; BehaviorContext JSON/ZIP/7z and modules are discovered.");
        println("  --force[=<BOOL>]              Replace report, default: true");
        println("  --dry-run                     Validate without program edits; save *-dry-run.json beside --out");
        println("  Validation runs automatically before applying. Any validation errors block program edits.");
        println("  -h, --help                    Print help");
        println("  -V, --version                 Print version");
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
        datatypeGenerics = genericInfoByTypeId;
        datatypeNames = rawTypeNamesByTypeId;
        enumUnderlyingTypes.clear();
        JsonObject enumMap = objectMember(root.getAsJsonObject(), "enumTypeIdToUnderlyingTypeIdMap");
        if (enumMap != null) {
            for (Map.Entry<String, JsonElement> entry : enumMap.entrySet()) {
                enumUnderlyingTypes.put(normalizeTypeId(entry.getKey()), normalizeTypeId(stringElement(entry.getValue())));
            }
        }
        nativeLayoutProfile = matchesNativeLayoutProfile();
        indexReflectedClassScopes(genericInfoByTypeId, rawTypeNamesByTypeId);

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
        appendAdditionalReflectionTypes(classData, rawTypeNamesByTypeId);

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

    private void appendAdditionalReflectionTypes(List<ClassData> classData, Map<String, String> names) {
        Set<String> existing = new HashSet<>();
        for (ClassData data : classData) existing.add(normalizeTypeId(data.typeId));
        if (behaviorContextEvidence != null) {
            for (BehaviorClassMetadata metadata : behaviorContextEvidence.classesByTypeId.values()) {
                appendReflectionType(classData, existing, metadata.typeId,
                    knownTypeName(metadata.typeId, metadata.name, names), "behavior-context");
            }
        }
        if (moduleEvidence != null) {
            for (Descriptor descriptor : moduleEvidence.descriptors) {
                String id = descriptorTypeId(descriptor);
                appendReflectionType(classData, existing, id,
                    knownTypeName(id, null, names), "module-descriptor");
            }
        }
    }

    private void appendReflectionType(List<ClassData> classData, Set<String> existing,
            String id, String name, String source) {
        if (id == null || knownSourceName(name) == null || !existing.add(id)) return;
        ClassData data = new ClassData();
        data.typeId = id;
        data.typeName = name;
        data.reflectionSource = source;
        data.objects = new LinkedHashMap<>();
        data.callbacks = new LinkedHashMap<>();
        data.elements = new ArrayList<>();
        classData.add(data);
    }

    private void reconcileBehaviorTypeScopes(List<ClassData> classData) {
        Map<String, ClassData> canonical = new LinkedHashMap<>();
        for (ClassData data : canonicalClassDataByTypeId(classData)) canonical.put(normalizeTypeId(data.typeId), data);
        for (BehaviorFunctionCandidate candidate : behaviorContextEvidence.functionCandidates) {
            ClassData owner = canonical.get(candidate.ownerTypeId);
            DatatypeTarget target = owner == null ? null : datatypeTarget(owner);
            if (target != null) candidate.scope = new ArrayList<>(target.scope);
            if (candidate.kind.equals("behavior.onDemandReflection")) {
                String typeId = stringMemberDirect(candidate.metadata, "reflectedTypeId");
                String typeName = datatypeNames.get(typeId);
                if (typeName != null) candidate.scope =
                    behaviorScope("AZ", "BehaviorContext", "OnDemandReflection<" + typeName + ">");
            }
        }
        for (BehaviorFunctionGroup group : behaviorContextEvidence.groupsByAddress.values()) {
            group.targetNames.clear();
            for (BehaviorFunctionCandidate candidate : group.candidates) {
                group.targetNames.add(fullName(candidate.scope, candidate.localName));
            }
        }
        behaviorContextEvidence.safeDirectFunctionCount = 0;
        behaviorContextEvidence.sharedFunctionGroupCount = 0;
        behaviorContextEvidence.virtualDispatchGroupCount = 0;
        behaviorContextEvidence.duplicateFunctionAddressGroupCount = 0;
        finalizeBehaviorContextEvidence(behaviorContextEvidence);
        for (RttiType type : behaviorContextEvidence.rttiTypes.values()) {
            ClassData owner = canonical.get(normalizeTypeId(type.typeId));
            if (owner != null) {
                type.typeName = owner.typeName;
                type.targetTypeName = owner.targetTypeName;
                type.ownerScope = owner.ownerScope;
            }
        }
    }

    private JsonArray reflectedTypesReport(List<ClassData> classData) {
        JsonArray result = new JsonArray();
        for (ClassData data : canonicalClassDataByTypeId(classData)) {
            JsonObject record = new JsonObject();
            record.addProperty("typeId", data.typeId);
            DatatypeTarget target = datatypeTarget(data);
            record.addProperty("name", target == null ? data.typeName : String.join("::", target.scope));
            if (target != null) record.addProperty("datatypePath", datatypePathName(target));
            record.addProperty("size", reflectedLayoutSize(data));
            record.addProperty("sizeEvidence", typeEvidence.nativeDataSizesByTypeId.containsKey(normalizeTypeId(data.typeId)) ?
                "captured-native-size" : nativeProfileSize(data) > 0 ? "verified-native-profile" : reflectedLayoutSize(data) > 0 ? "serialized-field-extent" : "unknown");
            JsonArray sources = new JsonArray();
            sources.add(data.reflectionSource == null ? "serialize-context" : data.reflectionSource);
            if (behaviorContextEvidence.classesByTypeId.containsKey(normalizeTypeId(data.typeId)) &&
                    !"behavior-context".equals(data.reflectionSource)) sources.add("behavior-context");
            if (moduleEvidence.ownersByTypeId.containsKey(normalizeTypeId(data.typeId)) &&
                    !"module-descriptor".equals(data.reflectionSource)) sources.add("module-descriptor");
            record.add("sources", sources);
            record.addProperty("reflectedFieldCount", reflectedLayoutFieldCount(data));
            record.addProperty("layoutCoverage", reflectedLayoutFieldCount(data) > 0 ? "reflected-subset" : "opaque");
            BehaviorClassMetadata behavior = behaviorContextEvidence.classesByTypeId.get(normalizeTypeId(data.typeId));
            if (behavior != null) {
                record.addProperty("behaviorExposedName", behavior.name);
                record.addProperty("behaviorNativeSize", behavior.size);
                record.addProperty("behaviorAlignment", behavior.alignment);
            }
            boolean ambiguous = target != null && datatypeConflictingPaths.contains(target.canonicalPathName);
            record.addProperty("identityAmbiguous", ambiguous);
            if (ambiguous) {
                record.addProperty("canonicalDatatypePath", target.canonicalPathName);
                record.addProperty("identityResolution", "uuid-specific-datatype");
            }
            DataType stored = target == null ? null : dataTypeManager.getDataType(target.categoryPath, target.name);
            record.addProperty("layoutInProgram", stored != null);
            if (stored instanceof Structure) {
                Structure structure = (Structure) stored;
                if (!isGeneratedReflectedDatatype(structure)) record.addProperty("layoutCoverage", "preserved-user-layout");
                else if (structure.isZeroLength()) record.addProperty("layoutCoverage", "incomplete-declaration");
                else if (structure.getDescription().contains("NewWorld 3-26 native ABI, verified callback offsets")) {
                    record.addProperty("layoutCoverage", "verified-native-profile");
                }
                record.addProperty("programSize", structure.isZeroLength() ? 0 : structure.getLength());
                record.addProperty("programAlignment", structure.getAlignment());
                JsonArray fields = new JsonArray();
                for (DataTypeComponent component : structure.getDefinedComponents()) {
                    JsonObject field = new JsonObject();
                    field.addProperty("name", component.getFieldName());
                    field.addProperty("offset", component.getOffset());
                    field.addProperty("length", component.getLength());
                    field.addProperty("type", component.getDataType().getPathName());
                    fields.add(field);
                }
                record.add("definedFields", fields);
            }
            result.add(record);
        }
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
        // Generic elements prove serialized value sizes, not container object offsets.
        for (JsonObject generic : datatypeGenerics.values()) {
            JsonArray elements = arrayMember(generic, "elements");
            if (elements == null) continue;
            for (JsonElement value : elements) {
                JsonObject member = resolveObject(value, jsonObjectsById);
                if (member == null || nativeElementIsPointer(member)) continue;
                ElementData element = new ElementData();
                element.dataSize = stringMember(member, "dataSize");
                rememberNativeDataSize(evidence, stringMember(member, "typeId"), element);
            }
        }
        if (behaviorContextEvidence != null) {
            for (BehaviorClassMetadata metadata : behaviorContextEvidence.classesByTypeId.values()) {
                if (metadata.size == null || metadata.size <= 0 || metadata.size > 16 * 1024 * 1024) continue;
                Integer existing = evidence.nativeDataSizesByTypeId.get(metadata.typeId);
                if (existing == null || existing.equals(metadata.size)) {
                    evidence.nativeDataSizesByTypeId.put(metadata.typeId, metadata.size);
                }
                else {
                    behaviorContextEvidence.layoutConflicts.add(metadata.typeId + ": SerializeContext size=" +
                        existing + ", BehaviorContext size=" + metadata.size);
                }
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
        List<String> parts = cppScopeParts(targetTypeName);
        return !parts.isEmpty() && scope != null && scope.size() >= parts.size() &&
            scope.subList(scope.size() - parts.size(), scope.size()).equals(parts);
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
        String qualified = String.join("::", scope);
        List<String> source = balancedCppDelimiters(qualified) ? cppScopeParts(qualified) : scope;
        for (String part : source) {
            for (String normalizedPart : cppScopeParts(part)) {
                if (!result.isEmpty() && result.get(result.size() - 1).equals(normalizedPart)) {
                    continue;
                }
                result.add(normalizedPart);
            }
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
        if (dataTypeManager != null) {
            Iterator<DataType> existingTypes = dataTypeManager.getAllDataTypes();
            while (existingTypes.hasNext()) {
                DataType type = existingTypes.next();
                if (!isGeneratedReflectedDatatype(type)) continue;
                Matcher identity = UUID_RE.matcher(type.getDescription());
                if (!identity.find()) continue;
                String category = type.getCategoryPath().getPath();
                String name = category.equals("/") ? type.getName() : category.substring(1).replace("/", "::") + "::" + type.getName();
                String known = result.get(normalizeTypeId(identity.group()));
                if (known == null || (!known.contains("::") && name.contains("::"))) {
                    rememberRawTypeName(result, identity.group(), name);
                }
            }
        }
        if (behaviorContextEvidence != null) {
            for (Map.Entry<String, String> entry : behaviorContextEvidence.typeNamesByTypeId.entrySet()) {
                String existing = result.get(entry.getKey());
                if (existing == null || (!existing.contains("::") && entry.getValue().contains("::"))) {
                    rememberRawTypeName(result, entry.getKey(), entry.getValue());
                }
            }
        }
        if (moduleEvidence != null) {
            for (Descriptor descriptor : moduleEvidence.descriptors) {
                String name = descriptor.azRtti == null ? null : descriptor.azRtti.typeName;
                if (knownSourceName(name) == null) name = descriptor.componentName;
                rememberRawTypeName(result, descriptorTypeId(descriptor), name);
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
        registerReflectedClassScope(cppScopeParts(candidate));
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
        // Smart pointers reflect their stored pointer, but their template argument is the pointee.
        if (Set.of("AZStd::shared_ptr", "AZStd::intrusive_ptr", "AZStd::unique_ptr").contains(baseName)
                && arguments.size() == 1 && arguments.get(0).endsWith("*")) {
            arguments.set(0, arguments.get(0).substring(0, arguments.get(0).length() - 1));
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
                return memberPointerTypeName(member, genericName);
            }
        }

        String typeId = stringMember(member, "typeId");
        return memberPointerTypeName(member, displayTypeName(
            typeId,
            rawTypeName(typeId, rawTypeNamesByTypeId),
            genericInfoByTypeId,
            rawTypeNamesByTypeId,
            visitingTypeIds));
    }

    private String memberPointerTypeName(JsonObject member, String name) {
        JsonElement pointer = member.get("is_pointer");
        JsonElement flags = member.get("flags");
        boolean isPointer = pointer != null && !pointer.isJsonNull() ? pointer.getAsBoolean() :
            flags != null && !flags.isJsonNull() && (flags.getAsInt() & 1) != 0;
        return name == null ? null : name + (isPointer ? "*" : "");
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
        String rawName = rawTypeName(typeId, rawTypeNamesByTypeId);
        if (knownSourceName(rawName) != null && rawName.contains("::") &&
            (knownSourceName == null || !knownSourceName.contains("::")) && balancedCppDelimiters(rawName)) {
            return rawName;
        }
        if (knownSourceName != null) {
            return knownSourceName;
        }
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
        List<String> parts = cppScopeParts(typeName);
        return parts.isEmpty() ? null : String.join("::", parts);
    }

    private ArrayList<String> cppScopeParts(String name) {
        ArrayList<String> parts = new ArrayList<>();
        if (name == null || name.trim().isEmpty()) return parts;
        String value = name.trim().replaceAll("\\b(?:class|struct|enum)\\s+", "");
        while (value.startsWith("::")) value = value.substring(2);
        if (value.isEmpty()) return parts;
        // Operator angle brackets are punctuation, not template delimiters.
        Map<String, String> operators = new LinkedHashMap<>();
        Matcher operatorMatcher = ANGLE_OPERATOR_RE.matcher(value);
        StringBuffer protectedValue = new StringBuffer();
        int tokenNumber = 0;
        while (operatorMatcher.find()) {
            String token;
            do { token = "__az_reflection_operator_" + tokenNumber++ + "__"; }
            while (value.contains(token));
            operators.put(token, "operator" + operatorMatcher.group(1));
            operatorMatcher.appendReplacement(protectedValue, token);
        }
        operatorMatcher.appendTail(protectedValue);
        if (!balancedCppDelimiters(protectedValue.toString())) {
            // Some runtime names are truncated by the game's fixed-size name buffer.
            // Keep the observed name atomic; Ghidra otherwise falls back to splitting every ::.
            unparsedTypeNames.add(name);
            parts.add(safeTypeName(value));
            return parts;
        }
        // Ghidra keeps :: inside templates and function signatures within one path entry.
        for (String part : SymbolPathParser.parse(protectedValue.toString(), false)) {
            for (Map.Entry<String, String> operator : operators.entrySet()) {
                part = part.replace(operator.getKey(), operator.getValue());
            }
            String safe = safeTypeName(part);
            if (safe != null) parts.add(safe);
        }
        return parts;
    }

    private boolean balancedCppDelimiters(String name) {
        int angles = 0;
        int parentheses = 0;
        for (int i = 0; i < name.length(); i++) {
            switch (name.charAt(i)) {
                case '<': angles++; break;
                case '>': if (--angles < 0) return false; break;
                case '(': parentheses++; break;
                case ')': if (--parentheses < 0) return false; break;
                default: break;
            }
        }
        return angles == 0 && parentheses == 0;
    }

    private void registerReflectedClassScope(List<String> parts) {
        if (parts.isEmpty()) return;
        reflectedClassScopePaths.add(String.join("::", parts));
        for (int i = 1; i < parts.size(); i++) {
            String prefix = String.join("::", parts.subList(0, i));
            if (parts.get(i - 1).contains("<")) reflectedClassScopePaths.add(prefix);
            else reflectedNamespaceScopePaths.add(prefix);
        }
    }

    private void indexReflectedClassScopes(Map<String, JsonObject> generics, Map<String, String> names) {
        for (Map.Entry<String, JsonObject> entry : generics.entrySet()) {
            String name = displayTypeName(entry.getKey(), rawTypeName(entry.getKey(), names),
                generics, names, new HashSet<>());
            registerReflectedClassScope(cppScopeParts(name));
        }
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

    private void processInferredStaticRttiHelperVtables(
        Set<String> functionSeen,
        Set<String> aliasSeen,
        ActionSink actions) throws Exception {

        ArrayList<InferredStaticRttiHelper> helpers = inferredStaticRttiHelpers();
        if (helpers.isEmpty()) {
            return;
        }

        Map<String, SlotGroup> groups =
            inferredStaticRttiSlotGroups(helpers, INFERRED_STATIC_RTTI_SLOT_NAMES);
        for (InferredStaticRttiHelper helper : helpers) {
            List<String> scope = inferredStaticRttiScope(helper.typeName);
            renameLabel(
                formatAddress(helper.vtableAddress),
                scope,
                "vftable",
                "inferred_static_rtti_vftable",
                actions);

            for (int slot = 0; slot < INFERRED_STATIC_RTTI_SLOT_NAMES.length; slot++) {
                Address slotPointer = readPointer(helper.vtableAddress.add(slot * 8L));
                Address bodyAddress = resolvedThunkTarget(slotPointer);
                if (!isProgramAddress(bodyAddress) || !isExecutableAddress(bodyAddress)) {
                    continue;
                }
                renameFunction(
                    "inferred_static_rtti_function",
                    formatAddress(bodyAddress),
                    scope,
                    INFERRED_STATIC_RTTI_SLOT_NAMES[slot],
                    groups,
                    functionSeen,
                    aliasSeen,
                    actions);
            }
        }
    }

    private ArrayList<InferredStaticRttiHelper> inferredStaticRttiHelpers() throws CancelledException {
        ArrayList<InferredStaticRttiHelper> result = new ArrayList<>();
        Set<String> seenVtables = new HashSet<>();
        for (Map.Entry<String, ArrayList<Address>> entry :
            definedStringAddressesByValue().entrySet()) {
            monitor.checkCancelled();
            String typeName = safeTypeName(entry.getKey());
            if (!isPlausibleRttiTypeName(typeName)) {
                continue;
            }

            for (Address stringAddress : entry.getValue()) {
                InferredStaticRttiHelper helper =
                    inferredStaticRttiHelperForString(typeName, stringAddress);
                if (helper == null) {
                    continue;
                }
                String key = helper.vtableAddress.toString();
                if (seenVtables.add(key)) {
                    result.add(helper);
                }
            }
        }
        return result;
    }

    private InferredStaticRttiHelper inferredStaticRttiHelperForString(
        String typeName,
        Address stringAddress) {

        if (!isProgramAddress(stringAddress)) return null;
        for (long delta = 8; delta <= 0x80; delta += 8) {
            Address vtableAddress = absoluteAddress(stringAddress.getOffset() - delta);
            if (!isProgramAddress(vtableAddress)) {
                continue;
            }
            if (moduleInstanceVtableAddresses.contains(vtableAddress.toString())) {
                continue;
            }
            if (!isInferredStaticRttiHelperVtable(vtableAddress, stringAddress)) {
                continue;
            }

            InferredStaticRttiHelper helper = new InferredStaticRttiHelper();
            helper.typeName = typeName;
            helper.typeNameAddress = stringAddress;
            helper.vtableAddress = vtableAddress;
            return helper;
        }
        return null;
    }

    private boolean isInferredStaticRttiHelperVtable(
        Address vtableAddress,
        Address typeNameAddress) {

        if (!isProgramAddress(vtableAddress) || !isProgramAddress(typeNameAddress)) return false;
        Address getTypeName = readPointer(vtableAddress.add(8));
        if (!isExecutableAddress(getTypeName)) {
            return false;
        }
        Address returnedString = stringAddressReturnedBySimpleFunction(getTypeName);
        if (returnedString == null || !returnedString.equals(typeNameAddress)) {
            return false;
        }

        int executableSlots = 0;
        for (int slot = 0; slot < INFERRED_STATIC_RTTI_SLOT_NAMES.length; slot++) {
            Address slotPointer = readPointer(vtableAddress.add(slot * 8L));
            Address bodyAddress = resolvedThunkTarget(slotPointer);
            if (isExecutableAddress(bodyAddress)) {
                executableSlots++;
            }
        }
        return executableSlots >= 4;
    }

    private boolean isPlausibleRttiTypeName(String value) {
        if (value == null || value.length() < 3) {
            return false;
        }
        if (value.startsWith("{") || value.indexOf('/') >= 0 || value.indexOf('\\') >= 0) {
            return false;
        }
        return value.indexOf(' ') < 0 || value.indexOf('<') >= 0;
    }

    private List<String> inferredStaticRttiScope(String typeName) {
        ArrayList<String> result = new ArrayList<>();
        result.add("AZ");
        result.add("Internal");
        result.add("RttiHelper<" + typeName + ">");
        return result;
    }

    private Map<String, SlotGroup> inferredStaticRttiSlotGroups(
        List<InferredStaticRttiHelper> helpers,
        String[] slotNames) {

        Map<String, SlotGroup> result = new HashMap<>();
        for (InferredStaticRttiHelper helper : helpers) {
            for (int slot = 0; slot < slotNames.length; slot++) {
                Address slotPointer = readPointer(helper.vtableAddress.add(slot * 8L));
                Address bodyAddress = resolvedThunkTarget(slotPointer);
                if (!isProgramAddress(bodyAddress)) {
                    continue;
                }
                String address = formatAddress(bodyAddress);
                String key = addressKey(address);
                SlotGroup group = result.get(key);
                if (group == null) {
                    group = new SlotGroup(address);
                    result.put(key, group);
                }
                group.slotNames.add(slotNames[slot]);
                group.useCount++;
            }
        }
        return result;
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

    private void processSerializeFactoryVtables(List<ClassData> classData, ActionSink actions) throws Exception {
        // SerializeContext.h IObjectFactory: destructor, Create(const char*), Destroy(void*).
        // These are dispatch-interface aliases; the concrete factory may be shared or custom.
        List<String> scope = List.of("AZ", "SerializeContext", "IObjectFactory");
        String[] names = {null, "Create", "Destroy"};
        Set<Address> seenObjects = new HashSet<>(), seenTables = new HashSet<>();
        Set<String> seenFunctions = new HashSet<>();
        for (ClassData data : classData) {
            monitor.checkCancelled();
            String captured = data.objects.get("factory");
            if (captured == null) continue;
            Address object = parseCaptureAddress(captured);
            if (object == null || !seenObjects.add(object)) continue;
            Address table = readPointer(object);
            JsonObject action = new JsonObject();
            action.addProperty("kind", "serialize_factory_vtable");
            action.addProperty("address", captured);
            action.addProperty("typeId", data.typeId);
            action.addProperty("owner", data.targetTypeName);
            action.addProperty("interface", String.join("::", scope));
            Address[] slots = new Address[3];
            String reason = null;
            if (!supportsContainerPointerLayout()) reason = "unsupported-factory-pointer-layout";
            else if (!isProgramAddress(object) || isExecutableAddress(object) || !isProgramAddress(table) ||
                isExecutableAddress(table) || (table.getOffset() & 7) != 0) reason = "factory-vtable-not-in-program";
            else {
                for (int slot = 0; slot < slots.length; slot++) {
                    slots[slot] = readPointer(table.add(slot * 8));
                    if (!isExecutableAddress(slots[slot])) { reason = "factory-vtable-invalid-slot-" + slot; break; }
                    Function containing = currentProgram.getFunctionManager().getFunctionContaining(slots[slot]);
                    Instruction instruction = currentProgram.getListing().getInstructionContaining(slots[slot]);
                    if ((containing != null && !containing.getEntryPoint().equals(slots[slot])) ||
                        (instruction != null && !instruction.getAddress().equals(slots[slot]))) {
                        reason = "factory-vtable-slot-not-function-entry-" + slot;
                        break;
                    }
                }
            }
            if (reason != null) {
                action.addProperty("reason", reason);
                action.addProperty("applied", false);
                actions.add(action);
                continue;
            }
            if (!seenTables.add(table)) continue;
            action.addProperty("vtable", formatAddress(table));
            action.addProperty("reason", "validated-factory-interface; concrete-type-unresolved");
            action.addProperty("applied", false);
            JsonArray slotAddresses = new JsonArray();
            for (Address slot : slots) slotAddresses.add(formatAddress(slot));
            action.add("slots", slotAddresses);
            actions.add(action);
            registerReflectedClassScope(scope);
            for (int slot = 1; slot < slots.length; slot++) {
                String address = formatAddress(slots[slot]);
                if (!seenFunctions.add(address + "|" + names[slot])) continue;
                JsonObject functionAction = new JsonObject();
                functionAction.addProperty("kind", "serialize_factory_function_alias");
                functionAction.addProperty("address", address);
                functionAction.addProperty("name", fullName(scope, names[slot]));
                functionAction.addProperty("vtable", formatAddress(table));
                functionAction.addProperty("slot", slot);
                functionAction.addProperty("evidence", "captured IObjectFactory pointer; source-order interface; validated native entries");
                Function function = currentProgram.getFunctionManager().getFunctionAt(slots[slot]);
                if (function == null && applyRenames) function = createMissingFunction(slots[slot], "FUN_" + slots[slot], functionAction);
                boolean applied = false;
                if (applyRenames && function == null) functionAction.addProperty("reason", "function-create-failed");
                else if (applyRenames) applied = ensureLabel(slots[slot], scope, names[slot], functionAction);
                functionAction.addProperty("applied", applied);
                functionAction.addProperty("wouldApply", !applyRenames);
                actions.add(functionAction);
            }
        }
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
            recordTargetNameAddress(targetName, address, action);

            boolean applied = false;
            if (applyRenames) {
                applied = applyFunctionRename(function, normalizedScope, localName, action);
                if (!applied && action.has("reason")) {
                    action.addProperty("applied", false);
                    actions.add(action);
                    continue;
                }
            }
            action.addProperty("applied", applied);
            if (!applyRenames) {
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
            actions.add(action);
        }
        cleanupTemplateLabelNamespaces(actions);
        pruneEmptyTemplateScopeFragments(actions);
    }

    private void processMbGetTypeNameFunctions(
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        Map<String, SlotGroup> groups = new HashMap<>();
        Set<String> seen = new HashSet<>();
        for (var entry : reflectionStrings().mbTypeNames.entrySet()) {
            monitor.checkCancelled();
            String typeName = serializedTypeName(entry.getKey());
            if (typeName == null) continue;
            for (Address stringAddress : entry.getValue()) {
                processMbGetTypeNameReferences(
                    stringAddress,
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
        if (!data.hasStringValue()) return null;
        Object value = data.getValue();
        return value instanceof String ? (String)value : null;
    }

    private static final class ReflectionStrings {
        final Map<String, ArrayList<Address>> byValue = new LinkedHashMap<>();
        final Map<String, ArrayList<Address>> mbTypeNames = new LinkedHashMap<>();
        final Map<String, ArrayList<Address>> coreTypeIds = new LinkedHashMap<>();
    }

    private Map<String, ArrayList<Address>> definedStringAddressesByValue() throws CancelledException {
        return reflectionStrings().byValue;
    }

    private ReflectionStrings reflectionStrings() throws CancelledException {
        monitor.checkCancelled();
        if (reflectionStrings != null) return reflectionStrings;

        ReflectionStrings index = new ReflectionStrings();
        Set<String> coreTypeIds = coreRttiCastTargetsByTypeId().keySet();
        DataIterator iterator = currentProgram.getListing().getDefinedData(true);
        while (iterator.hasNext()) {
            monitor.checkCancelled();
            Data data = iterator.next();
            String text = dataStringValue(data);
            if (text == null || text.isEmpty()) continue;
            Address address = data.getAddress();
            if (text.length() <= 512) indexStringAddress(index.byValue, text, address);
            if (text.contains("MB::GetTypeName<")) {
                Matcher matcher = MB_GET_TYPE_NAME_SIGNATURE_RE.matcher(text);
                while (matcher.find()) {
                    monitor.checkCancelled();
                    indexStringAddress(index.mbTypeNames, matcher.group("type"), address);
                }
            }
            Matcher matcher = UUID_RE.matcher(text);
            while (matcher.find()) {
                monitor.checkCancelled();
                String typeId = normalizeTypeId(matcher.group());
                if (coreTypeIds.contains(typeId)) indexStringAddress(index.coreTypeIds, typeId, address);
            }
        }
        monitor.checkCancelled();
        reflectionStrings = index;
        return index;
    }

    private static void indexStringAddress(Map<String, ArrayList<Address>> index, String key, Address address) {
        index.computeIfAbsent(key, ignored -> new ArrayList<>()).add(address);
    }

    private void processCoreRttiCastHelpers(
        Set<String> functionSeen,
        ActionSink actions) throws Exception {

        Map<String, CoreRttiCastTarget> targets = coreRttiCastTargetsByTypeId();
        Set<String> seen = new HashSet<>();
        for (var entry : reflectionStrings().coreTypeIds.entrySet()) {
            monitor.checkCancelled();
            CoreRttiCastTarget target = targets.get(entry.getKey());
            for (Address stringAddress : entry.getValue()) {
                processCoreRttiCastReferences(
                    stringAddress,
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

        recordTargetNameAddress(targetName, address, action);

        boolean renamed = false;
        boolean prototypeApplied = false;
        if (applyRenames) {
            if (!functionHasTargetName(function, scope, localName)) {
                renamed = applyFunctionRename(function, scope, localName, action);
                if (!renamed && action.has("reason")) {
                    action.addProperty("applied", false);
                    actions.add(action);
                    return;
                }
            }
        }
        prototypeApplied = applyCoreRttiCastPrototype(function, target, action);

        action.addProperty("applied", applyRenames && (renamed || prototypeApplied));
        action.addProperty("prototypeApplied", applyRenames && prototypeApplied);
        if (!applyRenames && (!functionHasTargetName(function, scope, localName) || prototypeApplied)) {
            action.addProperty("wouldApply", true);
            if (!action.has("reason") || "already-current".equals(stringMember(action, "reason"))) {
                action.addProperty("reason", "dry-run");
            }
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
            return updateReflectionSignature(function, new String[] { "context" },
                new DataType[] { reflectContextPointer }, returnPointer, action);
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
            moduleInstanceVtableAddresses.add(candidate.vtableAddress.toString());
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
                moduleInstanceVtableAddresses.add(candidate.vtableAddress.toString());
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
        String cacheKey = functionCacheKey(function, limit);
        if (calledFunctionsCache.containsKey(cacheKey)) {
            return calledFunctionsCache.get(cacheKey);
        }

        ArrayList<Function> result = new ArrayList<>();
        Set<String> seen = new HashSet<>();
        if (function == null || function.getBody() == null) {
            calledFunctionsCache.put(cacheKey, result);
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
                    calledFunctionsCache.put(cacheKey, result);
                    return result;
                }
            }
        }
        calledFunctionsCache.put(cacheKey, result);
        return result;
    }

    private List<InstanceVtableCandidate> instanceVtableCandidates(
        Function constructor,
        int limit) {

        String cacheKey = functionCacheKey(constructor, limit);
        if (instanceVtableCandidatesCache.containsKey(cacheKey)) {
            return instanceVtableCandidatesCache.get(cacheKey);
        }

        ArrayList<InstanceVtableCandidate> result = new ArrayList<>();
        Set<String> seen = new HashSet<>();
        if (constructor == null || constructor.getBody() == null) {
            instanceVtableCandidatesCache.put(cacheKey, result);
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
                    instanceVtableCandidatesCache.put(cacheKey, result);
                    return result;
                }
            }
        }
        instanceVtableCandidatesCache.put(cacheKey, result);
        return result;
    }

    private String functionCacheKey(Function function, int limit) {
        if (function == null || function.getEntryPoint() == null) {
            return "null|" + limit;
        }
        return function.getEntryPoint().toString() + "|" + limit;
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
                String baseRegister = memoryBaseRegister(instruction, 0);
                Integer baseOffset = thisAliasOffsetBefore(function, instruction, baseRegister);
                return baseOffset == null ? offset : baseOffset + offset;
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

    private Integer thisAliasOffsetBefore(
        Function function,
        Instruction beforeInstruction,
        String registerName) {

        if (registerName == null) {
            return null;
        }
        HashMap<String, Integer> aliases = new HashMap<>();
        aliases.put("RCX", 0);

        for (Instruction instruction :
            currentProgram.getListing().getInstructions(function.getBody(), true)) {
            if (instruction.getAddress().equals(beforeInstruction.getAddress())) {
                return aliases.get(canonicalRegisterName(registerName));
            }
            updateThisAliases(instruction, aliases);
        }
        return null;
    }

    private void updateThisAliases(Instruction instruction, Map<String, Integer> aliases) {
        String mnemonic = instruction.getMnemonicString();
        if (mnemonic == null) {
            return;
        }
        String upperMnemonic = mnemonic.toUpperCase(Locale.ROOT);
        if ("CALL".equals(upperMnemonic)) {
            removeVolatileRegisterAliases(aliases);
            return;
        }

        String destination = registerOperand(instruction, 0);
        if (destination == null) {
            return;
        }

        if ("MOV".equals(upperMnemonic)) {
            String source = registerOperand(instruction, 1);
            if (source != null && aliases.containsKey(source)) {
                aliases.put(destination, aliases.get(source));
            }
            else {
                aliases.remove(destination);
            }
        }
        else if ("LEA".equals(upperMnemonic)) {
            Integer sourceOffset = memoryThisAliasOffset(instruction, 1, aliases);
            if (sourceOffset != null) {
                aliases.put(destination, sourceOffset);
            }
            else {
                aliases.remove(destination);
            }
        }
        else if (writesRegister(instruction, destination)) {
            aliases.remove(destination);
        }
    }

    private void removeVolatileRegisterAliases(Map<String, Integer> aliases) {
        aliases.remove("RAX");
        aliases.remove("RCX");
        aliases.remove("RDX");
        aliases.remove("R8");
        aliases.remove("R9");
        aliases.remove("R10");
        aliases.remove("R11");
    }

    private boolean writesRegister(Instruction instruction, String destination) {
        if (destination == null) {
            return false;
        }
        String mnemonic = instruction.getMnemonicString();
        if (mnemonic == null) {
            return false;
        }
        String upperMnemonic = mnemonic.toUpperCase(Locale.ROOT);
        return "XOR".equals(upperMnemonic) ||
            "SUB".equals(upperMnemonic) ||
            "ADD".equals(upperMnemonic) ||
            "INC".equals(upperMnemonic) ||
            "DEC".equals(upperMnemonic) ||
            "POP".equals(upperMnemonic);
    }

    private Integer memoryThisAliasOffset(
        Instruction instruction,
        int operandIndex,
        Map<String, Integer> aliases) {

        String baseRegister = memoryBaseRegister(instruction, operandIndex);
        if (baseRegister == null || !aliases.containsKey(baseRegister)) {
            return null;
        }
        return aliases.get(baseRegister) + memoryDisplacement(instruction, operandIndex);
    }

    private String memoryBaseRegister(Instruction instruction, int operandIndex) {
        Object[] objects = operandObjects(instruction, operandIndex);
        for (Object object : objects) {
            if (object instanceof Register register) {
                String name = canonicalRegisterName(register.getName());
                if (!"RIP".equals(name) && !"RSP".equals(name)) {
                    return name;
                }
            }
        }
        return null;
    }

    private int memoryDisplacement(Instruction instruction, int operandIndex) {
        int displacement = 0;
        Object[] objects = operandObjects(instruction, operandIndex);
        for (Object object : objects) {
            if (object instanceof Scalar scalar) {
                long value = scalar.getSignedValue();
                if (value >= Integer.MIN_VALUE && value <= Integer.MAX_VALUE) {
                    displacement += (int)value;
                }
            }
        }
        return displacement;
    }

    private String registerOperand(Instruction instruction, int operandIndex) {
        Object[] objects = operandObjects(instruction, operandIndex);
        if (objects.length != 1 || !(objects[0] instanceof Register register)) {
            return null;
        }
        return canonicalRegisterName(register.getName());
    }

    private Object[] operandObjects(Instruction instruction, int operandIndex) {
        try {
            return instruction.getOpObjects(operandIndex);
        }
        catch (Exception ignored) {
            return new Object[0];
        }
    }

    private String canonicalRegisterName(String name) {
        if (name == null) {
            return null;
        }
        String upperName = name.toUpperCase(Locale.ROOT);
        if (upperName.length() == 2 && upperName.charAt(1) == 'X') {
            return "R" + upperName;
        }
        if (upperName.length() == 3 && upperName.charAt(0) == 'E') {
            return "R" + upperName.substring(1);
        }
        if (upperName.equals("DIL") || upperName.equals("EDI")) {
            return "RDI";
        }
        if (upperName.equals("SIL") || upperName.equals("ESI")) {
            return "RSI";
        }
        if (upperName.equals("BPL") || upperName.equals("EBP")) {
            return "RBP";
        }
        if (upperName.equals("SPL") || upperName.equals("ESP")) {
            return "RSP";
        }
        return upperName;
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

        List<String> componentScope = moduleDescriptorComponentScope(descriptor, names);
        InstanceVtableName vtableName = moduleDescriptorInstanceVtableName(candidate);
        String jsonAddress = formatAddress(candidate.vtableAddress);

        JsonObject action = new JsonObject();
        action.addProperty("kind", "module_descriptor_instance_vftable");
        action.addProperty("address", jsonAddress);
        action.addProperty("componentName", descriptorComponentName(descriptor));
        action.addProperty("componentUuid", descriptor.componentUuid);
        action.addProperty("createComponent", createComponentAddress);
        action.addProperty("constructor", formatAddress(constructor.getEntryPoint()));
        action.addProperty("sourceInstruction", formatAddress(candidate.sourceInstruction));
        action.addProperty("vptrOffset", "0x" + Integer.toHexString(candidate.vptrOffset));
        addModuleDescriptorEvidence(action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(candidate.vtableAddress, action);
        }

        if (vtableName == null) {
            boolean cleaned = false;
            if (applyRenames) {
                cleaned = removeStaleGeneratedVtableLabels(
                    candidate.vtableAddress,
                    instanceVtableCleanupScopes(descriptor, names, componentScope),
                    Collections.emptyList(),
                    null,
                    action);
            }
            action.addProperty("applied", cleaned);
            action.addProperty("reason", "semantic-vtable-unresolved");
            actions.add(action);
            processAzRttiInstanceVtableSlots(componentScope, candidate, actions);
            return;
        }

        List<String> scope = appendScope(componentScope, vtableName.extraScope);
        String targetName = fullName(scope, vtableName.localName);
        action.addProperty("name", targetName);
        action.addProperty("semanticVtable", vtableName.semanticName);
        recordTargetNameAddress(targetName, jsonAddress, action);

        boolean applied = false;
        if (applyRenames) {
            if (!vtableName.extraScope.isEmpty()) {
                createScope(componentScope);
            }
            boolean cleaned = removeStaleGeneratedVtableLabels(
                candidate.vtableAddress,
                instanceVtableCleanupScopes(descriptor, names, componentScope),
                scope,
                vtableName.localName,
                action);
            applied = ensurePrimaryLabel(
                candidate.vtableAddress,
                scope,
                vtableName.localName,
                action) || cleaned;
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
        processAzRttiInstanceVtableSlots(componentScope, candidate, actions);
        processKnownInterfaceVtableSlots(componentScope, vtableName, candidate, actions);
    }

    private void processAzRttiInstanceVtableSlots(
        List<String> componentScope,
        InstanceVtableCandidate candidate,
        ActionSink actions) throws Exception {

        if (!isAzRttiInstanceVtable(candidate.vtableAddress)) {
            return;
        }

        List<String> rttiScope = azRttiInstanceScope(componentScope);
        for (int slot = 0; slot < AZ_RTTI_INSTANCE_SLOT_NAMES.length; slot++) {
            Address slotPointer = readPointer(candidate.vtableAddress.add(slot * 8L));
            if (!isProgramAddress(slotPointer) || !isExecutableAddress(slotPointer)) {
                continue;
            }

            ThisAdjustThunk thunk = thisAdjustThunk(slotPointer);
            String localName = AZ_RTTI_INSTANCE_SLOT_NAMES[slot];
            Address bodyAddress = thunk == null ? slotPointer : thunk.targetAddress;
            if (isProgramAddress(bodyAddress) && isExecutableAddress(bodyAddress)) {
                addAzRttiInstanceFunctionAlias(bodyAddress, rttiScope, localName, actions);
            }
            if (thunk != null) {
                renameAzRttiInstanceAdjustorThunk(
                    thunk,
                    rttiScope,
                    localName,
                    actions);
            }
        }
    }

    private void processKnownInterfaceVtableSlots(
        List<String> componentScope,
        InstanceVtableName vtableName,
        InstanceVtableCandidate candidate,
        ActionSink actions) throws Exception {

        if (vtableName == null || !"AZ::TickEvents".equals(vtableName.semanticName)) {
            return;
        }

        List<String> interfaceScope = appendScope(componentScope, vtableName.extraScope);
        processEmbeddedAzRttiInstanceSlots(interfaceScope, candidate, 3, actions);
    }

    private void processEmbeddedAzRttiInstanceSlots(
        List<String> scope,
        InstanceVtableCandidate candidate,
        int startSlot,
        ActionSink actions) throws Exception {

        if (!embeddedAzRttiInstanceSlotsLookValid(candidate.vtableAddress, startSlot)) {
            return;
        }

        for (int slot = 0; slot < AZ_RTTI_INSTANCE_SLOT_NAMES.length; slot++) {
            Address slotPointer = readPointer(candidate.vtableAddress.add((startSlot + slot) * 8L));
            if (!isProgramAddress(slotPointer) || !isExecutableAddress(slotPointer)) {
                continue;
            }

            String localName = AZ_RTTI_INSTANCE_SLOT_NAMES[slot];
            ThisAdjustThunk thunk = thisAdjustThunk(slotPointer);
            if (thunk != null) {
                renameAzRttiInstanceAdjustorThunk(thunk, scope, localName, actions);
            }
            else {
                addAzRttiInstanceFunctionAlias(slotPointer, scope, localName, actions);
            }
        }
    }

    private boolean embeddedAzRttiInstanceSlotsLookValid(Address vtableAddress, int startSlot) {
        Address typeSlot = readPointer(vtableAddress.add(startSlot * 8L));
        Address nameSlot = readPointer(vtableAddress.add((startSlot + 1L) * 8L));
        if (!isExecutableAddress(typeSlot) || !isExecutableAddress(nameSlot)) {
            return false;
        }

        Address nameTarget = resolvedThunkTarget(nameSlot);
        return stringAddressReturnedBySimpleFunction(nameTarget) != null;
    }

    private List<String> azRttiInstanceScope(List<String> componentScope) {
        return componentScope;
    }

    private boolean isAzRttiInstanceVtable(Address vtableAddress) {
        return azRttiSlotResolvesTo(
                vtableAddress,
                0,
                Set.of("GetUuid", "GetType", "RTTI_GetType")) &&
            azRttiSlotResolvesTo(
                vtableAddress,
                1,
                Set.of("GetName", "GetTypeName", "RTTI_GetTypeName")) &&
            azRttiSlotResolvesTo(
                vtableAddress,
                2,
                Set.of("IsTypeOf", "RTTI_IsTypeOf"));
    }

    private boolean azRttiSlotResolvesTo(
        Address vtableAddress,
        int slot,
        Set<String> expectedNames) {

        Address slotPointer = readPointer(vtableAddress.add(slot * 8L));
        if (!isProgramAddress(slotPointer) || !isExecutableAddress(slotPointer)) {
            return false;
        }
        ThisAdjustThunk thunk = thisAdjustThunk(slotPointer);
        Address target = thunk == null ? slotPointer : thunk.targetAddress;
        if (!isProgramAddress(target) || !isExecutableAddress(target)) {
            return false;
        }
        for (Symbol symbol : symbolsAt(target)) {
            if (expectedNames.contains(symbol.getName())) {
                return true;
            }
        }
        return false;
    }

    private void addAzRttiInstanceFunctionAlias(
        Address address,
        List<String> scope,
        String localName,
        ActionSink actions) throws Exception {

        String jsonAddress = formatAddress(address);
        String targetName = fullName(scope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (labelSeen.contains(key)) {
            return;
        }
        labelSeen.add(key);

        JsonObject action = new JsonObject();
        action.addProperty("kind", "instance_rtti_function_alias");
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);
        addModuleDescriptorEvidence(action);
        recordTargetNameAddress(targetName, jsonAddress, action);
        if (actions.keepsFullActions()) {
            actions.captureOldNames(address, action);
        }
        boolean applied = false;
        if (applyRenames) {
            Function function = currentProgram.getFunctionManager().getFunctionAt(address);
            if (function == null) {
                function = createMissingFunction(address, localName, action);
            }
            if (function == null) {
                action.addProperty("applied", false);
                action.addProperty("reason", "function-create-failed");
                actions.add(action);
                return;
            }
            applied = ensureLabel(address, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private void renameAzRttiInstanceAdjustorThunk(
        ThisAdjustThunk thunk,
        List<String> scope,
        String localName,
        ActionSink actions) throws Exception {

        String jsonAddress = formatAddress(thunk.address);
        String targetName = fullName(scope, localName);
        String key = addressKey(jsonAddress) + "|" + targetName;
        if (labelSeen.contains(key)) {
            return;
        }
        labelSeen.add(key);

        JsonObject action = new JsonObject();
        action.addProperty("kind", "instance_rtti_adjustor_thunk");
        action.addProperty("address", jsonAddress);
        action.addProperty("name", targetName);
        action.addProperty("target", formatAddress(thunk.targetAddress));
        action.addProperty("thisAdjustment", "0x" + Integer.toHexString(thunk.adjustment));
        action.addProperty("duplicateNameAllowed", true);
        addModuleDescriptorEvidence(action);
        if (actions.keepsFullActions()) {
            actions.captureOldNames(thunk.address, action);
        }

        boolean applied = false;
        if (applyRenames) {
            Function function = currentProgram.getFunctionManager().getFunctionAt(thunk.address);
            boolean created = false;
            if (function == null) {
                function = createMissingFunction(thunk.address, localName, action);
                created = function != null;
            }
            action.addProperty("created", created);
            if (function == null) {
                action.addProperty("applied", false);
                action.addProperty("reason", "function-create-failed");
                actions.add(action);
                return;
            }
            action.addProperty("oldName", function.getName(true));
            if (!functionHasTargetName(function, scope, localName)) {
                applied = applyFunctionRename(function, scope, localName, action);
            }
        }
        action.addProperty("applied", applied);
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
        }
        actions.add(action);
    }

    private InstanceVtableName moduleDescriptorInstanceVtableName(
        InstanceVtableCandidate candidate) {

        if (candidate.vptrOffset == 0) {
            InstanceVtableName result = new InstanceVtableName();
            result.localName = "vftable";
            result.semanticName = "primary";
            return result;
        }

        if (isTickEventsVtable(candidate.vtableAddress)) {
            InstanceVtableName result = new InstanceVtableName();
            result.extraScope = List.of("TickEvents");
            result.localName = "vftable";
            result.semanticName = "AZ::TickEvents";
            return result;
        }

        return null;
    }

    private boolean isTickEventsVtable(Address address) {
        Address getTickOrder = readPointer(address.add(0x10));
        return isTickOrderGetDefault(getTickOrder);
    }

    private boolean isTickOrderGetDefault(Address address) {
        if (address == null) {
            return false;
        }
        SymbolIterator symbolIterator = symbols.getSymbolsAsIterator(address);
        while (symbolIterator.hasNext()) {
            Symbol symbol = symbolIterator.next();
            String fullName = symbol.getName(true);
            if (fullName.equals("TickOrder::Get_Default") ||
                fullName.endsWith("::TickOrder::Get_Default")) {
                return true;
            }
            if (fullName.contains("TickOrder") && fullName.endsWith("Get_Default")) {
                return true;
            }
        }
        return false;
    }

    private List<String> appendScope(List<String> scope, List<String> extraScope) {
        if (extraScope == null || extraScope.isEmpty()) {
            return scope;
        }
        ArrayList<String> result = new ArrayList<>(scope);
        result.addAll(extraScope);
        return result;
    }

    private ArrayList<List<String>> instanceVtableCleanupScopes(
        Descriptor descriptor,
        DescriptorNames names,
        List<String> componentScope) {

        ArrayList<List<String>> scopes = new ArrayList<>();
        addUniqueScope(scopes, componentScope);

        String componentName = descriptorComponentName(descriptor);
        if (componentName != null && !componentName.isEmpty()) {
            addUniqueScope(scopes, List.of(componentName));
        }
        else if (names != null && names.scope != null && !names.scope.isEmpty()) {
            addUniqueScope(scopes, names.scope);
        }

        return scopes;
    }

    private void addUniqueScope(ArrayList<List<String>> scopes, List<String> scope) {
        if (scope == null || scope.isEmpty()) {
            return;
        }
        for (List<String> existing : scopes) {
            if (existing.equals(scope)) {
                return;
            }
        }
        scopes.add(new ArrayList<>(scope));
    }

    private boolean removeStaleGeneratedVtableLabels(
        Address address,
        List<List<String>> cleanupScopes,
        List<String> keepScope,
        String keepLocalName,
        JsonObject action) throws Exception {

        JsonArray removed = new JsonArray();
        for (List<String> cleanupScope : cleanupScopes) {
            Namespace namespace = findScope(cleanupScope);
            if (namespace == null) {
                continue;
            }
            ArrayList<Symbol> staleSymbols = new ArrayList<>();
            SymbolIterator symbolIterator = symbols.getSymbolsAsIterator(address);
            while (symbolIterator.hasNext()) {
                Symbol symbol = symbolIterator.next();
                if (!symbol.getParentNamespace().equals(namespace) ||
                    !symbol.getSymbolType().equals(SymbolType.LABEL) ||
                    !isGeneratedVtableLabel(symbol)) {
                    continue;
                }
                if (keepLocalName != null &&
                    symbol.getName().equals(keepLocalName) &&
                    cleanupScope.equals(keepScope)) {
                    continue;
                }
                staleSymbols.add(symbol);
            }

            for (Symbol symbol : staleSymbols) {
                String oldName = symbol.getName(true);
                if (symbol.delete()) {
                    removed.add(oldName);
                }
            }
        }
        if (removed.size() > 0) {
            action.add("removedStaleLabels", removed);
            return true;
        }
        return false;
    }

    private boolean isGeneratedVtableLabel(Symbol symbol) {
        if (!symbol.getSource().equals(SourceType.USER_DEFINED)) {
            return false;
        }
        String name = symbol.getName();
        return name.equals("vftable") ||
            name.startsWith("vftable_at_0x") ||
            name.endsWith("_vftable");
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
        recordTargetNameAddress(targetName, jsonAddress, action);

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
        if (applyRenames && !functionHasTargetName(function, targetScope, targetLocal)) {
            applied = applyFunctionRename(function, targetScope, targetLocal, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !functionHasTargetName(function, targetScope, targetLocal)) {
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
        recordTargetNameAddress(targetName, address, action);
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

    private Address directJumpTarget(Address address) {
        if (!isExecutableAddress(address)) return null;
        Instruction instruction = currentProgram.getListing().getInstructionAt(address);
        if (instruction == null || !isDirectJump(instruction)) {
            return null;
        }
        Address[] flows = instruction.getFlows();
        if (flows == null || flows.length != 1 || !isExecutableAddress(flows[0])) {
            return null;
        }
        return flows[0];
    }

    private ThisAdjustThunk thisAdjustThunk(Address address) {
        if (!isExecutableAddress(address)) return null;
        Instruction first = currentProgram.getListing().getInstructionAt(address);
        Integer adjustment = subRcxImmediate(first);
        if (adjustment == null) {
            return null;
        }

        Instruction second = first.getNext();
        if (second == null || !isDirectJump(second)) {
            return null;
        }
        Address[] flows = second.getFlows();
        if (flows == null || flows.length != 1 || !isExecutableAddress(flows[0])) {
            return null;
        }

        ThisAdjustThunk thunk = new ThisAdjustThunk();
        thunk.address = address;
        thunk.targetAddress = flows[0];
        thunk.adjustment = adjustment;
        return thunk;
    }

    private Integer subRcxImmediate(Instruction instruction) {
        if (instruction == null) {
            return null;
        }
        byte[] bytes = instructionBytes(instruction);
        if (bytes.length == 4 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x83 &&
            unsignedByte(bytes[2]) == 0xe9) {
            return signedByte(bytes[3]);
        }
        if (bytes.length == 7 &&
            unsignedByte(bytes[0]) == 0x48 &&
            unsignedByte(bytes[1]) == 0x81 &&
            unsignedByte(bytes[2]) == 0xe9) {
            return int32(bytes, 3);
        }
        return null;
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
        recordTargetNameAddress(targetName, jsonAddress, action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(address, action);
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
        recordTargetNameAddress(targetName, jsonAddress, action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(address, action);
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

    private Map<String, Structure> processReflectedDatatypes(
        List<ClassData> classData,
        ActionSink actions) throws Exception {

        List<ClassData> canonicalClassData = canonicalClassDataByTypeId(classData);
        Map<String, Set<String>> ownersByPath = new LinkedHashMap<>();
        datatypeConflictingPaths.clear();
        for (ClassData data : canonicalClassData) {
            DatatypeTarget target = datatypeTarget(data);
            if (target != null) {
                ownersByPath.computeIfAbsent(datatypePathName(target), key -> new HashSet<>()).add(normalizeTypeId(data.typeId));
                rememberSplitTemplateScopes(target.scope);
            }
        }
        ownersByPath.forEach((path, owners) -> { if (owners.size() > 1) datatypeConflictingPaths.add(path); });
        LinkedHashMap<String, Structure> declaredStructures = new LinkedHashMap<>();
        repairQualifiedReflectionNames(actions);
        for (ClassData data : canonicalClassData) {
            monitor.checkCancelled();
            String typeId = normalizeTypeId(data.typeId);
            if (typeId == null || declaredStructures.containsKey(typeId)) continue;
            DatatypeTarget target = datatypeTarget(data);
            DataType before = target == null ? null : dataTypeManager.getDataType(target.categoryPath, target.name);
            int beforeSize = before == null ? 0 : before.getLength();
            int beforeAlignment = before == null ? 0 : before.getAlignment();
            Structure structure = declareReflectedDatatype(data);
            if (structure != null) {
                declaredStructures.put(typeId, structure);
                if (before == null || structure.getLength() > beforeSize || structure.getAlignment() != beforeAlignment) {
                    JsonObject action = new JsonObject();
                    action.addProperty("kind", "datatype_declaration");
                    action.addProperty("ownerTypeId", data.typeId);
                    action.addProperty("name", data.targetTypeName);
                    action.addProperty("datatypePath", datatypePathName(target));
                    if (datatypeConflictingPaths.contains(target.canonicalPathName)) {
                        action.addProperty("identityResolution", "uuid-specific-datatype");
                        action.addProperty("canonicalDatatypePath", target.canonicalPathName);
                    }
                    action.addProperty("layoutSize", structure.getLength());
                    action.addProperty("applied", applyRenames);
                    action.addProperty("wouldApply", !applyRenames);
                    actions.add(action);
                }
            }
        }

        Map<String, Structure> previousDeclaredStructures = declaredStructuresByTypeId;
        declaredStructuresByTypeId = declaredStructures;
        try {
            monitor.initialize(canonicalClassData.size());
            for (ClassData data : canonicalClassData) {
                monitor.checkCancelled();
                monitor.setMessage("Datatype layout: " + data.targetTypeName);
                DatatypeTarget target = datatypeTarget(data);
                if (target != null && datatypeConflictingPaths.contains(datatypePathName(target))) {
                    JsonObject action = new JsonObject();
                    action.addProperty("kind", "datatype_structure");
                    action.addProperty("ownerTypeId", data.typeId);
                    action.addProperty("name", data.targetTypeName);
                    action.addProperty("applied", false);
                    action.addProperty("reason", "ambiguous-datatype-path; distinct-UUIDs-preserved");
                    actions.add(action);
                }
                else if (!processNativeDatatype(data, actions)) processReflectedDatatype(data, actions);
                monitor.incrementProgress(1);
                monitor.checkCancelled();
            }
        }
        finally {
            declaredStructuresByTypeId = previousDeclaredStructures;
        }
        return declaredStructures;
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
        if (existing instanceof Structure && !isCoreReflectionDatatype(existing) && !isEmptyGhidraClassPlaceholder(existing)) {
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
            previewCoreDatatypes.put(name, structure);
            action.addProperty("applied", false);
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
            actions.add(action);
            return;
        }

        if (existing instanceof Structure) {
            ((Structure) existing).replaceWith(structure);
            existing.setDescription(structure.getDescription());
        }
        else dataTypeManager.addDataType(structure, DataTypeConflictHandler.DEFAULT_HANDLER);
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
        if (!applyRenames && previewCoreDatatypes.containsKey(name)) return previewCoreDatatypes.get(name);
        DataType dataType = dataTypeManager.getDataType(
            new CategoryPath(CategoryPath.ROOT, "AZ"),
            name);
        if (dataType instanceof Structure) return (Structure) dataType;
        if (dataType == null && !applyRenames) {
            switch (name) {
                case "ReflectContext": return coreReflectionStructure(name, 0x10, null);
                case "SerializeContext":
                case "BehaviorContext": return coreReflectionStructure(name, 0x10, "ReflectContext");
                case "ComponentDescriptor": return coreReflectionStructure(name, 0x8, null);
            }
        }
        return null;
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

        DataType reflectContextPointer = coreReflectionPointerType("ReflectContext");
        if (reflectContextPointer == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "reflect-context-datatype-missing");
            actions.add(action);
            return;
        }

        try {
            updateReflectionSignature(function, new String[] { "reflection" },
                new DataType[] { reflectContextPointer }, null, action);
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

        DataType descriptorPointer = coreReflectionPointerType("ComponentDescriptor");
        DataType reflectContextPointer = coreReflectionPointerType("ReflectContext");
        if (descriptorPointer == null || reflectContextPointer == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", "core-reflection-datatype-missing");
            actions.add(action);
            return;
        }

        try {
            updateReflectionSignature(function, new String[] { "this", "reflection" },
                new DataType[] { descriptorPointer, reflectContextPointer }, null, action);
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

    private boolean updateReflectionSignature(
        Function function,
        String[] names,
        DataType[] types,
        DataType returnType,
        JsonObject action) throws Exception {

        action.addProperty("signatureChecked", true);
        ReflectionSignaturePlan plan = planReflectionSignature(function, names, types, returnType, action);
        action.addProperty("applied", false);
        if (plan.skipReason != null) {
            action.addProperty("reason", plan.skipReason);
            return false;
        }
        if (!plan.changed) {
            action.addProperty("reason", "already-current");
            return false;
        }
        if (!applyRenames) {
            action.addProperty("wouldApply", true);
            action.addProperty("reason", "dry-run");
            return true;
        }
        function.updateFunction(null, plan.returnValue, plan.parameters,
            plan.customStorage ? FunctionUpdateType.CUSTOM_STORAGE : FunctionUpdateType.DYNAMIC_STORAGE_FORMAL_PARAMS,
            false, SourceType.USER_DEFINED);
        action.addProperty("applied", true);
        return true;
    }

    private ReflectionSignaturePlan planReflectionSignature(
        Function function, String[] names, DataType[] types, DataType returnType, JsonObject action) throws Exception {

        if (names.length != types.length || names.length == 0) {
            throw new IllegalArgumentException("Reflection parameter names and types must have matching nonzero lengths");
        }
        for (int i = 0; i < names.length; i++) {
            if (names[i] == null || names[i].isEmpty() || types[i] == null || types[i].getLength() <= 0) {
                throw new IllegalArgumentException("Reflection parameters require names and fixed datatypes");
            }
        }
        ReflectionSignaturePlan plan = new ReflectionSignaturePlan();
        plan.customStorage = function.hasCustomVariableStorage();
        ArrayList<Parameter> existing = new ArrayList<>();
        Parameter automaticThis = null;
        JsonArray automatic = new JsonArray();
        for (Parameter parameter : function.getParameters()) {
            if (parameter.isAutoParameter()) {
                automatic.add(signatureParameterEvidence(parameter));
                if (parameter.getAutoParameterType() == AutoParameterType.THIS) automaticThis = parameter;
            }
            else existing.add(parameter);
        }
        boolean materializeThis = names.length == 1 && "this".equals(names[0]) && automaticThis != null &&
            !sameDataType(automaticThis.getDataType(), types[0]);
        if (materializeThis) {
            if (automatic.size() != 1 || function.getReturn().isForcedIndirect() ||
                existing.stream().anyMatch(Parameter::isForcedIndirect)) {
                plan.skipReason = "signature-auto-this-type-unresolved";
                action.addProperty("storageDetail", "Automatic receiver type differs; hidden/indirect ABI requires independent evidence");
                return plan;
            }
            for (Symbol symbol : symbols.getSymbols(function)) {
                if (symbol.getSymbolType() != SymbolType.PARAMETER && "this".equals(symbol.getName()) &&
                    symbol.getSource() != SourceType.DEFAULT) {
                    plan.skipReason = "signature-auto-this-name-conflict";
                    return plan;
                }
            }
            existing.add(0, automaticThis);
            plan.customStorage = true;
            plan.changed = true;
            action.addProperty("materializedAutomaticThis", true);
        }
        action.add("automaticParameters", automatic);
        action.addProperty("customStoragePreserved", function.hasCustomVariableStorage());
        action.addProperty("callingConvention", function.getCallingConventionName());

        boolean receiverRequested = "this".equals(names[0]);
        if (receiverRequested && automaticThis == null && hiddenReturnPrecedesFormals(function, existing)) {
            // MSVC x64 members pass this in RCX and the return buffer in RDX; the model has them reversed.
            plan.skipReason = "signature-receiver-hidden-return-order-unresolved";
            action.addProperty("storageDetail", "Hidden return pointer occupies the receiver register");
            return plan;
        }
        int desiredStart = receiverRequested && automaticThis != null && !materializeThis ? 1 : 0;
        int existingStart = 0;
        while (existingStart < existing.size()) {
            Parameter parameter = existing.get(existingStart);
            boolean hiddenReturn = Function.RETURN_PTR_PARAM_NAME.equals(parameter.getName());
            boolean explicitReceiver = !receiverRequested && "this".equals(parameter.getName()) &&
                parameter.getDataType() instanceof ghidra.program.model.data.Pointer;
            if (!hiddenReturn && !explicitReceiver) break;
            existingStart++;
        }
        if (automaticThis != null && receiverRequested && !materializeThis) {
            action.addProperty("thisTypeManagedByClassNamespace", true);
            action.addProperty("requestedThisType", types[0].getPathName());
        }

        int desiredCount = names.length - desiredStart;
        int length = Math.max(existing.size(), existingStart + desiredCount);
        Set<String> occupied = new HashSet<>();
        for (Symbol symbol : symbols.getSymbols(function)) {
            if (symbol.getSymbolType() != SymbolType.PARAMETER && symbol.getSource() != SourceType.DEFAULT) {
                occupied.add(symbol.getName());
            }
        }
        if (!materializeThis) {
            for (JsonElement value : automatic) occupied.add(value.getAsJsonObject().get("name").getAsString());
        }
        for (int i = 0; i < existing.size(); i++) {
            if (i < existingStart || i >= existingStart + desiredCount) occupied.add(existing.get(i).getName());
        }
        JsonArray nameAdjustments = new JsonArray();
        JsonArray proposed = new JsonArray();
        for (int i = 0; i < length; i++) {
            Parameter old = i < existing.size() ? existing.get(i) : null;
            boolean update = i >= existingStart && i < existingStart + desiredCount;
            if (!update) {
                plan.parameters.add(old);
                proposed.add(signatureParameterEvidence(old));
                continue;
            }
            int desired = i - existingStart + desiredStart;
            String name = names[desired];
            for (int suffix = 2; occupied.contains(name); suffix++) name = names[desired] + "_" + suffix;
            occupied.add(name);
            if (!name.equals(names[desired])) {
                JsonObject adjustment = new JsonObject();
                adjustment.addProperty("requested", names[desired]);
                adjustment.addProperty("actual", name);
                adjustment.addProperty("formalOrdinal", i);
                nameAdjustments.add(adjustment);
            }
            VariableStorage storage = plan.customStorage && old != null ? old.getVariableStorage() :
                VariableStorage.UNASSIGNED_STORAGE;
            if (materializeThis && storage.isAutoStorage()) {
                storage = new VariableStorage(currentProgram, storage.getVarnodes());
            }
            if (plan.customStorage && (!storage.isValid() || storage.size() != types[desired].getLength())) {
                plan.skipReason = "signature-custom-storage-unresolved";
                action.addProperty("storageDetail", "Preserved custom storage cannot hold parameter '" + name + "'");
                return plan;
            }
            ParameterImpl parameter = new ParameterImpl(name, types[desired], storage, currentProgram, SourceType.USER_DEFINED);
            if (old != null) parameter.setComment(old.getComment());
            plan.parameters.add(parameter);
            proposed.add(signatureParameterEvidence(parameter));
            plan.changed |= old == null || !name.equals(old.getName()) ||
                !sameDataType(old.getFormalDataType(), types[desired]);
        }
        action.add("parameterNameAdjustments", nameAdjustments);
        action.add("plannedFormalParameters", proposed);
        if (returnType != null && !sameDataType(function.getReturn().getFormalDataType(), returnType)) {
            VariableStorage storage = plan.customStorage ? function.getReturn().getVariableStorage() :
                VariableStorage.UNASSIGNED_STORAGE;
            if (plan.customStorage && (!storage.isValid() || storage.size() != returnType.getLength() ||
                function.getReturn().isForcedIndirect())) {
                plan.skipReason = "signature-custom-return-storage-unresolved";
                return plan;
            }
            plan.returnValue = new ReturnParameterImpl(returnType, storage, currentProgram);
            plan.changed = true;
        }
        assessSignatureStorage(function, plan, action);
        return plan;
    }

    private boolean hiddenReturnPrecedesFormals(Function function, List<Parameter> existing) {
        for (Parameter parameter : function.getParameters()) {
            if (parameter.isAutoParameter() &&
                parameter.getAutoParameterType() == AutoParameterType.RETURN_STORAGE_PTR) {
                return true;
            }
        }
        return !existing.isEmpty() && Function.RETURN_PTR_PARAM_NAME.equals(existing.get(0).getName());
    }

    private JsonObject signatureParameterEvidence(Parameter parameter) {
        JsonObject evidence = new JsonObject();
        evidence.addProperty("name", parameter.getName());
        evidence.addProperty("type", parameter.getFormalDataType().getPathName());
        evidence.addProperty("storage", parameter.getVariableStorage().toString());
        return evidence;
    }

    private void assessSignatureStorage(Function function, ReflectionSignaturePlan plan, JsonObject action) {
        ArrayList<VariableStorage> storages = new ArrayList<>();
        if (plan.customStorage) {
            for (Parameter parameter : plan.parameters) storages.add(parameter.getVariableStorage());
        }
        else {
            var convention = function.getCallingConvention();
            if (convention == null) convention = currentProgram.getCompilerSpec().getDefaultCallingConvention();
            if (convention == null || convention.isErrorPlaceholder()) {
                plan.skipReason = "signature-calling-convention-unresolved";
                return;
            }
            DataType[] types = new DataType[plan.parameters.size() + 1];
            types[0] = plan.returnValue == null ? function.getReturn().getFormalDataType() : plan.returnValue.getFormalDataType();
            for (int i = 0; i < plan.parameters.size(); i++) types[i + 1] = plan.parameters.get(i).getFormalDataType();
            VariableStorage[] calculated = convention.getStorageLocations(currentProgram, types, true);
            for (int i = 1; i < calculated.length; i++) storages.add(calculated[i]);
            Parameter[] current = function.getParameters();
            if (current.length == storages.size()) {
                for (int i = 0; i < current.length; i++) {
                    if (!current[i].getVariableStorage().isValid() && storages.get(i).isValid()) {
                        plan.changed = true;
                        action.addProperty("storageRepair", "recompute-invalid-dynamic-parameter-storage");
                    }
                }
            }
        }
        JsonArray plannedStorage = new JsonArray();
        for (VariableStorage storage : storages) plannedStorage.add(storage.toString());
        action.add("plannedParameterStorage", plannedStorage);
        if (plan.customStorage) {
            for (int i = 0; i < storages.size(); i++) {
                if (!storages.get(i).isValid()) {
                    plan.skipReason = "signature-custom-storage-unresolved";
                    return;
                }
                for (int j = i + 1; j < storages.size(); j++) {
                    if (storages.get(i).intersects(storages.get(j))) {
                        plan.skipReason = "signature-custom-storage-conflict";
                        return;
                    }
                }
            }
        }
        for (Variable local : function.getLocalVariables()) {
            VariableStorage localStorage = local.getVariableStorage();
            if (localStorage.isBadStorage()) {
                plan.skipReason = "signature-invalid-local-storage";
                action.addProperty("storageDetail", "Local '" + local.getName() + "' needs independent storage evidence");
                return;
            }
            if (local.getFirstUseOffset() != 0) continue;
            for (VariableStorage storage : storages) {
                if ((plan.customStorage || localStorage.hasStackStorage() && storage.hasStackStorage()) &&
                    storage.intersects(localStorage)) {
                    plan.skipReason = localStorage.hasStackStorage() ? "signature-stack-storage-conflict" :
                        "signature-custom-local-storage-conflict";
                    action.addProperty("storageDetail", "Local '" + local.getName() + "' overlaps planned parameter storage");
                    return;
                }
            }
        }
    }

    private static class ReflectionSignaturePlan {
        final List<Parameter> parameters = new ArrayList<>();
        ReturnParameterImpl returnValue;
        boolean customStorage;
        boolean changed;
        String skipReason;
    }

    private void repairQualifiedReflectionNames(ActionSink actions) throws Exception {
        repairEngineNamespaceRoots(actions);
        ArrayList<GhidraClass> classes = new ArrayList<>();
        symbols.getClassNamespaces().forEachRemaining(classes::add);
        ArrayList<DataType> types = new ArrayList<>();
        dataTypeManager.getAllDataTypes().forEachRemaining(types::add);
        ArrayList<List<String>> movedScopes = new ArrayList<>();
        for (DataType type : types) {
            monitor.checkCancelled();
            String description = type.getDescription();
            if (description == null || !(isGeneratedReflectedDatatype(type) ||
                    description.startsWith("AZ SerializeContext reflected field type:"))) {
                continue;
            }
            List<String> parts = cppScopeParts(type.getName());
            DatatypeTarget canonical = null;
            Matcher identity = UUID_RE.matcher(description);
            if (identity.find()) {
                String id = normalizeTypeId(identity.group());
                ClassData body = typeEvidence == null ? null : typeEvidence.classBodiesByTypeId.get(id);
                canonical = body == null ? null : datatypeTarget(body);
                String qualified = datatypeNames.get(id);
                if (canonical != null && !datatypeConflictingPaths.contains(datatypePathName(canonical))) {
                    parts = new ArrayList<>(canonical.scope);
                }
                else if (canonical == null && qualified != null && balancedCppDelimiters(qualified) && !type.getName().contains("::")) {
                    List<String> qualifiedParts = cppScopeParts(qualified);
                    if (qualifiedParts.size() > 1) parts = qualifiedParts;
                }
            }
            if (parts.size() < 2 && canonical == null) continue;
            CategoryPath targetCategory = canonical == null ? datatypeCategoryPath(parts) : canonical.categoryPath;
            String localName = canonical == null ? parts.get(parts.size() - 1) : canonical.name;
            if (type.getCategoryPath().equals(targetCategory) && type.getName().equals(localName)) continue;
            registerReflectedClassScope(parts);
            JsonObject action = new JsonObject();
            action.addProperty("kind", "datatype_name_migration");
            action.addProperty("oldPath", type.getPathName());
            action.addProperty("newCategory", targetCategory.getPath());
            action.addProperty("newName", localName);
            DataType collision = dataTypeManager.getDataType(targetCategory, localName);
            if (collision != null && collision != type) {
                if (type instanceof Structure && collision instanceof Structure &&
                    ((Structure) type).getNumDefinedComponents() == 0 &&
                    ((Structure) collision).getNumDefinedComponents() > 0 && type.getLength() == collision.getLength()) {
                    // false: keep the existing layout at its canonical path; only references move.
                    if (applyRenames) dataTypeManager.replaceDataType(type, collision, false);
                    action.addProperty("applied", applyRenames);
                    action.addProperty("wouldApply", !applyRenames);
                    action.addProperty("reason", "empty-placeholder-references-redirected-to-existing-layout");
                    actions.add(action);
                    continue;
                }
                action.addProperty("applied", false);
                action.addProperty("reason", "target-datatype-exists; preserved-both");
                actions.add(action);
                continue;
            }
            if (applyRenames) {
                dataTypeManager.createCategory(targetCategory);
                // Keep the managed type and its identity so pointers, fields and signatures stay attached.
                type.setNameAndCategory(targetCategory, localName);
                createScope(parts.subList(0, parts.size() - 1));
                movedScopes.add(parts);
            }
            action.addProperty("applied", applyRenames);
            action.addProperty("wouldApply", !applyRenames);
            actions.add(action);
        }
        for (GhidraClass klass : classes) {
            monitor.checkCancelled();
            List<String> parts = normalizeRepeatedScope(namespaceScope(klass));
            if (balancedCppDelimiters(klass.getName(true))) rememberSplitTemplateScopes(parts);
            boolean qualifiedScope = balancedCppDelimiters(klass.getName(true)) &&
                namespaceScope(klass).stream().anyMatch(part -> cppScopeParts(part).size() > 1);
            if (parts.size() < 2 || (!hasReflectedClassOwner(parts) && !hasSplitTemplateScope(klass) && !qualifiedScope)) {
                continue;
            }
            if (namespaceScope(klass).equals(parts)) continue;
            rememberTemplateScopeFragments(klass);
            JsonObject action = new JsonObject();
            action.addProperty("kind", "class_name_migration");
            action.addProperty("oldPath", klass.getName(true));
            action.addProperty("newPath", String.join("::", parts));
            List<String> parentParts = parts.subList(0, parts.size() - 1);
            Namespace parent = findScope(parentParts);
            String localName = parts.get(parts.size() - 1);
            Namespace collision = parent == null ? null : symbols.getNamespace(localName, parent);
            if (collision != null && !collision.equals(klass)) {
                if (collision instanceof GhidraClass && !symbols.getSymbols(klass).hasNext()) {
                    action.addProperty("applied", applyRenames && klass.getSymbol().delete());
                    action.addProperty("wouldApply", !applyRenames);
                    action.addProperty("reason", "empty-flattened-class-duplicate");
                    actions.add(action);
                    continue;
                }
                action.addProperty("applied", false);
                action.addProperty("reason", "target-class-exists; preserved-both");
                actions.add(action);
                continue;
            }
            if (applyRenames) {
                parent = createScope(parentParts);
                klass.getSymbol().setNameAndNamespace(localName, parent, SourceType.USER_DEFINED);
                scopeCache.clear();
            }
            action.addProperty("applied", applyRenames);
            action.addProperty("wouldApply", !applyRenames);
            actions.add(action);
        }
        if (applyRenames) {
            for (List<String> parts : movedScopes) createScope(parts);
        }
    }

    private boolean hasReflectedClassOwner(List<String> scope) {
        for (int i = 1; i <= scope.size(); i++) {
            if (reflectedClassScopePaths.contains(String.join("::", scope.subList(0, i)))) return true;
        }
        return false;
    }

    private void repairEngineNamespaceRoots(ActionSink actions) throws Exception {
        for (String name : new TreeSet<>(ENGINE_NAMESPACE_ROOTS)) {
            Namespace root = symbols.getNamespace(name, currentProgram.getGlobalNamespace());
            if (!(root instanceof GhidraClass)) continue;
            monitor.checkCancelled();
            JsonObject action = new JsonObject();
            action.addProperty("kind", "namespace_kind_migration");
            action.addProperty("name", name);
            action.addProperty("oldSymbolId", root.getID());
            if (applyRenames) {
                Namespace replacement = convertClassRootToNamespace(root);
                action.addProperty("newSymbolId", replacement.getID());
                scopeCache.clear();
            }
            action.addProperty("applied", applyRenames);
            action.addProperty("wouldApply", !applyRenames);
            actions.add(action);
        }
    }

    private Namespace convertClassRootToNamespace(Namespace root) throws Exception {
        // Ghidra has no inverse of convertNamespaceToClass. Recreate only the root;
        // reparent every immediate child in place, including functions and namespaces.
        ArrayList<Symbol> children = new ArrayList<>();
        symbols.getSymbols(root).forEachRemaining(children::add);
        String name = root.getName();
        String temporaryName = "__AzReflectionLegacy_" + name + "_" + root.getID();
        while (symbols.getNamespace(temporaryName, root.getParentNamespace()) != null) {
            temporaryName += "_";
        }
        int transaction = currentProgram.startTransaction("Repair namespace root " + name);
        boolean commit = false;
        try {
            root.getSymbol().setName(temporaryName, SourceType.USER_DEFINED);
            Namespace replacement = symbols.createNameSpace(root.getParentNamespace(), name, SourceType.USER_DEFINED);
            for (Symbol child : children) {
                monitor.checkCancelled();
                child.setNamespace(replacement);
            }
            if (symbols.getSymbols(root).hasNext() || !root.getSymbol().delete()) {
                throw new IllegalStateException("Could not retire empty class root " + temporaryName);
            }
            commit = true;
            return replacement;
        }
        finally {
            currentProgram.endTransaction(transaction, commit);
        }
    }

    private boolean isGeneratedReflectedDatatype(DataType dataType) {
        String description = dataType.getDescription();
        return isEmptyGhidraClassPlaceholder(dataType) || description != null &&
            (description.startsWith("AZ SerializeContext reflected type") ||
             description.startsWith("AZ SerializeContext reflected field type"));
    }

    private boolean isEmptyGhidraClassPlaceholder(DataType dataType) {
        return dataType instanceof Structure && dataType.getLength() <= 1 &&
            ((Structure) dataType).getNumDefinedComponents() == 0 &&
            "PlaceHolder Class Structure".equals(dataType.getDescription());
    }

    private boolean sameStructure(Structure left, Structure right) {
        if (left == null || right == null) {
            return false;
        }
        if (left.getLength() != right.getLength()) {
            return false;
        }
        if (left.getAlignment() != right.getAlignment()) return false;

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

    private boolean matchesNativeLayoutProfile() {
        return supportsContainerPointerLayout() &&
            matchesProfileBytes(0xd5ce20, "488b4218c3") &&
            matchesProfileBytes(0xdb0880, "488b4a0848b88988888888888888482b0a48");
    }

    private boolean matchesProfileBytes(long rva, String hex) {
        try {
            byte[] bytes = new byte[hex.length() / 2];
            if (currentProgram.getMemory().getBytes(currentProgram.getImageBase().add(rva), bytes) != bytes.length) return false;
            for (int i = 0; i < bytes.length; i++) {
                if ((bytes[i] & 255) != Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16)) return false;
            }
            return true;
        }
        catch (Exception error) { return false; }
    }

    private String nativeFamily(ClassData data) {
        JsonObject generic = datatypeGenerics.get(normalizeTypeId(data.typeId));
        JsonObject body = generic == null ? null : objectMember(generic, "classData");
        return body == null ? data.targetTypeName : stringMember(body, "name");
    }

    private int nativeProfileSize(ClassData data) {
        if (!nativeLayoutProfile) return 0;
        String family = nativeFamily(data);
        if (family == null) return 0;
        if (datatypeGenerics.containsKey(normalizeTypeId(data.typeId)) &&
            (data.targetTypeName == null || !(data.targetTypeName.equals(family) || data.targetTypeName.startsWith(family + "<")))) return 0;
        int size = switch (family) {
            case "AZStd::vector", "AZStd::list" -> 32;
            case "AZStd::unordered_map", "AZStd::unordered_set" -> 144;
            case "AZStd::string" -> 40;
            case "AZStd::shared_ptr", "AZ::Uuid" -> 16;
            case "AZ::Vector3", "AZ::Vector4", "AZ::Quaternion" -> 16;
            case "AZStd::intrusive_ptr", "AZStd::unique_ptr", "AZ::EntityId" -> 8;
            case "AZ::Crc32" -> 4;
            case "AZ::Component" -> 24;
            case "AZStd::pair" -> typeEvidence == null ? 0 : typeEvidence.nativeDataSizesByTypeId.getOrDefault(normalizeTypeId(data.typeId), 0);
            default -> 0;
        };
        JsonObject generic = datatypeGenerics.get(normalizeTypeId(data.typeId));
        if ("AZStd::array".equals(family) && generic != null) {
            JsonObject member = genericMember(arrayMember(generic, "elements"), "element");
            JsonObject arguments = objectMember(generic, "nonTypeTemplateArguments");
            Integer count = arguments == null ? null : parseLayoutInteger(stringMember(arguments, "capacity"));
            Integer stride = member == null ? null : parseLayoutInteger(stringMember(member, "dataSize"));
            if (count != null && count > 0 && stride != null && stride > 0 && (long) count * stride <= 16 * 1024 * 1024) size = count * stride;
        }
        Integer observed = typeEvidence == null ? null : typeEvidence.nativeDataSizesByTypeId.get(normalizeTypeId(data.typeId));
        return observed == null || observed == size ? size : 0;
    }

    private boolean processNativeDatatype(ClassData data, ActionSink actions) throws Exception {
        int size = nativeProfileSize(data);
        if (size <= 0) return false;
        DatatypeTarget target = datatypeTarget(data);
        if (target == null) return false;
        JsonObject action = new JsonObject();
        action.addProperty("kind", "datatype_native_layout");
        action.addProperty("ownerTypeId", data.typeId);
        action.addProperty("name", data.targetTypeName);
        action.addProperty("layoutSize", size);
        action.addProperty("evidence", "NewWorld 3-26 Size/EnumElements/ClearElements callbacks, sizeof from reflected by-value members; AzCore member names");
        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existing != null && !isGeneratedReflectedDatatype(existing)) {
            action.addProperty("existingSize", existing.getLength());
            action.addProperty("reason", existing.getLength() == size ? "existing-user-datatype" : "existing-user-size-mismatch; preserved");
            action.addProperty("applied", false);
            actions.add(action);
            return true;
        }
        try {
            Structure structure = new StructureDataType(target.categoryPath, target.name, size, dataTypeManager);
            structure.setDescription(reflectedDatatypeDescription(data) + "; NewWorld 3-26 native ABI, verified callback offsets");
            fillNativeLayout(data, structure);
            String family = nativeFamily(data);
            if (Set.of("AZ::Uuid", "AZ::Vector3", "AZ::Vector4", "AZ::Quaternion").contains(family)) {
                structure.setExplicitMinimumAlignment(16);
            }
            else if ("AZ::Crc32".equals(family)) structure.setExplicitMinimumAlignment(4);
            else if (!Set.of("AZStd::array", "AZStd::pair").contains(family)) structure.setExplicitMinimumAlignment(8);
            applyCapturedAlignment(data, structure);
            action.addProperty("fieldsWritten", structure.getNumDefinedComponents());
            if (!applyRenames) {
                action.addProperty("applied", false);
                action.addProperty("wouldApply", true);
                action.addProperty("reason", "dry-run");
            }
            else if (existing instanceof Structure && sameStructure((Structure) existing, structure)) {
                action.addProperty("applied", false);
                action.addProperty("reason", "already-current");
            }
            else {
                saveGeneratedLayout(structure);
                action.addProperty("applied", true);
            }
        }
        catch (UnresolvedNativeLayoutException error) {
            action.addProperty("applied", false);
            action.addProperty("reason", "native-layout-unresolved");
            action.addProperty("detail", error.getMessage());
        }
        catch (Exception error) {
            action.addProperty("applied", false);
            action.addProperty("reason", "native-layout-unresolved");
            action.addProperty("error", error.getMessage());
            action.addProperty("errorType", error.getClass().getName());
        }
        actions.add(action);
        return true;
    }

    private void nativeField(Structure structure, int offset, DataType type, String name) {
        structure.replaceAtOffset(offset, type, type.getLength(), name, "NewWorld 3-26 native ABI");
    }

    private static class UnresolvedNativeLayoutException extends IllegalArgumentException {
        UnresolvedNativeLayoutException(String message) { super(message); }
    }

    private DataType saveGeneratedLayout(DataType candidate) {
        DataType existing = dataTypeManager.getDataType(candidate.getCategoryPath(), candidate.getName());
        if (existing != null && !isGeneratedReflectedDatatype(existing)) {
            if (existing.getLength() != candidate.getLength()) throw new UnresolvedNativeLayoutException("manual helper size differs: " + existing.getPathName());
            return existing;
        }
        if (!applyRenames) return candidate;
        if (existing instanceof Structure && candidate instanceof Structure) {
            if (!sameStructure((Structure) existing, (Structure) candidate)) ((Structure) existing).replaceWith(candidate);
            existing.setDescription(candidate.getDescription());
            return existing;
        }
        if (existing != null) return existing;
        return dataTypeManager.addDataType(candidate, DataTypeConflictHandler.DEFAULT_HANDLER);
    }

    private Structure nativeHelper(CategoryPath path, String name, int size) {
        Structure result = new StructureDataType(path, name, size, dataTypeManager);
        result.setDescription("AZ SerializeContext reflected type: NewWorld 3-26 native ABI helper " + name);
        return result;
    }

    private DataType nativeAllocator(boolean reference) {
        Structure result = nativeHelper(new CategoryPath("/AZStd"), reference ? "allocator_ref" : "allocator", reference ? 16 : 8);
        nativeField(result, 0, new PointerDataType(CharDataType.dataType, 8, dataTypeManager), "m_name");
        if (reference) nativeField(result, 8, new PointerDataType(nativeAllocator(false), 8, dataTypeManager), "m_allocator");
        return saveGeneratedLayout(result);
    }

    private ElementData genericLayoutElement(ClassData data, String memberName) {
        JsonObject generic = datatypeGenerics.get(normalizeTypeId(data.typeId));
        JsonObject member = generic == null ? null : genericMember(arrayMember(generic, "elements"), memberName);
        if (member == null) throw new UnresolvedNativeLayoutException("missing generic member " + memberName);
        ElementData element = new ElementData();
        element.typeId = stringMember(member, "typeId");
        element.typeName = datatypeNames.get(normalizeTypeId(element.typeId));
        ClassData body = typeEvidence.classBodiesByTypeId.get(normalizeTypeId(element.typeId));
        element.targetTypeName = body == null ? element.typeName : body.targetTypeName;
        element.dataSize = stringMember(member, "dataSize");
        element.isPointer = nativeElementIsPointer(member);
        element.name = memberName;
        element.safeName = memberName;
        return element;
    }

    private DataType genericValueType(ClassData data) {
        ElementData element = genericLayoutElement(data, "element");
        Integer stride = parseLayoutInteger(element.dataSize);
        if (stride == null || stride <= 0) throw new UnresolvedNativeLayoutException("unknown generic value stride");
        return fieldDataType(data, element, stride);
    }

    private boolean nativeElementIsPointer(JsonObject element) {
        Boolean pointer = boolMember(element, "is_pointer");
        if (pointer != null) return pointer;
        Integer flags = parseLayoutInteger(stringMember(element, "flags"));
        return flags != null && (flags & 1) != 0;
    }

    private void nativeVector(Structure structure, DataType element, boolean referenceAllocator) {
        DataType pointer = new PointerDataType(element, 8, dataTypeManager);
        nativeField(structure, 0, pointer, "m_start");
        nativeField(structure, 8, pointer, "m_last");
        nativeField(structure, 16, pointer, "m_end");
        nativeField(structure, 24, nativeAllocator(referenceAllocator), "m_allocator");
    }

    private void fillNativeLayout(ClassData data, Structure structure) throws Exception {
        String family = nativeFamily(data);
        DataType u64 = UnsignedLongLongDataType.dataType;
        switch (family) {
            case "AZ::Uuid":
                nativeField(structure, 0, new ArrayDataType(ByteDataType.dataType, 16, 1), "data");
                break;
            case "AZ::Vector3":
            case "AZ::Vector4":
            case "AZ::Quaternion":
                nativeField(structure, 0, new ArrayDataType(FloatDataType.dataType, 4, 4), "m_value");
                structure.getComponentAt(0).setComment("Native SIMD storage; Vector3's fourth lane is not a reflected coordinate");
                break;
            case "AZ::EntityId": nativeField(structure, 0, u64, "m_id"); break;
            case "AZ::Crc32": nativeField(structure, 0, UnsignedIntegerDataType.dataType, "m_value"); break;
            case "AZ::Component":
                nativeField(structure, 0, new PointerDataType(VoidDataType.dataType, 8, dataTypeManager), "vftable");
                ElementData entity = new ElementData();
                entity.typeId = "75651658-8663-478D-9090-2432DFCAFA44";
                entity.typeName = "AZ::Entity";
                nativeField(structure, 8, new PointerDataType(pointeeDataType(data, entity), 8, dataTypeManager), "m_entity");
                nativeField(structure, 16, u64, "Id");
                break;
            case "AZStd::vector": nativeVector(structure, genericValueType(data), false); break;
            case "AZStd::pair":
                fillNativePair(data, structure);
                break;
            case "AZStd::array":
                DataType value = genericValueType(data);
                nativeField(structure, 0, new ArrayDataType(value, structure.getLength() / value.getLength(), value.getLength()), "m_elements");
                break;
            case "AZStd::shared_ptr":
            case "AZStd::unique_ptr":
            case "AZStd::intrusive_ptr":
                nativeField(structure, 0, genericValueType(data), "AZStd::unique_ptr".equals(family) ? "pointer" : "px");
                if ("AZStd::shared_ptr".equals(family)) nativeField(structure, 8, new PointerDataType(VoidDataType.dataType, 8, dataTypeManager), "pn");
                break;
            case "AZStd::string":
                UnionDataType storage = new UnionDataType(new CategoryPath(structure.getCategoryPath(), structure.getName()), "storage", dataTypeManager);
                storage.setDescription("AZ SerializeContext reflected type: NewWorld 3-26 string small-buffer storage");
                storage.add(new ArrayDataType(CharDataType.dataType, 16, 1), "m_buffer", "capacity <= 15");
                storage.add(new PointerDataType(CharDataType.dataType, 8, dataTypeManager), "m_data", "capacity > 15");
                nativeField(structure, 0, saveGeneratedLayout(storage), "m_storage");
                nativeField(structure, 16, u64, "m_size");
                nativeField(structure, 24, u64, "m_capacity");
                nativeField(structure, 32, nativeAllocator(false), "m_allocator");
                break;
            case "AZStd::list":
            case "AZStd::unordered_map":
            case "AZStd::unordered_set":
                fillNativeHashOrList(data, structure, family);
                break;
            default: throw new IllegalArgumentException("native layout not implemented: " + family);
        }
    }

    private void fillNativeHashOrList(ClassData data, Structure structure, String family) {
        CategoryPath path = new CategoryPath(structure.getCategoryPath(), structure.getName());
        DataType value = genericValueType(data);
        int stride = parseLayoutInteger(genericLayoutElement(data, "element").dataSize);
        Structure node = nativeHelper(path, "node", (16 + stride + 7) & ~7);
        DataType nodePointer = new PointerDataType(node, 8, dataTypeManager);
        nativeField(node, 0, nodePointer, "m_next");
        nativeField(node, 8, nodePointer, "m_prev");
        nativeField(node, 16, value, "m_value");
        DataType savedNode = saveGeneratedLayout(node);
        nodePointer = new PointerDataType(savedNode, 8, dataTypeManager);
        Structure head = nativeHelper(path, "list_node_base", 16);
        nativeField(head, 0, nodePointer, "m_next");
        nativeField(head, 8, nodePointer, "m_prev");
        DataType savedHead = saveGeneratedLayout(head);
        if ("AZStd::list".equals(family)) {
            nativeField(structure, 0, savedHead, "m_head");
            nativeField(structure, 16, UnsignedLongLongDataType.dataType, "m_numElements");
            nativeField(structure, 24, nativeAllocator(false), "m_allocator");
            return;
        }
        Structure list = nativeHelper(path, "list_storage", 40);
        nativeField(list, 0, savedHead, "m_head");
        nativeField(list, 16, UnsignedLongLongDataType.dataType, "m_numElements");
        nativeField(list, 24, nativeAllocator(true), "m_allocator");
        Structure bucket = nativeHelper(path, "bucket", 16);
        nativeField(bucket, 0, UnsignedLongLongDataType.dataType, "first");
        nativeField(bucket, 8, nodePointer, "second");
        DataType savedBucket = saveGeneratedLayout(bucket);
        Structure vector = nativeHelper(path, "bucket_vector", 40);
        nativeVector(vector, savedBucket, true);
        nativeField(structure, 0, nativeAllocator(false), "m_allocator");
        nativeField(structure, 8, saveGeneratedLayout(list), "m_list");
        nativeField(structure, 0x30, saveGeneratedLayout(vector), "m_vector");
        nativeField(structure, 0x60, new PointerDataType(savedBucket, 8, dataTypeManager), "m_buckets");
        nativeField(structure, 0x68, UnsignedLongLongDataType.dataType, "m_numBuckets");
        nativeField(structure, 0x70, FloatDataType.dataType, "m_max_load_factor");
        nativeField(structure, 0x78, savedBucket, "m_startBucket");
        // Predicate/hasher storage is not exposed by reflection. Leave its tail undefined.
    }

    private void fillNativePair(ClassData data, Structure structure) throws Exception {
        JsonObject generic = datatypeGenerics.get(normalizeTypeId(data.typeId));
        JsonObject body = objectMember(generic, "classData");
        JsonObject info = objectMember(body, "containerInfo");
        JsonArray slots = info == null ? null : arrayMember(info, "slots");
        if (slots == null || slots.size() <= 3) throw new UnresolvedNativeLayoutException("missing pair EnumElements callback");
        JsonObject slot = resolveObject(slots.get(3), jsonObjectsById);
        Address callback = parseCaptureAddress(stringMember(slot, "address"));
        Address table = parseCaptureAddress(stringMember(info, "vtable"));
        if (callback == null || table == null || !callback.equals(readPointer(table.add(24)))) {
            throw new UnresolvedNativeLayoutException("pair callback does not match loaded binary");
        }
        List<Integer> offsets = pairCallbackOffsets(callback);
        if (offsets.size() != 2 || offsets.get(0) != 0) throw new UnresolvedNativeLayoutException("pair native offsets unresolved");
        ElementData first = genericLayoutElement(data, "value1");
        ElementData second = genericLayoutElement(data, "value2");
        int firstSize = parseLayoutInteger(first.dataSize);
        int secondSize = parseLayoutInteger(second.dataSize);
        if (firstSize > offsets.get(1) || (long) offsets.get(1) + secondSize > structure.getLength()) {
            throw new UnresolvedNativeLayoutException("pair native offsets exceed sizeof evidence");
        }
        nativeField(structure, 0, fieldDataType(data, first, firstSize), "first");
        nativeField(structure, offsets.get(1), fieldDataType(data, second, secondSize), "second");
    }

    private List<Integer> pairCallbackOffsets(Address callback) {
        Function function = currentProgram.getFunctionManager().getFunctionAt(callback);
        if (function == null || function.getBody().getNumAddresses() > 4096) return List.of();
        Register argument = currentProgram.getLanguage().getRegister("RDX");
        if (argument == null) return List.of();
        Map<Address, Long> tracked = new HashMap<>();
        tracked.put(argument.getAddress(), 0L);
        TreeSet<Integer> offsets = new TreeSet<>();
        Iterator<Instruction> instructions = currentProgram.getListing().getInstructions(function.getBody(), true);
        while (instructions.hasNext()) {
            Instruction instruction = instructions.next();
            // Unique temporaries are local to this instruction. Track only values derived
            // from the native object's second ABI argument, never arbitrary constants.
            tracked.keySet().removeIf(address -> address.getAddressSpace().isUniqueSpace());
            for (PcodeOp op : instruction.getPcode()) {
                if (op.getOpcode() == PcodeOp.CALL || op.getOpcode() == PcodeOp.CALLIND) {
                    Long offset = tracked.get(argument.getAddress());
                    if (offset != null && offset >= 0 && offset < 16 * 1024 * 1024) offsets.add(offset.intValue());
                    for (String name : List.of("RAX", "RCX", "RDX", "R8", "R9", "R10", "R11")) {
                        Register register = currentProgram.getLanguage().getRegister(name);
                        if (register != null) tracked.remove(register.getAddress());
                    }
                }
                Varnode output = op.getOutput();
                if (output == null) continue;
                Long value = null;
                Varnode left = op.getNumInputs() > 0 ? op.getInput(0) : null;
                Varnode right = op.getNumInputs() > 1 ? op.getInput(1) : null;
                if (op.getOpcode() == PcodeOp.COPY || op.getOpcode() == PcodeOp.INT_ZEXT || op.getOpcode() == PcodeOp.INT_SEXT) {
                    value = tracked.get(left.getAddress());
                }
                else if (op.getOpcode() == PcodeOp.INT_ADD || op.getOpcode() == PcodeOp.INT_SUB) {
                    Long base = tracked.get(left.getAddress());
                    if (base != null && right.isConstant()) {
                        value = base + (op.getOpcode() == PcodeOp.INT_ADD ? right.getOffset() : -right.getOffset());
                    }
                    else if (op.getOpcode() == PcodeOp.INT_ADD && left.isConstant()) {
                        base = tracked.get(right.getAddress());
                        if (base != null) value = base + left.getOffset();
                    }
                }
                if (value == null || output.getSize() != 8) tracked.remove(output.getAddress());
                else tracked.put(output.getAddress(), value);
            }
        }
        return new ArrayList<>(offsets);
    }

    private Structure declareReflectedDatatype(ClassData data) throws Exception {
        DatatypeTarget target = datatypeTarget(data);
        if (target == null) {
            return null;
        }
        if (enumUnderlyingTypes.containsKey(normalizeTypeId(data.typeId))) return null;
        if (datatypeConflictingPaths.contains(datatypePathName(target))) return null;
        int layoutSize = reflectedLayoutSize(data);
        if (layoutSize <= 0 && data.reflectionSource == null) {
            return null;
        }

        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existing instanceof Structure) {
            Structure existingStructure = (Structure) existing;
            if (!isGeneratedReflectedDatatype(existingStructure)) {
                return existingStructure;
            }
            boolean placeholder = isEmptyGhidraClassPlaceholder(existingStructure);
            // A zero-length StructureDB reports length 1 but grows from 0.
            int currentLength = existingStructure.isZeroLength() ? 0 : existingStructure.getLength();
            if (currentLength < layoutSize && applyRenames) {
                existingStructure.growStructure(layoutSize - currentLength);
            }
            else if (currentLength < layoutSize) {
                Structure result = new StructureDataType(target.categoryPath, target.name, layoutSize, dataTypeManager);
                result.setDescription(reflectedDatatypeDescription(data));
                applyCapturedAlignment(data, result);
                return result;
            }
            if (applyRenames) applyCapturedAlignment(data, existingStructure);
            if (applyRenames && placeholder) existingStructure.setDescription(reflectedDatatypeDescription(data));
            return existingStructure;
        }
        if (existing != null) {
            return null;
        }

        Structure structure =
            new StructureDataType(target.categoryPath, target.name, layoutSize, dataTypeManager);
        structure.setDescription(reflectedDatatypeDescription(data));
        applyCapturedAlignment(data, structure);
        if (!applyRenames) return structure;
        createScope(target.scope);
        return (Structure) dataTypeManager.addDataType(
            structure,
            DataTypeConflictHandler.DEFAULT_HANDLER);
    }

    private void applyCapturedAlignment(ClassData data, Structure structure) {
        if (behaviorContextEvidence == null) return;
        BehaviorClassMetadata metadata = behaviorContextEvidence.classesByTypeId.get(normalizeTypeId(data.typeId));
        Integer alignment = metadata == null ? null : metadata.alignment;
        if (alignment != null && alignment > 0 && alignment <= 4096 && (alignment & (alignment - 1)) == 0) {
            structure.setExplicitMinimumAlignment(alignment);
        }
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
        DataType existingType = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existingType != null && !isGeneratedReflectedDatatype(existingType)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "existing-user-datatype");
            actions.add(action);
            return;
        }

        int layoutSize = reflectedLayoutSize(data);
        int fieldCount = reflectedLayoutFieldCount(data);
        action.addProperty("layoutSize", layoutSize);
        action.addProperty("fieldCount", fieldCount);
        action.addProperty("layoutCoverage", typeEvidence.nativeDataSizesByTypeId.containsKey(normalizeTypeId(data.typeId)) ?
            "native-size-known; unreflected-members-undefined" : "serialized-field-extent-only; native-size-may-be-larger");
        if (layoutSize <= 0 || fieldCount == 0) {
            action.addProperty("applied", false);
            action.addProperty("reason", layoutSize > 0 ? "opaque-native-layout; serialized-fields-unavailable" : "unknown-native-size");
            actions.add(action);
            return;
        }

        Structure structure =
            new StructureDataType(target.categoryPath, target.name, layoutSize, dataTypeManager);
        structure.setDescription(reflectedDatatypeDescription(data));

        JsonArray fieldFailures = new JsonArray();
        int fieldsWritten = 0;
        applyCapturedAlignment(data, structure);
        List<ElementData> sortedElements = sortedLayoutElements(data);
        int previousEnd = 0;
        Set<String> writtenMetadata = new HashSet<>();
        int duplicateMetadataFields = 0;
        for (ElementData element : sortedElements) {
            Integer offset = parseLayoutInteger(element.offset);
            Integer dataSize = parseLayoutInteger(element.dataSize);
            if (!validLayoutRange(offset, dataSize)) {
                addDatatypeFieldFailure(fieldFailures, element, "invalid-layout");
                continue;
            }
            String identity = element.name + "|" + element.typeId + "|" + offset + "|" + dataSize + "|" + element.isPointer + "|" + element.isBaseClass;
            if (!writtenMetadata.add(identity)) {
                duplicateMetadataFields++;
                continue;
            }
            try {
                DataType fieldType = fieldDataType(data, element, dataSize);
                int replaceLength = datatypeFieldReplaceLength(element, fieldType, dataSize);
                if (offset < previousEnd) {
                    addDatatypeFieldFailure(fieldFailures, element, "overlapping-native-fields");
                    continue;
                }
                structure.replaceAtOffset(
                    offset,
                    fieldType,
                    replaceLength,
                    datatypeFieldName(element),
                    datatypeFieldComment(element));
                fieldsWritten++;
                previousEnd = offset + replaceLength;
            }
            catch (Exception error) {
                JsonObject failure = datatypeFieldFailure(element, "field-apply-failed");
                failure.addProperty("error", error.getMessage());
                fieldFailures.add(failure);
            }
        }
        action.addProperty("fieldsWritten", fieldsWritten);
        action.addProperty("duplicateMetadataFields", duplicateMetadataFields);
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
                    if (existing instanceof Structure) {
                        ((Structure) existing).replaceWith(structure);
                        existing.setDescription(structure.getDescription());
                    }
                    else dataTypeManager.addDataType(structure, DataTypeConflictHandler.DEFAULT_HANDLER);
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

        Structure structure = reflectedStructure(target.owner);
        if (structure == null) {
            structure = declaredStructures.get(target.ownerTypeId);
        }
        if (structure == null) {
            action.addProperty("applied", false);
            action.addProperty("reason", reflectedLayoutSize(target.owner) <= 0
                ? "datatype-missing-layout"
                : "datatype-missing");
            actions.add(action);
            return;
        }
        // Opaque sized owners (BehaviorContext size, no serialized fields) still name the receiver.
        if (structure.isZeroLength() || isEmptyGhidraClassPlaceholder(structure)) {
            action.addProperty("applied", false);
            action.addProperty("reason", "datatype-missing-layout");
            actions.add(action);
            return;
        }

        try {
            DataType thisPointer = new PointerDataType(structure, dataTypeManager);
            updateReflectionSignature(function, new String[] { "this" },
                new DataType[] { thisPointer }, null, action);
        }
        catch (Exception error) {
            action.addProperty("applied", false);
            action.addProperty("reason", "this-parameter-failed");
            action.addProperty("error", error.getMessage());
            actions.add(action);
            return;
        }
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
        target.canonicalPathName = datatypePathName(target);
        String identity = normalizeTypeId(typeId);
        if (identity != null && datatypeConflictingPaths.contains(target.canonicalPathName)) {
            target.categoryPath = new CategoryPath(new CategoryPath(target.categoryPath, "__typeid__"), identity);
        }
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
        List<String> parts = cppScopeParts(data != null ? data.targetTypeName : typeName);
        if (parts.size() == 1 && data != null && shouldUseDatatypeOwnerScope(data)) {
            result.addAll(data.ownerScope);
        }
        result.addAll(parts);
        registerReflectedClassScope(result);
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
            if (!validLayoutRange(offset, dataSize)) {
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
        int profileSize = nativeProfileSize(data);
        if (profileSize > 0) result = Math.max(result, profileSize);
        return result;
    }

    private boolean validLayoutRange(Integer offset, Integer size) {
        return offset != null && size != null && offset >= 0 && size > 0 &&
            (long) offset + size <= 16 * 1024 * 1024;
    }

    private int reflectedLayoutFieldCount(ClassData data) {
        int result = 0;
        for (ElementData element : data.elements) {
            Integer offset = parseLayoutInteger(element.offset);
            Integer dataSize = parseLayoutInteger(element.dataSize);
            if (validLayoutRange(offset, dataSize)) {
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

        if (element.isPointer) {
            if (dataSize != currentProgram.getDefaultPointerSize()) {
                throw new IllegalArgumentException("pointer-size-mismatch");
            }
            return new PointerDataType(pointeeDataType(owner, element), dataSize, dataTypeManager);
        }
        DataType enumType = enumFieldDataType(element, dataSize);
        if (enumType != null) return enumType;
        if (isReflectedValueWrapper(element)) {
            DataType wrapper = nestedFieldDataType(owner, element, dataSize);
            if (wrapper != null) return wrapper;
        }
        DataType scalar = scalarFieldDataType(element, dataSize);
        if (scalar != null) {
            return scalar;
        }
        DataType nested = nestedFieldDataType(owner, element, dataSize);
        if (nested != null) return nested;
        DataType named = namedFieldDataType(owner, element, dataSize);
        if (named != null) {
            return named;
        }
        return new ArrayDataType(new Undefined1DataType(), dataSize, 1);
    }

    private DataType pointeeDataType(ClassData owner, ElementData element) {
        String id = normalizeTypeId(element.typeId);
        ClassData body = typeEvidence == null ? null : typeEvidence.classBodiesByTypeId.get(id);
        DatatypeTarget bodyTarget = body == null ? null : datatypeTarget(body);
        if (bodyTarget != null && datatypeConflictingPaths.contains(datatypePathName(bodyTarget))) return VoidDataType.dataType;
        ElementData pointee = new ElementData();
        pointee.typeId = element.typeId;
        String name = body != null ? body.targetTypeName : datatypeNames.get(id);
        if (name == null) {
            name = element.typeName;
            if (name != null && element.isPointer && name.trim().endsWith("*")) {
                name = name.trim().substring(0, name.trim().length() - 1).trim();
            }
        }
        pointee.typeName = name;
        pointee.targetTypeName = safeTypeName(name);
        for (int size : new int[]{1, 2, 4, 8}) {
            if (enumUnderlyingTypes.containsKey(id)) {
                DataType enumType = enumFieldDataType(pointee, size);
                if (enumType != null) return enumType;
            }
            if (isReflectedValueWrapper(pointee)) break;
            DataType scalar = scalarFieldDataType(pointee, size);
            if (scalar != null) return scalar;
        }
        Structure declared = declaredStructuresByTypeId.get(id);
        if (declared != null) return declared;
        if (body != null) {
            Structure existing = reflectedStructure(body);
            if (existing != null) return existing;
        }
        if (name == null) return VoidDataType.dataType;
        DatatypeTarget target = datatypeTarget(id, name);
        if (target == null) return VoidDataType.dataType;
        if (datatypeConflictingPaths.contains(datatypePathName(target))) return VoidDataType.dataType;
        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existing != null) return existing;
        Integer nativeSize = typeEvidence == null ? null : typeEvidence.nativeDataSizesByTypeId.get(id);
        int size = nativeSize != null ? nativeSize : body != null ? reflectedLayoutSize(body) : 0;
        Structure result = new StructureDataType(target.categoryPath, target.name, size, dataTypeManager);
        result.setDescription("AZ SerializeContext reflected field type: " + name + " (" + id + "); " +
            (size > 0 ? "native size from by-value evidence" : "incomplete pointee; native size unknown"));
        return saveGeneratedLayout(result);
    }

    private boolean isReflectedValueWrapper(ElementData element) {
        String id = normalizeTypeId(element.typeId);
        return "6383F1D3-BB27-4E6B-A49A-6409B2059EAA".equals(id) ||
            "9F4E062E-06A0-46D4-85DF-E0DA96467D3A".equals(id);
    }

    private DataType enumFieldDataType(ElementData element, int size) {
        String underlyingId = enumUnderlyingTypes.get(normalizeTypeId(element.typeId));
        if (underlyingId == null) return null;
        ElementData underlying = new ElementData();
        underlying.typeId = underlyingId;
        DataType scalar = scalarFieldDataType(underlying, size);
        if (scalar == null) return null;
        String name = element.targetTypeName != null ? element.targetTypeName : element.typeName;
        if (name == null) name = datatypeNames.get(normalizeTypeId(element.typeId));
        DatatypeTarget target = datatypeTarget(element.typeId, name);
        if (target == null) return scalar;
        DataType existing = dataTypeManager.getDataType(target.categoryPath, target.name);
        if (existing != null && !isGeneratedReflectedDatatype(existing)) return existing;
        TypedefDataType result = new TypedefDataType(target.categoryPath, target.name, scalar, dataTypeManager);
        if (!applyRenames) return result;
        if (existing != null && !(existing instanceof Structure)) return existing;
        return dataTypeManager.addDataType(result, DataTypeConflictHandler.REPLACE_HANDLER);
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
        DatatypeTarget target = datatypeTarget(fieldClassData);
        if (target != null && datatypeConflictingPaths.contains(datatypePathName(target))) return null;
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
        if (datatypeConflictingPaths.contains(datatypePathName(target))) return null;

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
        if (element.isPointer) comment.append("; native pointer");
        String underlying = enumUnderlyingTypes.get(normalizeTypeId(element.typeId));
        if (underlying != null) comment.append("; enum underlying typeId: ").append(underlying);
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
        if (data.reflectionSource != null) description.append("; identity from ").append(data.reflectionSource);
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
        List<String> parts = cppScopeParts(data != null ? data.targetTypeName : typeName);
        if (parts.size() == 1 && data != null && hasOwner(data.ownerScope)) {
            result.addAll(data.ownerScope);
            if (ownerScopeNamesType(data.ownerScope, data.targetTypeName)) {
                registerReflectedClassScope(result);
                return result;
            }
        }
        result.addAll(parts);
        registerReflectedClassScope(result);
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
        recordTargetNameAddress(targetName, address, action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(callsite, action);
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
        recordTargetNameAddress(targetName, address, action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(callsite, action);
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

        recordTargetNameAddress(targetName, jsonAddress, action);

        action.addProperty("oldName", function.getName(true));
        boolean applied = false;
        if (applyRenames && !functionHasTargetName(function, scope, localName)) {
            applied = applyFunctionRename(function, scope, localName, action);
            if (!applied && action.has("reason")) {
                action.addProperty("applied", false);
                actions.add(action);
                return;
            }
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !functionHasTargetName(function, scope, localName)) {
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
        recordTargetNameAddress(targetName, jsonAddress, action);

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
        if (applyRenames && !functionHasTargetName(function, scope, localName)) {
            applied = applyFunctionRename(function, scope, localName, action);
        }
        action.addProperty("applied", applied);
        if (!applyRenames && !functionHasTargetName(function, scope, localName)) {
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

        if (functionHasTargetName(function, scope, localName)) {
            return false;
        }

        rememberTemplateScopeFragments(function.getParentNamespace());
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

        function.getSymbol().setNameAndNamespace(localName, namespace, SourceType.USER_DEFINED);
        return true;
    }

    private boolean functionHasTargetName(Function function, List<String> scope, String localName) {
        return function.getName().equals(localName) && namespaceScope(function.getParentNamespace()).equals(scope);
    }

    private boolean hasSplitTemplateScope(Namespace namespace) {
        if (!balancedCppDelimiters(namespace.getName(true))) return false;
        for (String part : namespaceScope(namespace)) if (!balancedCppDelimiters(part)) return true;
        return false;
    }

    private void rememberTemplateScopeFragments(Namespace namespace) {
        while (namespace != null && !namespace.isGlobal()) {
            if (!balancedCppDelimiters(namespace.getName()) || !balancedCppDelimiters(namespace.getName(true))) {
                templateScopeCandidates.add(namespace);
            }
            namespace = namespace.getParentNamespace();
        }
    }

    private void rememberSplitTemplateScopes(List<String> scope) {
        // Probe the legacy literal split for empty leftovers; never create scopes with it.
        Namespace parent = currentProgram.getGlobalNamespace();
        for (String part : String.join("::", scope).split("::")) {
            parent = symbols.getNamespace(part, parent);
            if (parent == null) return;
            if (!balancedCppDelimiters(part) || !balancedCppDelimiters(parent.getName(true))) templateScopeCandidates.add(parent);
        }
    }

    private void cleanupTemplateLabelNamespaces(ActionSink actions) throws Exception {
        for (Namespace owner : new ArrayList<>(templateScopeCandidates)) {
            monitor.checkCancelled();
            if (symbols.getSymbol(owner.getID()) == null) continue;
            List<String> oldScope = namespaceScope(owner);
            List<String> scope = normalizeRepeatedScope(oldScope);
            if (scope.equals(oldScope)) continue;
            ArrayList<Symbol> labels = new ArrayList<>();
            symbols.getSymbols(owner).forEachRemaining(symbol -> {
                if (symbol.getSymbolType().equals(SymbolType.LABEL)) labels.add(symbol);
            });
            for (Symbol label : labels) {
                monitor.checkCancelled();
                JsonObject action = new JsonObject();
                action.addProperty("kind", "template_label_namespace_cleanup");
                action.addProperty("symbolId", label.getID());
                action.addProperty("address", formatAddress(label.getAddress()));
                action.addProperty("name", fullName(scope, label.getName()));
                action.add("oldScope", listStringArray(oldScope));
                action.add("scope", listStringArray(scope));
                if (applyRenames) {
                    Namespace target = createScope(scope);
                    Symbol existing = symbols.getSymbol(label.getName(), label.getAddress(), target);
                    if (existing != null) {
                        if (label.isPrimary() && !existing.isPrimary()) existing.setPrimary();
                        if (!label.delete()) throw new IllegalStateException("Cannot merge template label: " + label.getName(true));
                        action.addProperty("mergedIntoSymbolId", existing.getID());
                    }
                    else label.setNamespace(target);
                }
                action.addProperty("applied", applyRenames);
                action.addProperty("wouldApply", !applyRenames);
                actions.add(action);
            }
        }
    }

    private void pruneEmptyTemplateScopeFragments(ActionSink actions) throws Exception {
        ArrayList<Namespace> candidates = new ArrayList<>(templateScopeCandidates);
        candidates.sort((left, right) -> Integer.compare(namespaceScope(right).size(), namespaceScope(left).size()));
        for (Namespace namespace : candidates) {
            monitor.checkCancelled();
            Symbol symbol = symbols.getSymbol(namespace.getID());
            if (symbol == null || (balancedCppDelimiters(symbol.getName()) && balancedCppDelimiters(namespace.getName(true))) ||
                (!balancedCppDelimiters(namespace.getName(true)) && reflectedClassScopePaths.contains(namespace.getName(true))) ||
                symbols.getSymbols(namespace).hasNext()) continue;
            JsonObject action = new JsonObject();
            action.addProperty("kind", "empty_template_namespace_cleanup");
            action.addProperty("name", namespace.getName(true));
            action.addProperty("symbolId", namespace.getID());
            if (applyRenames && !symbol.delete()) throw new IllegalStateException("Cannot remove empty template fragment: " + symbol.getName(true));
            action.addProperty("applied", applyRenames);
            action.addProperty("wouldApply", !applyRenames);
            actions.add(action);
        }
        if (applyRenames) scopeCache.clear();
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
        if (address == null) {
            return false;
        }
        String cacheKey = address.toString();
        Boolean cached = executableAddressCache.get(cacheKey);
        if (cached != null) {
            return cached;
        }
        MemoryBlock block = currentProgram.getMemory().getBlock(address);
        boolean executable = block != null && block.isExecute();
        executableAddressCache.put(cacheKey, executable);
        return executable;
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
        recordTargetNameAddress(targetName, jsonAddress, action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(address, action);
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
        recordTargetNameAddress(targetName, jsonAddress, action);

        if (actions.keepsFullActions()) {
            actions.captureOldNames(address, action);
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

    private void recordTargetNameAddress(
        String targetName,
        String jsonAddress,
        JsonObject action) {

        String targetAddress = addressKey(jsonAddress);
        String existingAddress = targetNameOwners.get(targetName);
        if (existingAddress == null) {
            targetNameOwners.put(targetName, targetAddress);
        }
        else if (!existingAddress.equals(targetAddress)) {
            action.addProperty("sameQualifiedNameAtOtherAddress", existingAddress);
        }
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
        if (scope == null || scope.isEmpty()) {
            return currentProgram.getGlobalNamespace();
        }
        String cacheKey = scopeKey(scope);
        Namespace cached = scopeCache.get(cacheKey);
        if (cached != null) {
            return cached;
        }

        Namespace parent = currentProgram.getGlobalNamespace();
        for (int i = 0; i < scope.size(); i++) {
            String part = scope.get(i);
            if (part == null || part.isEmpty()) {
                continue;
            }
            parent = getOrCreateScope(parent, part, isClassScope(scope, i));
        }
        scopeCache.put(cacheKey, parent);
        return parent;
    }

    private Namespace findScope(List<String> scope) {
        if (scope == null || scope.isEmpty()) {
            return currentProgram.getGlobalNamespace();
        }
        String cacheKey = scopeKey(scope);
        Namespace cached = scopeCache.get(cacheKey);
        if (cached != null) {
            return cached;
        }

        Namespace parent = currentProgram.getGlobalNamespace();
        for (String part : scope) {
            if (part == null || part.isEmpty()) {
                continue;
            }
            Namespace namespace = symbols.getNamespace(part, parent);
            if (namespace == null) {
                return null;
            }
            parent = namespace;
        }
        scopeCache.put(cacheKey, parent);
        return parent;
    }

    private String scopeKey(List<String> scope) {
        if (scope == null || scope.isEmpty()) {
            return "";
        }
        return String.join("\u0001", scope);
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
        if (index == 0 && ENGINE_NAMESPACE_ROOTS.contains(part)) {
            return false;
        }
        if (isBehaviorEventScope(part)) {
            return false;
        }
        String path = String.join("::", scope.subList(0, index + 1));
        if (reflectedClassScopePaths.contains(path)) return true;
        if (reflectedNamespaceScopePaths.contains(path)) return false;
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
        for (Symbol symbol : symbolsAt(address)) {
            result.add(symbol.getName(true));
        }
        return result;
    }

    private ArrayList<Symbol> symbolsAt(Address address) {
        ArrayList<Symbol> result = new ArrayList<>();
        SymbolIterator symbolIterator = symbols.getSymbolsAsIterator(address);
        while (symbolIterator.hasNext()) {
            result.add(symbolIterator.next());
        }
        return result;
    }

    private Address parseCaptureAddress(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        Matcher moduleMatch = MODULE_ADDR_RE.matcher(value);
        if (moduleMatch.matches()) {
            if (!isNewWorldCaptureModule(moduleMatch.group("module"))) return null;
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

    private static boolean isNewWorldCaptureModule(String module) {
        return "NewWorld".equalsIgnoreCase(module) || "NewWorld.exe".equalsIgnoreCase(module);
    }

    private Address readPointer(Address address) {
        if (!isProgramAddress(address)) {
            return null;
        }
        String cacheKey = address.toString();
        if (pointerReadCache.containsKey(cacheKey)) {
            return pointerReadCache.get(cacheKey);
        }
        try {
            long value = getLong(address);
            if (value == 0) {
                pointerReadCache.put(cacheKey, null);
                return null;
            }
            Address pointer = currentProgram.getAddressFactory()
                .getDefaultAddressSpace()
                .getAddress(value);
            pointerReadCache.put(cacheKey, pointer);
            return pointer;
        }
        catch (Exception ignored) {
            pointerReadCache.put(cacheKey, null);
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
            if (!isNewWorldCaptureModule(moduleMatch.group("module"))) return null;
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
            element.isPointer = nativeElementIsPointer(elementObject);
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
            return archiveEntryName == null ? "json" : file.getName().toLowerCase(Locale.ROOT).endsWith(".zip") ? "zip" : "7z";
        }

        Reader openReader() throws Exception {
            if (archiveEntryName == null) {
                return new InputStreamReader(new FileInputStream(file), StandardCharsets.UTF_8);
            }
            if (format().equals("zip")) {
                ZipFile archive = new ZipFile(file);
                try {
                    ZipEntry entry = archive.getEntry(archiveEntryName);
                    if (entry == null) throw new IOException("archive entry not found: " + archiveEntryName);
                    return new InputStreamReader(new java.io.FilterInputStream(archive.getInputStream(entry)) {
                        @Override
                        public void close() throws IOException {
                            try { super.close(); }
                            finally { archive.close(); }
                        }
                    }, StandardCharsets.UTF_8);
                }
                catch (Exception error) {
                    archive.close();
                    throw error;
                }
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

    private static class BehaviorClassMetadata {
        String typeId;
        String name;
        Integer size;
        Integer alignment;
    }

    private class BehaviorContextEvidence {
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
        final Set<String> seenClasses = new HashSet<>();
        final JsonArraySpool classInventory = new JsonArraySpool(true, !dryRun);
        final JsonArraySpool globalMethodInventory = new JsonArraySpool(true, !dryRun);
        final JsonArraySpool globalPropertyInventory = new JsonArraySpool(true, !dryRun);
        final JsonArraySpool ebusInventory = new JsonArraySpool(true, !dryRun);
        final JsonArray typeToClassInventory = new JsonArray();
        final JsonArray onDemandInventory = new JsonArray();
        final JsonArray constantInventory = new JsonArray();
        final JsonObject contextMetadata = new JsonObject();
        final Map<String, Integer> unresolvedCounts = new LinkedHashMap<>();
        final Map<String, Integer> attributeCounts = new LinkedHashMap<>();
        final ArrayList<BehaviorConstantProperty> globalProperties = new ArrayList<>();
        final Map<String, JsonObject> classFactsByTypeId = new LinkedHashMap<>();
        final ArrayList<BehaviorFunctionCandidate> functionCandidates = new ArrayList<>();
        final LinkedHashMap<String, BehaviorFunctionGroup> groupsByAddress =
            new LinkedHashMap<>();
        final LinkedHashMap<String, RttiType> rttiTypes = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> kindCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> classNameCounts = new LinkedHashMap<>();
        final LinkedHashMap<String, Integer> busNameCounts = new LinkedHashMap<>();
        final LinkedHashSet<String> classTypeIds = new LinkedHashSet<>();
        final LinkedHashMap<String, String> typeNamesByTypeId = new LinkedHashMap<>();
        final LinkedHashMap<String, BehaviorClassMetadata> classesByTypeId = new LinkedHashMap<>();
        final LinkedHashSet<String> layoutConflicts = new LinkedHashSet<>();
        final LinkedHashSet<String> duplicateBusNames = new LinkedHashSet<>();
        final ArrayList<String> skippedReasons = new ArrayList<>();
    }

    private static class BehaviorConstantProperty {
        String name;
        String getter;
        boolean writable;
    }

    private static class BehaviorFunctionCandidate {
        String address;
        String ownerTypeId;
        ArrayList<String> scope;
        String localName;
        String kind;
        String slot;
        String methodName;
        JsonObject metadata;
        JsonObject ownerMetadata;
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
        String reflectionSource;
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
        final LinkedHashMap<String, Integer> signatureReasonCounts = new LinkedHashMap<>();
        int checkedSignatureCount;
        int wouldApplyCount;
        int appliedCount;
    }

    private class ActionSink {
        final ActionStats stats = new ActionStats();
        final JsonObject signatureSamples = new JsonObject();
        private final boolean keepFullActions;
        private final JsonArraySpool fullActionSpool;
        private final JsonArraySpool symbolNameSnapshots;
        private final Set<String> capturedSymbolNames = new HashSet<>();
        private final LinkedHashMap<String, JsonArray> samplesByReason =
            new LinkedHashMap<>();
        private int actionCount;

        ActionSink(boolean keepFullActions) {
            this.keepFullActions = keepFullActions;
            this.fullActionSpool = new JsonArraySpool(false, keepFullActions && !dryRun, "actions");
            this.symbolNameSnapshots = new JsonArraySpool(false, keepFullActions && !dryRun, "symbol-names");
        }

        void captureOldNames(Address address, JsonObject action) {
            String key = address.toString();
            if (capturedSymbolNames.add(key)) {
                JsonObject snapshot = new JsonObject();
                snapshot.addProperty("address", key);
                snapshot.add("names", oldNames(address));
                symbolNameSnapshots.add(snapshot);
            }
            action.addProperty("oldNamesRef", key);
        }

        void add(JsonObject action) {
            if (validating || (applyRenames && validationReport != null)) {
                int before = validationErrors;
                if (action.has("error")) recordValidationActionError(action, action);
                JsonArray failures = action.getAsJsonArray("fieldFailureSamples");
                if (failures != null) {
                    for (JsonElement value : failures) {
                        JsonObject failure = value.getAsJsonObject();
                        if (failure.has("error")) recordValidationActionError(action, failure);
                    }
                }
                if (!validating && validationErrors > before) throw new ReflectionActionException(action);
            }
            actionCount++;
            increment(stats.kindCounts, stringMember(action, "kind"));
            String reason = stringMember(action, "reason");
            increment(stats.reasonCounts, reason);
            if (boolMember(action, "signatureChecked") == Boolean.TRUE) {
                stats.checkedSignatureCount++;
                increment(stats.signatureReasonCounts, reason);
                String key = stringMember(action, "kind") + "|" + (reason == null ? "none" : reason);
                JsonArray signatures = signatureSamples.getAsJsonArray(key);
                if (signatures == null) {
                    signatures = new JsonArray();
                    signatureSamples.add(key, signatures);
                }
                if (signatures.size() < MAX_SUMMARY_ACTION_EXAMPLES) signatures.add(actionSample(action));
            }
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
                fullActionSpool.add(action);
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
            return fullActionSpool.readArray();
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
        String canonicalPathName;
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
        int malformedRecords;
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
        int malformedRecords;
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

    private static class InstanceVtableName {
        List<String> extraScope = Collections.emptyList();
        String localName;
        String semanticName;
    }

    private static class ThisAdjustThunk {
        Address address;
        Address targetAddress;
        int adjustment;
    }

    private static class InferredStaticRttiHelper {
        String typeName;
        Address typeNameAddress;
        Address vtableAddress;
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
