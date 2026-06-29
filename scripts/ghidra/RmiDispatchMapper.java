// Apply RMIDispatch / target-actor route datatypes and validated function names.
//@category NewWorld

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.BooleanDataType;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.FloatDataType;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.data.UnsignedLongLongDataType;
import ghidra.program.model.data.VoidDataType;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Function.FunctionUpdateType;
import ghidra.program.model.listing.Parameter;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.program.model.symbol.SymbolType;

public class RmiDispatchMapper extends GhidraScript {
    private static final String VERSION = "rmi-dispatch-mapper-20260629";
    private static final String TYPE_DESCRIPTION_PREFIX =
        "New World 3-26 RMIDispatch route evidence";

    private static final FunctionSpec[] FUNCTION_SPECS = {
        new FunctionSpec(
            0x16bcba0L,
            new String[] { "Amazon", "Hub" },
            "FindActorFacetTarget"),
        new FunctionSpec(
            0x167b950L,
            new String[] { "Amazon", "Hub" },
            "RegisterActorFacetTarget"),
        new FunctionSpec(
            0x17abe30L,
            new String[] { "Amazon", "Hub" },
            "UnregisterActorFacetTarget"),
        new FunctionSpec(
            0x17b4a60L,
            new String[] { "Amazon", "Hub" },
            "EraseActorDispatcherSlot"),
        new FunctionSpec(
            0x16c1020L,
            new String[] { "Amazon", "Hub" },
            "GetTargetLocalIdFromFacetHandle"),
        new FunctionSpec(
            0x16bddd0L,
            new String[] { "Amazon", "Hub" },
            "GetSourceActorRefFromLocalActorContext"),
        new FunctionSpec(
            0x17da460L,
            new String[] { "Amazon", "Hub" },
            "FindActorDispatcherSlotIndex"),
        new FunctionSpec(
            0x51e600L,
            new String[] { "Amazon", "Hub" },
            "FindOrInsertFacetRouteKey"),
        new FunctionSpec(
            0x14a0fa0L,
            new String[] { "Amazon", "Hub" },
            "HashFacetRouteKey"),
    };

    private SymbolTable symbols;
    private DataTypeManager dataTypes;
    private CategoryPath hubCategory;
    private boolean apply;
    private int applied;
    private int skipped;

    @Override
    protected void run() throws Exception {
        if (currentProgram == null) {
            popup("No current program is open.");
            return;
        }

        println("RmiDispatchMapper version: " + VERSION);
        symbols = currentProgram.getSymbolTable();
        dataTypes = currentProgram.getDataTypeManager();
        hubCategory = new CategoryPath(new CategoryPath(CategoryPath.ROOT, "Amazon"), "Hub");
        apply = askYesNo(
            "Apply RMIDispatch mapping?",
            "Apply validated RMIDispatch datatypes, function names, and signatures?\n\n" +
                "No runs a dry-run report only.");

        Structure slot = ensureActorDispatcherSlot();
        Structure lookupResult = ensureActorDispatcherLookupResult();
        Structure hashControl = ensureActorDispatcherHashControl(slot);
        Structure dispatcher = ensureActorDispatcher(hashControl);
        Structure componentEntry = ensureTargetActorComponentEntry();
        Structure facetFactory = ensureTargetActorFacetFactory(componentEntry, dispatcher);
        Structure facetHandle = ensureTargetActorFacetHandle(facetFactory);
        Structure routeEntry = ensureFacetRouteKeyEntry();
        Structure routeTable = ensureFacetRouteKeyTable(routeEntry);

        for (FunctionSpec spec : FUNCTION_SPECS) {
            applyFunctionSpec(
                spec,
                dispatcher,
                hashControl,
                lookupResult,
                facetHandle,
                routeTable);
        }

        println("RMIDispatch mapping " + (apply ? "applied" : "dry-run") +
            ": applied=" + applied + " skipped=" + skipped);
    }

