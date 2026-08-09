// Helper model classes for NetworkSchemaExtractor.
// Keep these package-less so Ghidra compiles them in the same source bundle as
// the script.

import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

final class SerializeFieldInfo {
    String name;
    String typeId;
    String typeName;
    String wireShape;
    String genericClassName;
    final ArrayList<String> templatedTypeIds = new ArrayList<>();
    Long offset;
    Long dataSize;
    boolean baseClass;
}

record ReflectedSetIdentity(
    String nativeType,
    String elementTypeName,
    String elementTypeId,
    String wireShape,
    Address marshalHelper,
    String ownerTypeName,
    String ownerTypeId,
    String fieldName,
    long fieldOffset) {

    String identityKey() {
        return nativeType + "|" + elementTypeId + "|" + wireShape;
    }
}

final class SerializeWireSlot {
    final long offset;
    final String wireShape;
    final String path;
    final String typeId;
    final String nativeType;

    SerializeWireSlot(
            long offset, String wireShape, String path, String typeId, String nativeType) {
        this.offset = offset;
        this.wireShape = wireShape;
        this.path = path;
        this.typeId = typeId;
        this.nativeType = nativeType;
    }
}

final class FixedNamedFieldValue {
    Address nameAddress;
    String name;
    Address nameWrite;
    Integer handlerOffset;
    String handlerExpression;
}

final class MemoryReference {
    final String baseRegister;
    final int displacement;

    MemoryReference(String baseRegister, int displacement) {
        this.baseRegister = baseRegister;
        this.displacement = displacement;
    }
}

final class MemoryAddress {
    final List<MemoryTerm> terms;
    final int displacement;

    MemoryAddress(List<MemoryTerm> terms, int displacement) {
        this.terms = Collections.unmodifiableList(new ArrayList<>(terms));
        this.displacement = displacement;
    }
}

final class MemoryTerm {
    final String register;
    final int scale;

    MemoryTerm(String register, int scale) {
        this.register = register;
        this.scale = scale;
    }
}

final class VectorSlotAlias {
    final int ownerOffset;
    final int slotOffset;

    VectorSlotAlias(int ownerOffset, int slotOffset) {
        this.ownerOffset = ownerOffset;
        this.slotOffset = slotOffset;
    }
}

final class NetworkTemplateType {
    final String qualifiedName;
    final String ownerName;
    final String simpleName;
    final List<String> args;

    NetworkTemplateType(
            String qualifiedName, String ownerName, String simpleName, List<String> args) {
        this.qualifiedName = qualifiedName;
        this.ownerName = ownerName;
        this.simpleName = simpleName;
        this.args = Collections.unmodifiableList(new ArrayList<>(args));
    }
}

final class GenericType {
    final String qualifiedName;
    final String ownerName;
    final String simpleName;
    final List<String> args;

    GenericType(String qualifiedName, String ownerName, String simpleName, List<String> args) {
        this.qualifiedName = qualifiedName;
        this.ownerName = ownerName;
        this.simpleName = simpleName;
        this.args = Collections.unmodifiableList(new ArrayList<>(args));
    }
}

final class TypeIdOperands {
    final List<String> typeNames;
    final List<String> typeIds;

    TypeIdOperands(List<String> typeNames, List<String> typeIds) {
        this.typeNames = Collections.unmodifiableList(new ArrayList<>(typeNames));
        this.typeIds = Collections.unmodifiableList(new ArrayList<>(typeIds));
    }
}

final class FoldedTypeId {
    final String sourceTypeName;
    final String formula;
    final String typeId;
    final List<String> operandTypeNames;
    final List<String> operandTypeIds;

    FoldedTypeId(String sourceTypeName, String formula, String typeId,
            List<String> operandTypeNames, List<String> operandTypeIds) {
        this.sourceTypeName = sourceTypeName;
        this.formula = formula;
        this.typeId = typeId;
        this.operandTypeNames = Collections.unmodifiableList(new ArrayList<>(operandTypeNames));
        this.operandTypeIds = Collections.unmodifiableList(new ArrayList<>(operandTypeIds));
    }
}

