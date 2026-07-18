import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.block.BasicBlockModel;
import ghidra.program.model.block.CodeBlock;
import ghidra.program.model.block.CodeBlockIterator;
import ghidra.program.model.block.CodeBlockReferenceIterator;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Program;
import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeBlockBasic;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.Varnode;
import ghidra.util.task.TaskMonitor;

/** Exact register constants propagated over instruction p-code and decompiler CFG edges. */
final class NetworkSchemaPcodeConstants {
    private NetworkSchemaPcodeConstants() {}

    static Map<Integer, Long> incomingRegisterValues(
        Program program,
        HighFunction high,
        PcodeBlockBasic target,
        Address before,
        Register register,
        TaskMonitor monitor) {

        if (program == null || high == null || target == null || before == null ||
            register == null) {
            return Map.of();
        }
        Map<Integer, RegisterState> entries = blockEntries(program, high, monitor);
        LinkedHashMap<Integer, Long> result = new LinkedHashMap<>();
        for (PcodeBlockBasic predecessor : NetworkSchemaControlFlow.predecessors(target)) {
            RegisterState entry = entries.get(predecessor.getIndex());
            RegisterState state = transfer(
                program,
                predecessor,
                null,
                entry == null ? new RegisterState() : entry.copy());
            state = transfer(program, target, before, state);
            Long value = state.read(
                register.getAddress().getOffset(),
                register.getMinimumByteSize());
            if (value == null) {
                return Map.of();
            }
            result.put(predecessor.getIndex(), value);
        }
        return Map.copyOf(result);
    }

    /**
     * Recovers a register's value on each instruction-CFG edge entering the
     * block containing {@code before}. This preserves machine blocks that the
     * decompiler may fold into a path-dependent SSA value.
     */
    static Map<Address, Long> incomingInstructionRegisterValues(
        Program program,
        Address before,
        Register register,
        TaskMonitor monitor) {

        if (program == null || before == null || register == null) {
            return Map.of();
        }
        try {
            Function owner = program.getFunctionManager().getFunctionContaining(before);
            if (owner == null) {
                return Map.of();
            }
            BasicBlockModel model = new BasicBlockModel(program);
            CodeBlock target = model.getFirstCodeBlockContaining(before, monitor);
            if (target == null) {
                return Map.of();
            }
            Map<Address, CodeBlock> blocks = instructionBlocks(model, owner, monitor);
            Address targetKey = target.getFirstStartAddress();
            if (targetKey == null || !blocks.containsKey(targetKey)) {
                return Map.of();
            }
            Map<Address, RegisterState> entries = instructionBlockEntries(
                program,
                model,
                owner,
                blocks,
                monitor);
            LinkedHashMap<Address, Long> result = new LinkedHashMap<>();
            CodeBlockReferenceIterator sources = target.getSources(monitor);
            while (sources.hasNext()) {
                CodeBlock source = sources.next().getSourceBlock();
                Address sourceKey = source == null ? null : source.getFirstStartAddress();
                RegisterState entry = sourceKey == null ? null : entries.get(sourceKey);
                if (entry == null || !blocks.containsKey(sourceKey)) {
                    return Map.of();
                }
                RegisterState state = transfer(program, source, null, entry.copy());
                state = transfer(program, target, before, state);
                Long value = state.read(
                    register.getAddress().getOffset(),
                    register.getMinimumByteSize());
                if (value == null) {
                    return Map.of();
                }
                result.put(sourceKey, value);
            }
            return result.isEmpty() ? Map.of() : Map.copyOf(result);
        }
        catch (Exception ignored) {
            return Map.of();
        }
    }

    private static Map<Address, CodeBlock> instructionBlocks(
        BasicBlockModel model,
        Function owner,
        TaskMonitor monitor) throws Exception {

        LinkedHashMap<Address, CodeBlock> result = new LinkedHashMap<>();
        CodeBlockIterator blocks = model.getCodeBlocksContaining(owner.getBody(), monitor);
        while (blocks.hasNext()) {
            CodeBlock block = blocks.next();
            Address key = block.getFirstStartAddress();
            if (key != null) {
                result.put(key, block);
            }
        }
        return Map.copyOf(result);
    }

