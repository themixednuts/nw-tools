import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashSet;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;

import ghidra.program.model.address.Address;
import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeBlock;
import ghidra.program.model.pcode.PcodeBlockBasic;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.PcodeOpAST;

/** Deterministic control-flow ordering for decompiler p-code. */
final class NetworkSchemaControlFlow {
    private static final Comparator<PcodeBlockBasic> BLOCK_ORDER = Comparator
        .comparing(NetworkSchemaControlFlow::startAddress, Comparator.nullsLast(Address::compareTo))
        .thenComparingInt(PcodeBlock::getIndex);

    private NetworkSchemaControlFlow() {}

    /**
     * Returns reachable operations in reverse-postorder, preserving sequence order within a block.
     * Each loop body appears once; back edges never cause repeated synthetic fields.
     */
    static List<PcodeOpAST> orderedOperations(HighFunction high) {
        if (high == null) {
            return List.of();
        }
        ArrayList<PcodeBlockBasic> blocks = high.getBasicBlocks();
        if (blocks == null || blocks.isEmpty()) {
            return operationsWithoutCfg(high);
        }

        ArrayList<PcodeBlockBasic> entries = new ArrayList<>();
        for (PcodeBlockBasic block : blocks) {
            if (block.getInSize() == 0) {
                entries.add(block);
            }
        }
        if (entries.isEmpty()) {
            entries.add(Collections.min(blocks, BLOCK_ORDER));
        }
        entries.sort(BLOCK_ORDER);

        HashSet<PcodeBlockBasic> visited = new HashSet<>();
        ArrayList<PcodeBlockBasic> postorder = new ArrayList<>(blocks.size());
        for (PcodeBlockBasic entry : entries) {
            appendPostorder(entry, visited, postorder);
        }
        ArrayList<PcodeBlockBasic> remaining = new ArrayList<>(blocks);
        remaining.removeAll(visited);
        remaining.sort(BLOCK_ORDER);
        for (PcodeBlockBasic block : remaining) {
            appendPostorder(block, visited, postorder);
        }
        Collections.reverse(postorder);

        ArrayList<PcodeOpAST> operations = new ArrayList<>();
        for (PcodeBlockBasic block : postorder) {
            Iterator<PcodeOp> iterator = block.getIterator();
            while (iterator.hasNext()) {
                PcodeOp operation = iterator.next();
                if (operation instanceof PcodeOpAST ast) {
                    operations.add(ast);
                }
            }
        }
        return operations;
    }

    static boolean reaches(PcodeBlockBasic source, PcodeBlockBasic target) {
        if (source == null || target == null) {
            return false;
        }
        if (source == target) {
            return true;
        }
        ArrayList<PcodeBlockBasic> pending = new ArrayList<>();
        HashSet<PcodeBlockBasic> visited = new HashSet<>();
        pending.add(source);
        while (!pending.isEmpty()) {
            PcodeBlockBasic block = pending.remove(pending.size() - 1);
            if (!visited.add(block)) {
                continue;
            }
            for (PcodeBlockBasic successor : successors(block)) {
                if (successor == target) {
                    return true;
                }
                pending.add(successor);
            }
        }
        return false;
    }

    /** Returns true when every entry-to-target path passes through {@code source}. */
    static boolean dominates(HighFunction high, PcodeBlockBasic source, PcodeBlockBasic target) {
        if (high == null || source == null || target == null) {
            return false;
        }
        if (source == target) {
            return true;
        }

        ArrayList<PcodeBlockBasic> blocks = high.getBasicBlocks();
        if (blocks == null || blocks.isEmpty()) {
            return false;
        }
        Set<PcodeBlockBasic> targetDominators = dominators(blocks).get(target);
        return targetDominators != null && targetDominators.contains(source);
    }

    /** CFG-aware program order for operations that are guaranteed to execute in sequence. */
    static boolean precedes(HighFunction high, PcodeOpAST source, PcodeOpAST target) {
        if (high == null || source == null || target == null || source == target) {
            return false;
        }
        PcodeBlockBasic sourceBlock = parentBlock(source);
        PcodeBlockBasic targetBlock = parentBlock(target);
        if (sourceBlock == null || targetBlock == null) {
            return source.getSeqnum().compareTo(target.getSeqnum()) < 0;
        }
        if (sourceBlock == targetBlock) {
            return source.getSeqnum().compareTo(target.getSeqnum()) < 0;
        }
        return dominates(high, sourceBlock, targetBlock);
    }

    /** Natural loops recovered from CFG back edges, ordered by header address. */
    static List<NetworkSchemaNaturalLoop> naturalLoops(HighFunction high) {
        if (high == null) {
            return List.of();
        }
        ArrayList<PcodeBlockBasic> blocks = high.getBasicBlocks();
        if (blocks == null || blocks.isEmpty()) {
            return List.of();
        }
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators = dominators(blocks);
        LinkedHashMap<PcodeBlockBasic, LinkedHashSet<PcodeBlockBasic>> bodies =
            new LinkedHashMap<>();
        for (PcodeBlockBasic tail : blocks) {
            for (PcodeBlockBasic header : successors(tail)) {
                Set<PcodeBlockBasic> tailDominators = dominators.get(tail);
                if (tailDominators == null || !tailDominators.contains(header)) {
                    continue;
                }
                bodies.computeIfAbsent(header, ignored -> new LinkedHashSet<>())
                    .addAll(naturalLoopBody(header, tail));
            }
        }
        ArrayList<PcodeBlockBasic> headers = new ArrayList<>(bodies.keySet());
        headers.sort(BLOCK_ORDER);
        ArrayList<NetworkSchemaNaturalLoop> result = new ArrayList<>(headers.size());
        for (PcodeBlockBasic header : headers) {
            result.add(new NetworkSchemaNaturalLoop(header, bodies.get(header)));
        }
        return List.copyOf(result);
    }

