// Nested type and container-shape models for NetworkSchemaExtractor.
// Kept package-less so Ghidra compiles this file with the script source bundle.

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import ghidra.program.model.address.Address;
import java.util.ArrayList;
import java.util.List;

final class NestedTypeShape {
    String typeId;
    String typeIdSource;
    String typeName;
    String typeNameFull;
    String typeNameSource;
    Address function;
    String functionName;
    String factory;
    String azRttiAddress;
    Address constructor;
    Address vtable;
    String memberBase;
    String memberNameSource;
    Boolean memberNamesProven;
    String datatypePath;
    String validation;
    final ArrayList<NestedTypeMember> members = new ArrayList<>();

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.add(object, "typeId", typeId);
        NetworkSchemaJson.add(object, "typeIdSource", typeIdSource);
        NetworkSchemaJson.add(object, "typeName", typeName);
        NetworkSchemaJson.add(object, "typeNameFull", typeNameFull);
        NetworkSchemaJson.add(object, "typeNameSource", typeNameSource);
        NetworkSchemaJson.addAddress(object, "function", function, addresses);
        NetworkSchemaJson.add(object, "functionName", functionName);
        NetworkSchemaJson.add(object, "factory", factory);
        NetworkSchemaJson.add(object, "azRttiAddress", azRttiAddress);
        NetworkSchemaJson.addAddress(object, "constructor", constructor, addresses);
        NetworkSchemaJson.addAddress(object, "vtable", vtable, addresses);
        NetworkSchemaJson.add(object, "memberBase", memberBase);
        NetworkSchemaJson.add(object, "memberNameSource", memberNameSource);
        if (memberNamesProven != null) {
            object.addProperty("memberNamesProven", memberNamesProven);
        }
        NetworkSchemaJson.add(object, "datatypePath", datatypePath);
        NetworkSchemaJson.add(object, "validation", validation);
        JsonArray memberJson = new JsonArray();
        for (NestedTypeMember member : members) {
            memberJson.add(member.toJson(addresses));
        }
        object.add("members", memberJson);
        return object;
    }
}

final class NestedDirectTypeCallShape {
    final PcodeStorage storage;
    final NestedTypeShape shape;

    NestedDirectTypeCallShape(PcodeStorage storage, NestedTypeShape shape) {
        this.storage = storage;
        this.shape = shape;
    }
}

final class SerializeTypeInfo {
    String typeId;
    String name;
    String factory;
    String azRttiAddress;
    final ArrayList<SerializeFieldInfo> fields = new ArrayList<>();
}

final class NestedTypeMember {
    int index;
    long offset;
    Long nativeOffset;
    String name;
    String nameSource;
    Boolean nameProven;
    String nameEvidence;
    String nativeType;
    String wireShape;
    Integer byteWidth;
    String evidenceSource;
    Address callsite;
    Address target;
    String targetName;
    Boolean typeConflict;

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        object.addProperty("index", index);
        object.addProperty("offset", "0x" + Long.toHexString(offset));
        if (nativeOffset != null) {
            object.addProperty("nativeOffset", "0x" + Long.toHexString(nativeOffset));
        }
        NetworkSchemaJson.add(object, "name", name);
        NetworkSchemaJson.add(object, "nameSource", nameSource);
        if (nameProven != null) {
            object.addProperty("nameProven", nameProven);
        }
        NetworkSchemaJson.add(object, "nameEvidence", nameEvidence);
        NetworkSchemaJson.add(object, "nativeType", nativeType);
        NetworkSchemaJson.add(object, "wireShape", wireShape);
        if (byteWidth != null) {
            object.addProperty("byteWidth", byteWidth);
        }
        NetworkSchemaJson.add(object, "evidenceSource", evidenceSource);
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        NetworkSchemaJson.addAddress(object, "target", target, addresses);
        NetworkSchemaJson.add(object, "targetName", targetName);
        if (typeConflict != null) {
            object.addProperty("typeConflict", typeConflict);
        }
        return object;
    }
}

final class ContainerWireShape {
    final WireShape primaryShape;
    final String deltaShape;
    final String fullShape;
    final String keyNativeType;
    final String keyNativeTypeSource;
    final List<String> deltaMarshalShapes;
    final List<String> fullMarshalShapes;
    final NativeTypeInfoEvidence valueTypeInfo;
    final NestedTypeShape valueTypeShape;
    final List<NativeTypeInfoEvidence> valueTypeInfoCandidates;
    final List<NestedTypeShape> embeddedValueTypeShapes;

    ContainerWireShape(WireShape primaryShape, String deltaShape, String fullShape,
            List<String> deltaMarshalShapes, List<String> fullMarshalShapes,
            NativeTypeInfoEvidence valueTypeInfo, NestedTypeShape valueTypeShape,
            List<NativeTypeInfoEvidence> valueTypeInfoCandidates,
            List<NestedTypeShape> embeddedValueTypeShapes) {
        this(primaryShape, deltaShape, fullShape, null, null, deltaMarshalShapes, fullMarshalShapes,
                valueTypeInfo, valueTypeShape, valueTypeInfoCandidates, embeddedValueTypeShapes);
    }

    ContainerWireShape(WireShape primaryShape, String deltaShape, String fullShape,
            String keyNativeType, String keyNativeTypeSource, List<String> deltaMarshalShapes,
            List<String> fullMarshalShapes, NativeTypeInfoEvidence valueTypeInfo,
            NestedTypeShape valueTypeShape, List<NativeTypeInfoEvidence> valueTypeInfoCandidates,
            List<NestedTypeShape> embeddedValueTypeShapes) {
        this.primaryShape = primaryShape;
        this.deltaShape = deltaShape;
        this.fullShape = fullShape;
        this.keyNativeType = keyNativeType;
        this.keyNativeTypeSource = keyNativeTypeSource;
        this.deltaMarshalShapes = List.copyOf(deltaMarshalShapes);
        this.fullMarshalShapes = List.copyOf(fullMarshalShapes);
        this.valueTypeInfo = valueTypeInfo;
        this.valueTypeShape = valueTypeShape;
        this.valueTypeInfoCandidates = List.copyOf(valueTypeInfoCandidates);
        this.embeddedValueTypeShapes = List.copyOf(embeddedValueTypeShapes);
    }
}

final class NativeTypeInfoEvidence {
    final Address address;
    final String name;
    final String typeId;
    final String source;
    final String nameSource;

    NativeTypeInfoEvidence(
            Address address, String name, String typeId, String source, String nameSource) {
        this.address = address;
        this.name = name;
        this.typeId = typeId;
        this.source = source;
        this.nameSource = nameSource;
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "address", address, addresses);
        NetworkSchemaJson.add(object, "name", name);
        NetworkSchemaJson.add(object, "typeId", typeId);
        NetworkSchemaJson.add(object, "source", source);
        NetworkSchemaJson.add(object, "nameSource", nameSource);
        return object;
    }
}