final class PcodeStorage {
    final String base;
    final long offset;

    PcodeStorage(String base, long offset) {
        this.base = base;
        this.offset = offset;
    }

    PcodeStorage plus(long delta) {
        return new PcodeStorage(base, offset + delta);
    }

    boolean sameLocation(PcodeStorage other) {
        return other != null && base.equals(other.base) && offset == other.offset;
    }

    String expression() {
        if (offset < 0) {
            return base + " - 0x" + Long.toHexString(-offset);
        }
        return base + " + 0x" + Long.toHexString(offset);
    }
}

final class CollectionOutputShape {
    final PcodeStorage storage;
    final ContainerStorageKind storageKind;
    final String nativeType;
    final WireShape wireEvidence;
    final Address countCallsite;
    final Address loopHeader;
    final Set<Address> codecCallsites;
    final Long interiorSpan;

    CollectionOutputShape(
            PcodeStorage storage,
            ContainerStorageKind storageKind,
            String nativeType,
            WireShape wireEvidence,
            Address countCallsite,
            Address loopHeader,
            Set<Address> codecCallsites,
            Long interiorSpan) {
        this.storage = storage;
        this.storageKind = storageKind;
        this.nativeType = nativeType;
        this.wireEvidence = wireEvidence;
        this.countCallsite = countCallsite;
        this.loopHeader = loopHeader;
        this.codecCallsites = Set.copyOf(codecCallsites);
        this.interiorSpan = interiorSpan;
    }
}

final class MessageHelperCall {
    Address callsite;
    Address target;
    String targetName;
}

final class RawPcodeWrite {
    final PcodeStorage storage;
    final String nativeType;
    final WireShape wireEvidence;

    RawPcodeWrite(PcodeStorage storage, String nativeType, WireShape wireEvidence) {
        this.storage = storage;
        this.nativeType = nativeType;
        this.wireEvidence = wireEvidence;
    }
}

final class ConstructorFieldTypeIdentity {
    final String typeId;
    final String typeName;
    final Address initializer;
    final Address ownerConstructor;
    final Address ownerVtable;
    final String ownerTypeId;
    final String ownerTypeName;
    final String fieldName;
    final long fieldOffset;
    final long byteWidth;

    ConstructorFieldTypeIdentity(
            String typeId,
            String typeName,
            Address initializer,
            Address ownerConstructor,
            Address ownerVtable,
            String ownerTypeId,
            String ownerTypeName,
            String fieldName,
            long fieldOffset,
            long byteWidth) {
        this.typeId = typeId;
        this.typeName = typeName;
        this.initializer = initializer;
        this.ownerConstructor = ownerConstructor;
        this.ownerVtable = ownerVtable;
        this.ownerTypeId = ownerTypeId;
        this.ownerTypeName = ownerTypeName;
        this.fieldName = fieldName;
        this.fieldOffset = fieldOffset;
        this.byteWidth = byteWidth;
    }

    String identityKey() {
        return typeId.toLowerCase(java.util.Locale.ROOT);
    }
}

final class MessageConstructorCall {
    Address callsite;
    Address target;
    String targetName;
}

final class IndexedWireShape {
    final int textIndex;
    final String shape;

    IndexedWireShape(int textIndex, String shape) {
        this.textIndex = textIndex;
        this.shape = shape;
    }
}

final class WireShape {
    final String shape;
    final String source;
    final String layout;
    final String layoutSource;

    private WireShape(
            String shape,
            String source,
            String layout,
            String layoutSource) {
        this.shape = shape;
        this.source = source;
        this.layout = layout;
        this.layoutSource = layoutSource;
    }

    static WireShape semantic(String shape, String source) {
        return new WireShape(shape, source, shape, source);
    }

