// Replicated-container plans recovered from full-marshaling control flow.

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import ghidra.program.model.address.Address;
import ghidra.program.model.pcode.PcodeOpAST;
import java.util.ArrayList;
import java.util.List;

enum ContainerStorageKind {
    INDEX_MAP("index-map"),
    VECTOR("vector");

    final String jsonName;

    ContainerStorageKind(String jsonName) {
        this.jsonName = jsonName;
    }
}

final class ContainerCodecCall {
    PcodeOpAST operation;
    Address callsite;
    Address target;
    String targetName;
    String nativeType;
    String typeId;
    String typeIdSource;
    boolean typeIdentityProven;
    boolean sourceTypeLayoutComplete;
    WireShape wireEvidence;
    String evidenceSource;
    String memberSemantics;
    String analysisStatus;
    String elementBase;
    Long elementOffset;
    final ArrayList<ContainerCodecCall> members = new ArrayList<>();
    final ArrayList<ContainerCodecCall> optionalMembers = new ArrayList<>();
    final ArrayList<ContainerCodecGuard> guards = new ArrayList<>();

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        NetworkSchemaJson.addAddress(object, "target", target, addresses);
        NetworkSchemaJson.add(object, "targetName", targetName);
        NetworkSchemaJson.add(object, "nativeType", nativeType);
        NetworkSchemaJson.add(object, "typeId", typeId);
        NetworkSchemaJson.add(object, "typeIdSource", typeIdSource);
        object.addProperty("typeIdentityProven", typeIdentityProven);
        object.addProperty("sourceTypeLayoutComplete", sourceTypeLayoutComplete);
        if (wireEvidence != null) {
            NetworkSchemaJson.add(object, "wireShape", wireEvidence.shape);
            NetworkSchemaJson.add(object, "wireShapeSource", wireEvidence.source);
            NetworkSchemaJson.add(object, "wireLayout", wireEvidence.layout);
            NetworkSchemaJson.add(object, "wireLayoutSource", wireEvidence.layoutSource);
        }
        NetworkSchemaJson.add(object, "evidenceSource", evidenceSource);
        NetworkSchemaJson.add(object, "memberSemantics", memberSemantics);
        NetworkSchemaJson.add(object, "analysisStatus", analysisStatus);
        NetworkSchemaJson.add(object, "elementBase", elementBase);
        if (elementOffset != null) {
            object.addProperty("elementOffset", "0x" + Long.toHexString(elementOffset));
        }
        object.add("members", codecArray(members, addresses));
        if (!optionalMembers.isEmpty()) {
            object.add("optionalMembers", codecArray(optionalMembers, addresses));
        }
        if (!guards.isEmpty()) {
            JsonArray guardArray = new JsonArray();
            for (ContainerCodecGuard guard : guards) {
                guardArray.add(guard.toJson(addresses));
            }
            object.add("guards", guardArray);
        }
        return object;
    }

    private static JsonArray codecArray(
        List<ContainerCodecCall> codecs,
        NetworkSchemaAddressFormatter addresses) {

        JsonArray result = new JsonArray();
        for (ContainerCodecCall codec : codecs) {
            result.add(codec.toJson(addresses));
        }
        return result;
    }
}

record ContainerCodecGuard(
    Address branch,
    String kind,
    String condition,
    boolean memberOnTrue,
    String storageBase,
    Long storageOffset,
    Address storageAddress,
    Long mask,
    ContainerExternalBooleanCondition externalCondition,
    String externalConditionDiagnostic,
    String evidenceSource) {

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "branch", branch, addresses);
        NetworkSchemaJson.add(object, "kind", kind);
        NetworkSchemaJson.add(object, "condition", condition);
        object.addProperty("memberOnTrue", memberOnTrue);
        NetworkSchemaJson.add(object, "storageBase", storageBase);
        if (storageOffset != null) {
            object.addProperty("storageOffset", "0x" + Long.toHexString(storageOffset));
        }
        NetworkSchemaJson.addAddress(object, "storageAddress", storageAddress, addresses);
        if (mask != null) {
            object.addProperty("mask", "0x" + Long.toHexString(mask));
        }
        if (externalCondition != null) {
            object.add("externalCondition", externalCondition.toJson(addresses));
        }
        NetworkSchemaJson.add(
            object,
            "externalConditionDiagnostic",
            externalConditionDiagnostic);
        NetworkSchemaJson.add(object, "evidenceSource", evidenceSource);
        return object;
    }
}

