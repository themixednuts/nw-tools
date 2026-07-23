import java.util.HashSet;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.OptionalLong;
import java.util.Set;
import java.util.concurrent.atomic.LongAdder;

import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.Varnode;
import ghidra.program.model.pcode.VarnodeAST;

/** Exact integer constant propagation over p-code SSA values up to 64 bits. */
final class NetworkSchemaIntegerEvaluator {
    private static final int MAX_DEPTH = 128;
    private static final int MAX_UNIQUE_STATES = 8_192;
    private static final LongAdder QUERY_COUNT = new LongAdder();
    private static final LongAdder EXACT_CACHE_HIT_COUNT = new LongAdder();
    private static final LongAdder BIT_CACHE_HIT_COUNT = new LongAdder();
    private static final LongAdder BUDGET_EXHAUSTION_COUNT = new LongAdder();

    private NetworkSchemaIntegerEvaluator() {}

    static Long evaluate(Varnode node) {
        QUERY_COUNT.increment();
        return evaluate(node, new EvaluationContext(), 0);
    }

    static boolean equals(Varnode node, long expected) {
        Long value = evaluate(node);
        return value != null && value == truncate(expected, node.getSize());
    }

    static Long evaluateByte(Varnode node, int byteOffset) {
        if (node == null || byteOffset < 0 || byteOffset >= node.getSize()) {
            return null;
        }
        QUERY_COUNT.increment();
        return knownByte(knownBits(node, new EvaluationContext(), 0), byteOffset);
    }

    static Long evaluateOutputByte(PcodeOp operation, int byteOffset) {
        Varnode output = operation == null ? null : operation.getOutput();
        if (output == null || byteOffset < 0 || byteOffset >= output.getSize()) {
            return null;
        }
        QUERY_COUNT.increment();
        EvaluationContext context = new EvaluationContext();
        Long exact = evaluateDefinition(output, operation, context, 0);
        BitKnowledge knowledge = exact == null
            ? knownBitsDefinition(output, operation, context, 0)
            : BitKnowledge.full(output.getSize(), exact);
        return knownByte(knowledge, byteOffset);
    }

    private static Long knownByte(BitKnowledge knowledge, int byteOffset) {
        int shift = Math.multiplyExact(byteOffset, Byte.SIZE);
        long mask = 0xffL << shift;
        return knowledge == null || (knowledge.mask() & mask) != mask
            ? null
            : (knowledge.value() >>> shift) & 0xffL;
    }

    private static Long evaluate(Varnode node, EvaluationContext context, int depth) {
        if (node == null || node.getSize() <= 0 || node.getSize() > Long.BYTES ||
            depth > MAX_DEPTH) {
            return null;
        }
        if (node.isConstant()) {
            return truncate(node.getOffset(), node.getSize());
        }
        String key = key(node);
        OptionalLong cached = context.exact.get(key);
        if (cached != null) {
            EXACT_CACHE_HIT_COUNT.increment();
            return cached.isPresent() ? cached.getAsLong() : null;
        }
        if (!context.exactActive.add(key)) {
            return null;
        }
        if (!context.consume("exact:" + key)) {
            context.exactActive.remove(key);
            return null;
        }
        Long result;
        try {
            PcodeOp definition = node.getDef();
            if (definition == null) {
                result = null;
            }
            else {
                result = evaluateDefinition(node, definition, context, depth + 1);
            }
        }
        finally {
            context.exactActive.remove(key);
        }
        context.exact.put(
            key,
            result == null ? OptionalLong.empty() : OptionalLong.of(result));
        return result;
    }

