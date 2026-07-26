// Directional message codec evidence recovered from handler implementations.

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import ghidra.program.model.address.Address;
import java.util.ArrayList;
import java.util.List;

record MessageMarshalFrame(
    Address callsite,
    Address target,
    String targetName,
    String parameter,
    PcodeStorage source) {

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        NetworkSchemaJson.addAddress(object, "target", target, addresses);
        NetworkSchemaJson.add(object, "targetName", targetName);
        NetworkSchemaJson.add(object, "parameter", parameter);
        addStorage(object, "source", source);
        return object;
    }

    private static void addStorage(
        JsonObject object,
        String prefix,
        PcodeStorage storage) {

        if (storage == null) {
            return;
        }
        NetworkSchemaJson.add(object, prefix + "Base", storage.base);
        NetworkSchemaJson.add(
            object,
            prefix + "Offset",
            "0x" + Long.toHexString(storage.offset));
        NetworkSchemaJson.add(object, prefix + "Expression", storage.expression());
    }
}

record MessageMarshalSource(
    PcodeStorage local,
    PcodeStorage absolute) {}

final class MessageMarshalEvent {
    Address callsite;
    Address target;
    String targetName;
    String nativeType;
    WireShape wireEvidence;
    PcodeStorage localSource;
    PcodeStorage source;
    String evidenceSource;
    final ArrayList<MessageMarshalFrame> frames = new ArrayList<>();

    MessageMarshalEvent copy() {
        MessageMarshalEvent copy = new MessageMarshalEvent();
        copy.callsite = callsite;
        copy.target = target;
        copy.targetName = targetName;
        copy.nativeType = nativeType;
        copy.wireEvidence = wireEvidence;
        copy.localSource = localSource;
        copy.source = source;
        copy.evidenceSource = evidenceSource;
        copy.frames.addAll(frames);
        return copy;
    }

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        NetworkSchemaJson.addAddress(object, "target", target, addresses);
        NetworkSchemaJson.add(object, "targetName", targetName);
        NetworkSchemaJson.add(object, "nativeType", nativeType);
        NetworkSchemaJson.add(object, "confidence", "message-marshal-pcode-stack");
        if (wireEvidence != null) {
            NetworkSchemaJson.add(object, "wireShape", wireEvidence.shape);
            NetworkSchemaJson.add(object, "wireShapeSource", wireEvidence.source);
            NetworkSchemaJson.add(object, "wireLayout", wireEvidence.layout);
            NetworkSchemaJson.add(object, "wireLayoutSource", wireEvidence.layoutSource);
        }
        addStorage(object, "localSource", localSource);
        addStorage(object, "source", source);
        NetworkSchemaJson.add(object, "evidenceSource", evidenceSource);
        if (!frames.isEmpty()) {
            JsonArray array = new JsonArray();
            for (MessageMarshalFrame frame : frames) {
                array.add(frame.toJson(addresses));
            }
            object.add("callFrames", array);
        }
        return object;
    }

    private static void addStorage(
        JsonObject object,
        String prefix,
        PcodeStorage storage) {

        if (storage == null) {
            return;
        }
        NetworkSchemaJson.add(object, prefix + "Base", storage.base);
        NetworkSchemaJson.add(
            object,
            prefix + "Offset",
            "0x" + Long.toHexString(storage.offset));
        NetworkSchemaJson.add(object, prefix + "Expression", storage.expression());
    }
}

final class MessageMarshalField {
    int index;
    Address callsite;
    PcodeStorage storage;
    String nativeType;
    WireShape wireEvidence;
    final ArrayList<MessageMarshalEvent> events = new ArrayList<>();

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        object.addProperty("index", index);
        // Marshal fields are recovered positionally, exactly like unmarshal
        // fields, and neither direction carries a source-level name. Emitting
        // the same positional name and per-field confidence the unmarshal side
        // emits is what makes a marshal-only message plannable; without them it
        // reads as an unnamed, unknown-confidence field even though every
        // constituent event is structurally proven.
        NetworkSchemaJson.add(object, "name", "field_" + index);
        NetworkSchemaJson.add(object, "confidence", "message-marshal-pcode-stack");
        NetworkSchemaJson.addAddress(object, "callsite", callsite, addresses);
        if (storage != null) {
            NetworkSchemaJson.add(object, "storageBase", storage.base);
            NetworkSchemaJson.add(
                object,
                "storageOffset",
                "0x" + Long.toHexString(storage.offset));
            NetworkSchemaJson.add(object, "storageExpression", storage.expression());
        }
        NetworkSchemaJson.add(object, "nativeType", nativeType);
        if (wireEvidence != null) {
            NetworkSchemaJson.add(object, "wireShape", wireEvidence.shape);
            NetworkSchemaJson.add(object, "wireShapeSource", wireEvidence.source);
            NetworkSchemaJson.add(object, "wireLayout", wireEvidence.layout);
            NetworkSchemaJson.add(object, "wireLayoutSource", wireEvidence.layoutSource);
        }
        JsonArray array = new JsonArray();
        for (MessageMarshalEvent event : events) {
            array.add(event.toJson(addresses));
        }
        object.add("events", array);
        return object;
    }
}

final class MessageMarshalPlan {
    Address wrapper;
    String wrapperName;
    String writeBufferParameter;
    String rootStorageBase;
    String analysisStatus;
    final ArrayList<MessageMarshalEvent> events = new ArrayList<>();
    final ArrayList<MessageMarshalField> fields = new ArrayList<>();

    JsonObject toJson(NetworkSchemaAddressFormatter addresses) {
        JsonObject object = new JsonObject();
        NetworkSchemaJson.addAddress(object, "wrapper", wrapper, addresses);
        NetworkSchemaJson.add(object, "wrapperName", wrapperName);
        NetworkSchemaJson.add(object, "writeBufferParameter", writeBufferParameter);
        NetworkSchemaJson.add(object, "rootStorageBase", rootStorageBase);
        NetworkSchemaJson.add(object, "analysisStatus", analysisStatus);
        JsonArray eventArray = new JsonArray();
        for (MessageMarshalEvent event : events) {
            eventArray.add(event.toJson(addresses));
        }
        object.add("events", eventArray);
        JsonArray fieldArray = new JsonArray();
        for (MessageMarshalField field : fields) {
            fieldArray.add(field.toJson(addresses));
        }
        object.add("fields", fieldArray);
        return object;
    }
}