    private static Map<Address, RegisterState> instructionBlockEntries(
        Program program,
        BasicBlockModel model,
        Function owner,
        Map<Address, CodeBlock> blocks,
        TaskMonitor monitor) throws Exception {

        CodeBlock entry = model.getFirstCodeBlockContaining(owner.getEntryPoint(), monitor);
        Address entryKey = entry == null ? null : entry.getFirstStartAddress();
        if (entryKey == null || !blocks.containsKey(entryKey)) {
            return Map.of();
        }
        LinkedHashMap<Address, RegisterState> entries = new LinkedHashMap<>();
        ArrayDeque<Address> pending = new ArrayDeque<>();
        entries.put(entryKey, new RegisterState());
        pending.addLast(entryKey);
        while (!pending.isEmpty()) {
            if (monitor != null && monitor.isCancelled()) {
                return Map.of();
            }
            Address key = pending.removeFirst();
            CodeBlock block = blocks.get(key);
            RegisterState input = entries.get(key);
            if (block == null || input == null) {
                continue;
            }
            RegisterState output = transfer(program, block, null, input.copy());
            CodeBlockReferenceIterator destinations = block.getDestinations(monitor);
            while (destinations.hasNext()) {
                CodeBlock destination = destinations.next().getDestinationBlock();
                Address destinationKey = destination == null
                    ? null
                    : destination.getFirstStartAddress();
                if (destinationKey == null || !blocks.containsKey(destinationKey)) {
                    continue;
                }
                RegisterState current = entries.get(destinationKey);
                boolean changed;
                if (current == null) {
                    entries.put(destinationKey, output.copy());
                    changed = true;
                }
                else {
                    changed = current.retainCommon(output);
                }
                if (changed && !pending.contains(destinationKey)) {
                    pending.addLast(destinationKey);
                }
            }
        }
        return Map.copyOf(entries);
    }

    private static Map<Integer, RegisterState> blockEntries(
        Program program,
        HighFunction high,
        TaskMonitor monitor) {

        ArrayList<PcodeBlockBasic> blocks = high.getBasicBlocks();
        LinkedHashMap<Integer, RegisterState> entries = new LinkedHashMap<>();
        ArrayDeque<PcodeBlockBasic> pending = new ArrayDeque<>();
        for (PcodeBlockBasic block : blocks) {
            if (block.getInSize() == 0) {
                entries.put(block.getIndex(), new RegisterState());
                pending.addLast(block);
            }
        }
        if (pending.isEmpty() && !blocks.isEmpty()) {
            PcodeBlockBasic first = blocks.stream()
                .min((left, right) -> left.getStart().compareTo(right.getStart()))
                .orElseThrow();
            entries.put(first.getIndex(), new RegisterState());
            pending.addLast(first);
        }

        while (!pending.isEmpty()) {
            if (monitor != null && monitor.isCancelled()) {
                return Map.of();
            }
            PcodeBlockBasic block = pending.removeFirst();
            RegisterState input = entries.get(block.getIndex());
            if (input == null) {
                continue;
            }
            RegisterState output = transfer(program, block, null, input.copy());
            for (PcodeBlockBasic successor : NetworkSchemaControlFlow.successors(block)) {
                RegisterState current = entries.get(successor.getIndex());
                boolean changed;
                if (current == null) {
                    entries.put(successor.getIndex(), output.copy());
                    changed = true;
                }
                else {
                    changed = current.retainCommon(output);
                }
                if (changed && !pending.contains(successor)) {
                    pending.addLast(successor);
                }
            }
        }
        return Map.copyOf(entries);
    }