    private static Long evaluateDefinition(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        return switch (definition.getOpcode()) {
            case PcodeOp.COPY, PcodeOp.CAST ->
                unary(output, definition, context, depth, value -> value);
            // INDIRECT is the value after an opaque call or memory effect. Its
            // first input is the pre-effect value, not an identity operand.
            case PcodeOp.INDIRECT -> null;
            case PcodeOp.INT_ZEXT -> unary(
                output,
                definition,
                context,
                depth,
                value -> truncate(value, definition.getInput(0).getSize()));
            case PcodeOp.INT_SEXT -> unary(
                output,
                definition,
                context,
                depth,
                value -> signed(value, definition.getInput(0).getSize()));
            case PcodeOp.INT_NEGATE ->
                unary(output, definition, context, depth, value -> ~value);
            case PcodeOp.INT_2COMP ->
                unary(output, definition, context, depth, value -> -value);
            case PcodeOp.BOOL_NEGATE ->
                unary(output, definition, context, depth, value -> value == 0 ? 1L : 0L);
            case PcodeOp.INT_ADD, PcodeOp.PTRSUB -> binary(
                output, definition, context, depth, (left, right) -> left + right);
            case PcodeOp.INT_SUB -> binary(
                output, definition, context, depth, (left, right) -> left - right);
            case PcodeOp.INT_MULT -> binary(
                output, definition, context, depth, (left, right) -> left * right);
            case PcodeOp.INT_AND -> binary(
                output, definition, context, depth, (left, right) -> left & right);
            case PcodeOp.INT_OR -> binary(
                output, definition, context, depth, (left, right) -> left | right);
            case PcodeOp.INT_XOR -> binary(
                output, definition, context, depth, (left, right) -> left ^ right);
            case PcodeOp.INT_LEFT -> shift(output, definition, context, depth, Shift.LEFT);
            case PcodeOp.INT_RIGHT -> shift(output, definition, context, depth, Shift.RIGHT);
            case PcodeOp.INT_SRIGHT -> shift(output, definition, context, depth, Shift.SIGNED_RIGHT);
            case PcodeOp.INT_DIV -> divide(output, definition, context, depth, false, false);
            case PcodeOp.INT_SDIV -> divide(output, definition, context, depth, true, false);
            case PcodeOp.INT_REM -> divide(output, definition, context, depth, false, true);
            case PcodeOp.INT_SREM -> divide(output, definition, context, depth, true, true);
            case PcodeOp.PTRADD -> pointerAdd(output, definition, context, depth);
            case PcodeOp.SUBPIECE -> subpiece(output, definition, context, depth);
            case PcodeOp.PIECE -> piece(output, definition, context, depth);
            case PcodeOp.MULTIEQUAL -> phi(output, definition, context, depth);
            case PcodeOp.INT_EQUAL -> comparison(
                output, definition, context, depth, (left, right) -> left == right);
            case PcodeOp.INT_NOTEQUAL -> comparison(
                output, definition, context, depth, (left, right) -> left != right);
            case PcodeOp.INT_LESS -> comparison(
                output, definition, context, depth,
                (left, right) -> Long.compareUnsigned(left, right) < 0);
            case PcodeOp.INT_LESSEQUAL -> comparison(
                output, definition, context, depth,
                (left, right) -> Long.compareUnsigned(left, right) <= 0);
            case PcodeOp.INT_SLESS -> signedComparison(
                output, definition, context, depth, (left, right) -> left < right);
            case PcodeOp.INT_SLESSEQUAL -> signedComparison(
                output, definition, context, depth, (left, right) -> left <= right);
            default -> null;
        };
    }

    private static Long unary(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth,
        Unary operation) {

        if (definition.getNumInputs() < 1) {
            return null;
        }
        Long value = evaluate(definition.getInput(0), context, depth);
        return value == null ? null : truncate(operation.apply(value), output.getSize());
    }

    private static Long binary(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth,
        Binary operation) {

        Long[] values = binaryInputs(definition, context, depth);
        return values == null
            ? null
            : truncate(operation.apply(values[0], values[1]), output.getSize());
    }

    private static Long shift(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth,
        Shift kind) {

        Long[] values = binaryInputs(definition, context, depth);
        if (values == null) {
            return null;
        }
        long bits = Math.multiplyExact(output.getSize(), Byte.SIZE);
        long amount = values[1];
        if (Long.compareUnsigned(amount, bits) >= 0) {
            return 0L;
        }
        int shift = Math.toIntExact(amount);
        long value = switch (kind) {
            case LEFT -> values[0] << shift;
            case RIGHT -> values[0] >>> shift;
            case SIGNED_RIGHT -> signed(values[0], definition.getInput(0).getSize()) >> shift;
        };
        return truncate(value, output.getSize());
    }

    private static Long divide(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth,
        boolean signed,
        boolean remainder) {

        Long[] values = binaryInputs(definition, context, depth);
        if (values == null || values[1] == 0) {
            return null;
        }
        long left = signed ? signed(values[0], definition.getInput(0).getSize()) : values[0];
        long right = signed ? signed(values[1], definition.getInput(1).getSize()) : values[1];
        long result;
        if (signed) {
            result = remainder ? left % right : left / right;
        } else {
            result = remainder
                ? Long.remainderUnsigned(left, right)
                : Long.divideUnsigned(left, right);
        }
        return truncate(result, output.getSize());
    }