    static WireShape semantic(String shape, String layout, String source) {
        return new WireShape(shape, source, layout, source);
    }

    static WireShape layout(String layout, String source) {
        return new WireShape(null, null, layout, source);
    }

    boolean hasProvenSemantics() {
        return shape != null && source != null;
    }
}

final class HandlerWireShapeEvidence {
    WireShape selected;
    WireShape marshal;
    WireShape unmarshal;
    FixedSequenceShape selectedSequence;
    FixedSequenceShape marshalSequence;
    FixedSequenceShape unmarshalSequence;
    String resolution;
    String conflict;
    String unmarshalReadBufferParameter;
    Integer unmarshalEventCount;
    String unmarshalDiagnostic;
    String quantizedVec3Diagnostic;
    final List<CodecWireEvent> unmarshalEvents = new ArrayList<>();
}

record EntityRefCodecEvidence(
        Address function,
        String bufferParameter,
        String direction,
        boolean conditionalFlag,
        boolean nestedString,
        boolean flagsByte,
        boolean uuidBytes) {

    boolean isComplete() {
        return conditionalFlag && nestedString && flagsByte && uuidBytes;
    }
}

final class NestedDatatypeScalar {
    final long offset;
    final String path;
    final String nativeType;
    final String wireShape;

    NestedDatatypeScalar(long offset, String path, String nativeType, String wireShape) {
        this.offset = offset;
        this.path = path;
        this.nativeType = nativeType;
        this.wireShape = wireShape;
    }
}

final class HandlerScanFrame {
    final Address address;
    final int depth;

    HandlerScanFrame(Address address, int depth) {
        this.address = address;
        this.depth = depth;
    }
}

final class VtableWrite {
    final Address function;
    final Address instruction;
    final Address vtable;
    final int order;
    final Integer thisOffset;
    final String baseKey;
    final Integer baseOffset;
    final String pattern;

    VtableWrite(Address function, Address instruction, Address vtable, int order) {
        this(function, instruction, vtable, order, null, null, null, null);
    }

    VtableWrite(Address function, Address instruction, Address vtable, int order,
            Integer thisOffset, String pattern) {
        this(function, instruction, vtable, order, thisOffset, null, null, pattern);
    }

    VtableWrite(Address function, Address instruction, Address vtable, int order,
            Integer thisOffset, String baseKey, Integer baseOffset, String pattern) {
        this.function = function;
        this.instruction = instruction;
        this.vtable = vtable;
        this.order = order;
        this.thisOffset = thisOffset;
        this.baseKey = baseKey;
        this.baseOffset = baseOffset;
        this.pattern = pattern;
    }
}

final class TrackedValue {
    final Address address;
    final Integer thisOffset;
    final Integer stackOffset;
    final String baseKey;
    final Integer baseOffset;
    final Long immediate;
    final Address fieldNameAddress;
    final String fieldName;
    final String expression;
    final Set<PcodeStorage> storageDependencies;

    private TrackedValue(Address address, Integer thisOffset, Integer stackOffset, String baseKey,
            Integer baseOffset, Long immediate, Address fieldNameAddress, String fieldName,
            String expression, Set<PcodeStorage> storageDependencies) {
        this.address = address;
        this.thisOffset = thisOffset;
        this.stackOffset = stackOffset;
        this.baseKey = baseKey;
        this.baseOffset = baseOffset;
        this.immediate = immediate;
        this.fieldNameAddress = fieldNameAddress;
        this.fieldName = fieldName;
        this.expression = expression;
        this.storageDependencies = Set.copyOf(storageDependencies);
    }

    static TrackedValue address(Address address) {
        return new TrackedValue(
                address, null, null, null, null, null, null, null, null, Set.of());
    }

    static TrackedValue thisOffset(int offset) {
        return new TrackedValue(
                null, offset, null, null, null, null, null, null, thisExpression(offset), Set.of());
    }