    private static RegisterState transfer(
        Program program,
        PcodeBlockBasic block,
        Address before,
        RegisterState state) {

        return transfer(
            program,
            new AddressSet(block.getStart(), block.getStop()),
            before,
            state);
    }

    private static RegisterState transfer(
        Program program,
        CodeBlock block,
        Address before,
        RegisterState state) {

        return transfer(program, (AddressSetView) block, before, state);
    }

    private static RegisterState transfer(
        Program program,
        AddressSetView body,
        Address before,
        RegisterState state) {

        for (Instruction instruction : program.getListing().getInstructions(body, true)) {
            if (before != null && instruction.getMinAddress().compareTo(before) >= 0) {
                break;
            }
            HashMap<NodeKey, Long> temporaries = new HashMap<>();
            for (PcodeOp operation : instruction.getPcode()) {
                apply(operation, state, temporaries);
            }
        }
        return state;
    }

    private static void apply(
        PcodeOp operation,
        RegisterState state,
        Map<NodeKey, Long> temporaries) {

        if (operation.getOpcode() == PcodeOp.CALL ||
            operation.getOpcode() == PcodeOp.CALLIND) {
            state.clear();
            temporaries.clear();
            return;
        }
        Varnode output = operation.getOutput();
        if (output == null) {
            return;
        }
        Long value = evaluate(operation, state, temporaries);
        if (output.isRegister()) {
            state.write(output.getOffset(), output.getSize(), value);
        }
        else if (output.isUnique()) {
            NodeKey key = NodeKey.of(output);
            if (value == null) {
                temporaries.remove(key);
            }
            else {
                temporaries.put(key, truncate(value, output.getSize()));
            }
        }
    }

    private static Long evaluate(
        PcodeOp operation,
        RegisterState state,
        Map<NodeKey, Long> temporaries) {

        Varnode output = operation.getOutput();
        int opcode = operation.getOpcode();
        if (opcode == PcodeOp.INT_XOR && operation.getNumInputs() == 2 &&
            NodeKey.of(operation.getInput(0)).equals(NodeKey.of(operation.getInput(1)))) {
            return 0L;
        }
        Long left = operation.getNumInputs() > 0
            ? value(operation.getInput(0), state, temporaries)
            : null;
        Long right = operation.getNumInputs() > 1
            ? value(operation.getInput(1), state, temporaries)
            : null;
        Long result;
        switch (opcode) {
            case PcodeOp.COPY, PcodeOp.CAST, PcodeOp.INDIRECT, PcodeOp.INT_ZEXT -> {
                return left;
            }
            case PcodeOp.INT_SEXT -> {
                return left == null
                    ? null
                    : truncate(signed(left, operation.getInput(0).getSize()), output.getSize());
            }
            case PcodeOp.INT_NEGATE -> {
                return left == null ? null : truncate(~left, output.getSize());
            }
            case PcodeOp.INT_2COMP -> {
                return left == null ? null : truncate(-left, output.getSize());
            }
            case PcodeOp.BOOL_NEGATE -> {
                return left == null ? null : left == 0L ? 1L : 0L;
            }
            case PcodeOp.SUBPIECE -> {
                if (left == null || right == null || right >= Long.BYTES) {
                    return null;
                }
                return truncate(left >>> Math.toIntExact(right * Byte.SIZE), output.getSize());
            }
            case PcodeOp.PIECE -> {
                if (left == null || right == null) {
                    return null;
                }
                int shift = Math.multiplyExact(operation.getInput(1).getSize(), Byte.SIZE);
                return truncate(left << shift | right, output.getSize());
            }
            default -> {
                if (left == null || right == null) {
                    return null;
                }
            }
        }
        int bits = Math.multiplyExact(output.getSize(), Byte.SIZE);
        if ((opcode == PcodeOp.INT_LEFT || opcode == PcodeOp.INT_RIGHT ||
                opcode == PcodeOp.INT_SRIGHT) &&
            Long.compareUnsigned(right, bits) >= 0) {
            return 0L;
        }
        result = switch (opcode) {
            case PcodeOp.INT_ADD, PcodeOp.PTRSUB -> left + right;
            case PcodeOp.INT_SUB -> left - right;
            case PcodeOp.INT_MULT -> left * right;
            case PcodeOp.INT_AND -> left & right;
            case PcodeOp.INT_OR -> left | right;
            case PcodeOp.INT_XOR -> left ^ right;
            case PcodeOp.INT_LEFT -> left << Math.toIntExact(right);
            case PcodeOp.INT_RIGHT -> left >>> Math.toIntExact(right);
            case PcodeOp.INT_SRIGHT ->
                signed(left, operation.getInput(0).getSize()) >> Math.toIntExact(right);
            case PcodeOp.INT_EQUAL -> left.equals(right) ? 1L : 0L;
            case PcodeOp.INT_NOTEQUAL -> !left.equals(right) ? 1L : 0L;
            case PcodeOp.INT_LESS -> Long.compareUnsigned(left, right) < 0 ? 1L : 0L;
            case PcodeOp.INT_LESSEQUAL -> Long.compareUnsigned(left, right) <= 0 ? 1L : 0L;
            case PcodeOp.INT_SLESS ->
                signed(left, operation.getInput(0).getSize()) <
                    signed(right, operation.getInput(1).getSize()) ? 1L : 0L;
            case PcodeOp.INT_SLESSEQUAL ->
                signed(left, operation.getInput(0).getSize()) <=
                    signed(right, operation.getInput(1).getSize()) ? 1L : 0L;
            default -> null;
        };
        return result == null ? null : truncate(result, output.getSize());
    }

