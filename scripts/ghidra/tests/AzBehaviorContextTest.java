import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import com.google.gson.JsonArray;
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
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.Enum;
import ghidra.program.model.data.EnumDataType;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.CommentType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Checks that BehaviorContext evidence is applied to an isolated database. */
public class AzBehaviorContextTest {
    private static final String TYPE = "11111111-1111-4111-8111-111111111111";

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("NewWorld.exe", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        Path resources = Files.createTempDirectory("az-behavior-evidence-");
        int transaction = program.startTransaction("BehaviorContext regression");
        try {
            program.setImageBase(program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
            for (int offset : List.of(0x100, 0x110, 0x120, 0x130, 0x140, 0x150, 0x160, 0x170, 0x180, 0x190, 0x1a0, 0x1b0)) {
                var address = program.getImageBase().add(offset);
                program.getMemory().createInitializedBlock("fixture" + offset, address, 16, (byte) 0xcc,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().setByte(address, (byte) 0xc3);
                program.getFunctionManager().createFunction("FUN_" + offset, address, new AddressSet(address), SourceType.DEFAULT);
            }
            program.getMemory().setBytes(program.getImageBase().add(0x100), new byte[]{(byte) 0xb8, 2, 0, 0, 0, (byte) 0xc3});
            program.getMemory().setBytes(program.getImageBase().add(0x110),
                new byte[]{(byte) 0xb8, (byte) 0xff, (byte) 0xff, (byte) 0xff, (byte) 0xff, (byte) 0xc3});
            program.getMemory().setBytes(program.getImageBase().add(0x120),
                new byte[]{0x48, (byte) 0xb8, 0, 0, 0, 0, 0, 0, 0, (byte) 0x80, (byte) 0xc3});
            program.getMemory().setBytes(program.getImageBase().add(0x130), new byte[]{0x31, (byte) 0xc0, (byte) 0xc3});
            program.getMemory().setBytes(program.getImageBase().add(0x140), new byte[]{0x48, (byte) 0x8b, 0x01, (byte) 0xc3});
            var vtableAddress = program.getImageBase().add(0x1000);
            program.getMemory().createInitializedBlock("adapter", vtableAddress, 16, (byte) 0, TaskMonitor.DUMMY, false);
            EnumDataType manual = new EnumDataType(new CategoryPath("/AZ/BehaviorContext/Constants"), "Manual_32bit", 4);
            manual.add("Keep", 7);
            program.getDataTypeManager().addDataType(manual, DataTypeConflictHandler.DEFAULT_HANDLER);

            String capture = """
                {
                  "m_onDemandReflection":[{"key":"%s","value":"NewWorld+0x150"}],
                  "typeToClassMap":[{"typeId":"%s","className":"MB::Box<Javelin::Payload>",
                    "second":{"name":"MB::Box<Javelin::Payload>","typeId":"%s","size":"16","alignment":"8",
                      "baseClasses":["22222222-2222-4222-8222-222222222222"],
                      "requestBuses":["MB::RequestBus<Javelin::Id>"],"notificationBuses":["MB::NotificationBus"],
                      "wrappedTypeId":"33333333-3333-4333-8333-333333333333",
                      "attributes":[{"attributeName":"ScriptStorage","attributeId":123,"value":{"kind":"unknown"}}],
                      "constructors":[{"implementationFunction":"NewWorld+0x1a0"},
                        {"implementationFunction":"NewWorld+0x1b0"}],
                      "methods":[{"first":"Convert<AZ::Uuid>","second":{
                        "implementationFunction":"NewWorld+0x160","vtable":"NewWorld+0x1000",
                        "m_reflectFunction":["NewWorld+0x170"],"isConst":true,
                        "attributes":[{"attributeName":"ExcludeFrom","attributeId":2844357374,"value":{"kind":"ExcludeFlags",
                          "value":18446744073709551615,"flags":{"all":true}}}]}}],
                      "properties":[{"first":"Count","second":{"getter":{"implementationFunction":"NewWorld+0x180"},
                        "attributes":[{"name":"ToolTip","value":{"kind":"string","value":"Captured property"}}]}}]}}],
                  "methods":[{"first":"GlobalConvert","second":{"implementationFunction":"NewWorld+0x160"}}],
                  "properties":[
                    {"first":"Options_Two","second":{"getter":{"implementationFunction":"NewWorld+0x100"}}},
                    {"name":"Options_AllBits","getter":{"implementationFunction":"NewWorld+0x110"}},
                    {"name":"Wide_HighBit","getter":{"implementationFunction":"NewWorld+0x120"}},
                    {"name":"Options_Zero","getter":{"implementationFunction":"NewWorld+0x130"}},
                    {"name":"Options_Dynamic","getter":{"implementationFunction":"NewWorld+0x140"}},
                    {"name":"Manual_Two","getter":{"implementationFunction":"NewWorld+0x100"}},
                    {"name":"Options_Writable","getter":{"implementationFunction":"NewWorld+0x100"},
                      "setter":{"implementationFunction":"NewWorld+0x190"}}],
                  "ebuses":[{"name":"MB::RequestBus<Javelin::Id>",
                    "idParam":{"name":"BusIdType","m_typeId":"44444444-4444-4444-8444-444444444444",
                      "m_traits":4,"traits":{"isReference":true}},
                    "createHandler":{"implementationFunction":"NewWorld+0x180"},
                    "destroyHandler":{"implementationFunction":"NewWorld+0x190"},
                    "queueFunction":"NewWorld+0x190",
                    "getCurrentId":{"implementationFunction":"NewWorld+0x190"},
                    "handlerReflector":"NewWorld+0x190",
                    "events":[{"name":"GetCount","address":"0x20212340000",
                      "attributes":[{"name":"Deprecated","value":{"kind":"bool","value":true}}],
                      "broadcast":{"implementationFunction":"NewWorld+0x180","virtualDispatchSlotOffset":"0x8"},
                      "event":{"implementationFunction":"NewWorld+0x180","virtualDispatchSlotOffset":"0x8"},
                      "queueBroadcast":{"implementationFunction":"NewWorld+0x190"},
                      "queueEvent":{"implementationFunction":"NewWorld+0x190"}}],
                    "virtualProperties":[{"name":"Count","getter":{"address":"0x20212340000",
                      "broadcast":{"implementationFunction":"NewWorld+0x180","virtualDispatchSlotOffset":"0x8"}}}],
                    "attributes":[{"name":"Category","value":{"kind":"string","value":"Tests"}}]}],
                  "m_methods":[{"name":"AliasCopyMustNotImport","m_functionPtr":"NewWorld+0x100"}]
                }
                """.formatted(TYPE, TYPE, TYPE);
            Files.writeString(resources.resolve("behavior-context.json"), capture);
            int typeCount = program.getDataTypeManager().getDataTypeCount(true);
            run(program, resources, "--dry-run");
            check(program.getDataTypeManager().getDataTypeCount(true) == typeCount, "preview keeps datatypes intact");
            check(!Files.exists(resources.resolve("reflection-report.json")), "preview preserves applied report");
            JsonObject validation = JsonParser.parseString(Files.readString(resources.resolve("reflection-dry-run.json"))).getAsJsonObject();
            check(validation.get("status").getAsString().equals("passed") &&
                validation.get("behaviorEbusEventCount").getAsInt() > 0, "preview validates BehaviorContext buses");
            run(program, resources);
            JsonObject evidence = report(resources).getAsJsonObject("behaviorContextEvidence");
            JsonObject inventory = evidence.getAsJsonObject("inventory");
            check(evidence.get("classes").getAsInt() == 1 && evidence.get("onDemandReflectors").getAsInt() == 1,
                "map-only classes and on-demand reflectors are processed");
            check(evidence.get("globalMethods").getAsInt() == 1, "top-level alias copies do not import twice");
            check(inventory.getAsJsonArray("classes").get(0).getAsJsonObject().getAsJsonArray("requestBuses").size() == 1,
                "class bus relationships retained");
            var type = program.getDataTypeManager().getDataType(new CategoryPath("/MB"), "Box<Javelin::Payload>");
            check(type != null && type.getDescription().contains("wrappedTypeId") &&
                type.getDescription().contains("notificationBuses"), "class metadata visible on datatype");
            check(hasSymbol(program, 0x150, "AZ::BehaviorContext::OnDemandReflection<MB::Box<Javelin::Payload>>::Reflect"),
                "on-demand reflection uses canonical template identity");
            check(hasSymbol(program, 0x160, "MB::Box<Javelin::Payload>::Convert<AZ::Uuid>") &&
                hasSymbol(program, 0x160, "AZ::BehaviorContext::GlobalConvert"), "every shared owner gets an alias");
            check(hasSymbol(program, 0x180, "MB::RequestBus<Javelin::Id>::Broadcast::GetCount") &&
                hasSymbol(program, 0x180, "MB::RequestBus<Javelin::Id>::Event::GetCount") &&
                hasSymbol(program, 0x190, "MB::RequestBus<Javelin::Id>::QueueBroadcast::GetCount") &&
                hasSymbol(program, 0x190, "MB::RequestBus<Javelin::Id>::QueueEvent::GetCount"), "all dispatch variants applied");
            check(hasSymbol(program, 0x190, "MB::RequestBus<Javelin::Id>::QueueFunction") &&
                hasSymbol(program, 0x190, "MB::RequestBus<Javelin::Id>::GetCurrentId") &&
                hasSymbol(program, 0x190, "MB::RequestBus<Javelin::Id>::OnDemandReflect"), "all EBus callbacks applied");
            check(evidence.get("ebusVirtualPropertyEventNamesResolved").getAsInt() == 1, "virtual property event joined by captured address");
            String comment = program.getListing().getComment(CommentType.PLATE, program.getImageBase().add(0x180));
            check(comment.contains("BusIdType") && comment.contains("44444444-4444-4444-8444-444444444444") &&
                comment.contains("virtualProperties") && comment.contains("Deprecated") && comment.contains("isReference"),
                "bus ID, decoded parameter traits, virtual properties and attributes applied");
            String sharedComment = program.getListing().getComment(CommentType.PLATE, program.getImageBase().add(0x160));
            check(sharedComment.contains("18446744073709551615") && sharedComment.contains("isConst"),
                "uint64 attributes and method qualifiers retain exact values");
            check(evidence.getAsJsonObject("attributes").has("ExcludeFrom") &&
                evidence.getAsJsonObject("attributes").has("ScriptStorage"), "native attributeName and CRC parsed");
            check(program.getSymbolTable().getSymbols(program.getImageBase().add(0x1000)).length > 0, "adapter vtable ownership labeled");
            check(hasSymbol(program, 0x170,
                "AZ::BehaviorContext::Method<MB::Box<Javelin::Payload>::Convert<AZ::Uuid>>::Reflect<0x170>"),
                "captured reflector source labeled");
            check(hasSymbol(program, 0x160,
                "AZ::BehaviorContext::Method<MB::Box<Javelin::Payload>::Convert<AZ::Uuid>>::Implementation<0x160>"),
                "implementation aliases retain each reflected owner");
            check(hasSymbol(program, 0x1a0,
                "AZ::BehaviorContext::Method<MB::Box<Javelin::Payload>::Constructor>::Implementation<0x1a0>") &&
                hasSymbol(program, 0x1b0,
                "AZ::BehaviorContext::Method<MB::Box<Javelin::Payload>::Constructor>::Implementation<0x1b0>"),
                "constructor overloads retain both bodies without guessed argument types");
            check(inventory.getAsJsonArray("functionUses").size() > 10,
                "complete function use inventory includes canonical names and implementation aliases");
            Enum options = (Enum) program.getDataTypeManager().getDataType(
                new CategoryPath("/AZ/BehaviorContext/Constants"), "Options_32bit");
            check(options.getCount() == 3 && options.getValue("Options_Two") == 2 &&
                options.getValue("Options_AllBits") == 0xffffffffL && options.getValue("Options_Zero") == 0,
                "integer constants recovered with correct register width and unsigned bits");
            Enum wide = (Enum) program.getDataTypeManager().getDataType(
                new CategoryPath("/AZ/BehaviorContext/Constants"), "Wide_64bit");
            check(wide.getValue("Wide_HighBit") == Long.MIN_VALUE, "64-bit return bits retained");
            Enum kept = (Enum) program.getDataTypeManager().getDataType(
                new CategoryPath("/AZ/BehaviorContext/Constants"), "Manual_32bit");
            check(kept.getCount() == 1 && kept.getValue("Keep") == 7, "manual enum preserved");
            check(evidence.getAsJsonObject("unresolvedMetadata").get("constant-value-not-recovered").getAsInt() == 1,
                "dynamic getter stays unresolved");
            check(evidence.getAsJsonObject("unresolvedMetadata").has("method-signature-not-captured") &&
                evidence.getAsJsonObject("unresolvedMetadata").has("attribute-value-not-captured"), "missing metadata is explicit");
            Object enumId = options.getUniversalID();
            String beforeComment = sharedComment;
            run(program, resources);
            check(report(resources).get("appliedCount").getAsInt() == 0, "all BehaviorContext edits idempotent");
            check(enumId.equals(program.getDataTypeManager().getDataType(
                new CategoryPath("/AZ/BehaviorContext/Constants"), "Options_32bit").getUniversalID()), "enum identity retained");
            check(beforeComment.equals(program.getListing().getComment(CommentType.PLATE,
                program.getImageBase().add(0x160))), "rerun does not duplicate metadata comments");
            String stale = "BehaviorContext evidence: {\"name\":\"Earlier capture\",\"method\":{\"address\":\"0x2022815d200\"}}";
            program.getListing().setComment(program.getImageBase().add(0x160), CommentType.PLATE,
                beforeComment + "\n" + stale + "\nAnalyst note");
            run(program, resources);
            check((beforeComment + "\nAnalyst note").equals(program.getListing().getComment(CommentType.PLATE,
                program.getImageBase().add(0x160))), "a new capture replaces stale evidence and keeps other lines");
            System.out.println("PASS: BehaviorContext map-only classes, normalized aliases, shared owners, attributes, EBus variants/IDs/virtual properties, on-demand reflectors, constant enums, manual conflicts, preview and idempotence");
        }
        finally {
            program.endTransaction(transaction, false);
            program.release(consumer);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static boolean hasSymbol(ProgramDB program, int offset, String name) {
        for (var symbol : program.getSymbolTable().getSymbols(program.getImageBase().add(offset))) {
            if (symbol.getName(true).equals(name)) return true;
        }
        return false;
    }

    private static void run(ProgramDB program, Path resources, String... extra) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        var arguments = new java.util.ArrayList<>(List.of("--resources", resources.toString()));
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
