// Helper model classes for NetworkSchemaExtractor.
// Keep these package-less so Ghidra compiles them in the same source bundle as
// the script.

import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

final class SerializeFieldInfo {
    String name;
    String typeId;
    String typeName;
    String wireShape;
    Long offset;
    Long dataSize;
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
    final String parameterBase;
    final String nativeType;
    final String wireShape;

    CollectionOutputShape(String parameterBase, String nativeType, String wireShape) {
        this.parameterBase = parameterBase;
        this.nativeType = nativeType;
        this.wireShape = wireShape;
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
    final String wireShape;

    RawPcodeWrite(PcodeStorage storage, String nativeType, String wireShape) {
        this.storage = storage;
        this.nativeType = nativeType;
        this.wireShape = wireShape;
    }
}

final class MessageConstructorCall {
    Address callsite;
    Address target;
    String targetName;
}

final class ParsedUnmarshalFieldsCall {
    final ArrayList<String> templateTypes = new ArrayList<>();
    final ArrayList<ParsedArgument> fieldArgs = new ArrayList<>();
}

final class ParsedUnmarshalCall {
    final String templateType;
    final String functionName;
    final int textIndex;
    final List<String> args;

    ParsedUnmarshalCall(
            String templateType, String functionName, int textIndex, List<String> args) {
        this.templateType = templateType;
        this.functionName = functionName;
        this.textIndex = textIndex;
        this.args = Collections.unmodifiableList(new ArrayList<>(args));
    }
}

final class ParsedArgument {
    String castType;
    String expression;
}

final class ParsedReadRawCall {
    final String storageExpression;
    final int byteLength;
    final int textIndex;

    ParsedReadRawCall(String storageExpression, int byteLength, int textIndex) {
        this.storageExpression = storageExpression;
        this.byteLength = byteLength;
        this.textIndex = textIndex;
    }
}

final class IndexedWireShape {
    final int textIndex;
    final String shape;

    IndexedWireShape(int textIndex, String shape) {
        this.textIndex = textIndex;
        this.shape = shape;
    }
}

final class WholeMessageStore {
    String storageExpression;
    String nativeType;
    int textIndex;
}

final class WholeMessageHelperFrame {
    final Address callsite;
    final Function helper;
    final String helperText;
    final Map<String, String> baseExpressions;
    final int recoveryOrder;

    WholeMessageHelperFrame(Address callsite, Function helper, String helperText,
            Map<String, String> baseExpressions, int recoveryOrder) {
        this.callsite = callsite;
        this.helper = helper;
        this.helperText = helperText;
        this.baseExpressions = Collections.unmodifiableMap(new LinkedHashMap<>(baseExpressions));
        this.recoveryOrder = recoveryOrder;
    }
}

final class MarshalPathFrame {
    final Address address;
    final String sourcePrefix;

    MarshalPathFrame(Address address, String sourcePrefix) {
        this.address = address;
        this.sourcePrefix = sourcePrefix == null ? "" : sourcePrefix;
    }

    MarshalPathFrame nested(Address address) {
        return new MarshalPathFrame(address, sourcePrefix + "marshal-call:");
    }

    WireShape wrap(WireShape shape) {
        if (shape == null || sourcePrefix.isEmpty()) {
            return shape;
        }
        return new WireShape(shape.shape, sourcePrefix + shape.source);
    }
}

final class WireShape {
    final String shape;
    final String source;

    WireShape(String shape, String source) {
        this.shape = shape;
        this.source = source;
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

final class TypeNameCandidate {
    final String typeName;
    final String source;
    final Address address;
    final int score;

    TypeNameCandidate(String typeName, String source, Address address, int score) {
        this.typeName = typeName;
        this.source = source;
        this.address = address;
        this.score = score;
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

    private TrackedValue(Address address, Integer thisOffset, Integer stackOffset, String baseKey,
            Integer baseOffset, Long immediate, Address fieldNameAddress, String fieldName,
            String expression) {
        this.address = address;
        this.thisOffset = thisOffset;
        this.stackOffset = stackOffset;
        this.baseKey = baseKey;
        this.baseOffset = baseOffset;
        this.immediate = immediate;
        this.fieldNameAddress = fieldNameAddress;
        this.fieldName = fieldName;
        this.expression = expression;
    }

    static TrackedValue address(Address address) {
        return new TrackedValue(address, null, null, null, null, null, null, null, null);
    }

    static TrackedValue thisOffset(int offset) {
        return new TrackedValue(
                null, offset, null, null, null, null, null, null, thisExpression(offset));
    }

    static TrackedValue stackOffset(int offset) {
        return new TrackedValue(
                null, null, offset, null, null, null, null, null, stackExpression(offset));
    }

    static TrackedValue baseOffset(String baseKey, int offset) {
        return new TrackedValue(null, null, null, baseKey, offset, null, null, null,
                baseExpression(baseKey, offset));
    }

    static TrackedValue immediate(long value) {
        return new TrackedValue(
                null, null, null, null, null, value, null, null, Long.toUnsignedString(value));
    }

    static TrackedValue fieldName(Address formatAddress, String fieldName) {
        return new TrackedValue(
                null, null, null, null, null, null, formatAddress, fieldName, fieldName);
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
        return this;
    }

    TrackedValue copy() {
        return new TrackedValue(address, thisOffset, stackOffset, baseKey, baseOffset, immediate,
                fieldNameAddress, fieldName, expression);
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
                && sameObject(fieldName, other.fieldName);
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
