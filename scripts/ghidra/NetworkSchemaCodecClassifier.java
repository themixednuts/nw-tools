import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeBlock;
import ghidra.program.model.pcode.PcodeBlockBasic;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.PcodeOpAST;
import ghidra.program.model.pcode.Varnode;
import ghidra.program.model.pcode.VarnodeAST;

/** Structural codec recognizers derived from p-code SSA and control flow. */
final class NetworkSchemaCodecClassifier {
    enum CodecDirection {
        ENCODE,
        DECODE
    }

    record VariableIntegerCodec(String shape, CodecDirection direction) {}

    record SmallestThreeCoreEvidence(
        boolean hasFlagMasks,
        boolean hasLoopBound,
        long squaredComponents,
        long floatAdds,
        long zeroComparisons,
        boolean hasSimdNormalization,
        boolean hasClamp255,
        long integerToFloatConversions,
        long floatSubtracts,
        long floatSquareRoots,
        CodecDirection direction) {}

    private static final List<Long> U32_ENCODER_THRESHOLDS = List.of(
        0x80L,
        0x4000L,
        0x20_0000L,
        0x1000_0000L);
    private static final List<Long> U32_DECODER_THRESHOLDS = List.of(
        0x80L,
        0xc0L,
        0xe0L,
        0xf0L);
    private static final List<Long> U64_ENCODER_THRESHOLDS = List.of(
        0x80L,
        0x4000L,
        0x20_0000L,
        0x1000_0000L,
        0x8_0000_0000L,
        0x400_0000_0000L,
        0x2_0000_0000_0000L,
        0x100_0000_0000_0000L);
    private static final List<Long> U64_DECODER_THRESHOLDS = List.of(
        0x80L,
        0xc0L,
        0xe0L,
        0xf0L,
        0xf8L,
        0xfcL,
        0xfeL,
        0xffL);

    private NetworkSchemaCodecClassifier() {}

    static String variableIntegerShape(HighFunction high) {
        VariableIntegerCodec codec = variableIntegerCodec(high);
        return codec == null ? null : codec.shape();
    }

    static VariableIntegerCodec variableIntegerCodec(HighFunction high) {
        if (high == null) {
            return null;
        }
        Map<String, List<BranchComparison>> byLineage = branchComparisons(high);
        for (List<BranchComparison> comparisons : byLineage.values()) {
            if (matchesDecisionChain(comparisons, U64_ENCODER_THRESHOLDS)) {
                return new VariableIntegerCodec("vlq-u64", CodecDirection.ENCODE);
            }
            if (matchesDecisionChain(comparisons, U64_DECODER_THRESHOLDS)) {
                return new VariableIntegerCodec("vlq-u64", CodecDirection.DECODE);
            }
        }
        for (List<BranchComparison> comparisons : byLineage.values()) {
            if (matchesDecisionChain(comparisons, U32_ENCODER_THRESHOLDS)) {
                return new VariableIntegerCodec("vlq-u32", CodecDirection.ENCODE);
            }
            if (matchesDecisionChain(comparisons, U32_DECODER_THRESHOLDS)) {
                return new VariableIntegerCodec("vlq-u32", CodecDirection.DECODE);
            }
        }
        return null;
    }

    /**
     * Recognizes the IEEE binary16-to-binary32 expansion graph.
     *
     * <p>The proof is independent of symbols: a two-byte loaded value must flow
     * through byte-order normalization, split into sign/exponent/mantissa with
     * the binary16 masks, branch on the all-ones exponent, and produce a
     * four-byte result including the canonical NaN case.</p>
     */
    static boolean isBinary16Decode(HighFunction high) {
        if (high == null) {
            return false;
        }
        List<PcodeOpAST> operations = NetworkSchemaControlFlow.orderedOperations(high);
        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() != PcodeOp.CALL ||
                operation.getOutput() == null ||
                operation.getOutput().getSize() != Short.BYTES ||
                operation.getNumInputs() != 2 ||
                !dependsOnSizedLoad(operation.getInput(1), Short.BYTES, new HashSet<>())) {
                continue;
            }