    private static Long pointerAdd(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        if (definition.getNumInputs() != 3) {
            return null;
        }
        Long base = evaluate(definition.getInput(0), context, depth);
        Long index = evaluate(definition.getInput(1), context, depth);
        Long scale = evaluate(definition.getInput(2), context, depth);
        return base == null || index == null || scale == null
            ? null
            : truncate(base + index * scale, output.getSize());
    }

    private static Long subpiece(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        Long[] values = binaryInputs(definition, context, depth);
        if (values == null || values[1] < 0 || values[1] >= Long.BYTES) {
            return null;
        }
        int shift = Math.toIntExact(values[1] * Byte.SIZE);
        return truncate(values[0] >>> shift, output.getSize());
    }

    private static Long piece(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        Long[] values = binaryInputs(definition, context, depth);
        if (values == null || output.getSize() > Long.BYTES) {
            return null;
        }
        int lowBits = Math.multiplyExact(definition.getInput(1).getSize(), Byte.SIZE);
        if (lowBits >= Long.SIZE) {
            return values[0] == 0 ? truncate(values[1], output.getSize()) : null;
        }
        return truncate(values[0] << lowBits | values[1], output.getSize());
    }

    private static Long phi(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        Long selected = null;
        for (int index = 0; index < definition.getNumInputs(); index++) {
            Long value = evaluate(definition.getInput(index), context, depth);
            if (value == null || selected != null && !selected.equals(value)) {
                return null;
            }
            selected = value;
        }
        return selected == null ? null : truncate(selected, output.getSize());
    }

    private static Long comparison(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth,
        Comparison comparison) {

        Long[] values = binaryInputs(definition, context, depth);
        return values == null
            ? null
            : truncate(comparison.test(values[0], values[1]) ? 1L : 0L, output.getSize());
    }

    private static Long signedComparison(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth,
        Comparison comparison) {

        Long[] values = binaryInputs(definition, context, depth);
        if (values == null) {
            return null;
        }
        long left = signed(values[0], definition.getInput(0).getSize());
        long right = signed(values[1], definition.getInput(1).getSize());
        return truncate(comparison.test(left, right) ? 1L : 0L, output.getSize());
    }

    private static Long[] binaryInputs(
        PcodeOp definition,
        EvaluationContext context,
        int depth) {
        if (definition.getNumInputs() < 2) {
            return null;
        }
        Long left = evaluate(definition.getInput(0), context, depth);
        Long right = evaluate(definition.getInput(1), context, depth);
        return left == null || right == null ? null : new Long[] { left, right };
    }

    private static BitKnowledge knownBits(
        Varnode node,
        EvaluationContext context,
        int depth) {

        if (node == null || node.getSize() <= 0 || node.getSize() > Long.BYTES ||
            depth > MAX_DEPTH) {
            return null;
        }
        if (node.isConstant()) {
            return BitKnowledge.full(node.getSize(), node.getOffset());
        }
        String key = key(node);
        BitKnowledge cached = context.bits.get(key);
        if (cached != null) {
            BIT_CACHE_HIT_COUNT.increment();
            return cached;
        }
        if (!context.bitsActive.add(key)) {
            return null;
        }
        if (!context.consume("bits:" + key)) {
            context.bitsActive.remove(key);
            return BitKnowledge.unknown();
        }
        BitKnowledge result;
        try {
            PcodeOp definition = node.getDef();
            if (definition == null) {
                result = BitKnowledge.unknown();
            }
            else {
                BitKnowledge exact = exactKnowledge(node, context, depth);
                result = exact != null
                    ? exact
                    : knownBitsDefinition(node, definition, context, depth + 1);
            }
        }
        finally {
            context.bitsActive.remove(key);
        }
        if (result == null) {
            result = BitKnowledge.unknown();
        }
        context.bits.put(key, result);
        return result;
    }

