import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;

import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Factory addresses expose interface callbacks, including shared null/no-op implementations. */
public class AzReflectionFactoryTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-factory-test-");
        try {
            var base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
            int tx = program.startTransaction("native factory fixtures");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("data", base.add(0x2000), 0x200, (byte) 0,
                    TaskMonitor.DUMMY, false);
                program.getMemory().setLong(base.add(0x2000), base.add(0x2040).getOffset());
                for (int i = 0; i < 3; i++) program.getMemory().setLong(base.add(0x2040 + i * 8), base.add(0x100 + i * 0x10).getOffset());
                program.getMemory().setLong(base.add(0x2010), base.add(0x2080).getOffset());
                program.getMemory().setLong(base.add(0x2080), base.add(0x100).getOffset());
                program.getMemory().setLong(base.add(0x2088), 0);
                program.getMemory().setLong(base.add(0x2090), base.add(0x120).getOffset());
                program.getMemory().setLong(base.add(0x2020), base.add(0x20c0).getOffset());
                program.getMemory().setLong(base.add(0x20c0), base.add(0x100).getOffset());
                program.getMemory().setLong(base.add(0x20c8), base.add(0x2000).getOffset());
                program.getMemory().setLong(base.add(0x20d0), base.add(0x120).getOffset());
                program.getMemory().setLong(base.add(0x2030), base.add(0x2100).getOffset());
                program.getMemory().setLong(base.add(0x2100), base.add(0x100).getOffset());
                program.getMemory().setLong(base.add(0x2108), base.add(0x130).getOffset());
                program.getMemory().setLong(base.add(0x2110), base.add(0x140).getOffset());
                program.getMemory().setLong(base.add(0x2038), base.add(0x2140).getOffset());
                program.getMemory().setLong(base.add(0x2140), base.add(0x100).getOffset());
                program.getMemory().setLong(base.add(0x2148), base.add(0x111).getOffset());
                program.getMemory().setLong(base.add(0x2150), base.add(0x120).getOffset());
                for (long offset : new long[]{0x100, 0x110, 0x120}) {
                    program.getFunctionManager().createFunction("Manual_" + offset, base.add(offset),
                        new AddressSet(base.add(offset), base.add(offset == 0x110 ? offset + 5 : offset)), SourceType.USER_DEFINED);
                }
            }
            finally { program.endTransaction(tx, true); }
            Files.writeString(resources.resolve("serialize.json"), """
                {"classData":[
                  {"name":"MB::Payload<Javelin::Key>","typeId":"AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA","version":"0","attributes":[],"factory":"NewWorld+0x2000","elements":[]},
                  {"name":"AZStd::vector<AZ::Uuid>","typeId":"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB","version":"0","attributes":[],"factory":"NewWorld+0x2000","elements":[]},
                  {"name":"Javelin::Sparse","typeId":"CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC","version":"0","attributes":[],"factory":"NewWorld+0x2010","elements":[]},
                  {"name":"Javelin::NonCode","typeId":"DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD","version":"0","attributes":[],"factory":"NewWorld+0x2020","elements":[]},
                  {"name":"Javelin::MissingFunctions","typeId":"EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE","version":"0","attributes":[],"factory":"NewWorld+0x2030","elements":[]},
                  {"name":"Javelin::Interior","typeId":"FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF","version":"0","attributes":[],"factory":"NewWorld+0x2038","elements":[]}
                ]}
                """);
            long before = program.getSymbolTable().getNumSymbols();
            byte[] beforeBytes = new byte[0x200];
            program.getMemory().getBytes(base.add(0x2000), beforeBytes);
            run(program, resources, true);
            check(program.getSymbolTable().getNumSymbols() == before, "preview changed symbols");
            run(program, resources, false);
            var report = com.google.gson.JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
            check(report.get("classDataCount").getAsInt() == 6, "factory records were not recognized as captured ClassData");
            var create = program.getSymbolTable().getSymbols(base.add(0x110));
            check(java.util.Arrays.stream(create).anyMatch(symbol -> symbol.getName(true).equals("AZ::SerializeContext::IObjectFactory::Create")),
                "SerializeContext factory Create slot was not imported");
            check(java.util.Arrays.stream(program.getSymbolTable().getSymbols(base.add(0x120)))
                .anyMatch(symbol -> symbol.getName(true).equals("AZ::SerializeContext::IObjectFactory::Destroy")), "factory Destroy slot was not imported");
            check(program.getFunctionManager().getFunctionAt(base.add(0x110)).getName().equals("Manual_272") &&
                program.getFunctionManager().getFunctionAt(base.add(0x120)).getName().equals("Manual_288"), "shared factory callback replaced a manual primary name");
            check(java.util.Arrays.stream(program.getSymbolTable().getSymbols(base.add(0x2000)))
                .noneMatch(symbol -> symbol.getName().equals("Create")), "non-code factory pointer was imported as a callable");
            check(program.getFunctionManager().getFunctionAt(base.add(0x130)) != null &&
                program.getFunctionManager().getFunctionAt(base.add(0x140)) != null, "valid missing factory functions were not created");
            check(java.util.Arrays.stream(program.getSymbolTable().getSymbols(base.add(0x111)))
                .noneMatch(symbol -> symbol.getName().equals("Create")), "interior function address was imported as a factory entry");
            byte[] afterBytes = new byte[0x200];
            program.getMemory().getBytes(base.add(0x2000), afterBytes);
            check(java.util.Arrays.equals(beforeBytes, afterBytes), "factory table/object memory was rewritten");
            long after = program.getSymbolTable().getNumSymbols();
            run(program, resources, false);
            check(program.getSymbolTable().getNumSymbols() == after, "factory retry duplicated symbols");
            System.out.println("PASS: factory Create/Destroy interface aliases, shared owners/manual primary names, missing functions, " +
                "zero/non-code/interior slot rejection, unchanged object/table bytes, preview and retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }
    static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} : new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); } finally { script.end(true); }
    }
    static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
