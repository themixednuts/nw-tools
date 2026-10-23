import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/**
 * Names uuidAnyCreationMap creators: one per type, an alias per type on a body several types share, and
 * skips for a creator outside code and a type id no capture names.
 */
public class AzReflectionAnyCreatorTest {
    private static final String WIDGET = "11111111-1111-4111-8111-111111111111";
    private static final String GADGET = "22222222-2222-4222-8222-222222222222";
    private static final String GIZMO = "33333333-3333-4333-8333-333333333333";
    private static final String DATA = "44444444-4444-4444-8444-444444444444";
    private static final String UNNAMED = "55555555-5555-4555-8555-555555555555";

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-any-creator-test-");
        try {
            Address base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
            int setup = program.startTransaction("any creator fixture");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock(".text", base.add(0x1000), 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock(".rdata", base.add(0x3000), 0x100, (byte) 0,
                    TaskMonitor.DUMMY, false);
                for (long offset : new long[]{0x1000, 0x1010}) {
                    Address entry = base.add(offset);
                    program.getFunctionManager().createFunction("FUN_" + Long.toHexString(offset), entry,
                        new AddressSet(entry), SourceType.DEFAULT);
                }
            }
            finally { program.endTransaction(setup, true); }

            Files.writeString(resources.resolve("serialize.json"), """
                {"uuidMap":{
                  "%1$s":{"name":"Javelin::Widget","typeId":"%1$s","version":"1","factory":null,"attributes":[],"elements":[]},
                  "%2$s":{"name":"Javelin::Gadget","typeId":"%2$s","version":"1","factory":null,"attributes":[],"elements":[]},
                  "%3$s":{"name":"Gizmo","typeId":"%3$s","version":"1","factory":null,"attributes":[],"elements":[]},
                  "%4$s":{"name":"Data","typeId":"%4$s","version":"1","factory":null,"attributes":[],"elements":[]}},
                 "uuidAnyCreationMap":{"%1$s":"NewWorld+0x1000","%2$s":"NewWorld+0x1010","%3$s":"NewWorld+0x1010",
                   "%4$s":"NewWorld+0x3000","%5$s":"NewWorld+0x1020"}}
                """.formatted(WIDGET, GADGET, GIZMO, DATA, UNNAMED));

            long symbolCount = program.getSymbolTable().getNumSymbols();
            run(program, resources, true);
            check(program.getSymbolTable().getNumSymbols() == symbolCount, "dry run edited symbols");
            run(program, resources, false);
            check(json(resources.resolve("reflection-dry-run.json")).get("status").getAsString().equals("passed"),
                "validation failed");

            String widget = function(program, base, 0x1000).getName(true);
            check(widget.equals("AZ::SerializeContext::Class<Javelin::Widget>::AnyCreator"), "own creator: " + widget + " " + creatorActions(resources));
            check(has(program, base, 0x1010, "AZ::SerializeContext::Class<Javelin::Gadget>::AnyCreator") &&
                has(program, base, 0x1010, "AZ::SerializeContext::Class<Gizmo>::AnyCreator"),
                "shared creator aliases: " + names(program, base, 0x1010));
            check(program.getFunctionManager().getFunctionAt(base.add(0x1020)) == null &&
                program.getSymbolTable().getSymbols(base.add(0x3000)).length == 0, "skipped creators were named");

            JsonObject report = json(resources.resolve("reflection-report.json"));
            JsonObject reasons = report.getAsJsonObject("actionReasonCounts");
            check(report.get("anyCreatorCount").getAsInt() == 5 &&
                reasons.get("non-executable-target").getAsInt() == 1 &&
                reasons.get("missing-type-name").getAsInt() >= 1, "creator report: " + reasons);
            long after = program.getSymbolTable().getNumSymbols();
            run(program, resources, false);
            check(json(resources.resolve("reflection-report.json")).get("appliedCount").getAsInt() == 0 &&
                program.getSymbolTable().getNumSymbols() == after, "retry changed AnyCreator names");
            System.out.println("PASS: uuidAnyCreationMap creators, shared bodies, non-code and unnamed entries, retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static ghidra.program.model.listing.Function function(ProgramDB program, Address base, long offset) {
        return program.getFunctionManager().getFunctionAt(base.add(offset));
    }

    private static boolean has(ProgramDB program, Address base, long offset, String name) {
        return Arrays.stream(program.getSymbolTable().getSymbols(base.add(offset))).anyMatch(s -> s.getName(true).equals(name));
    }

    private static String names(ProgramDB program, Address base, long offset) {
        return Arrays.toString(Arrays.stream(program.getSymbolTable().getSymbols(base.add(offset))).map(s -> s.getName(true)).toArray());
    }

    private static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY,
            new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} :
            new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
    }

    private static String creatorActions(Path resources) throws Exception {
        java.util.List<String> result = new java.util.ArrayList<>();
        collect(JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))), result);
        return String.join("\n", result);
    }

    private static void collect(com.google.gson.JsonElement value, java.util.List<String> result) {
        if (value.isJsonArray()) value.getAsJsonArray().forEach(v -> collect(v, result));
        if (!value.isJsonObject()) return;
        JsonObject object = value.getAsJsonObject();
        if (object.has("kind") && object.get("kind").toString().contains("any_creator")) result.add(object.toString());
        else object.entrySet().forEach(e -> collect(e.getValue(), result));
    }

    private static JsonObject json(Path path) throws Exception {
        return JsonParser.parseString(Files.readString(path)).getAsJsonObject();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