record ContainerExternalBooleanCondition(
    Address resolverObject,
    Address resolverVtable,
    Integer resolverSlot,
    Address resolver,
    Address conditionStorage,
    Long conditionOffset,
    Address owner,
    Long subobjectOffset,
    Address destructorThunk,
    Address completeDestructor,
    Address initializer,
    Address nameField,
    Long nameOffset,
    Address nameBegin,
    Address nameEnd,
    String name,
    Boolean defaultValue,
    Address defaultWrite,
    Address defaultCallsite,
    Address defaultTarget,
    String evidenceSource) {

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "resolverObject", resolverObject, addresses);
        NetworkSchemaJson.addAddress(object, "resolverVtable", resolverVtable, addresses);
        if (resolverSlot != null) {
            object.addProperty("resolverSlot", resolverSlot);
        }
        NetworkSchemaJson.addAddress(object, "resolver", resolver, addresses);
        NetworkSchemaJson.addAddress(object, "conditionStorage", conditionStorage, addresses);
        addOffset(object, "conditionOffset", conditionOffset);
        NetworkSchemaJson.addAddress(object, "owner", owner, addresses);
        addOffset(object, "subobjectOffset", subobjectOffset);
        NetworkSchemaJson.addAddress(object, "destructorThunk", destructorThunk, addresses);
        NetworkSchemaJson.addAddress(object, "completeDestructor", completeDestructor, addresses);
        NetworkSchemaJson.addAddress(object, "initializer", initializer, addresses);
        NetworkSchemaJson.addAddress(object, "nameField", nameField, addresses);
        addOffset(object, "nameOffset", nameOffset);
        NetworkSchemaJson.addAddress(object, "nameBegin", nameBegin, addresses);
        NetworkSchemaJson.addAddress(object, "nameEnd", nameEnd, addresses);
        NetworkSchemaJson.add(object, "name", name);
        if (defaultValue != null) {
            object.addProperty("defaultValue", defaultValue);
        }
        NetworkSchemaJson.addAddress(
            object,
            "defaultWrite",
            defaultWrite,
            addresses);
        NetworkSchemaJson.addAddress(
            object,
            "defaultCallsite",
            defaultCallsite,
            addresses);
        NetworkSchemaJson.addAddress(
            object,
            "defaultTarget",
            defaultTarget,
            addresses);
        NetworkSchemaJson.add(object, "evidenceSource", evidenceSource);
        return object;
    }

    private static void addOffset(JsonObject object, String key, Long value) {
        if (value != null) {
            object.addProperty(key, "0x" + Long.toHexString(value));
        }
    }
}

record ContainerCodecMembers(
    List<ContainerCodecCall> members,
    List<ContainerCodecCall> optionalMembers,
    boolean linear,
    boolean exact,
    String analysisStatus) {
    ContainerCodecMembers {
        members = List.copyOf(members);
        optionalMembers = List.copyOf(optionalMembers);
    }

    static ContainerCodecMembers unresolved(String status) {
        return new ContainerCodecMembers(List.of(), List.of(), false, false, status);
    }
}

final class FullContainerPlan {
    Address marshalFull;
    Address analysisFunction;
    Address loopHeader;
    ContainerStorageKind storageKind;
    Long elementStride;
    String inductionSource;
    String unmarshalStorageProof;
    String unmarshalReconciliation;
    String unmarshalAnalysisStatus;
    final ArrayList<String> unmarshalDiagnostics = new ArrayList<>();
    int helperDepth;
    final ArrayList<ContainerCodecCall> keyCodecs = new ArrayList<>();
    final ArrayList<ContainerCodecCall> valueCodecs = new ArrayList<>();
    final ArrayList<ContainerCodecCall> unmarshalCodecs = new ArrayList<>();