    private Structure ensureActorDispatcherSlot() throws Exception {
        Structure structure = newOwnedStructure("ActorDispatcherSlot", 0x10);
        structure.replaceAtOffset(
            0x00,
            u64(),
            8,
            "targetLocalId",
            "Key read from ActorRequestId at message + 0x08.");
        structure.replaceAtOffset(
            0x08,
            voidPointer(),
            pointerSize(),
            "facetHandler",
            "Registered facet handler / target handle.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureActorDispatcherLookupResult() throws Exception {
        Structure structure = newOwnedStructure("ActorDispatcherLookupResult", 0x10);
        structure.replaceAtOffset(
            0x00,
            u64(),
            8,
            "foundIndex",
            "Slot index, or UINT64_MAX when the key is absent.");
        structure.replaceAtOffset(
            0x08,
            u64(),
            8,
            "insertIndex",
            "Insertion candidate, or UINT64_MAX when the key was found.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureActorDispatcherHashControl(Structure slot) throws Exception {
        Structure structure = newOwnedStructure("ActorDispatcherHashControl", 0x70);
        structure.replaceAtOffset(
            0x18,
            new BooleanDataType(),
            1,
            "needsRehash",
            "Set on erase at dispatcher + 0x160.");
        structure.replaceAtOffset(
            0x28,
            u64(),
            8,
            "tombstoneKey",
            "Tombstone sentinel key.");
        structure.replaceAtOffset(
            0x30,
            u64(),
            8,
            "erasedCount",
            "Number of tombstoned slots.");
        structure.replaceAtOffset(
            0x38,
            u64(),
            8,
            "liveCount",
            "Number of occupied or tombstoned slots.");
        structure.replaceAtOffset(
            0x40,
            u64(),
            8,
            "capacity",
            "Power-of-two slot count.");
        structure.replaceAtOffset(
            0x50,
            u64(),
            8,
            "emptyKey",
            "Empty-slot sentinel key.");
        structure.replaceAtOffset(
            0x68,
            pointerTo(slot),
            pointerSize(),
            "slots",
            "ActorDispatcherSlot array.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureActorDispatcher(Structure hashControl) throws Exception {
        Structure structure = newOwnedStructure("ActorDispatcher", 0x1b8);
        structure.replaceAtOffset(
            0x00,
            voidPointer(),
            pointerSize(),
            "vftable",
            null);
        structure.replaceAtOffset(
            0x148,
            hashControl,
            hashControl.getLength(),
            "hashControl",
            "Native helper receives this subobject as dispatcher + 0x148.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureTargetActorComponentEntry() throws Exception {
        Structure structure = newOwnedStructure("TargetActorComponentEntry", 0x10);
        structure.replaceAtOffset(
            0x00,
            voidPointer(),
            pointerSize(),
            "vftable",
            null);
        structure.replaceAtOffset(
            0x08,
            u64(),
            8,
            "componentBaseLocalId",
            "Base actor ref used by GetTargetLocalIdFromFacetHandle.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureTargetActorFacetFactory(
        Structure componentEntry,
        Structure dispatcher) throws Exception {

        Structure structure = newOwnedStructure("TargetActorFacetFactory", 0x90);
        structure.replaceAtOffset(
            0x00,
            voidPointer(),
            pointerSize(),
            "vftable",
            null);
        structure.replaceAtOffset(
            0x08,
            pointerTo(componentEntry),
            pointerSize(),
            "componentEntry",
            "Owner component entry; +0x08 holds componentBaseLocalId.");
        structure.replaceAtOffset(
            0x10,
            u64(),
            8,
            "facetTargetId",
            "Added to componentBaseLocalId to produce targetLocalId.");
        structure.replaceAtOffset(
            0x78,
            pointerTo(dispatcher),
            pointerSize(),
            "dispatcher",
            "Dispatcher used by register/unregister paths.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureTargetActorFacetHandle(Structure facetFactory) throws Exception {
        Structure structure = newOwnedStructure("TargetActorFacetHandle", 0x10);
        structure.replaceAtOffset(
            0x00,
            voidPointer(),
            pointerSize(),
            "vftable",
            null);
        structure.replaceAtOffset(
            0x08,
            pointerTo(facetFactory),
            pointerSize(),
            "inner",
            "Handle payload read by the target-local-id getter.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureFacetRouteKeyEntry() throws Exception {
        Structure structure = newOwnedStructure("FacetRouteKeyEntry", 0x40);
        structure.replaceAtOffset(0x00, voidPointer(), pointerSize(), "next", null);
        structure.replaceAtOffset(0x08, voidPointer(), pointerSize(), "prev", null);
        structure.replaceAtOffset(
            0x10,
            u64(),
            8,
            "key",
            "Route key copied by FindOrInsertFacetRouteKey.");
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure ensureFacetRouteKeyTable(Structure routeEntry) throws Exception {
        Structure structure = newOwnedStructure("FacetRouteKeyTable", 0x78);
        structure.replaceAtOffset(0x08, voidPointer(), pointerSize(), "listHead", null);
        structure.replaceAtOffset(0x18, u64(), 8, "entryCount", null);
        structure.replaceAtOffset(0x28, voidPointer(), pointerSize(), "allocator", null);
        structure.replaceAtOffset(0x58, u64(), 8, "bucketMask", null);
        structure.replaceAtOffset(
            0x60,
            pointerTo(routeEntry),
            pointerSize(),
            "buckets",
            "Bucket array used by FindOrInsertFacetRouteKey.");
        structure.replaceAtOffset(0x68, u64(), 8, "bucketCount", null);
        structure.replaceAtOffset(0x70, new FloatDataType(), 4, "maxLoadFactor", null);
        return addOrUpdateOwnedStructure(structure);
    }

    private Structure newOwnedStructure(String name, int length) {
        Structure structure = new StructureDataType(hubCategory, name, length, dataTypes);
        structure.setDescription(TYPE_DESCRIPTION_PREFIX + ": Amazon::Hub::" + name);
        return structure;
    }

    private Structure addOrUpdateOwnedStructure(Structure structure) throws Exception {
        DataType existing = dataTypes.getDataType(structure.getCategoryPath(), structure.getName());
        if (existing instanceof Structure && isOwnedStructure((Structure) existing)) {
            if (apply) {
                Structure added = (Structure) dataTypes.addDataType(
                    structure,
                    DataTypeConflictHandler.REPLACE_HANDLER);
                applied++;
                println("datatype updated: " + added.getPathName());
                return added;
            }
            skipped++;
            println("dry-run datatype update: " + structure.getPathName());
            return (Structure) existing;
        }
        if (existing != null) {
            skipped++;
            println("datatype skipped, existing user type: " + existing.getPathName());
            return existing instanceof Structure ? (Structure) existing : structure;
        }
        if (!apply) {
            skipped++;
            println("dry-run datatype create: " + structure.getPathName());
            return structure;
        }
        Structure added = (Structure) dataTypes.addDataType(
            structure,
            DataTypeConflictHandler.DEFAULT_HANDLER);
        applied++;
        println("datatype created: " + added.getPathName());
        return added;
    }

    private boolean isOwnedStructure(Structure structure) {
        String description = structure.getDescription();
        return description != null && description.startsWith(TYPE_DESCRIPTION_PREFIX);
    }

    private void applyFunctionSpec(
        FunctionSpec spec,
        Structure dispatcher,
        Structure hashControl,
        Structure lookupResult,
        Structure facetHandle,
        Structure routeTable) throws Exception {

        Address address = currentProgram.getImageBase().add(spec.rva);
        if (!isExecutableAddress(address)) {
            skipped++;
            println("function skipped, non-executable: " + spec.qualifiedName());
            return;
        }

        Function function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function == null && apply) {
            function = createFunctionAt(address, spec.localName);
        }
        if (function == null) {
            skipped++;
            println("dry-run function rename: " + formatAddress(address) +
                " -> " + spec.qualifiedName());
            return;
        }

        boolean changed = false;
        if (apply && !function.getName(true).equals(spec.qualifiedName())) {
            changed |= renameFunction(function, spec.scope, spec.localName);
        }
        if (apply) {
            changed |= applySignature(
                function,
                spec,
                dispatcher,
                hashControl,
                lookupResult,
                facetHandle,
                routeTable);
        }

        if (changed) {
            applied++;
            println("function mapped: " + formatAddress(address) + " -> " + spec.qualifiedName());
        }
        else {
            skipped++;
            println((apply ? "function already current: " : "dry-run function: ") +
                formatAddress(address) + " -> " + spec.qualifiedName());
        }
    }

    private Function createFunctionAt(Address address, String localName) throws Exception {
        CodeUnit codeUnit = currentProgram.getListing().getCodeUnitContaining(address);
        if (codeUnit != null && !codeUnit.getMinAddress().equals(address)) {
            return null;
        }
        if (currentProgram.getListing().getInstructionAt(address) == null) {
            disassemble(address);
        }
        return createFunction(address, localName);
    }

    private boolean renameFunction(Function function, String[] scope, String localName)
        throws Exception {

        Namespace namespace = createScope(scope);
        Symbol blocker = symbols.getSymbol(localName, function.getEntryPoint(), namespace);
        if (blocker != null && !blocker.getSymbolType().equals(SymbolType.FUNCTION)) {
            blocker.delete();
        }
        function.setName(localName, SourceType.USER_DEFINED);
        function.setParentNamespace(namespace);
        return true;
    }

    private Namespace createScope(String[] scope) throws Exception {
        Namespace parent = currentProgram.getGlobalNamespace();
        for (String part : scope) {
            Namespace existing = symbols.getNamespace(part, parent);
            parent = existing != null
                ? existing
                : symbols.createNameSpace(parent, part, SourceType.USER_DEFINED);
        }
        return parent;
    }

    private boolean applySignature(
        Function function,
        FunctionSpec spec,
        Structure dispatcher,
        Structure hashControl,
        Structure lookupResult,
        Structure facetHandle,
        Structure routeTable) throws Exception {

        Signature signature = signatureFor(
            spec,
            dispatcher,
            hashControl,
            lookupResult,
            facetHandle,
            routeTable);
        if (signature == null) {
            return false;
        }

        boolean changed = false;
        if (!sameDataType(function.getReturnType(), signature.returnType)) {
            function.setReturnType(signature.returnType, SourceType.USER_DEFINED);
            changed = true;
        }
        Parameter[] existing = function.getParameters();
        boolean same = existing.length == signature.parameterNames.length;
        if (same) {
            for (int i = 0; i < existing.length; i++) {
                if (!signature.parameterNames[i].equals(existing[i].getName()) ||
                    !sameDataType(existing[i].getDataType(), signature.parameterTypes[i])) {
                    same = false;
                    break;
                }
            }
        }
        if (!same) {
            Parameter[] parameters = new Parameter[signature.parameterNames.length];
            for (int i = 0; i < parameters.length; i++) {
                parameters[i] = new ParameterImpl(
                    signature.parameterNames[i],
                    signature.parameterTypes[i],
                    currentProgram);
            }
            function.replaceParameters(
                FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS,
                true,
                SourceType.USER_DEFINED,
                parameters);
            changed = true;
        }
        return changed;
    }

    private Signature signatureFor(
        FunctionSpec spec,
        Structure dispatcher,
        Structure hashControl,
        Structure lookupResult,
        Structure facetHandle,
        Structure routeTable) {

        if ("FindActorFacetTarget".equals(spec.localName)) {
            return new Signature(
                voidPointer(),
                new String[] { "message", "dispatcher" },
                new DataType[] { voidPointer(), pointerTo(dispatcher) });
        }
        if ("RegisterActorFacetTarget".equals(spec.localName) ||
            "UnregisterActorFacetTarget".equals(spec.localName)) {
            return new Signature(
                VoidDataType.dataType,
                new String[] { "handle" },
                new DataType[] { pointerTo(facetHandle) });
        }
        if ("EraseActorDispatcherSlot".equals(spec.localName)) {
            return new Signature(
                VoidDataType.dataType,
                new String[] { "dispatcher", "targetLocalId" },
                new DataType[] { pointerTo(dispatcher), pointerTo(u64()) });
        }
        if ("GetTargetLocalIdFromFacetHandle".equals(spec.localName)) {
            return new Signature(
                u64(),
                new String[] { "handle" },
                new DataType[] { pointerTo(facetHandle) });
        }
        if ("GetSourceActorRefFromLocalActorContext".equals(spec.localName)) {
            return new Signature(
                pointerTo(u64()),
                new String[] { "context", "outSourceActorRef" },
                new DataType[] { voidPointer(), pointerTo(u64()) });
        }
        if ("FindActorDispatcherSlotIndex".equals(spec.localName)) {
            return new Signature(
                pointerTo(lookupResult),
                new String[] { "control", "result", "targetLocalId" },
                new DataType[] { pointerTo(hashControl), pointerTo(lookupResult), pointerTo(u64()) });
        }
        if ("FindOrInsertFacetRouteKey".equals(spec.localName)) {
            return new Signature(
                voidPointer(),
                new String[] { "table", "result", "key" },
                new DataType[] { pointerTo(routeTable), voidPointer(), pointerTo(u64()) });
        }
        if ("HashFacetRouteKey".equals(spec.localName)) {
            return new Signature(
                u64(),
                new String[] { "key" },
                new DataType[] { u64() });
        }
        return null;
    }

    private boolean isExecutableAddress(Address address) {
        MemoryBlock block = currentProgram.getMemory().getBlock(address);
        return block != null && block.isExecute();
    }

    private String formatAddress(Address address) {
        long base = currentProgram.getImageBase().getOffset();
        long value = address.getOffset();
        return "NewWorld+0x" + Long.toHexString(value - base);
    }

    private int pointerSize() {
        return currentProgram.getDefaultPointerSize();
    }

    private DataType voidPointer() {
        return new PointerDataType(VoidDataType.dataType, dataTypes);
    }

    private DataType pointerTo(DataType type) {
        return new PointerDataType(type, dataTypes);
    }

    private DataType u64() {
        return new UnsignedLongLongDataType();
    }

    private boolean sameDataType(DataType left, DataType right) {
        if (left == right) {
            return true;
        }
        if (left == null || right == null) {
            return false;
        }
        return left.isEquivalent(right) ||
            left.getPathName().equals(right.getPathName()) ||
            left.getName().equals(right.getName());
    }

    private static final class FunctionSpec {
        final long rva;
        final String[] scope;
        final String localName;

        FunctionSpec(long rva, String[] scope, String localName) {
            this.rva = rva;
            this.scope = scope;
            this.localName = localName;
        }

        String qualifiedName() {
            List<String> parts = new ArrayList<>();
            parts.addAll(Arrays.asList(scope));
            parts.add(localName);
            return String.join("::", parts);
        }
    }

    private static final class Signature {
        final DataType returnType;
        final String[] parameterNames;
        final DataType[] parameterTypes;

        Signature(DataType returnType, String[] parameterNames, DataType[] parameterTypes) {
            this.returnType = returnType;
            this.parameterNames = parameterNames;
            this.parameterTypes = parameterTypes;
        }
    }
}
