// P-code evidence models for NetworkSchemaExtractor.
// Kept package-less so Ghidra compiles this file with the script source bundle.

import com.google.gson.JsonArray;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

final class PcodeArgStorageSelection {
    PcodeStorage storage;
    Integer storageArgSlot;
    PcodeStorage fallbackStorage;
    Integer fallbackStorageArgSlot;
    String selectionRule;
    final JsonArray argStorageEvidence = new JsonArray();
}

final class PcodeCallTargetInfo {
    Address rawTarget;
    Address resolvedTarget;
    Function target;
    boolean targetExactStart;
    Function containing;

    Address targetAddress() {
        return target == null ? resolvedTarget : target.getEntryPoint();
    }
}

final class PcodeUnmarshalEvidence {
    Address callsite;
    Address targetRawAddress;
    Address target;
    Address valueCallTarget;
    String targetName;
    String targetKind;
    Boolean targetExactStart;
    Address containingTarget;
    String containingTargetName;
    PcodeStorage storage;
    Integer storageArgSlot;
    String evidenceSource;
    JsonArray argStorageEvidence;
}