    static TrackedValue stackOffset(int offset) {
        return new TrackedValue(
                null, null, offset, null, null, null, null, null, stackExpression(offset), Set.of());
    }

    static TrackedValue baseOffset(String baseKey, int offset) {
        return new TrackedValue(null, null, null, baseKey, offset, null, null, null,
                baseExpression(baseKey, offset), Set.of());
    }

    static TrackedValue immediate(long value) {
        return new TrackedValue(
                null, null, null, null, null, value, null, null, Long.toUnsignedString(value),
                Set.of());
    }

    static TrackedValue fieldName(Address formatAddress, String fieldName) {
        return new TrackedValue(
                null, null, null, null, null, null, formatAddress, fieldName, fieldName, Set.of());
    }

    static TrackedValue storageDependency(PcodeStorage storage) {
        return storage == null ? null : storageDependencies(Set.of(storage));
    }

    static TrackedValue storageDependencies(Iterable<PcodeStorage> dependencies) {
        LinkedHashSet<PcodeStorage> unique = new LinkedHashSet<>();
        dependencies.forEach(unique::add);
        return unique.isEmpty()
                ? null
                : new TrackedValue(
                    null, null, null, null, null, null, null, null, null, unique);
    }

    TrackedValue addOffset(int delta) {
        try {
            if (thisOffset != null) {
                return thisOffset(Math.addExact(thisOffset, delta));
            }
            if (stackOffset != null) {
                return stackOffset(Math.addExact(stackOffset, delta));
            }
            if (baseKey != null && baseOffset != null) {
                return baseOffset(baseKey, Math.addExact(baseOffset, delta));
            }
            if (immediate != null) {
                return immediate(Math.addExact(immediate, (long) delta));
            }
        } catch (ArithmeticException ignored) {
            return null;
        }
        return storageDependencies.isEmpty() ? this : storageDependencies(storageDependencies);
    }

    TrackedValue copy() {
        return new TrackedValue(address, thisOffset, stackOffset, baseKey, baseOffset, immediate,
                fieldNameAddress, fieldName, expression, storageDependencies);
    }

    boolean sameValue(TrackedValue other) {
        if (other == null) {
            return false;
        }
        return sameAddress(address, other.address) && sameObject(thisOffset, other.thisOffset)
                && sameObject(stackOffset, other.stackOffset) && sameObject(baseKey, other.baseKey)
                && sameObject(baseOffset, other.baseOffset)
                && sameObject(immediate, other.immediate)
                && sameAddress(fieldNameAddress, other.fieldNameAddress)
                && sameObject(fieldName, other.fieldName)
                && storageDependencies.equals(other.storageDependencies);
    }

    private static boolean sameAddress(Address left, Address right) {
        if (left == null || right == null) {
            return left == right;
        }
        return left.equals(right);
    }

    private static boolean sameObject(Object left, Object right) {
        if (left == null || right == null) {
            return left == right;
        }
        return left.equals(right);
    }

    private static String thisExpression(int offset) {
        if (offset == 0) {
            return "this";
        }
        if (offset > 0) {
            return "this+0x" + Integer.toHexString(offset);
        }
        return "this-0x" + Long.toHexString(-(long) offset);
    }

    private static String stackExpression(int offset) {
        if (offset == 0) {
            return "stack";
        }
        if (offset > 0) {
            return "stack+0x" + Integer.toHexString(offset);
        }
        return "stack-0x" + Long.toHexString(-(long) offset);
    }

    private static String baseExpression(String baseKey, int offset) {
        if (offset == 0) {
            return baseKey;
        }
        if (offset > 0) {
            return baseKey + "+0x" + Integer.toHexString(offset);
        }
        return baseKey + "-0x" + Long.toHexString(-(long) offset);
    }
}

final class StringDecode {
    final Address address;
    final String value;

    StringDecode(Address address, String value) {
        this.address = address;
        this.value = value;
    }
}
