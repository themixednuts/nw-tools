// Nested type and container-shape models for NetworkSchemaExtractor.
// Kept package-less so Ghidra compiles this file with the script source bundle.

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import ghidra.program.model.address.Address;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

final class NestedTypeShape {
    String typeId;
    String typeIdSource;
    Boolean identityProven;
    String identitySource;
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
    Boolean layoutProven;
    Boolean memberCoverageProven;
    Boolean wireOrderProven;
    String wireOrderSource;
    String datatypePath;
    String validation;
    Long nativeSize;
    String nativeSizeSource;
    final ArrayList<NestedTypeMember> members = new ArrayList<>();

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.add(object, "typeId", typeId);
        NetworkSchemaJson.add(object, "typeIdSource", typeIdSource);
        if (identityProven != null) {
            object.addProperty("identityProven", identityProven);
        }
        NetworkSchemaJson.add(object, "identitySource", identitySource);
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
        if (layoutProven != null) {
            object.addProperty("layoutProven", layoutProven);
        }
        if (memberCoverageProven != null) {
            object.addProperty("memberCoverageProven", memberCoverageProven);
        }
        if (wireOrderProven != null) {
            object.addProperty("wireOrderProven", wireOrderProven);
        }
        NetworkSchemaJson.add(object, "wireOrderSource", wireOrderSource);
        NetworkSchemaJson.add(object, "datatypePath", datatypePath);
        NetworkSchemaJson.add(object, "validation", validation);
        if (nativeSize != null) {
            object.addProperty("nativeSize", nativeSize);
        }
        NetworkSchemaJson.add(object, "nativeSizeSource", nativeSizeSource);
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
    final ArrayList<SerializeFieldInfo> layoutFields = new ArrayList<>();
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
    String typeId;
    String typeIdSource;
    Boolean typeIdentityProven;
    String typeIdentitySource;
    String wireShape;
    String wireShapeSource;
    String wireLayout;
    String wireLayoutSource;
    Integer byteWidth;
    Integer wireOrdinal;
    String wireOrderSource;
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
        NetworkSchemaJson.add(object, "typeId", typeId);
        NetworkSchemaJson.add(object, "typeIdSource", typeIdSource);
        if (typeIdentityProven != null) {
            object.addProperty("typeIdentityProven", typeIdentityProven);
        }
        NetworkSchemaJson.add(object, "typeIdentitySource", typeIdentitySource);
        NetworkSchemaJson.add(object, "wireShape", wireShape);
        NetworkSchemaJson.add(object, "wireShapeSource", wireShapeSource);
        NetworkSchemaJson.add(object, "wireLayout", wireLayout);
        NetworkSchemaJson.add(object, "wireLayoutSource", wireLayoutSource);
        if (byteWidth != null) {
            object.addProperty("byteWidth", byteWidth);
        }
        if (wireOrdinal != null) {
            object.addProperty("wireOrdinal", wireOrdinal);
        }
        NetworkSchemaJson.add(object, "wireOrderSource", wireOrderSource);
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
    final WireShape deltaShape;
    final WireShape fullShape;
    final String keyNativeType;
    final String keyNativeTypeSource;
    final List<WireShape> deltaMarshalEvidence;
    final List<WireShape> fullMarshalEvidence;
    final NativeTypeInfoEvidence valueTypeInfo;
    final NestedTypeShape valueTypeShape;
    final List<NativeTypeInfoEvidence> valueTypeInfoCandidates;
    final List<NestedTypeShape> embeddedValueTypeShapes;
    final FullContainerPlan fullContainerPlan;
    final List<ContainerPlanDiagnostic> fullContainerPlanDiagnostics;

    ContainerWireShape(WireShape primaryShape, WireShape deltaShape, WireShape fullShape,
            String keyNativeType, String keyNativeTypeSource,
            List<WireShape> deltaMarshalEvidence,
            List<WireShape> fullMarshalEvidence, NativeTypeInfoEvidence valueTypeInfo,
            NestedTypeShape valueTypeShape, List<NativeTypeInfoEvidence> valueTypeInfoCandidates,
            List<NestedTypeShape> embeddedValueTypeShapes,
            FullContainerPlan fullContainerPlan,
            List<ContainerPlanDiagnostic> fullContainerPlanDiagnostics) {
        this.primaryShape = primaryShape;
        this.deltaShape = deltaShape;
        this.fullShape = fullShape;
        this.keyNativeType = keyNativeType;
        this.keyNativeTypeSource = keyNativeTypeSource;
        this.deltaMarshalEvidence = List.copyOf(deltaMarshalEvidence);
        this.fullMarshalEvidence = List.copyOf(fullMarshalEvidence);
        this.valueTypeInfo = valueTypeInfo;
        this.valueTypeShape = valueTypeShape;
        this.valueTypeInfoCandidates = List.copyOf(valueTypeInfoCandidates);
        this.embeddedValueTypeShapes = List.copyOf(embeddedValueTypeShapes);
        this.fullContainerPlan = fullContainerPlan;
        this.fullContainerPlanDiagnostics = List.copyOf(fullContainerPlanDiagnostics);
    }
}

final class NativeTypeInfoEvidence {
    final Address address;
    final String name;
    final String typeId;
    final String source;
    final String nameSource;
    final Long nativeSize;
    final String nativeSizeSource;

    NativeTypeInfoEvidence(
            Address address, String name, String typeId, String source, String nameSource) {
        this(address, name, typeId, source, nameSource, null, null);
    }

