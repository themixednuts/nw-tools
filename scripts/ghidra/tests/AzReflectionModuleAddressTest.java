import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.CommentType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Replays external DLL offsets that alias valid NewWorld code addresses. */
public class AzReflectionModuleAddressTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-module-address-test-");
        try {
            var base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x7ff600000000L);
            byte[] instruction = {0x0f, (byte) 0x95, (byte) 0xc2, (byte) 0xc3};
            int tx = program.startTransaction("external offset fixture");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("collision", base.add(0xab99e), 16, (byte) 0,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("adapter", base.add(0x2000), 16, (byte) 0,
                    TaskMonitor.DUMMY, false);
                program.getMemory().setBytes(base.add(0xab99e), instruction);
                check(new DisassembleCommand(base.add(0xab99e), null, true).applyTo(program, TaskMonitor.DUMMY),
                    "could not decode exact SETNZ collision");
                for (long offset : new long[]{0x100, 0x110, 0x120}) {
                    var entry = base.add(offset);
                    program.getFunctionManager().createFunction("Keep_" + offset, entry, new AddressSet(entry), SourceType.USER_DEFINED);
                }
                program.getListing().setComment(base.add(0x100), CommentType.PLATE, "manual comment");
                program.getSymbolTable().createLabel(base.add(0xab99e), "Keep_instruction", SourceType.USER_DEFINED);
            }
            finally { program.endTransaction(tx, true); }
            Files.writeString(resources.resolve("behavior-context.json"), """
                {"classes":[{"name":"Math","typeId":"35D44724-7470-42F2-A0E3-4E4349793B98","methods":[
                  {"name":"Round","m_functionPtr":"ucrtbase+0xab9a0","vtable":"NewWorld+0x2000",
                   "m_reflectFunction":["NewWorld+0x120"]},
                  {"name":"AllowedCall","m_functionPtr":"NewWorld+0x110"}]}],
                 "methods":[{"name":"CRT::MustNotRename","implementationFunction":"ucrtbase+0x100"},
                  {"name":"API::MustNotRename","implementationFunction":"api-ms-win-core-test+0x100"}]}
                """);
            long symbols = program.getSymbolTable().getNumSymbols();
            run(program, resources, true);
            check(program.getSymbolTable().getNumSymbols() == symbols, "preview changed symbols");
            run(program, resources, false);
            check(program.getSymbolTable().getSymbols(base.add(0xab9a0)).length == 0,
                "ucrtbase Math::Round was mislabeled inside the NewWorld SETNZ instruction");
            check(program.getFunctionManager().getFunctionAt(base.add(0x100)).getName().equals("Keep_256") &&
                "manual comment".equals(program.getListing().getComment(CommentType.PLATE, base.add(0x100))),
                "DLL offset changed a coinciding NewWorld function or its comment");
            byte[] after = new byte[instruction.length];
            program.getMemory().getBytes(base.add(0xab99e), after);
            check(Arrays.equals(after, instruction) && program.getListing().getInstructionAt(base.add(0xab99e)).getLength() == 3,
                "external address changed instruction bytes or boundaries");
            check(program.getFunctionManager().getFunctionAt(base.add(0x110)).getName(true).equals("Math::AllowedCall"),
                "valid NewWorld method was skipped with external methods");
            check(program.getSymbolTable().getSymbol("Keep_instruction", base.add(0xab99e), null) != null,
                "manual instruction label changed");
            JsonObject report = JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
            check(!report.getAsJsonObject("actionReasonCounts").has("function-create-failed"), "external DLL function was still created");
            for (var item : report.getAsJsonArray("actions")) {
                JsonObject action = item.getAsJsonObject();
                if (action.has("address") && action.get("address").getAsString().contains("ucrtbase+"))
                    check(!action.get("applied").getAsBoolean(), "external DLL action was applied");
            }
            AzReflectionRenamer parser = script(program);
            for (String value : new String[]{"ucrtbase+0x110", "kernel32+0x110", "api-ms-win-core-test+0x110"}) {
                check(call(parser, "parseCaptureAddress", value) == null && call(parser, "captureOffset", value) == null,
                    "external module offset accepted: " + value);
            }
            for (String value : new String[]{"NewWorld+0x110", "NewWorld.exe+0x110", "newworld+0x110", "0x7ff600000110"})
                check(base.add(0x110).equals(call(parser, "parseCaptureAddress", value)), "valid image address lost: " + value);
            run(program, resources, false);
            check(program.getSymbolTable().getSymbols(base.add(0xab9a0)).length == 0, "retry reintroduced external labels");
            System.out.println("PASS: exact ucrtbase Math::Round collision, DLL/API-set rejection, unchanged instructions/manual symbols/" +
                "comments, valid NewWorld/case/executable/absolute addresses, preview and retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }
    static AzReflectionRenamer script(ProgramDB program) {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        return script;
    }
    static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = script(program);
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} :
            new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); } finally { script.end(true); }
    }
    static Object call(AzReflectionRenamer script, String name, String value) throws Exception {
        Method method = AzReflectionRenamer.class.getDeclaredMethod(name, String.class);
        method.setAccessible(true);
        return method.invoke(script, value);
    }
    static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
