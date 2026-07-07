// Stack and constructor evidence models for NetworkSchemaExtractor.
// These are package-less so Ghidra compiles them in the same source bundle.

import com.google.gson.JsonObject;
import ghidra.program.model.address.Address;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

final class HandlerConstructorWrite {
    final Address write;
    final int handlerOffset;
    final int relativeOffset;
    final Integer widthBits;
    final Integer byteLength;
    final String valueKind;
    final String value;
    final String valueHex;
    final String sourceOperand;
    final String source;

    HandlerConstructorWrite(Address write, int handlerOffset, int relativeOffset, Integer widthBits,
            Integer byteLength, String valueKind, String value, String valueHex,
            String sourceOperand, String source) {
        this.write = write;
        this.handlerOffset = handlerOffset;
        this.relativeOffset = relativeOffset;
        this.widthBits = widthBits;
        this.byteLength = byteLength;
        this.valueKind = valueKind;
        this.value = value;
        this.valueHex = valueHex;
        this.sourceOperand = sourceOperand;
        this.source = source;
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "write", write, addresses);
        object.addProperty("handlerOffset", "0x" + Integer.toHexString(handlerOffset));
        object.addProperty("relativeOffset", "0x" + Integer.toHexString(relativeOffset));
        if (widthBits != null) {
            object.addProperty("widthBits", widthBits);
        }
        if (byteLength != null) {
            object.addProperty("byteLength", byteLength);
        }
        NetworkSchemaJson.add(object, "valueKind", valueKind);
        NetworkSchemaJson.add(object, "value", value);
        NetworkSchemaJson.add(object, "valueHex", valueHex);
        NetworkSchemaJson.add(object, "sourceOperand", sourceOperand);
        NetworkSchemaJson.add(object, "source", source);
        return object;
    }
}

final class HandlerConstruction {
    final String pattern;
    final Address callsite;
    final Address constructor;
    final String constructorName;
    final Address vtable;

    HandlerConstruction(String pattern, Address callsite, Address constructor,
            String constructorName, Address vtable) {
        this.pattern = pattern;
        this.callsite = callsite;
        this.constructor = constructor;
        this.constructorName = constructorName;
        this.vtable = vtable;
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.add(object, "pattern", pattern);
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        NetworkSchemaJson.addAddress(object, "constructor", constructor, addresses);
        NetworkSchemaJson.add(object, "constructorName", constructorName);
        NetworkSchemaJson.addAddress(object, "vtable", vtable, addresses);
        return object;
    }
}

final class ArgState {
    Address nameAddress;
    String name;
    boolean groupKnown;
    int group;
    boolean handlerKnown;
    Integer handlerOffset;
    String handlerExpression;
    Address handlerVtable;
    HandlerConstruction handlerConstruction;
    List<HandlerConstructorWrite> handlerConstructorWrites;

    void fillMissingFrom(ArgState fallback) {
        if (nameAddress == null) {
            nameAddress = fallback.nameAddress;
            name = fallback.name;
        }
        if (!groupKnown && fallback.groupKnown) {
            groupKnown = true;
            group = fallback.group;
        }
        if (!handlerKnown && fallback.handlerKnown) {
            handlerKnown = true;
            handlerOffset = fallback.handlerOffset;
            handlerExpression = fallback.handlerExpression;
            handlerVtable = fallback.handlerVtable;
            handlerConstruction = fallback.handlerConstruction;
            handlerConstructorWrites = fallback.handlerConstructorWrites;
        } else if (handlerVtable == null) {
            handlerVtable = fallback.handlerVtable;
        }
        if (handlerConstruction == null) {
            handlerConstruction = fallback.handlerConstruction;
        }
        if (handlerConstructorWrites == null) {
            handlerConstructorWrites = fallback.handlerConstructorWrites;
        }
    }
}

final class ForwardArgState {
    final Map<String, TrackedValue> registers = new HashMap<>();
    final Map<Integer, Address> vtablesByThisOffset = new HashMap<>();
    final Map<Integer, HandlerConstruction> handlerConstructionsByThisOffset = new HashMap<>();
    final Map<Integer, List<HandlerConstructorWrite>> constructorWritesByHandlerOffset =
            new HashMap<>();
    final Map<String, Address> vtablesByBaseOffset = new HashMap<>();
    final Map<String, HandlerConstruction> handlerConstructionsByBaseOffset = new HashMap<>();
    final Map<String, List<HandlerConstructorWrite>> constructorWritesByBaseOffset =
            new HashMap<>();
    final Set<String> allocatorDispatchRegisters = new LinkedHashSet<>();
    final Map<Integer, TrackedValue> valuesByThisOffset = new HashMap<>();
    final Map<Integer, TrackedValue> valuesByStackSlot = new HashMap<>();
    int nextFilterGroupIndex = 1;
    Boolean zeroFlag;
    Integer compareSigned;
    Integer compareUnsigned;