    boolean hasValueCodecs() {
        return !valueCodecs.isEmpty();
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "marshalFull", marshalFull, addresses);
        NetworkSchemaJson.addAddress(object, "analysisFunction", analysisFunction, addresses);
        NetworkSchemaJson.addAddress(object, "loopHeader", loopHeader, addresses);
        if (storageKind != null) {
            object.addProperty("storageKind", storageKind.jsonName);
        }
        if (elementStride != null) {
            object.addProperty("elementStride", "0x" + Long.toHexString(elementStride));
        }
        NetworkSchemaJson.add(object, "inductionSource", inductionSource);
        NetworkSchemaJson.add(object, "unmarshalStorageProof", unmarshalStorageProof);
        NetworkSchemaJson.add(object, "unmarshalReconciliation", unmarshalReconciliation);
        NetworkSchemaJson.add(object, "unmarshalAnalysisStatus", unmarshalAnalysisStatus);
        if (!unmarshalDiagnostics.isEmpty()) {
            JsonArray diagnostics = new JsonArray();
            unmarshalDiagnostics.forEach(diagnostics::add);
            object.add("unmarshalDiagnostics", diagnostics);
        }
        object.addProperty("helperDepth", helperDepth);
        object.add("keyCodecs", codecArray(keyCodecs, addresses));
        object.add("valueCodecs", codecArray(valueCodecs, addresses));
        object.add("unmarshalCodecs", codecArray(unmarshalCodecs, addresses));
        return object;
    }

    private static JsonArray codecArray(
        List<ContainerCodecCall> codecs,
        NetworkSchemaAddressFormatter addresses) {

        JsonArray result = new JsonArray();
        for (ContainerCodecCall codec : codecs) {
            result.add(codec.toJson(addresses));
        }
        return result;
    }
}

final class FullContainerPlanRecovery {
    final FullContainerPlan plan;
    final List<ContainerPlanDiagnostic> diagnostics;

    FullContainerPlanRecovery(
            FullContainerPlan plan,
            List<ContainerPlanDiagnostic> diagnostics) {
        this.plan = plan;
        this.diagnostics = List.copyOf(diagnostics);
    }
}

record ContainerPlanDiagnostic(
    Address function,
    Address loopHeader,
    Address callsite,
    Address target,
    String targetName,
    String stage,
    String reason,
    String expectedStorage,
    Integer pcodeBufferSlot,
    Integer abiBufferSlot,
    Integer codecCount,
    String induction) {

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "function", function, addresses);
        NetworkSchemaJson.addAddress(object, "loopHeader", loopHeader, addresses);
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        NetworkSchemaJson.addAddress(object, "target", target, addresses);
        NetworkSchemaJson.add(object, "targetName", targetName);
        NetworkSchemaJson.add(object, "stage", stage);
        NetworkSchemaJson.add(object, "reason", reason);
        NetworkSchemaJson.add(object, "expectedStorage", expectedStorage);
        if (pcodeBufferSlot != null) {
            object.addProperty("pcodeBufferSlot", pcodeBufferSlot);
        }
        if (abiBufferSlot != null) {
            object.addProperty("abiBufferSlot", abiBufferSlot);
        }
        if (codecCount != null) {
            object.addProperty("codecCount", codecCount);
        }
        NetworkSchemaJson.add(object, "induction", induction);
        return object;
    }
}

record ContainerLoopInduction(
    ContainerStorageKind storageKind,
    Long stride,
    String source) {
}

record HandlerContainerTypeEvidence(
    ContainerStorageKind storageKind,
    String keyNativeType,
    String valueNativeType,
    String storageNativeType,
    String keyMarshalerType,
    String valueMarshalerType,
    String source) {

    JsonObject toJson() {
        JsonObject object = new JsonObject();
        object.addProperty("storageKind", storageKind.jsonName);
        NetworkSchemaJson.add(object, "keyNativeType", keyNativeType);
        NetworkSchemaJson.add(object, "valueNativeType", valueNativeType);
        NetworkSchemaJson.add(object, "storageNativeType", storageNativeType);
        NetworkSchemaJson.add(object, "keyMarshalerType", keyMarshalerType);
        NetworkSchemaJson.add(object, "valueMarshalerType", valueMarshalerType);
        NetworkSchemaJson.add(object, "source", source);
        return object;
    }
}
