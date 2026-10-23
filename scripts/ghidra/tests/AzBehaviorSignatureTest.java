import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/**
 * Types BehaviorContext functions from captured signatures in the MSVC x64 ABI: receivers, by-value class results
 * through a caller buffer after the receiver, large by-value classes by address, float registers, argument names,
 * shared bodies whose signatures disagree, unsized by-value types, undecoded signatures, EBus events (the bus
 * interface's member, shared by Broadcast and Event), callbacks not yet checked against the binary and retries.
 */
public class AzBehaviorSignatureTest {
    private static final String WIDGET = "11111111-1111-4111-8111-111111111111";
    private static final String HANDLE = "22222222-2222-4222-8222-222222222222";
    private static final String UNSIZED = "33333333-3333-4333-8333-333333333333";
    private static final String BOOL = "A0CA880C-AFE4-43CB-926C-59AC48496112";
    private static final String INT = "72039442-EB38-4D42-A1AD-CB68F7E0EEF6";
    private static final String FLOAT = "EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D";
    private static final String VOID = "C0F1AFAD-5CB3-450E-B0F5-ADB5D46B0E22";

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-behavior-signature-");
        int transaction = program.startTransaction("BehaviorContext signatures");
        try {
            program.setImageBase(program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
            program.getMemory().createInitializedBlock(".text", program.getImageBase().add(0x100), 0x100, (byte) 0xc3,
                TaskMonitor.DUMMY, false).setExecute(true);
            for (int offset = 0x100; offset < 0x190; offset += 0x10) {
                var address = program.getImageBase().add(offset);
                program.getFunctionManager().createFunction("FUN_" + offset, address, new AddressSet(address), SourceType.DEFAULT);
            }

            String capture = """
                {
                  "typeToClassMap":[
                    {"typeId":"%1$s","className":"Javelin::Widget","second":{"name":"Javelin::Widget","typeId":"%1$s",
                      "size":"16","alignment":"8",
                      "methods":[
                        {"first":"Contains","second":{"implementationFunction":"NewWorld+0x100","signature":%3$s}},
                        {"first":"GetHandle","second":{"implementationFunction":"NewWorld+0x110","signature":%4$s}},
                        {"first":"IsEmpty","second":{"implementationFunction":"NewWorld+0x140","signature":%7$s}},
                        {"first":"Count","second":{"implementationFunction":"NewWorld+0x140","signature":%8$s}}]}},
                    {"typeId":"%2$s","className":"Javelin::Handle","second":{"name":"Javelin::Handle","typeId":"%2$s",
                      "size":"8","alignment":"8"}}],
                  "methods":[
                    {"first":"MakeHandle","second":{"implementationFunction":"NewWorld+0x120","signature":%5$s}},
                    {"first":"Scale","second":{"implementationFunction":"NewWorld+0x130","signature":%6$s}},
                    {"first":"Consume","second":{"implementationFunction":"NewWorld+0x150","signature":%9$s}},
                    {"first":"Opaque","second":{"implementationFunction":"NewWorld+0x160","signature":%10$s}}],
                  "ebuses":[{"name":"Javelin::LookupBus",
                    "createHandler":{"implementationFunction":"NewWorld+0x180","signature":%13$s},
                    "events":[{"name":"Lookup",
                      "broadcast":{"implementationFunction":"NewWorld+0x170","virtualDispatchSlotOffset":"0x28","signature":%11$s},
                      "event":{"implementationFunction":"NewWorld+0x170","virtualDispatchSlotOffset":"0x28","signature":%12$s}}]}]
                }
                """.formatted(WIDGET, HANDLE,
                signature(param("bool", BOOL, 0), param("Widget*", WIDGET, 9), param("const Widget&", WIDGET, 4)),
                signature(param("Handle", HANDLE, 0), param("Widget*", WIDGET, 9)),
                signature(param("Handle", HANDLE, 0), param("int", INT, 0)),
                signature(param("float", FLOAT, 0), named(param("const Widget", WIDGET, 2), "widget"),
                    named(param("float", FLOAT, 0), "factor")),
                signature(param("bool", BOOL, 0), param("Widget*", WIDGET, 9)),
                signature(param("int", INT, 0), param("Widget*", WIDGET, 9)),
                signature(param("void", VOID, 0), param("Mystery", UNSIZED, 0)),
                "{\"result\":null,\"arguments\":[],\"argumentMetadata\":[],\"accessors\":{},\"error\":\"unrecognized accessor\"}",
                signature(param("Handle", HANDLE, 0), param("int", INT, 0)),
                signature(param("Handle", HANDLE, 0), param("Handle", HANDLE, 0), param("int", INT, 0)),
                signature(param("bool", BOOL, 0), param("BehaviorEBusHandler*", UNSIZED, 1)));
            Files.writeString(resources.resolve("behavior-context.json"), capture);

            run(program, resources, "--dry-run");
            check(function(program, 0x100).getParameterCount() == 0, "dry run typed a function");
            run(program, resources);

            check(signature(program, 0x100).equals("bool Contains(Widget * this, Widget * arg1)"),
                "receiver and reference: " + signature(program, 0x100));
            check(signature(program, 0x110).equals("Handle * GetHandle(Widget * this, Handle * result)") &&
                storage(program, 0x110).equals(List.of("RCX:8", "RDX:8")),
                "member class result after receiver: " + signature(program, 0x110) + " " + storage(program, 0x110));
            check(signature(program, 0x120).equals("Handle * MakeHandle(Handle * result, int arg1)") &&
                storage(program, 0x120).equals(List.of("RCX:8", "EDX:4")),
                "free class result first: " + signature(program, 0x120) + " " + storage(program, 0x120));
            check(signature(program, 0x130).equals("float Scale(Widget * widget, float factor)") &&
                storage(program, 0x130).get(1).startsWith("XMM1"),
                "large by-value class by address, float register: " + signature(program, 0x130) + " " + storage(program, 0x130));
            check(function(program, 0x140).getParameterCount() == 0 && function(program, 0x150).getParameterCount() == 0 &&
                function(program, 0x160).getParameterCount() == 0 && function(program, 0x180).getParameterCount() == 0,
                "skipped bodies were typed");
            check(signature(program, 0x170).startsWith("Handle * ") &&
                signature(program, 0x170).endsWith("(void * this, Handle * result, int arg1)") &&
                storage(program, 0x170).equals(List.of("RCX:8", "RDX:8", "R8D:4")),
                "EBus interface member: " + signature(program, 0x170) + " " + storage(program, 0x170));

            JsonObject report = report(resources);
            List<String> reasons = new ArrayList<>();
            collectReasons(report, reasons);
            check(reasons.contains("shared-body-signature-conflict") && reasons.contains("parameter-size-unknown") &&
                reasons.contains("signature-not-decoded") && reasons.contains("signature-kind-unverified"),
                "skip reasons: " + reasons);
            run(program, resources);
            check(report(resources).get("appliedCount").getAsInt() == 0, "retry changed signatures");
            System.out.println("PASS: BehaviorContext signatures: receivers, class results through caller buffers, " +
                "large by-value classes, float registers, argument names, shared-body conflicts, unsized types, " +
                "undecoded signatures, EBus interface members, unverified callbacks and retries");
        }
        finally {
            program.endTransaction(transaction, false);
            program.release(consumer);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static String param(String spelling, String typeId, int traits) {
        return "{\"name\":\"" + spelling + "\",\"typeId\":\"" + typeId + "\",\"azRtti\":null,\"traits\":" + traits + "}";
    }

    private static String named(String parameter, String name) {
        return parameter + "|" + name;
    }

    private static String signature(String result, String... arguments) {
        StringBuilder values = new StringBuilder();
        StringBuilder metadata = new StringBuilder();
        for (String argument : arguments) {
            String[] parts = argument.split("\\|");
            if (values.length() > 0) {
                values.append(',');
                metadata.append(',');
            }
            values.append(parts[0]);
            metadata.append("{\"name\":\"").append(parts.length > 1 ? parts[1] : "")
                .append("\",\"toolTip\":\"\",\"defaultValue\":null}");
        }
        return "{\"result\":" + result + ",\"arguments\":[" + values + "],\"argumentMetadata\":[" + metadata +
            "],\"accessors\":{}}";
    }

    private static Function function(ProgramDB program, int offset) {
        return program.getFunctionManager().getFunctionAt(program.getImageBase().add(offset));
    }

    private static String signature(ProgramDB program, int offset) {
        Function function = function(program, offset);
        StringBuilder text = new StringBuilder(function.getReturnType().getName()).append(' ')
            .append(function.getName()).append('(');
        var parameters = function.getParameters();
        for (int i = 0; i < parameters.length; i++) {
            if (i > 0) text.append(", ");
            text.append(parameters[i].getDataType().getName()).append(' ').append(parameters[i].getName());
        }
        return text.append(')').toString();
    }

    private static List<String> storage(ProgramDB program, int offset) {
        return Arrays.stream(function(program, offset).getParameters())
            .map(parameter -> parameter.getVariableStorage().toString()).toList();
    }

    private static void collectReasons(JsonElement value, List<String> reasons) {
        if (value.isJsonArray()) value.getAsJsonArray().forEach(item -> collectReasons(item, reasons));
        if (!value.isJsonObject()) return;
        JsonObject object = value.getAsJsonObject();
        if (object.has("kind") && object.get("kind").getAsString().equals("behavior_signature") && object.has("reason")) {
            reasons.add(object.get("reason").getAsString());
        }
        object.entrySet().forEach(entry -> collectReasons(entry.getValue(), reasons));
    }

    private static void run(ProgramDB program, Path resources, String... extra) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        var arguments = new ArrayList<>(List.of("--resources", resources.toString(), "--full-actions"));
        arguments.addAll(List.of(extra));
        script.setScriptArgs(arguments.toArray(String[]::new));
        script.run();
    }

    private static JsonObject report(Path resources) throws Exception {
        return JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
