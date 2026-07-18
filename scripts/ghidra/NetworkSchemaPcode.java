// P-code evidence models for NetworkSchemaExtractor.
// Kept package-less so Ghidra compiles this file with the script source bundle.

import com.google.gson.JsonArray;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.pcode.PcodeOpAST;
import ghidra.program.model.pcode.Varnode;
import java.util.ArrayList;
import java.util.List;

final class PcodeArgStorageSelection {
    PcodeStorage storage;
    Integer storageArgSlot;
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

/**
 * Symbolic pointer provenance rooted at storage or a static program address.
 *
 * <p>Each entry in {@code dereferenceOffsets} is the offset applied before a
 * load. The trailing {@code offset} is pointer arithmetic performed after the
 * final load. For example, a virtual slot expression
 * {@code LOAD(LOAD(param_3) + 0x40)} is represented as root {@code param_3},
 * dereference offsets {@code [0, 0x40]}, and trailing offset {@code 0}.</p>
 */
record PcodeMemoryRoot(String storageBase, Address staticAddress) {
    PcodeMemoryRoot {
        if ((storageBase == null) == (staticAddress == null)) {
            throw new IllegalArgumentException(
                "memory root must be either storage or a static address");
        }
    }

    static PcodeMemoryRoot storage(String storageBase) {
        return new PcodeMemoryRoot(storageBase, null);
    }

    static PcodeMemoryRoot staticAddress(Address address) {
        return new PcodeMemoryRoot(null, address);
    }

    boolean isStorage(String expected) {
        return expected != null && expected.equals(storageBase);
    }
}

record PcodeMemoryPath(PcodeMemoryRoot root, List<Long> dereferenceOffsets, long offset) {
    PcodeMemoryPath {
        dereferenceOffsets = List.copyOf(dereferenceOffsets);
    }

    static PcodeMemoryPath storageRoot(String storageBase, long offset) {
        return new PcodeMemoryPath(PcodeMemoryRoot.storage(storageBase), List.of(), offset);
    }

    static PcodeMemoryPath staticRoot(Address address) {
        return new PcodeMemoryPath(PcodeMemoryRoot.staticAddress(address), List.of(), 0L);
    }

    PcodeMemoryPath plus(long delta) {
        try {
            return new PcodeMemoryPath(
                root,
                dereferenceOffsets,
                Math.addExact(offset, delta));
        }
        catch (ArithmeticException ignored) {
            return null;
        }
    }

    PcodeMemoryPath dereference() {
        ArrayList<Long> offsets = new ArrayList<>(dereferenceOffsets);
        offsets.add(offset);
        return new PcodeMemoryPath(root, offsets, 0L);
    }

    PcodeStorage directStorage() {
        return dereferenceOffsets.isEmpty() && root.storageBase() != null
            ? new PcodeStorage(root.storageBase(), offset)
            : null;
    }

    PcodeStorage loadedStorage() {
        return dereferenceOffsets.size() == 1 && offset == 0L &&
            root.storageBase() != null
            ? new PcodeStorage(root.storageBase(), dereferenceOffsets.get(0))
            : null;
    }

    PcodeVirtualDispatch virtualDispatch() {
        return dereferenceOffsets.size() == 2 && offset == 0L
            ? new PcodeVirtualDispatch(
                root,
                dereferenceOffsets.get(0),
                dereferenceOffsets.get(1))
            : null;
    }
}

record PcodeVirtualDispatch(
        PcodeMemoryRoot root,
        long objectOffset,
        long slotOffset) {
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

record CodecWireEvent(
    String nativeType,
    WireShape wireEvidence,
    Address callsite,
    Address target,
    String targetName) {
}

final class CodecWireTrace {
    final ArrayList<CodecWireEvent> events = new ArrayList<>();

    CodecWireTrace() {
    }

    CodecWireTrace(List<CodecWireEvent> source) {
        events.addAll(source);
    }
}

record CursorAdvanceEvidence(long byteCount, Varnode sourcePointer) {
}

record StaticF32Evidence(int bits, Address address) {
    float value() {
        return Float.intBitsToFloat(bits);
    }
}

record PackedPositionBounds(
        StaticF32Evidence minimum,
        StaticF32Evidence maximum,
        Address constructorCallsite) {

    boolean isValid() {
        return minimum != null && maximum != null &&
            Float.isFinite(minimum.value()) && Float.isFinite(maximum.value()) &&
            minimum.value() < maximum.value();
    }

    String wireShape() {
        return "packed-position<0x%08x,0x%08x>".formatted(
            minimum.bits(),
            maximum.bits());
    }

    boolean sameRange(PackedPositionBounds other) {
        return other != null &&
            minimum.bits() == other.minimum.bits() &&
            maximum.bits() == other.maximum.bits();
    }
}

record LoopExecutionProof(long maxSteps, int loopCount) {
}

record ProvenLoopRange(Address target, Address branch, long iterations) {
    boolean contains(ProvenLoopRange other) {
        return other != null && target.compareTo(other.target) <= 0 &&
            branch.compareTo(other.branch) >= 0 &&
            (!target.equals(other.target) || !branch.equals(other.branch));
    }
}

record PcodeStorageWrite(PcodeOpAST operation, Varnode value) {
}

record PcodeIdentityLiteral(String typeId, Address address, String source) {
}

record DeclaredCodecShape(String shape) {
}
