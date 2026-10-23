import java.io.File;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.HashMap;
import java.util.List;

import ghidra.GhidraApplicationLayout;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.flatapi.FlatProgramAPI;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.Pointer;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.GhidraClass;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.SymbolType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.exception.CancelledException;
import ghidra.util.task.TaskMonitor;
import ghidra.util.task.TaskMonitorAdapter;

/** Exercises the repair against a disposable Ghidra database, not the game program. */
public class AzReflectionNameRepairTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])),
            new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object consumer = new Object();
        ProgramDB program = new ProgramDB("Renamed Game Program", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), consumer);
        int transaction = program.startTransaction("reflection repair regression");
        try {
            var manager = program.getDataTypeManager();
            var symbols = program.getSymbolTable();
            StructureDataType source = new StructureDataType(CategoryPath.ROOT, "AZStd::array<AZ::Vector3,2>", 32);
            source.setDescription("AZ SerializeContext reflected type: AZStd::array<AZ::Vector3,2>");
            Structure original = (Structure) manager.addDataType(source, DataTypeConflictHandler.DEFAULT_HANDLER);
            Object originalId = original.getUniversalID();
            StructureDataType holderSource = new StructureDataType(CategoryPath.ROOT, "Holder", 0);
            holderSource.add(new PointerDataType(original, 8, manager), "values", null);
            Structure holder = (Structure) manager.addDataType(holderSource, DataTypeConflictHandler.DEFAULT_HANDLER);

            StructureDataType manual = new StructureDataType(CategoryPath.ROOT, "AZStd::ManualNative", 16);
            manual.setDescription("Manually recovered native layout");
            DataType manualType = manager.addDataType(manual, DataTypeConflictHandler.DEFAULT_HANDLER);
            StructureDataType collision = new StructureDataType(CategoryPath.ROOT, "MB::Collision<AZ::Uuid>", 8);
            collision.setDescription("AZ SerializeContext reflected field type: MB::Collision<AZ::Uuid>");
            DataType collisionOld = manager.addDataType(collision, DataTypeConflictHandler.DEFAULT_HANDLER);
            DataType collisionTarget = manager.addDataType(new StructureDataType(new CategoryPath("/MB"),
                "Collision<AZ::Uuid>", 24), DataTypeConflictHandler.DEFAULT_HANDLER);
            StructureDataType replica = new StructureDataType(CategoryPath.ROOT, "MB::Replica<AZ::Uuid>", 16);
            replica.setDescription("AZ SerializeContext reflected type: MB::Replica<AZ::Uuid>");
            DataType replicaType = manager.addDataType(replica, DataTypeConflictHandler.DEFAULT_HANDLER);
            StructureDataType emptyString = new StructureDataType(CategoryPath.ROOT, "AZStd::string", 40);
            emptyString.setDescription("AZ SerializeContext reflected field type: AZStd::string");
            DataType stringPlaceholder = manager.addDataType(emptyString, DataTypeConflictHandler.DEFAULT_HANDLER);
            StructureDataType canonicalString = new StructureDataType(new CategoryPath("/AZStd"), "string", 40);
            canonicalString.replaceAtOffset(16, ghidra.program.model.data.UnsignedLongLongDataType.dataType, 8, "m_size", null);
            DataType actualString = manager.addDataType(canonicalString, DataTypeConflictHandler.DEFAULT_HANDLER);
            StructureDataType stringHolderSource = new StructureDataType(CategoryPath.ROOT, "StringHolder", 0);
            stringHolderSource.add(new PointerDataType(stringPlaceholder, 8, manager), "text", null);
            Structure stringHolder = (Structure) manager.addDataType(stringHolderSource, DataTypeConflictHandler.DEFAULT_HANDLER);
            Namespace azNamespace = symbols.createNameSpace(program.getGlobalNamespace(), "AZ", SourceType.USER_DEFINED);
            Namespace canonicalUuidClass = symbols.createClass(azNamespace, "Uuid", SourceType.USER_DEFINED);
            Namespace emptyFlatUuidClass = symbols.createClass(program.getGlobalNamespace(), "AZ::Uuid", SourceType.USER_DEFINED);
            long emptyFlatUuidId = emptyFlatUuidClass.getID();

            GhidraClass legacyMB = symbols.createClass(program.getGlobalNamespace(), "MB", SourceType.ANALYSIS);
            long legacyMBId = legacyMB.getID();
            Namespace existingMBChild = symbols.createClass(legacyMB, "ExistingReplica", SourceType.USER_DEFINED);
            long existingMBChildId = existingMBChild.getID();
            Namespace existingMBNested = symbols.createNameSpace(existingMBChild, "Details", SourceType.USER_DEFINED);

            String className = "Javelin::ClientMessages::Payload<MB::Key>";
            GhidraClass oldClass = symbols.createClass(program.getGlobalNamespace(), className, SourceType.USER_DEFINED);
            long classId = oldClass.getID();
            Namespace nested = symbols.createClass(oldClass, "Nested", SourceType.USER_DEFINED);
            Address code = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x1000);
            program.getMemory().createInitializedBlock("test", code, 0x1000, (byte) 0, TaskMonitor.DUMMY, false);
            Function method = program.getFunctionManager().createFunction("Decode<AZ::EntityId>", oldClass,
                code, new AddressSet(code), SourceType.USER_DEFINED);
            method.addParameter(new ParameterImpl("payload", new PointerDataType(original, 8, manager), program),
                SourceType.USER_DEFINED);
            long methodId = method.getSymbol().getID();
            Address mbCode = code.add(0x10);
            Function existingMBFunction = program.getFunctionManager().createFunction("GetTypeName<AZ::Uuid>",
                legacyMB, mbCode, new AddressSet(mbCode), SourceType.USER_DEFINED);
            long existingMBFunctionId = existingMBFunction.getSymbol().getID();

            AzReflectionRenamer script = new AzReflectionRenamer();
            set(FlatProgramAPI.class, script, "currentProgram", program);
            set(FlatProgramAPI.class, script, "monitor", TaskMonitor.DUMMY);
            set(AzReflectionRenamer.class, script, "dataTypeManager", manager);
            set(AzReflectionRenamer.class, script, "symbols", symbols);
            set(AzReflectionRenamer.class, script, "scopeCache", new HashMap<>());
            List<?> parts = (List<?>) call(script, "cppScopeParts", new Class<?>[]{String.class}, className);
            call(script, "registerReflectedClassScope", new Class<?>[]{List.class}, parts);
            call(script, "registerReflectedClassScope", new Class<?>[]{List.class}, List.of("AZ", "Uuid"));
            // Exercise cancellation in its own transaction. Ghidra nested aborts
            // take effect when the outer transaction ends.
            program.endTransaction(transaction, true);
            transaction = -1;
            set(FlatProgramAPI.class, script, "monitor", new TaskMonitorAdapter() {
                private int checks;

                @Override
                public void checkCancelled() throws CancelledException {
                    if (++checks == 2) throw new CancelledException();
                }
            });
            boolean cancelled = false;
            try {
                call(script, "convertClassRootToNamespace", new Class<?>[]{Namespace.class}, legacyMB);
            }
            catch (java.lang.reflect.InvocationTargetException error) {
                if (!(error.getCause() instanceof CancelledException)) throw error;
                cancelled = true;
            }
            check(cancelled && symbols.getNamespace("MB", program.getGlobalNamespace()).getID() == legacyMBId,
                "cancelled root conversion restores original class");
            check(existingMBChild.getParentNamespace().getID() == legacyMBId &&
                existingMBFunction.getParentNamespace().getID() == legacyMBId,
                "cancelled root conversion rolls back moved children");
            transaction = program.startTransaction("reflection repair assertions");
            set(FlatProgramAPI.class, script, "monitor", TaskMonitor.DUMMY);
            Class<?> sinkClass = Class.forName("AzReflectionRenamer$ActionSink");
            Constructor<?> sinkConstructor = sinkClass.getDeclaredConstructor(AzReflectionRenamer.class, boolean.class);
            sinkConstructor.setAccessible(true);

            Object preview = sinkConstructor.newInstance(script, true);
            call(script, "repairQualifiedReflectionNames", new Class<?>[]{sinkClass}, preview);
            check(original.getPathName().equals("/AZStd::array<AZ::Vector3,2>"), "preview preserves datatype path");
            check(oldClass.getName().equals(className), "preview preserves class path");
            check(symbols.getNamespace("MB", program.getGlobalNamespace()).equals(legacyMB),
                "preview preserves legacy namespace class root");

            set(AzReflectionRenamer.class, script, "applyRenames", true);
            Object sink = sinkConstructor.newInstance(script, true);
            call(script, "repairQualifiedReflectionNames", new Class<?>[]{sinkClass}, sink);
            check(original.getPathName().equals("/AZStd/array<AZ::Vector3,2>"), "datatype category and leaf repaired");
            check(originalId.equals(original.getUniversalID()), "managed datatype identity preserved");
            check(((Pointer) holder.getComponent(0).getDataType()).getDataType() == original,
                "existing structure pointer reference preserved");
            check(manualType.getPathName().equals("/AZStd::ManualNative"), "manual layout untouched");
            check(replicaType.getPathName().equals("/MB/Replica<AZ::Uuid>"), "MB generic datatype path repaired");
            check(collisionOld.getPathName().equals("/MB::Collision<AZ::Uuid>") &&
                manager.getDataType(new CategoryPath("/MB"), "Collision<AZ::Uuid>") == collisionTarget,
                "destination conflict preserves both types");
            check(oldClass.getID() == classId && oldClass.getName().equals("Payload<MB::Key>"),
                "class moved in place");
            check(symbols.getSymbol(emptyFlatUuidId) == null && symbols.getNamespace("Uuid", azNamespace).equals(canonicalUuidClass),
                "only empty flattened duplicate class removed");
            check(manager.getDataType(CategoryPath.ROOT, "AZStd::string") == null &&
                ((Pointer) stringHolder.getComponent(0).getDataType()).getDataType() == actualString,
                "empty same-size placeholder references redirected to existing native layout");
            check(oldClass.getParentNamespace().getName(true).equals("Javelin::ClientMessages"),
                "qualified class parents repaired");
            check(nested.getParentNamespace().equals(oldClass), "nested class retained");
            check(method.getSymbol().getID() == methodId && method.getParentNamespace().equals(oldClass),
                "template method symbol and ownership preserved");
            var payloadParameter = java.util.Arrays.stream(method.getParameters())
                .filter(parameter -> parameter.getName().equals("payload")).findFirst().orElseThrow();
            check(((Pointer) payloadParameter.getDataType()).getDataType() == original,
                "existing function parameter type reference preserved");
            for (String namespace : List.of("AZStd", "Javelin", "MB")) {
                check(symbols.getNamespace(namespace, program.getGlobalNamespace()).getSymbol().getSymbolType()
                    .equals(SymbolType.NAMESPACE), namespace + " created as namespace");
            }
            Namespace repairedMB = symbols.getNamespace("MB", program.getGlobalNamespace());
            check(symbols.getSymbol(legacyMBId) == null, "only retired empty root class removed");
            check(existingMBChild.getID() == existingMBChildId &&
                existingMBChild.getParentNamespace().equals(repairedMB), "existing MB class identity preserved");
            check(existingMBNested.getParentNamespace().equals(existingMBChild), "MB descendants retained");
            check(existingMBFunction.getSymbol().getID() == existingMBFunctionId &&
                existingMBFunction.getParentNamespace().equals(repairedMB), "existing MB template function retained");
            Object repeated = sinkConstructor.newInstance(script, true);
            call(script, "repairQualifiedReflectionNames", new Class<?>[]{sinkClass}, repeated);
            Object stats = field(repeated, "stats");
            check(((Number) field(stats, "appliedCount")).intValue() == 0, "repeated repair is idempotent");
            System.out.println("PASS: Ghidra database repair preserves type IDs, pointer references, nested classes, " +
                "template methods, legacy namespace roots, previews, manual layouts, conflicts and repeated-run behavior");
        }
        finally {
            if (transaction >= 0) program.endTransaction(transaction, false);
            program.release(consumer);
        }
    }

    private static void set(Class<?> type, Object instance, String name, Object value) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        field.set(instance, value);
    }

    private static Object field(Object instance, String name) throws Exception {
        Field field = instance.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(instance);
    }

    private static Object call(Object instance, String name, Class<?>[] types, Object... values) throws Exception {
        Method method = AzReflectionRenamer.class.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return method.invoke(instance, values);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