    private static Long value(
        Varnode node,
        RegisterState state,
        Map<NodeKey, Long> temporaries) {

        if (node.isConstant()) {
            return truncate(node.getOffset(), node.getSize());
        }
        if (node.isRegister()) {
            return state.read(node.getOffset(), node.getSize());
        }
        return node.isUnique() ? temporaries.get(NodeKey.of(node)) : null;
    }

    private static long truncate(long value, int byteWidth) {
        return byteWidth >= Long.BYTES
            ? value
            : value & ((1L << Math.multiplyExact(byteWidth, Byte.SIZE)) - 1L);
    }

    private static long signed(long value, int byteWidth) {
        int shift = Long.SIZE - Math.multiplyExact(byteWidth, Byte.SIZE);
        return shift == 0 ? value : value << shift >> shift;
    }

    private record NodeKey(int space, long offset, int size) {
        static NodeKey of(Varnode node) {
            return new NodeKey(
                node.getAddress().getAddressSpace().getSpaceID(),
                node.getOffset(),
                node.getSize());
        }
    }

    private static final class RegisterState {
        private final Map<Long, Byte> bytes = new HashMap<>();

        RegisterState copy() {
            RegisterState result = new RegisterState();
            result.bytes.putAll(bytes);
            return result;
        }

        void clear() {
            bytes.clear();
        }

        Long read(long offset, int size) {
            if (size <= 0 || size > Long.BYTES) {
                return null;
            }
            long result = 0L;
            for (int index = 0; index < size; index++) {
                Byte value = bytes.get(offset + index);
                if (value == null) {
                    return null;
                }
                result |= (long) Byte.toUnsignedInt(value) << (index * Byte.SIZE);
            }
            return result;
        }

        void write(long offset, int size, Long value) {
            for (int index = 0; index < size; index++) {
                bytes.remove(offset + index);
            }
            if (value == null || size <= 0 || size > Long.BYTES) {
                return;
            }
            for (int index = 0; index < size; index++) {
                bytes.put(offset + index, (byte)(value >>> (index * Byte.SIZE)));
            }
        }

        boolean retainCommon(RegisterState other) {
            int previous = bytes.size();
            bytes.entrySet().removeIf(entry ->
                !entry.getValue().equals(other.bytes.get(entry.getKey())));
            return bytes.size() != previous;
        }
    }
}