            Varnode decoded = operation.getOutput();
            Map<Long, Varnode> masks = maskOutputs(operations, decoded);
            Varnode exponent = masks.get(0x7c00L);
            if (!masks.keySet().containsAll(Set.of(0x7fffL, 0x8000L, 0x3ffL, 0x7c00L)) ||
                exponent == null ||
                !hasEqualityComparison(operations, exponent, 0x7c00L) ||
                !hasDerivedShift(operations, decoded, 13L) ||
                !hasDerivedShift(operations, decoded, 16L) ||
                !hasDerivedStore(operations, decoded, Integer.BYTES) ||
                !hasConstantStore(operations, 0xffc0_0000L, Integer.BYTES)) {
                continue;
            }
            return true;
        }
        return false;
    }

    static boolean hasLengthPrefixedByteCopyLoop(HighFunction high) {
        if (high == null) {
            return false;
        }
        List<PcodeOpAST> operations = NetworkSchemaControlFlow.orderedOperations(high);
        for (NetworkSchemaNaturalLoop loop : NetworkSchemaControlFlow.naturalLoops(high)) {
            int pointerSteps = 0;
            boolean byteLoad = false;
            boolean byteStore = false;
            for (PcodeOpAST operation : operations) {
                if (!loop.contains(operation)) {
                    continue;
                }
                if (isUnitPointerStep(operation)) {
                    pointerSteps++;
                }
                if (operation.getOpcode() == PcodeOp.LOAD &&
                    operation.getOutput() != null && operation.getOutput().getSize() == 1) {
                    byteLoad = true;
                }
                if (operation.getOpcode() == PcodeOp.STORE &&
                    operation.getNumInputs() >= 3 && operation.getInput(2).getSize() == 1) {
                    byteStore = true;
                }
            }
            if (pointerSteps >= 2 && byteLoad && byteStore) {
                return true;
            }
        }
        return false;
    }

    static CodecDirection smallestThreeCoreDirection(HighFunction high) {
        SmallestThreeCoreEvidence evidence = smallestThreeCoreEvidence(high);
        return evidence == null ? null : evidence.direction();
    }

    static SmallestThreeCoreEvidence smallestThreeCoreEvidence(HighFunction high) {
        if (high == null) {
            return null;
        }
        List<PcodeOpAST> operations = NetworkSchemaControlFlow.orderedOperations(high);
        boolean flagMasks = hasFlagMasks(operations, Set.of(2L, 4L, 8L, 0x10L));
        boolean loopBound = hasLoopBound(operations, 3L) || hasLoopBound(operations, 4L);

        long squaredComponents = operations.stream()
            .filter(operation -> operation.getOpcode() == PcodeOp.FLOAT_MULT)
            .filter(operation -> operation.getNumInputs() == 2)
            .filter(operation -> sameVarnode(operation.getInput(0), operation.getInput(1)))
            .count();
        long floatAdds = countOpcode(operations, PcodeOp.FLOAT_ADD);
        long zeroComparisons = operations.stream()
            .filter(operation -> operation.getOpcode() == PcodeOp.FLOAT_EQUAL)
            .filter(NetworkSchemaCodecClassifier::hasZeroConstantOperand)
            .filter(operation -> operation.getOutput() != null)
            .filter(operation -> feedsConditionalBranch(operation.getOutput()))
            .count();
        boolean simdNormalization = operations.stream().anyMatch(operation ->
            operation.getOpcode() == PcodeOp.CALLOTHER &&
                operation.getOutput() != null && operation.getOutput().getSize() == 16);
        boolean clamp255 = hasClamp255(operations);
        CodecDirection direction = null;
        if (flagMasks && loopBound && squaredComponents >= 4 && floatAdds >= 3 &&
            zeroComparisons >= 4 && simdNormalization && clamp255) {
            direction = CodecDirection.ENCODE;
        }

        long integerToFloatConversions = countOpcode(operations, PcodeOp.FLOAT_INT2FLOAT);
        long floatSubtracts = countOpcode(operations, PcodeOp.FLOAT_SUB);
        long floatSquareRoots = countOpcode(operations, PcodeOp.FLOAT_SQRT);
        if (direction == null && flagMasks && loopBound && squaredComponents >= 1 &&
            floatAdds >= 1 && integerToFloatConversions >= 1 &&
            floatSubtracts >= 1 && floatSquareRoots >= 1) {
            direction = CodecDirection.DECODE;
        }
        return new SmallestThreeCoreEvidence(
            flagMasks,
            loopBound,
            squaredComponents,
            floatAdds,
            zeroComparisons,
            simdNormalization,
            clamp255,
            integerToFloatConversions,
            floatSubtracts,
            floatSquareRoots,
            direction);
    }

    static boolean hasConditionalBitMask(HighFunction high, long mask) {
        if (high == null) {
            return false;
        }
        for (PcodeOpAST operation : NetworkSchemaControlFlow.orderedOperations(high)) {
            if (operation.getOpcode() != PcodeOp.INT_AND ||
                operation.getOutput() == null ||
                !hasConstantOperand(operation, mask) ||
                !feedsConditionalBranch(operation.getOutput())) {
                continue;
            }
            return true;
        }
        return false;
    }

    static boolean isProjectedVec3SmallestThreeEncodeWrapper(HighFunction high) {
        if (high == null) {
            return false;
        }
        for (PcodeOpAST piece : NetworkSchemaControlFlow.orderedOperations(high)) {
            if (piece.getOpcode() != PcodeOp.PIECE || piece.getOutput() == null ||
                piece.getOutput().getSize() != 16 || piece.getNumInputs() != 2) {
                continue;
            }
            for (int zeroSlot = 0; zeroSlot < 2; zeroSlot++) {
                Varnode zero = piece.getInput(zeroSlot);
                Varnode xyz = piece.getInput(1 - zeroSlot);
                if (zero.getSize() != 4 || !NetworkSchemaIntegerEvaluator.equals(zero, 0L) ||
                    xyz.getSize() != 12 || !dependsOnSizedLoad(xyz, 16, new HashSet<>())) {
                    continue;
                }
                return true;
            }
        }
        return false;
    }

    static boolean isQuaternionSmallestThreeEncodeWrapper(HighFunction high) {
        if (high == null || isProjectedVec3SmallestThreeEncodeWrapper(high)) {
            return false;
        }
        long componentLoads = NetworkSchemaControlFlow.orderedOperations(high).stream()
            .filter(operation -> operation.getOpcode() == PcodeOp.LOAD)
            .filter(operation -> operation.getOutput() != null)
            .filter(operation -> operation.getOutput().getSize() == Float.BYTES)
            .count();
        return componentLoads >= 4;
    }

    static boolean isProjectedVec3SmallestThreeDecodeWrapper(HighFunction high) {
        if (high == null) {
            return false;
        }
        List<PcodeOpAST> operations = NetworkSchemaControlFlow.orderedOperations(high);
        long componentStores = operations.stream()
            .filter(operation -> operation.getOpcode() == PcodeOp.STORE)
            .filter(operation -> operation.getNumInputs() >= 3)
            .filter(operation -> operation.getInput(2).getSize() == Float.BYTES)
            .count();
        long projectedComponents = operations.stream()
            .filter(operation -> operation.getOpcode() == PcodeOp.SUBPIECE)
            .filter(operation -> operation.getOutput() != null)
            .filter(operation -> operation.getOutput().getSize() == Float.BYTES)
            .filter(operation -> operation.getNumInputs() > 0)
            .filter(operation -> operation.getInput(0).getSize() >= 8)
            .count();
        return componentStores >= 4 && projectedComponents >= 3;
    }

    static boolean isQuaternionSmallestThreeDecodeWrapper(HighFunction high) {
        if (high == null || isProjectedVec3SmallestThreeDecodeWrapper(high)) {
            return false;
        }
        long vectorStores = NetworkSchemaControlFlow.orderedOperations(high).stream()
            .filter(operation -> operation.getOpcode() == PcodeOp.STORE)
            .filter(operation -> operation.getNumInputs() >= 3)
            .filter(operation -> operation.getInput(2).getSize() == Long.BYTES)
            .count();
        return vectorStores >= 2;
    }

    private static boolean hasFlagMasks(List<PcodeOpAST> operations, Set<Long> masks) {
        HashSet<Long> observed = new HashSet<>();
        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() != PcodeOp.INT_AND &&
                operation.getOpcode() != PcodeOp.INT_OR) {
                continue;
            }
            for (long mask : masks) {
                if (hasConstantOperand(operation, mask)) {
                    observed.add(mask);
                }
            }
        }
        return observed.containsAll(masks);
    }

    private static boolean hasLoopBound(List<PcodeOpAST> operations, long bound) {
        return operations.stream().anyMatch(operation ->
            (operation.getOpcode() == PcodeOp.INT_LESS ||
                operation.getOpcode() == PcodeOp.INT_LESSEQUAL) &&
                operation.getOutput() != null &&
                hasConstantOperand(operation, bound) &&
                feedsConditionalBranch(operation.getOutput()));
    }

    private static boolean hasClamp255(List<PcodeOpAST> operations) {
        boolean guarded = operations.stream().anyMatch(operation ->
            (operation.getOpcode() == PcodeOp.INT_SLESS ||
                operation.getOpcode() == PcodeOp.INT_SLESSEQUAL) &&
                operation.getOutput() != null &&
                hasConstantOperand(operation, 0xffL) &&
                feedsConditionalBranch(operation.getOutput()));
        boolean assigned = operations.stream().anyMatch(operation ->
            operation.getOpcode() == PcodeOp.COPY && hasConstantOperand(operation, 0xffL));
        return guarded && assigned;
    }

    private static long countOpcode(List<PcodeOpAST> operations, int opcode) {
        return operations.stream().filter(operation -> operation.getOpcode() == opcode).count();
    }

    private static boolean hasZeroConstantOperand(PcodeOp operation) {
        return hasConstantOperand(operation, 0L);
    }

    private static boolean hasConstantOperand(PcodeOp operation, long value) {
        if (operation == null) {
            return false;
        }
        for (int index = 0; index < operation.getNumInputs(); index++) {
            Varnode input = operation.getInput(index);
            if (input.isConstant() && normalizedConstant(input, input.getSize()) == value) {
                return true;
            }
        }
        return false;
    }

    private static boolean isUnitPointerStep(PcodeOp operation) {
        if (operation == null || operation.getOutput() == null ||
            operation.getOutput().getSize() < 4) {
            return false;
        }
        if (operation.getOpcode() == PcodeOp.INT_ADD && operation.getNumInputs() == 2) {
            return isConstantOne(operation.getInput(0)) ^
                isConstantOne(operation.getInput(1));
        }
        return operation.getOpcode() == PcodeOp.PTRADD &&
            operation.getNumInputs() == 3 &&
            isConstantOne(operation.getInput(1)) &&
            isConstantOne(operation.getInput(2));
    }

    private static Map<Long, Varnode> maskOutputs(
        List<PcodeOpAST> operations,
        Varnode source) {

        HashMap<Long, Varnode> result = new HashMap<>();
        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() != PcodeOp.INT_AND ||
                operation.getOutput() == null ||
                operation.getNumInputs() != 2) {
                continue;
            }
            int constantSlot = operation.getInput(0).isConstant() ? 0 :
                operation.getInput(1).isConstant() ? 1 : -1;
            if (constantSlot < 0 ||
                !dependsOn(operation.getInput(1 - constantSlot), source, new HashSet<>())) {
                continue;
            }
            long mask = normalizedConstant(
                operation.getInput(constantSlot),
                operation.getInput(1 - constantSlot).getSize());
            result.putIfAbsent(mask, operation.getOutput());
        }
        return result;
    }

    private static boolean hasEqualityComparison(
        List<PcodeOpAST> operations,
        Varnode source,
        long expected) {

        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() != PcodeOp.INT_EQUAL || operation.getNumInputs() != 2) {
                continue;
            }
            int constantSlot = operation.getInput(0).isConstant() ? 0 :
                operation.getInput(1).isConstant() ? 1 : -1;
            if (constantSlot >= 0 &&
                normalizedConstant(
                    operation.getInput(constantSlot),
                    operation.getInput(1 - constantSlot).getSize()) == expected &&
                dependsOn(operation.getInput(1 - constantSlot), source, new HashSet<>()) &&
                operation.getOutput() != null &&
                feedsConditionalBranch(operation.getOutput())) {
                return true;
            }
        }
        return false;
    }

    private static boolean hasDerivedShift(
        List<PcodeOpAST> operations,
        Varnode source,
        long shift) {

        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() != PcodeOp.INT_LEFT || operation.getNumInputs() != 2 ||
                !operation.getInput(1).isConstant()) {
                continue;
            }
            if (normalizedConstant(operation.getInput(1), operation.getInput(1).getSize()) == shift &&
                dependsOn(operation.getInput(0), source, new HashSet<>())) {
                return true;
            }
        }
        return false;
    }

    private static boolean hasDerivedStore(
        List<PcodeOpAST> operations,
        Varnode source,
        int width) {

        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() == PcodeOp.STORE && operation.getNumInputs() >= 3 &&
                operation.getInput(2).getSize() == width &&
                dependsOn(operation.getInput(2), source, new HashSet<>())) {
                return true;
            }
        }
        return false;
    }

    private static boolean hasConstantStore(
        List<PcodeOpAST> operations,
        long value,
        int width) {

        for (PcodeOpAST operation : operations) {
            if (operation.getOpcode() != PcodeOp.STORE || operation.getNumInputs() < 3) {
                continue;
            }
            Varnode stored = operation.getInput(2);
            if (stored.isConstant() && stored.getSize() == width &&
                normalizedConstant(stored, width) == value) {
                return true;
            }
        }
        return false;
    }

    private static boolean dependsOnSizedLoad(
        Varnode node,
        int width,
        Set<String> seen) {

        if (node == null || !seen.add(varnodeKey(node))) {
            return false;
        }
        PcodeOp definition = node.getDef();
        if (definition == null) {
            return false;
        }
        if (definition.getOpcode() == PcodeOp.LOAD && node.getSize() == width) {
            return true;
        }
        if (!isValueForwarder(definition)) {
            return false;
        }
        for (int slot = 0; slot < definition.getNumInputs(); slot++) {
            if (dependsOnSizedLoad(definition.getInput(slot), width, seen)) {
                return true;
            }
        }
        return false;
    }

    private static boolean dependsOn(
        Varnode candidate,
        Varnode source,
        Set<String> seen) {

        if (candidate == null || source == null) {
            return false;
        }
        if (sameVarnode(candidate, source)) {
            return true;
        }
        if (candidate.isConstant() || !seen.add(varnodeKey(candidate))) {
            return false;
        }
        PcodeOp definition = candidate.getDef();
        if (definition == null || definition.getOpcode() == PcodeOp.LOAD ||
            definition.getOpcode() == PcodeOp.CALL ||
            definition.getOpcode() == PcodeOp.CALLIND) {
            return false;
        }
        for (int slot = 0; slot < definition.getNumInputs(); slot++) {
            if (dependsOn(definition.getInput(slot), source, seen)) {
                return true;
            }
        }
        return false;
    }

    private static boolean sameVarnode(Varnode left, Varnode right) {
        return left == right || left != null && right != null &&
            varnodeKey(left).equals(varnodeKey(right));
    }

    private static boolean isConstantOne(Varnode node) {
        return node != null && node.isConstant() && node.getOffset() == 1L;
    }

    private static Map<String, List<BranchComparison>> branchComparisons(HighFunction high) {
        LinkedHashMap<String, List<BranchComparison>> result = new LinkedHashMap<>();
        Iterator<PcodeOpAST> operations = high.getPcodeOps();
        while (operations.hasNext()) {
            PcodeOpAST operation = operations.next();
            if (!isOrderedComparison(operation) || operation.getNumInputs() != 2 ||
                operation.getOutput() == null ||
                !feedsConditionalBranch(operation.getOutput())) {
                continue;
            }
            int constantIndex = operation.getInput(0).isConstant() ? 0 :
                operation.getInput(1).isConstant() ? 1 : -1;
            if (constantIndex < 0) {
                continue;
            }
            Varnode value = operation.getInput(1 - constantIndex);
            String lineage = lineageKey(value, new HashMap<>(), new HashSet<>(), 0);
            PcodeBlockBasic block = operation.getParent();
            if (lineage == null || block == null) {
                continue;
            }
            long threshold = canonicalThreshold(operation, constantIndex, value.getSize());
            result.computeIfAbsent(lineage, ignored -> new ArrayList<>())
                .add(new BranchComparison(threshold, block));
        }
        return result;
    }

    private static boolean isOrderedComparison(PcodeOp operation) {
        return switch (operation.getOpcode()) {
            case PcodeOp.INT_LESS, PcodeOp.INT_LESSEQUAL,
                PcodeOp.INT_SLESS, PcodeOp.INT_SLESSEQUAL -> true;
            default -> false;
        };
    }

    private static boolean feedsConditionalBranch(Varnode output) {
        ArrayDeque<Varnode> pending = new ArrayDeque<>();
        HashSet<String> seen = new HashSet<>();
        pending.add(output);
        while (!pending.isEmpty() && seen.size() < 32) {
            Varnode current = pending.removeFirst();
            if (!seen.add(varnodeKey(current))) {
                continue;
            }
            Iterator<PcodeOp> descendants = current.getDescendants();
            while (descendants.hasNext()) {
                PcodeOp descendant = descendants.next();
                if (descendant.getOpcode() == PcodeOp.CBRANCH) {
                    return true;
                }
                if (isBooleanForwarder(descendant) && descendant.getOutput() != null) {
                    pending.addLast(descendant.getOutput());
                }
            }
        }
        return false;
    }

    private static boolean isBooleanForwarder(PcodeOp operation) {
        return switch (operation.getOpcode()) {
            case PcodeOp.COPY, PcodeOp.CAST, PcodeOp.BOOL_NEGATE,
                PcodeOp.INT_EQUAL, PcodeOp.INT_NOTEQUAL -> true;
            default -> false;
        };
    }

    private static String lineageKey(
        Varnode node,
        Map<String, String> memo,
        Set<String> active,
        int depth) {

        if (node == null || node.isConstant() || depth > 32) {
            return null;
        }
        String nodeKey = varnodeKey(node);
        if (memo.containsKey(nodeKey)) {
            return memo.get(nodeKey);
        }
        if (!active.add(nodeKey)) {
            return null;
        }
        String result;
        PcodeOp definition = node.getDef();
        if (definition != null && isValueForwarder(definition) &&
            definition.getNumInputs() > 0) {
            result = lineageKey(definition.getInput(0), memo, active, depth + 1);
        }
        else if (definition != null && definition.getOpcode() == PcodeOp.MULTIEQUAL) {
            HashSet<String> inputs = new HashSet<>();
            for (int index = 0; index < definition.getNumInputs(); index++) {
                String input = lineageKey(
                    definition.getInput(index),
                    memo,
                    active,
                    depth + 1);
                if (input != null) {
                    inputs.add(input);
                }
            }
            result = inputs.size() == 1 ? inputs.iterator().next() : nodeKey;
        }
        else {
            result = nodeKey;
        }
        active.remove(nodeKey);
        memo.put(nodeKey, result);
        return result;
    }

    private static boolean isValueForwarder(PcodeOp operation) {
        return switch (operation.getOpcode()) {
            case PcodeOp.COPY, PcodeOp.CAST, PcodeOp.INT_ZEXT,
                PcodeOp.INT_SEXT, PcodeOp.SUBPIECE -> true;
            default -> false;
        };
    }

    private static boolean matchesDecisionChain(
        List<BranchComparison> comparisons,
        List<Long> thresholds) {

        Map<Long, PcodeBlockBasic> byThreshold = new HashMap<>();
        for (BranchComparison comparison : comparisons) {
            if (thresholds.contains(comparison.threshold()) &&
                byThreshold.putIfAbsent(comparison.threshold(), comparison.block()) != null) {
                return false;
            }
        }
        if (!byThreshold.keySet().containsAll(thresholds)) {
            return false;
        }
        Set<PcodeBlockBasic> decisionBlocks = new HashSet<>(byThreshold.values());
        for (int index = 0; index + 1 < thresholds.size(); index++) {
            PcodeBlockBasic current = byThreshold.get(thresholds.get(index));
            PcodeBlockBasic next = byThreshold.get(thresholds.get(index + 1));
            if (!reachesNextDecision(current, next, decisionBlocks)) {
                return false;
            }
        }
        return true;
    }

    private static boolean reachesNextDecision(
        PcodeBlockBasic start,
        PcodeBlockBasic target,
        Set<PcodeBlockBasic> decisionBlocks) {

        ArrayDeque<PcodeBlock> pending = new ArrayDeque<>();
        HashSet<PcodeBlock> seen = new HashSet<>();
        for (int index = 0; index < start.getOutSize(); index++) {
            pending.addLast(start.getOut(index));
        }
        while (!pending.isEmpty() && seen.size() < 128) {
            PcodeBlock block = pending.removeFirst();
            if (!seen.add(block)) {
                continue;
            }
            if (block == target) {
                return true;
            }
            if (block instanceof PcodeBlockBasic basic && decisionBlocks.contains(basic)) {
                continue;
            }
            for (int index = 0; index < block.getOutSize(); index++) {
                pending.addLast(block.getOut(index));
            }
        }
        return false;
    }

    private static long normalizedConstant(Varnode constant, int valueSize) {
        long value = constant.getOffset();
        if (valueSize <= 0 || valueSize >= Long.BYTES) {
            return value;
        }
        return value & ((1L << (valueSize * Byte.SIZE)) - 1L);
    }

    private static long canonicalThreshold(
        PcodeOp comparison,
        int constantIndex,
        int valueSize) {

        long constant = normalizedConstant(
            comparison.getInput(constantIndex),
            valueSize);
        boolean inclusive = comparison.getOpcode() == PcodeOp.INT_LESSEQUAL ||
            comparison.getOpcode() == PcodeOp.INT_SLESSEQUAL;
        boolean constantFirst = constantIndex == 0;
        return inclusive == constantFirst
            ? constant
            : incrementConstant(constant, valueSize);
    }

    private static long incrementConstant(long value, int valueSize) {
        long incremented = value + 1L;
        if (valueSize <= 0 || valueSize >= Long.BYTES) {
            return incremented;
        }
        return incremented & ((1L << (valueSize * Byte.SIZE)) - 1L);
    }

    private static String varnodeKey(Varnode node) {
        return node instanceof VarnodeAST ast
            ? "ssa:" + ast.getUniqueId()
            : node.getAddress() + ":" + node.getOffset() + ":" + node.getSize();
    }

    private record BranchComparison(long threshold, PcodeBlockBasic block) {}
}