    NativeTypeInfoEvidence(
            Address address,
            String name,
            String typeId,
            String source,
            String nameSource,
            Long nativeSize,
            String nativeSizeSource) {
        this.address = address;
        this.name = name;
        this.typeId = typeId;
        this.source = source;
        this.nameSource = nameSource;
        this.nativeSize = nativeSize;
        this.nativeSizeSource = nativeSizeSource;
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "address", address, addresses);
        NetworkSchemaJson.add(object, "name", name);
        NetworkSchemaJson.add(object, "typeId", typeId);
        NetworkSchemaJson.add(object, "source", source);
        NetworkSchemaJson.add(object, "nameSource", nameSource);
        if (nativeSize != null) {
            object.addProperty("nativeSize", "0x" + Long.toHexString(nativeSize));
        }
        NetworkSchemaJson.add(object, "nativeSizeSource", nativeSizeSource);
        return object;
    }
}

/** Proven fixed-capacity sequence storage and its element codec. */
final class FixedSequenceShape {
    final int capacity;
    final long elementStride;
    final long dataOffset;
    final long endOffset;
    final WireShape elementWireEvidence;
    final WireShape wireEvidence;
    final NativeTypeInfoEvidence elementTypeInfo;
    final Address countCallsite;
    final Address loopHeader;
    final String source;

    FixedSequenceShape(
            int capacity,
            long elementStride,
            long dataOffset,
            long endOffset,
            WireShape elementWireEvidence,
            NativeTypeInfoEvidence elementTypeInfo,
            Address countCallsite,
            Address loopHeader,
            String source) {
        this.capacity = capacity;
        this.elementStride = elementStride;
        this.dataOffset = dataOffset;
        this.endOffset = endOffset;
        this.elementWireEvidence = elementWireEvidence;
        this.elementTypeInfo = elementTypeInfo;
        this.countCallsite = countCallsite;
        this.loopHeader = loopHeader;
        this.source = source;

        String elementShape = elementWireEvidence == null
            ? null
            : elementWireEvidence.shape == null
                ? elementWireEvidence.layout
                : elementWireEvidence.shape;
        String elementLayout = elementWireEvidence == null
            ? null
            : elementWireEvidence.layout;
        this.wireEvidence = elementShape == null || elementLayout == null
            ? null
            : WireShape.semantic(
                "fixed-vector<" + elementShape + "," + capacity + ">",
                "vec<" + elementLayout + ">",
                source);
    }

    boolean sameStorageAndWire(FixedSequenceShape other) {
        return other != null &&
            capacity == other.capacity &&
            elementStride == other.elementStride &&
            Objects.equals(
                elementWireEvidence == null ? null : elementWireEvidence.layout,
                other.elementWireEvidence == null ? null : other.elementWireEvidence.layout);
    }

    FixedSequenceShape withElementTypeInfo(NativeTypeInfoEvidence typeInfo, String mergedSource) {
        return new FixedSequenceShape(
            capacity,
            elementStride,
            dataOffset,
            endOffset,
            elementWireEvidence,
            typeInfo,
            countCallsite,
            loopHeader,
            mergedSource);
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        object.addProperty("storageKind", "inline-fixed");
        object.addProperty("capacity", capacity);
        object.addProperty("elementStride", "0x" + Long.toHexString(elementStride));
        object.addProperty("dataOffset", "0x" + Long.toHexString(dataOffset));
        object.addProperty("endOffset", "0x" + Long.toHexString(endOffset));
        NetworkSchemaJson.add(object, "source", source);
        NetworkSchemaJson.addAddress(object, "countCallsite", countCallsite, addresses);
        NetworkSchemaJson.addAddress(object, "loopHeader", loopHeader, addresses);
        if (elementWireEvidence != null) {
            NetworkSchemaJson.add(object, "elementWireShape", elementWireEvidence.shape);
            NetworkSchemaJson.add(object, "elementWireShapeSource", elementWireEvidence.source);
            NetworkSchemaJson.add(object, "elementWireLayout", elementWireEvidence.layout);
            NetworkSchemaJson.add(
                object,
                "elementWireLayoutSource",
                elementWireEvidence.layoutSource);
        }
        if (elementTypeInfo != null) {
            object.add("elementTypeInfo", elementTypeInfo.toJson(addresses));
        }
        return object;
    }
}

/** Structural proof that an instance vtable carries the shared replicated-state ABI. */
final class ReplicatedStateAbiEvidence {
    final int firstSlot;
    final List<Address> targets;
    final int cohortCount;
    final String abiKind;
    final String source;

    ReplicatedStateAbiEvidence(
            int firstSlot,
            List<Address> targets,
            int cohortCount,
            String abiKind,
            String source) {
        this.firstSlot = firstSlot;
        this.targets = List.copyOf(targets);
        this.cohortCount = cohortCount;
        this.abiKind = abiKind;
        this.source = source;
    }

    String signatureKey() {
        return targets.stream()
            .map(Address::toString)
            .collect(java.util.stream.Collectors.joining(":"));
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        object.addProperty("source", source);
        object.addProperty("firstSlot", firstSlot);
        object.addProperty("slotCount", targets.size());
        object.addProperty("cohortCount", cohortCount);
        object.addProperty("abiKind", abiKind);
        JsonArray functions = new JsonArray();
        for (int index = 0; index < targets.size(); index++) {
            JsonObject function = new JsonObject();
            function.addProperty("slot", firstSlot + index);
            NetworkSchemaJson.addAddress(
                function,
                "function",
                targets.get(index),
                addresses);
            functions.add(function);
        }
        object.add("functions", functions);
        return object;
    }
}