    private static BitKnowledge knownBitsDefinition(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        return switch (definition.getOpcode()) {
            case PcodeOp.COPY, PcodeOp.CAST ->
                resizedKnowledge(
                    knownBits(definition.getInput(0), context, depth),
                    output.getSize());
            case PcodeOp.INDIRECT -> BitKnowledge.unknown();
            case PcodeOp.INT_ZEXT -> zeroExtendedKnowledge(
                knownBits(definition.getInput(0), context, depth),
                definition.getInput(0).getSize(),
                output.getSize());
            case PcodeOp.INT_SEXT -> signExtendedKnowledge(
                knownBits(definition.getInput(0), context, depth),
                definition.getInput(0).getSize(),
                output.getSize());
            case PcodeOp.INT_AND, PcodeOp.INT_OR, PcodeOp.INT_XOR ->
                bitwiseKnowledge(output, definition, context, depth);
            case PcodeOp.INT_NEGATE -> complementedKnowledge(
                knownBits(definition.getInput(0), context, depth),
                output.getSize());
            case PcodeOp.SUBPIECE -> subpieceKnowledge(
                output,
                definition,
                context,
                depth);
            case PcodeOp.PIECE -> pieceKnowledge(output, definition, context, depth);
            case PcodeOp.MULTIEQUAL -> mergedKnowledge(output, definition, context, depth);
            default -> BitKnowledge.unknown();
        };
    }

    private static BitKnowledge exactKnowledge(
        Varnode node,
        EvaluationContext context,
        int depth) {

        Long value = evaluate(node, context, depth);
        return value == null ? null : BitKnowledge.full(node.getSize(), value);
    }

    private static BitKnowledge bitwiseKnowledge(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        if (definition.getNumInputs() != 2) {
            return BitKnowledge.unknown();
        }
        if (definition.getOpcode() == PcodeOp.INT_XOR &&
            key(definition.getInput(0)).equals(key(definition.getInput(1)))) {
            return BitKnowledge.full(output.getSize(), 0L);
        }
        BitKnowledge left = resizedKnowledge(
            knownBits(definition.getInput(0), context, depth),
            output.getSize());
        BitKnowledge right = resizedKnowledge(
            knownBits(definition.getInput(1), context, depth),
            output.getSize());
        long widthMask = widthMask(output.getSize());
        long leftOne = left.mask() & left.value();
        long rightOne = right.mask() & right.value();
        long leftZero = left.mask() & ~left.value();
        long rightZero = right.mask() & ~right.value();
        return switch (definition.getOpcode()) {
            case PcodeOp.INT_AND -> BitKnowledge.fromOneZero(
                leftOne & rightOne,
                leftZero | rightZero,
                widthMask);
            case PcodeOp.INT_OR -> BitKnowledge.fromOneZero(
                leftOne | rightOne,
                leftZero & rightZero,
                widthMask);
            case PcodeOp.INT_XOR -> {
                long known = left.mask() & right.mask() & widthMask;
                yield new BitKnowledge(known, (left.value() ^ right.value()) & known);
            }
            default -> BitKnowledge.unknown();
        };
    }

    private static BitKnowledge zeroExtendedKnowledge(
        BitKnowledge input,
        int inputSize,
        int outputSize) {

        long inputMask = widthMask(inputSize);
        long outputMask = widthMask(outputSize);
        return new BitKnowledge(
            (input.mask() & inputMask) | (outputMask & ~inputMask),
            input.value() & inputMask);
    }

    private static BitKnowledge signExtendedKnowledge(
        BitKnowledge input,
        int inputSize,
        int outputSize) {

        long inputMask = widthMask(inputSize);
        long outputMask = widthMask(outputSize);
        long signBit = 1L << (Math.multiplyExact(inputSize, Byte.SIZE) - 1);
        long known = input.mask() & inputMask;
        long value = input.value() & known;
        if ((known & signBit) != 0) {
            long extension = outputMask & ~inputMask;
            known |= extension;
            if ((value & signBit) != 0) {
                value |= extension;
            }
        }
        return new BitKnowledge(known, value);
    }

    private static BitKnowledge complementedKnowledge(BitKnowledge input, int outputSize) {
        long mask = input.mask() & widthMask(outputSize);
        return new BitKnowledge(mask, ~input.value() & mask);
    }

    private static BitKnowledge subpieceKnowledge(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        Long byteOffset = definition.getNumInputs() == 2
            ? evaluate(definition.getInput(1))
            : null;
        if (byteOffset == null || byteOffset < 0 || byteOffset >= Long.BYTES) {
            return BitKnowledge.unknown();
        }
        BitKnowledge input = knownBits(definition.getInput(0), context, depth);
        int shift = Math.toIntExact(byteOffset * Byte.SIZE);
        long outputMask = widthMask(output.getSize());
        return new BitKnowledge(
            input.mask() >>> shift & outputMask,
            input.value() >>> shift & outputMask);
    }