    ForwardArgState copy() {
        ForwardArgState state = new ForwardArgState();
        for (Map.Entry<String, TrackedValue> entry : registers.entrySet()) {
            state.registers.put(entry.getKey(), entry.getValue().copy());
        }
        state.allocatorDispatchRegisters.addAll(allocatorDispatchRegisters);
        state.copyObjectEvidenceFrom(this);
        for (Map.Entry<Integer, TrackedValue> entry : valuesByStackSlot.entrySet()) {
            state.valuesByStackSlot.put(entry.getKey(), entry.getValue().copy());
        }
        state.nextFilterGroupIndex = nextFilterGroupIndex;
        state.zeroFlag = zeroFlag;
        state.compareSigned = compareSigned;
        state.compareUnsigned = compareUnsigned;
        return state;
    }

    void copyObjectEvidenceFrom(ForwardArgState other) {
        vtablesByThisOffset.putAll(other.vtablesByThisOffset);
        handlerConstructionsByThisOffset.putAll(other.handlerConstructionsByThisOffset);
        for (Map.Entry<Integer, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByHandlerOffset.entrySet()) {
            constructorWritesByHandlerOffset.put(entry.getKey(), new ArrayList<>(entry.getValue()));
        }
        vtablesByBaseOffset.putAll(other.vtablesByBaseOffset);
        handlerConstructionsByBaseOffset.putAll(other.handlerConstructionsByBaseOffset);
        for (Map.Entry<String, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByBaseOffset.entrySet()) {
            constructorWritesByBaseOffset.put(entry.getKey(), new ArrayList<>(entry.getValue()));
        }
        for (Map.Entry<Integer, TrackedValue> entry : other.valuesByThisOffset.entrySet()) {
            valuesByThisOffset.put(entry.getKey(), entry.getValue().copy());
        }
    }

    boolean mergeCompatibleObjectEvidenceFrom(ForwardArgState other) {
        if (!compatibleAddressMap(vtablesByThisOffset, other.vtablesByThisOffset)) {
            return false;
        }
        if (!compatibleAddressMapByString(vtablesByBaseOffset, other.vtablesByBaseOffset)) {
            return false;
        }
        if (!compatibleRegister("RCX", other)) {
            return false;
        }
        mergeOptionalRegister("RDX", other);
        mergeOptionalRegister("R8", other);
        mergeOptionalRegister("R9", other);
        mergeOptionalRegister("RSP", other);
        if (!Objects.equals(zeroFlag, other.zeroFlag)) {
            zeroFlag = null;
        }
        if (!Objects.equals(compareSigned, other.compareSigned)) {
            compareSigned = null;
        }
        if (!Objects.equals(compareUnsigned, other.compareUnsigned)) {
            compareUnsigned = null;
        }
        allocatorDispatchRegisters.retainAll(other.allocatorDispatchRegisters);

        vtablesByThisOffset.putAll(other.vtablesByThisOffset);
        vtablesByBaseOffset.putAll(other.vtablesByBaseOffset);
        for (Map.Entry<Integer, HandlerConstruction> entry :
                other.handlerConstructionsByThisOffset.entrySet()) {
            handlerConstructionsByThisOffset.putIfAbsent(entry.getKey(), entry.getValue());
        }
        for (Map.Entry<String, HandlerConstruction> entry :
                other.handlerConstructionsByBaseOffset.entrySet()) {
            handlerConstructionsByBaseOffset.putIfAbsent(entry.getKey(), entry.getValue());
        }
        for (Map.Entry<Integer, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByHandlerOffset.entrySet()) {
            constructorWritesByHandlerOffset
                    .computeIfAbsent(entry.getKey(), ignored -> new ArrayList<>())
                    .addAll(entry.getValue());
        }
        for (Map.Entry<String, List<HandlerConstructorWrite>> entry :
                other.constructorWritesByBaseOffset.entrySet()) {
            constructorWritesByBaseOffset
                    .computeIfAbsent(entry.getKey(), ignored -> new ArrayList<>())
                    .addAll(entry.getValue());
        }
        for (Map.Entry<Integer, TrackedValue> entry : other.valuesByThisOffset.entrySet()) {
            TrackedValue existing = valuesByThisOffset.get(entry.getKey());
            if (existing != null && !existing.sameValue(entry.getValue())) {
                return false;
            }
            valuesByThisOffset.putIfAbsent(entry.getKey(), entry.getValue().copy());
        }
        return true;
    }

    private boolean compatibleRegister(String register, ForwardArgState other) {
        TrackedValue left = registers.get(register);
        TrackedValue right = other.registers.get(register);
        if (left == null || right == null) {
            return left == right;
        }
        return left.sameValue(right);
    }

    private void mergeOptionalRegister(String register, ForwardArgState other) {
        TrackedValue left = registers.get(register);
        TrackedValue right = other.registers.get(register);
        if (left == null || right == null) {
            registers.remove(register);
            return;
        }
        if (!left.sameValue(right)) {
            registers.remove(register);
        }
    }

    private static boolean compatibleAddressMap(
            Map<Integer, Address> left, Map<Integer, Address> right) {
        for (Map.Entry<Integer, Address> entry : right.entrySet()) {
            Address existing = left.get(entry.getKey());
            if (existing != null && !existing.equals(entry.getValue())) {
                return false;
            }
        }
        return true;
    }

    private static boolean compatibleAddressMapByString(
            Map<String, Address> left, Map<String, Address> right) {
        for (Map.Entry<String, Address> entry : right.entrySet()) {
            Address existing = left.get(entry.getKey());
            if (existing != null && !existing.equals(entry.getValue())) {
                return false;
            }
        }
        return true;
    }
}
