import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

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
import ghidra.program.model.listing.GhidraClass;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Reproduces the live ranged_enum namespace fragments, whose flattened names already look correct. */
public class AzReflectionTemplateScopeRepairTest {
    static final String TYPE = "ranged_enum<Javelin::TaxOrFeeType,_7>";
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-template-scope-test-");
        try {
            var symbols = program.getSymbolTable();
            Namespace azstd, mb, enumFragment, vectorFragment, unknownFragment, pairFragment, pairMiddle;
            GhidraClass canonicalPair;
            GhidraClass flatMap, redundantMap;
            Namespace manualParent;
            GhidraClass manualNested;
            Function flatAllocate;
            GhidraClass enumClass, brokenVector, canonicalVector, nested, truncated;
            Function setter, destructor, converted, keep, unknownMethod, truncatedMethod;
            Symbol enumTable, oldVectorTable, canonicalTable, vectorAlias, sharedTable, namespaceTable, unknownTable;
            Namespace listFragment, listLeaf, tableOnlyFragment;
            GhidraClass sharedOwner, tableOnlyClass;
            int tx = program.startTransaction("split native template scopes");
            try {
                var base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L);
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", base.add(0x100), 0x200, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().createInitializedBlock("tables", base.add(0x2000), 0x100, (byte) 0,
                    TaskMonitor.DUMMY, false);
                for (int i = 0; i < 6; i++) program.getMemory().setLong(base.add(0x2000 + i * 8), base.add(0x100 + i * 0x10).getOffset());
                azstd = symbols.createNameSpace(program.getGlobalNamespace(), "AZStd", SourceType.USER_DEFINED);
                mb = symbols.createNameSpace(program.getGlobalNamespace(), "MB", SourceType.USER_DEFINED);
                pairFragment = symbols.createNameSpace(azstd, "pair<AZ", SourceType.USER_DEFINED);
                pairMiddle = symbols.createNameSpace(pairFragment, "Uuid,_AZStd", SourceType.USER_DEFINED);
                canonicalPair = symbols.createClass(azstd,
                    "pair<AZ::Uuid,_AZStd::basic_string<char,_AZStd::char_traits<char>,_allocator>>", SourceType.USER_DEFINED);
                flatMap = symbols.createClass(program.getGlobalNamespace(),
                    "AZStd::unordered_map<AZStd::basic_string<char,_AZStd::char_traits<char>,_allocator>,_AZStd::vector<Crc32,_allocator>>",
                    SourceType.USER_DEFINED);
                redundantMap = symbols.createClass(azstd, "AZStd::unordered_set<Javelin::Key>", SourceType.USER_DEFINED);
                Namespace manualRoot = symbols.createNameSpace(program.getGlobalNamespace(), "Manual", SourceType.USER_DEFINED);
                manualParent = symbols.createNameSpace(manualRoot, "Manual", SourceType.USER_DEFINED);
                manualNested = symbols.createClass(manualParent, "Payload", SourceType.USER_DEFINED);
                flatAllocate = program.getFunctionManager().createFunction("Allocate", flatMap, base.add(0x160),
                    new AddressSet(base.add(0x160)), SourceType.USER_DEFINED);
                enumFragment = symbols.createNameSpace(azstd, "ranged_enum<Javelin", SourceType.USER_DEFINED);
                enumClass = symbols.createClass(enumFragment, "TaxOrFeeType,_7>", SourceType.USER_DEFINED);
                setter = program.getFunctionManager().createFunction("Set_value", enumClass, base.add(0x100),
                    new AddressSet(base.add(0x100)), SourceType.USER_DEFINED);
                destructor = program.getFunctionManager().createFunction("~" + TYPE, enumClass, base.add(0x110),
                    new AddressSet(base.add(0x110)), SourceType.USER_DEFINED);
                vectorFragment = symbols.createNameSpace(azstd, "vector<MB", SourceType.USER_DEFINED);
                brokenVector = symbols.createClass(vectorFragment, "Key>", SourceType.USER_DEFINED);
                canonicalVector = symbols.createClass(azstd, "vector<MB::Key>", SourceType.USER_DEFINED);
                nested = symbols.createClass(brokenVector, "Helper", SourceType.USER_DEFINED);
                converted = program.getFunctionManager().createFunction("Convert<Javelin::Key>", brokenVector, base.add(0x120),
                    new AddressSet(base.add(0x120)), SourceType.USER_DEFINED);
                keep = program.getFunctionManager().createFunction("Keep", canonicalVector, base.add(0x130),
                    new AddressSet(base.add(0x130)), SourceType.USER_DEFINED);
                unknownFragment = symbols.createNameSpace(mb, "Holder<AZ", SourceType.USER_DEFINED);
                var unknown = symbols.createClass(unknownFragment, "Uuid>", SourceType.USER_DEFINED);
                unknownMethod = program.getFunctionManager().createFunction("Decode<AZStd::vector<Javelin::Key>>", unknown,
                    base.add(0x140), new AddressSet(base.add(0x140)), SourceType.USER_DEFINED);
                truncated = symbols.createClass(azstd, "vector<MB::Incomplete", SourceType.USER_DEFINED);
                truncatedMethod = program.getFunctionManager().createFunction("Observe", truncated, base.add(0x150),
                    new AddressSet(base.add(0x150)), SourceType.USER_DEFINED);
                enumTable = symbols.createLabel(base.add(0x2000), "vftable", enumClass, SourceType.USER_DEFINED);
                oldVectorTable = symbols.createLabel(base.add(0x2010), "vftable", brokenVector, SourceType.USER_DEFINED);
                canonicalTable = symbols.createLabel(base.add(0x2010), "vftable", canonicalVector, SourceType.USER_DEFINED);
                oldVectorTable.setPrimary();
                vectorAlias = symbols.createLabel(base.add(0x2020), "adapter_vftable", brokenVector, SourceType.USER_DEFINED);
                sharedOwner = symbols.createClass(azstd, "AnotherOwner<MB::Key>", SourceType.USER_DEFINED);
                sharedTable = symbols.createLabel(base.add(0x2010), "vftable", sharedOwner, SourceType.USER_DEFINED);
                listFragment = symbols.createNameSpace(azstd, "list<Javelin", SourceType.USER_DEFINED);
                listLeaf = symbols.createNameSpace(listFragment, "Key>", SourceType.USER_DEFINED);
                namespaceTable = symbols.createLabel(base.add(0x2030), "vftable", listLeaf, SourceType.USER_DEFINED);
                tableOnlyFragment = symbols.createNameSpace(mb, "Bucket<AZ", SourceType.USER_DEFINED);
                tableOnlyClass = symbols.createClass(tableOnlyFragment, "Uuid>", SourceType.USER_DEFINED);
                unknownTable = symbols.createLabel(base.add(0x2040), "vftable", tableOnlyClass, SourceType.USER_DEFINED);
            }
            finally { program.endTransaction(tx, true); }
            Files.writeString(resources.resolve("behavior-context.json"), """
                {"classes":[
                  {"name":"AZStd::ranged_enum<Javelin::TaxOrFeeType, 7>","typeId":"23831001-39C8-5ED6-A893-DB6FA45915FA","size":"1","alignment":"1"},
                  {"name":"AZStd::vector<MB::Key>","typeId":"AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA","size":"32","alignment":"8"},
                  {"name":"AZStd::list<Javelin::Key>","typeId":"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB","size":"32","alignment":"8"}
                ]}
                """);
            long classId = enumClass.getID(), nestedId = nested.getID(), canonicalId = canonicalVector.getID();
            String setterName = setter.getName(true), destructorName = destructor.getName(true);
            long[] functionIds = {setter.getID(), destructor.getID(), converted.getID(), keep.getID(), unknownMethod.getID(), truncatedMethod.getID()};
            long count = symbols.getNumSymbols();
            byte[] tableBytes = new byte[0x100];
            program.getMemory().getBytes(enumTable.getAddress(), tableBytes);
            long enumTableId = enumTable.getID(), canonicalTableId = canonicalTable.getID(), aliasId = vectorAlias.getID();
            run(program, resources, true);
            check(symbols.getNumSymbols() == count && enumClass.getParentNamespace().equals(enumFragment), "preview changed namespace tree");
            check(symbols.getSymbol(pairMiddle.getID()) != null, "preview removed the live pair namespace leftovers");
            run(program, resources, false);
            check(manualNested.getParentNamespace().equals(manualParent), "valid repeated namespace names in an unrelated class were collapsed");
            check(flatMap.getParentNamespace().equals(azstd) && flatMap.getName().startsWith("unordered_map<") &&
                flatAllocate.getParentNamespace().equals(flatMap), "fully qualified unreflected unordered_map remained a global flat class");
            check(redundantMap.getParentNamespace().equals(azstd) && redundantMap.getName().equals("unordered_set<Javelin::Key>"),
                "redundant AZStd class prefix was not normalized");
            check(symbols.getSymbol(pairMiddle.getID()) == null && symbols.getSymbol(pairFragment.getID()) == null &&
                symbols.getSymbol(canonicalPair.getID()) != null && canonicalPair.getParentNamespace().equals(azstd),
                "live pair<AZ / Uuid,_AZStd empty intermediate namespaces survived an already-canonical class");
            check(enumClass.getID() == classId && enumClass.getName().equals(TYPE) && enumClass.getParentNamespace().equals(azstd),
                "live ranged_enum class was not repaired in place");
            check(setter.getName(true).equals(setterName) && destructor.getName(true).equals(destructorName),
                "scope repair changed already-correct flattened names or destructor spelling");
            check(setter.getParentNamespace().getID() == classId && destructor.getParentNamespace().getID() == classId,
                "members detached from repaired class");
            check(symbols.getSymbol(enumFragment.getID()) == null, "empty ranged_enum namespace fragment remains");
            check(converted.getParentNamespace().equals(canonicalVector) && canonicalVector.getID() == canonicalId &&
                keep.getParentNamespace().equals(canonicalVector), "existing canonical class/method was replaced or split function skipped");
            check(nested.getID() == nestedId && nested.getParentNamespace().equals(canonicalVector), "nested class identity/ownership lost");
            check(enumTable.getID() == enumTableId && enumTable.getParentNamespace().equals(enumClass), "vtable detached from repaired class");
            check(symbols.getSymbol(oldVectorTable.getID()) == null && canonicalTable.getID() == canonicalTableId && canonicalTable.isPrimary(),
                "existing canonical vtable alias did not merge while preserving its identity and primary status");
            check(vectorAlias.getID() == aliasId && vectorAlias.getParentNamespace().equals(canonicalVector), "vtable alias lost its ID or correct owner");
            check(sharedTable.getParentNamespace().equals(sharedOwner) && symbols.getSymbol(sharedTable.getID()) != null,
                "another owner's alias for the shared native vtable was removed");
            check(namespaceTable.getParentNamespace().getName().equals("list<Javelin::Key>") &&
                namespaceTable.getParentNamespace().getParentNamespace().equals(azstd) &&
                symbols.getSymbol(listFragment.getID()) == null && symbols.getSymbol(listLeaf.getID()) == null,
                "namespace-only split vtable owner was not repaired");
            check(unknownTable.getParentNamespace().equals(tableOnlyClass) && tableOnlyClass.getName().equals("Bucket<AZ::Uuid>") &&
                tableOnlyClass.getParentNamespace().equals(mb) && symbols.getSymbol(tableOnlyFragment.getID()) == null,
                "uncaptured balanced template with only a vtable label was not repaired in place");
            byte[] afterBytes = new byte[0x100];
            program.getMemory().getBytes(enumTable.getAddress(), afterBytes);
            check(java.util.Arrays.equals(tableBytes, afterBytes), "vtable slots were modified during namespace repair");
            check(symbols.getSymbol(vectorFragment.getID()) == null && symbols.getSymbol(brokenVector.getID()) == null,
                "empty duplicate template fragments remain");
            Namespace unknownOwner = unknownMethod.getParentNamespace();
            check(unknownOwner.getName().equals("Holder<AZ::Uuid>") && unknownOwner.getParentNamespace().equals(mb),
                "uncaptured balanced template scope was not repaired structurally");
            check(symbols.getSymbol(unknownFragment.getID()) == null, "empty unknown template namespace remains");
            check(truncatedMethod.getParentNamespace().equals(truncated) && truncated.getName().equals("vector<MB::Incomplete"),
                "genuinely truncated type was guessed or flattened");
            long after = symbols.getNumSymbols();
            tx = program.startTransaction("empty fragment left by an earlier repair");
            Namespace stale;
            try { stale = symbols.createNameSpace(azstd, "vector<MB", SourceType.USER_DEFINED); }
            finally { program.endTransaction(tx, true); }
            run(program, resources, true);
            check(symbols.getSymbol(stale.getID()) != null, "preview removed stale fragment");
            run(program, resources, false);
            check(symbols.getSymbol(stale.getID()) == null, "stale empty fragment from an earlier repair remains");
            check(symbols.getNumSymbols() == after && symbols.getSymbol(classId).getObject().equals(enumClass), "retry duplicated classes/scopes");
            List<Function> functions = List.of(setter, destructor, converted, keep, unknownMethod, truncatedMethod);
            for (int i = 0; i < functions.size(); i++) check(functions.get(i).getID() == functionIds[i], "function ID changed");
            System.out.println("PASS: live pair intermediate leftovers, flat global/redundant unordered containers, exact ranged_enum split class, unchanged flattened names and destructor, structural namespace checks, " +
                "existing canonical class, nested classes, function/vtable IDs, shared aliases, primary label merge, unchanged native slots, " +
                "namespace-only vtable owners, empty-fragment cleanup, uncaptured balanced types, truncated-name preservation, preview and retry");
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