    private static BitKnowledge pieceKnowledge(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        if (definition.getNumInputs() != 2) {
            return BitKnowledge.unknown();
        }
        BitKnowledge high = knownBits(definition.getInput(0), context, depth);
        BitKnowledge low = knownBits(definition.getInput(1), context, depth);
        int shift = Math.multiplyExact(definition.getInput(1).getSize(), Byte.SIZE);
        long outputMask = widthMask(output.getSize());
        return new BitKnowledge(
            (high.mask() << shift | low.mask()) & outputMask,
            (high.value() << shift | low.value()) & outputMask);
    }

    private static BitKnowledge mergedKnowledge(
        Varnode output,
        PcodeOp definition,
        EvaluationContext context,
        int depth) {

        BitKnowledge merged = null;
        for (int index = 0; index < definition.getNumInputs(); index++) {
            BitKnowledge candidate = resizedKnowledge(
                knownBits(definition.getInput(index), context, depth),
                output.getSize());
            merged = merged == null ? candidate : merged.intersection(candidate);
        }
        return merged == null ? BitKnowledge.unknown() : merged;
    }

    private static BitKnowledge resizedKnowledge(BitKnowledge input, int outputSize) {
        if (input == null) {
            return BitKnowledge.unknown();
        }
        long mask = widthMask(outputSize);
        return new BitKnowledge(input.mask() & mask, input.value() & mask);
    }

    private static long widthMask(int byteWidth) {
        return byteWidth >= Long.BYTES
            ? -1L
            : (1L << Math.multiplyExact(byteWidth, Byte.SIZE)) - 1L;
    }

    private static long truncate(long value, int byteWidth) {
        if (byteWidth >= Long.BYTES) {
            return value;
        }
        return value & ((1L << Math.multiplyExact(byteWidth, Byte.SIZE)) - 1L);
    }

    private static long signed(long value, int byteWidth) {
        int shift = Long.SIZE - Math.multiplyExact(byteWidth, Byte.SIZE);
        return shift == 0 ? value : value << shift >> shift;
    }

    private static String key(Varnode node) {
        return node instanceof VarnodeAST ast
            ? "ssa:" + ast.getUniqueId()
            : node.getAddress() + ":" + node.getOffset() + ":" + node.getSize();
    }

    static void resetMetrics() {
        QUERY_COUNT.reset();
        EXACT_CACHE_HIT_COUNT.reset();
        BIT_CACHE_HIT_COUNT.reset();
        BUDGET_EXHAUSTION_COUNT.reset();
    }

    static long queryCount() {
        return QUERY_COUNT.sum();
    }

    static long exactCacheHitCount() {
        return EXACT_CACHE_HIT_COUNT.sum();
    }

    static long bitCacheHitCount() {
        return BIT_CACHE_HIT_COUNT.sum();
    }

    static long budgetExhaustionCount() {
        return BUDGET_EXHAUSTION_COUNT.sum();
    }

    private static final class EvaluationContext {
        final Map<String, OptionalLong> exact = new HashMap<>();
        final Map<String, BitKnowledge> bits = new HashMap<>();
        final Set<String> exactActive = new HashSet<>();
        final Set<String> bitsActive = new HashSet<>();
        final Set<String> visitedStates = new HashSet<>();
        boolean budgetExhausted;

        boolean consume(String state) {
            if (visitedStates.contains(state)) {
                return true;
            }
            if (visitedStates.size() >= MAX_UNIQUE_STATES) {
                if (!budgetExhausted) {
                    budgetExhausted = true;
                    BUDGET_EXHAUSTION_COUNT.increment();
                }
                return false;
            }
            visitedStates.add(state);
            return true;
        }
    }

    private enum Shift {
        LEFT,
        RIGHT,
        SIGNED_RIGHT
    }

    @FunctionalInterface
    private interface Unary {
        long apply(long value);
    }

    @FunctionalInterface
    private interface Binary {
        long apply(long left, long right);
    }

    @FunctionalInterface
    private interface Comparison {
        boolean test(long left, long right);
    }

    private record BitKnowledge(long mask, long value) {
        static BitKnowledge unknown() {
            return new BitKnowledge(0L, 0L);
        }

        static BitKnowledge full(int byteWidth, long value) {
            long mask = widthMask(byteWidth);
            return new BitKnowledge(mask, value & mask);
        }

        static BitKnowledge fromOneZero(long one, long zero, long widthMask) {
            long known = (one | zero) & widthMask;
            return new BitKnowledge(known, one & known);
        }

        BitKnowledge intersection(BitKnowledge other) {
            long shared = mask & other.mask & ~(value ^ other.value);
            return new BitKnowledge(shared, value & shared);
        }
    }
}
