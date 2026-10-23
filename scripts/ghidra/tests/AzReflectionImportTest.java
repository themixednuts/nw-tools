import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

import com.google.gson.GsonBuilder;
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
import ghidra.program.model.data.Structure;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Exercises the single entrypoint in an isolated ProgramDB and streams the real resources. */
public class AzReflectionImportTest {
    private static final Class<?> SCRIPT = AzReflectionRenamer.class;
    private static final String BOX = "11111111-1111-4111-8111-111111111111";
    private static final String ONLY = "22222222-2222-4222-8222-222222222222";
    private static final String MODULE = "33333333-3333-4333-8333-333333333333";

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        Path resources = Files.createTempDirectory("az-reflection-import-");
        int transaction = program.startTransaction("combined import regression");
        try {
            program.setImageBase(program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
            for (int offset : List.of(0x100, 0x110, 0x120, 0x130)) {
                var address = program.getImageBase().add(offset);
                program.getMemory().createInitializedBlock("fixture" + offset, address, 8, (byte) 0xc3, TaskMonitor.DUMMY, false).setExecute(true);
                program.getFunctionManager().createFunction("FUN_" + offset, address, new AddressSet(address), SourceType.DEFAULT);
            }
            // Module vtable whose slot 1 (RTTI_GetTypeName) is LEA RAX,[name]; RET.
            var moduleVtable = program.getImageBase().add(0x2000);
            program.getMemory().createInitializedBlock("module-vtable", moduleVtable, 16, (byte) 0, TaskMonitor.DUMMY, false);
            program.getMemory().setLong(moduleVtable.add(8), program.getImageBase().add(0x3000).getOffset());
            var moduleName = program.getImageBase().add(0x3000);
            program.getMemory().createInitializedBlock("module-name", moduleName, 0x30, (byte) 0, TaskMonitor.DUMMY, false).setExecute(true);
            program.getMemory().setBytes(moduleName, new byte[]{0x48, (byte) 0x8d, 0x05, 0x09, 0, 0, 0, (byte) 0xc3});
            program.getMemory().setBytes(moduleName.add(0x10), "BinaryNamedModule\0".getBytes(StandardCharsets.US_ASCII));
            String serialize = """
                {"uuidMap":{"%s":{"name":"Box","typeId":"%s","version":"1","attributes":[],
                 "azRtti":{"typeName":"MB::Box<Javelin::Payload>","typeId":"%s"},
                 "elements":[{"name":"m_count","typeId":"43DA906B-7DEF-4CA8-9790-854106D3F983",
                   "offset":"0","dataSize":"4","is_pointer":false,"is_base_class":false,"attributes":[]}]}}}
                """.formatted(BOX, BOX, BOX);
            String behavior = """
                {"classes":[
                  {"name":"Box","typeId":"%s","size":"32","alignment":"16",
                   "m_defaultConstructor":"NewWorld+0x100",
                   "m_methods":[{"m_name":"Convert<Javelin::Message>","m_functionPtr":"NewWorld+0x110"}],
                   "m_properties":[{"name":"Count","m_getter":{"m_functionPtr":"NewWorld+0x120"}}]},
                  {"name":"Javelin::Only<AZ::Uuid>","typeId":"%s","m_size":"16","m_alignment":"8",
                   "m_destructor":"NewWorld+0x130"}], "methods":[],"properties":[],"ebuses":[]}
                """.formatted(BOX, ONLY);
            String module = """
                {"vftable":"NewWorld+0x7f4e080","descriptors":[
                 {"componentName":"Javelin::ModuleOnly<MB::Key>","componentUuid":"%s",
                  "azRtti":{"typeId":"%s","typeName":"Javelin::ModuleOnly<MB::Key>"},"vtableSlots":[]}]}
                """.formatted(MODULE, MODULE);
            Files.writeString(resources.resolve("serialize.json"), serialize);
            Files.writeString(resources.resolve("behavior-context.json"), behavior);
            Files.writeString(resources.resolve("typeregistry.json"), "invalid JSON: must stay excluded");
            Files.writeString(resources.resolve("reflection-name-repair.json"), "old report: must not be imported");
            Files.writeString(resources.resolve("output"), "legacy report file: keep intact");
            Files.createDirectories(resources.resolve("modules"));
            Files.writeString(resources.resolve("modules/cry-hooks-module.json"), module);
            Files.writeString(resources.resolve("modules/binary-named-module.json"), "{\"vftable\":\"NewWorld+0x2000\",\"descriptors\":[]}");
            Files.writeString(resources.resolve("modules/unknown-module.json"), "{\"vftable\":\"NewWorld+0x2fff0\",\"descriptors\":[]}");
            Path output = Files.createDirectories(resources.resolve("output-directory"));
            Path priorReport = output.resolve("old-report.json");
            Files.writeString(priorReport, "{}");
            check(call(script(program), "reflectionResourcesDirectory", output.toFile()).equals(resources.toFile()),
                "choosing an output directory inside reflection resources did not select the capture root");
            check(call(script(program), "reflectionResourcesDirectory", priorReport.toFile()).equals(resources.toFile()),
                "choosing a file inside output did not select the capture root");
            check(call(script(program), "reflectionResourcesDirectory", resources.resolve("modules/cry-hooks-module.json").toFile()).equals(resources.toFile()),
                "choosing a module capture did not select the capture root");

            int beforePreview = program.getDataTypeManager().getDataTypeCount(true);
            run(program, resources, "--dry-run");
            check(program.getDataTypeManager().getDataTypeCount(true) == beforePreview, "preview performs no datatype writes");
            check(!Files.exists(resources.resolve("reflection-report.json")), "preview preserves applied report");
            JsonObject validation = JsonParser.parseString(Files.readString(resources.resolve("reflection-dry-run.json"))).getAsJsonObject();
            check(validation.get("status").getAsString().equals("passed") &&
                validation.get("appliedCount").getAsInt() == 0, "preview saves diagnostics without program edits");
            run(program, resources);
            check(Files.readString(resources.resolve("output")).equals("legacy report file: keep intact"), "legacy output file remains intact");
            JsonObject moduleReport = report(resources).getAsJsonObject("moduleEvidence");
            check(moduleReport.getAsJsonArray("modules").toString().contains("\"BinaryNamedModule\"") &&
                moduleReport.getAsJsonArray("modules").toString().contains("\"CryHooksModule\"") &&
                moduleReport.get("unrecognizedInputs").getAsInt() == 1 && moduleReport.get("skippedInputs").getAsInt() == 0,
                "module names come from RTTI_GetTypeName; an unnamed capture is reported without blocking: " + moduleReport);
            Structure box = type(program, "/MB", "Box<Javelin::Payload>");
            check(box.getLength() == 32 && box.getAlignment() == 16, "BehaviorContext native size and alignment extend serialized subset");
            check("m_count".equals(box.getComponentAt(0).getFieldName()), "serialized member name retained");
            check(box.getNumDefinedComponents() == 1, "API property does not become a guessed object field");
            Structure only = type(program, "/Javelin", "Only<AZ::Uuid>");
            check(only.getLength() == 16 && only.getAlignment() == 8 && only.getNumDefinedComponents() == 0,
                "Behavior-only class is named opaque native storage");
            Structure moduleOnly = type(program, "/Javelin", "ModuleOnly<MB::Key>");
            check(moduleOnly.isZeroLength(), "module-only identity is an incomplete declaration");
            check(program.getFunctionManager().getFunctionAt(program.getImageBase().add(0x100)).getName(true)
                .equals("MB::Box<Javelin::Payload>::DefaultConstructor"), "string callback addresses are imported");
            check(program.getFunctionManager().getFunctionAt(program.getImageBase().add(0x110)).getName(true)
                .equals("MB::Box<Javelin::Payload>::Convert<Javelin::Message>"), "method resolves canonical UUID owner with both templates");
            JsonObject report = report(resources);
            check(report.get("typeRegistry").getAsString().equals("excluded"), "TypeRegistry is excluded explicitly");
            check(report.getAsJsonArray("reflectedTypes").size() == 3, "one joined type inventory");
            JsonObject boxReport = report.getAsJsonArray("reflectedTypes").get(0).getAsJsonObject();
            check(boxReport.getAsJsonArray("definedFields").get(0).getAsJsonObject().get("name").getAsString().equals("m_count"),
                "report includes actual defined fields and types");
            check(report.getAsJsonArray("actions").size() > 0, "full action evidence included by default");
            Object boxId = box.getUniversalID();
            run(program, resources);
            check(report(resources).get("appliedCount").getAsInt() == 0, "combined import is idempotent");
            check(boxId.equals(type(program, "/MB", "Box<Javelin::Payload>").getUniversalID()), "rerun keeps datatype identity");

            try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(resources.resolve("behavior-context.json.zip")))) {
                zip.putNextEntry(new ZipEntry("capture/behavior-context.json"));
                zip.write(behavior.getBytes(StandardCharsets.UTF_8));
                zip.closeEntry();
            }
            Files.delete(resources.resolve("behavior-context.json"));
            Files.delete(resources.resolve("serialize.json"));
            run(program, resources);
            report = report(resources);
            check(!report.get("serializeContextLoaded").getAsBoolean(), "SerializeContext optional for Behavior/module import");
            check(report.getAsJsonObject("behaviorContextEvidence").get("inputFormat").getAsString().equals("zip"), "ZIP streamed without extraction");
            check(program.getFunctionManager().getFunctionAt(program.getImageBase().add(0x110)).getName(true)
                .equals("MB::Box<Javelin::Payload>::Convert<Javelin::Message>"), "stored UUID identity keeps qualified owner when SerializeContext absent");