    /** True only when the reachable CFG is one unbranched, acyclic path. */
    static boolean isLinear(HighFunction high) {
        if (high == null) {
            return false;
        }
        ArrayList<PcodeBlockBasic> blocks = high.getBasicBlocks();
        if (blocks == null || blocks.isEmpty() || !naturalLoops(high).isEmpty()) {
            return false;
        }
        int entries = 0;
        for (PcodeBlockBasic block : blocks) {
            if (block.getInSize() == 0) {
                entries++;
            }
            if (block.getInSize() > 1 || block.getOutSize() > 1) {
                return false;
            }
        }
        return entries == 1;
    }

    /** Returns the most deeply nested natural loop containing the operation. */
    static NetworkSchemaNaturalLoop innermostLoopContaining(
        HighFunction high,
        PcodeOpAST operation) {

        PcodeBlockBasic block = parentBlock(operation);
        if (block == null) {
            return null;
        }
        return naturalLoops(high).stream()
            .filter(loop -> loop.contains(block))
            .min(Comparator.comparingInt(NetworkSchemaNaturalLoop::blockCount))
            .orElse(null);
    }

    private static void appendPostorder(
        PcodeBlockBasic block,
        Set<PcodeBlockBasic> visited,
        List<PcodeBlockBasic> postorder) {

        if (block == null || !visited.add(block)) {
            return;
        }
        for (PcodeBlockBasic successor : successors(block)) {
            appendPostorder(successor, visited, postorder);
        }
        postorder.add(block);
    }

    private static Set<PcodeBlockBasic> naturalLoopBody(
        PcodeBlockBasic header,
        PcodeBlockBasic tail) {

        LinkedHashSet<PcodeBlockBasic> body = new LinkedHashSet<>();
        body.add(header);
        body.add(tail);
        ArrayList<PcodeBlockBasic> pending = new ArrayList<>();
        if (tail != header) {
            pending.add(tail);
        }
        while (!pending.isEmpty()) {
            PcodeBlockBasic block = pending.remove(pending.size() - 1);
            for (PcodeBlockBasic predecessor : predecessors(block)) {
                if (body.add(predecessor) && predecessor != header) {
                    pending.add(predecessor);
                }
            }
        }
        return body;
    }

    private static Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators(
        List<PcodeBlockBasic> blocks) {

        HashSet<PcodeBlockBasic> all = new HashSet<>(blocks);
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators = new HashMap<>();
        for (PcodeBlockBasic block : blocks) {
            dominators.put(
                block,
                block.getInSize() == 0
                    ? new HashSet<>(Set.of(block))
                    : new HashSet<>(all));
        }
        boolean changed;
        do {
            changed = false;
            for (PcodeBlockBasic block : blocks) {
                if (block.getInSize() == 0) {
                    continue;
                }
                Set<PcodeBlockBasic> next = null;
                for (PcodeBlockBasic predecessor : predecessors(block)) {
                    Set<PcodeBlockBasic> predecessorDominators = dominators.get(predecessor);
                    if (predecessorDominators == null) {
                        continue;
                    }
                    if (next == null) {
                        next = new HashSet<>(predecessorDominators);
                    }
                    else {
                        next.retainAll(predecessorDominators);
                    }
                }
                if (next == null) {
                    next = new HashSet<>();
                }
                next.add(block);
                if (!next.equals(dominators.get(block))) {
                    dominators.put(block, next);
                    changed = true;
                }
            }
        } while (changed);
        return dominators;
    }

    static List<PcodeBlockBasic> successors(PcodeBlockBasic block) {
        ArrayList<PcodeBlockBasic> result = new ArrayList<>(block.getOutSize());
        for (int index = 0; index < block.getOutSize(); index++) {
            PcodeBlock successor = block.getOut(index);
            PcodeBlock leaf = successor == null ? null : successor.getFrontLeaf();
            if (leaf instanceof PcodeBlockBasic basic) {
                result.add(basic);
            }
        }
        result.sort(BLOCK_ORDER);
        return result;
    }

    static List<PcodeBlockBasic> predecessors(PcodeBlockBasic block) {
        ArrayList<PcodeBlockBasic> result = new ArrayList<>(block.getInSize());
        for (int index = 0; index < block.getInSize(); index++) {
            PcodeBlock predecessor = block.getIn(index);
            PcodeBlock leaf = predecessor == null ? null : predecessor.getFrontLeaf();
            if (leaf instanceof PcodeBlockBasic basic) {
                result.add(basic);
            }
        }
        result.sort(BLOCK_ORDER);
        return result;
    }

    static PcodeBlockBasic parentBlock(PcodeOpAST operation) {
        PcodeBlock parent = operation == null ? null : operation.getParent();
        return parent instanceof PcodeBlockBasic basic ? basic : null;
    }

    private static Address startAddress(PcodeBlockBasic block) {
        return block == null ? null : block.getStart();
    }

    private static List<PcodeOpAST> operationsWithoutCfg(HighFunction high) {
        ArrayList<PcodeOpAST> result = new ArrayList<>();
        Iterator<PcodeOpAST> iterator = high.getPcodeOps();
        while (iterator.hasNext()) {
            result.add(iterator.next());
        }
        result.sort((left, right) -> left.getSeqnum().compareTo(right.getSeqnum()));
        return result;
    }
}