            Files.writeString(resources.resolve("behavior-context.json"), "{\"classes\":[");
            int beforeMalformed = program.getDataTypeManager().getDataTypeCount(true);
            try {
                run(program, resources);
                throw new AssertionError("malformed capture must fail before mutation");
            }
            catch (java.io.IOException expected) {
                check(program.getDataTypeManager().getDataTypeCount(true) == beforeMalformed, "malformed input leaves datatypes intact");
            }
            Files.delete(resources.resolve("behavior-context.json"));
            Files.delete(resources.resolve("behavior-context.json.zip"));

            Files.writeString(resources.resolve("serialize-field-registration.jsonl"), "{\"truncated\":");
            try {
                run(program, resources);
                throw new AssertionError("malformed registration trace must fail before mutation");
            }
            catch (java.io.IOException expected) {
                check(program.getDataTypeManager().getDataTypeCount(true) == beforeMalformed, "malformed trace leaves datatypes intact");
            }
            Files.delete(resources.resolve("serialize-field-registration.jsonl"));
            try {
                run(program, resources, "--class-registration", resources.resolve("missing.jsonl").toString());
                throw new AssertionError("missing explicit trace must fail");
            }
            catch (IllegalArgumentException expected) {
                check(expected.getMessage().contains("missing.jsonl"), "missing explicit trace names its path");
            }
            run(program, resources);
            check(report(resources).getAsJsonObject("moduleEvidence").get("descriptorCount").getAsInt() == 1,
                "module-only resources work");
            if (args.length > 1) checkRealResources(program, new File(args[1]));
            System.out.println("PASS: unified import, templates, native sizes/alignment, properties, module-only identities, ZIP, optional captures, exclusion, preview, errors and reruns");
        }
        finally {
            program.endTransaction(transaction, false);
            program.release(consumer);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void checkRealResources(ProgramDB program, File resources) throws Exception {
        AzReflectionRenamer script = script(program);
        try {
        set(script, "cli", GhidraCli.parse(new String[0], Set.of(), Set.of(), 0));
        set(script, "gson", new GsonBuilder().create());
        File anchor = new File(resources, "serialize.json");
        Object modules = call(script, "loadModuleEvidence", anchor);
        set(script, "moduleEvidence", modules);
        check((int) field(modules, "skippedInputs") == 0 && (int) field(modules, "unrecognizedInputs") == 0,
            "all actual module captures recognized");
        Object behavior = call(script, "loadBehaviorContextEvidence", anchor);
        set(script, "behaviorContextEvidence", behavior);
        check((int) field(behavior, "skippedInputs") == 0 && (int) field(behavior, "classCount") == 611,
            "full plaintext BehaviorContext capture streamed");
        check((int) field(behavior, "globalPropertyCount") == 2647 &&
            ((com.google.gson.JsonArray) field(behavior, "onDemandInventory")).size() == 168,
            "all global properties and on-demand reflection entries parsed");
        check(spoolSize(field(behavior, "classInventory")) == 611 &&
            spoolSize(field(behavior, "ebusInventory")) == 626 &&
            ((com.google.gson.JsonArray) field(behavior, "typeToClassInventory")).size() == 611,
            "class, EBus and type map inventories retain every record without duplicating classes");
        check((int) field(behavior, "ebusEventCount") > 2000 &&
            (int) field(behavior, "virtualEventFunctionCount") > 1000,
            "actual normalized event dispatch fields are parsed as dispatchers, not wrappers");
        script.setScriptArgs(new String[0]);
        set(script, "cli", GhidraCli.parse(new String[]{"--behavior-context", new File(resources, "behavior-context.json.zip").getAbsolutePath()},
            Set.of("behavior-context"), Set.of(), 0));
        Object zip = call(script, "loadBehaviorContextEvidence", anchor);
        check((int) field(zip, "skippedInputs") == 0 && field(zip, "classCount").equals(field(behavior, "classCount")) &&
            field(zip, "classMethodCount").equals(field(behavior, "classMethodCount")) &&
            field(zip, "ebusEventCount").equals(field(behavior, "ebusEventCount")) &&
            field(zip, "virtualEventFunctionCount").equals(field(behavior, "virtualEventFunctionCount")),
            "real ZIP and JSON capture agree including full EBus dispatch");
        System.out.println("Real resource counts: modules=" + field(modules, "inputCount") + ", descriptors=" + field(modules, "descriptorCount") +
            ", BehaviorContext classes=" + field(behavior, "classCount") + ", class methods=" + field(behavior, "classMethodCount") +
            ", class properties=" + field(behavior, "classPropertyCount") + ", EBuses=" + field(behavior, "ebusCount") +
            ", EBus events=" + field(behavior, "ebusEventCount") + ", virtual dispatch uses=" + field(behavior, "virtualEventFunctionCount") +
            ", resolved/missing virtual properties=" + field(behavior, "virtualPropertyEventNameResolvedCount") +
            "/" + field(behavior, "virtualPropertyEventNameMissingCount") +
            ", on-demand reflectors=" + ((com.google.gson.JsonArray) field(behavior, "onDemandInventory")).size());
        }
        finally {
            Method close = SCRIPT.getDeclaredMethod("closeTemporarySpools");
            close.setAccessible(true);
            close.invoke(script);
        }
    }

    private static AzReflectionRenamer script(ProgramDB program) {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        return script;
    }

    private static void run(ProgramDB program, Path resources, String... extra) throws Exception {
        AzReflectionRenamer script = script(program);
        var arguments = new java.util.ArrayList<>(List.of("--resources", resources.toString()));
        arguments.addAll(List.of(extra));
        script.setScriptArgs(arguments.toArray(String[]::new));
        script.run();
    }

    private static JsonObject report(Path resources) throws Exception {
        return JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
    }

    private static Structure type(ProgramDB program, String category, String name) {
        var value = program.getDataTypeManager().getDataType(new CategoryPath(category), name);
        check(value instanceof Structure, "missing datatype " + category + "/" + name);
        return (Structure) value;
    }

    private static Object call(Object target, String name, File file) throws Exception {
        Method method = SCRIPT.getDeclaredMethod(name, File.class);
        method.setAccessible(true);
        return method.invoke(target, file);
    }

    private static Object field(Object target, String name) throws Exception {
        Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    private static int spoolSize(Object spool) throws Exception {
        Method size = spool.getClass().getDeclaredMethod("size");
        size.setAccessible(true);
        return (int) size.invoke(spool);
    }

    private static void set(Object target, String name, Object value) throws Exception {
        Field field = SCRIPT.getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
