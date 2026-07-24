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
    private static final int ANALYSIS_CACHE_LIMIT = 512;
    private static final Comparator<PcodeBlockBasic> BLOCK_ORDER = Comparator
        .comparing(NetworkSchemaControlFlow::startAddress, Comparator.nullsLast(Address::compareTo))
        .thenComparingInt(PcodeBlock::getIndex);
    private static final Map<HighFunction, ControlFlowAnalysis> ANALYSIS_CACHE =
        new LinkedHashMap<>(ANALYSIS_CACHE_LIMIT, 0.75f, true) {
            @Override
            protected boolean removeEldestEntry(
                Map.Entry<HighFunction, ControlFlowAnalysis> eldest) {

                return size() > ANALYSIS_CACHE_LIMIT;
            }
        };
    private static final ThreadLocal<Runnable> CANCELLATION_CHECKPOINT = new ThreadLocal<>();

    private NetworkSchemaControlFlow() {}

    static void clearCaches() {
        synchronized (ANALYSIS_CACHE) {
            ANALYSIS_CACHE.clear();
        }
    }

    static void setCancellationCheckpoint(Runnable checkpoint) {
        CANCELLATION_CHECKPOINT.set(checkpoint);
    }

    static void clearCancellationCheckpoint() {
        CANCELLATION_CHECKPOINT.remove();
    }

    private static void checkCancelled() {
        Runnable checkpoint = CANCELLATION_CHECKPOINT.get();
        if (checkpoint != null) {
            checkpoint.run();
        }
    }

    /**
     * Returns reachable operations in reverse-postorder, preserving sequence order within a block.
     * Each loop body appears once; back edges never cause repeated synthetic fields.
     */
    static List<PcodeOpAST> orderedOperations(HighFunction high) {
        if (high == null) {
            return List.of();
        }
        ControlFlowAnalysis analysis = analysis(high);
        List<PcodeBlockBasic> blocks = analysis.blocks();
        if (blocks.isEmpty()) {
            return analysis.operationsWithoutCfg();
        }
        List<PcodeOpAST> cached = analysis.orderedOperations();
        if (cached != null) {
            return cached;
        }

        ArrayList<PcodeBlockBasic> entries = new ArrayList<>();
        for (PcodeBlockBasic block : blocks) {
            checkCancelled();
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
            checkCancelled();
            appendPostorder(entry, visited, postorder);
        }
        ArrayList<PcodeBlockBasic> remaining = new ArrayList<>(blocks);
        remaining.removeAll(visited);
        remaining.sort(BLOCK_ORDER);
        for (PcodeBlockBasic block : remaining) {
            checkCancelled();
            appendPostorder(block, visited, postorder);
        }
        Collections.reverse(postorder);

        ArrayList<PcodeOpAST> operations = new ArrayList<>();
        for (PcodeBlockBasic block : postorder) {
            checkCancelled();
            Iterator<PcodeOp> iterator = block.getIterator();
            while (iterator.hasNext()) {
                checkCancelled();
                PcodeOp operation = iterator.next();
                if (operation instanceof PcodeOpAST ast) {
                    operations.add(ast);
                }
            }
        }
        return analysis.cacheOrderedOperations(operations);
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
            checkCancelled();
            PcodeBlockBasic block = pending.remove(pending.size() - 1);
            if (!visited.add(block)) {
                continue;
            }
            for (PcodeBlockBasic successor : successors(block)) {
                checkCancelled();
                if (successor == target) {
                    return true;
                }
                pending.add(successor);
            }
        }
        return false;
    }

    /**
     * Returns whether {@code target} is reachable during the current loop iteration.
     * Back edges to the loop header begin the next iteration and are not traversed.
     */
    static boolean reachesWithinIteration(
        PcodeBlockBasic source,
        PcodeBlockBasic target,
        NetworkSchemaNaturalLoop loop) {

        if (source == null || target == null || loop == null ||
            !loop.contains(source) || !loop.contains(target)) {
            return false;
        }
        if (source == target) {
            return true;
        }
        ArrayList<PcodeBlockBasic> pending = new ArrayList<>();
        HashSet<PcodeBlockBasic> visited = new HashSet<>();
        pending.add(source);
        while (!pending.isEmpty()) {
            checkCancelled();
            PcodeBlockBasic block = pending.remove(pending.size() - 1);
            if (!visited.add(block)) {
                continue;
            }
            for (PcodeBlockBasic successor : successors(block)) {
                checkCancelled();
                if (successor == target) {
                    return true;
                }
                if (successor != loop.header() && loop.contains(successor)) {
                    pending.add(successor);
                }
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

        ControlFlowAnalysis analysis = analysis(high);
        List<PcodeBlockBasic> blocks = analysis.blocks();
        if (blocks.isEmpty()) {
            return false;
        }
        Set<PcodeBlockBasic> targetDominators = analysis.dominators().get(target);
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
        ControlFlowAnalysis analysis = analysis(high);
        List<PcodeBlockBasic> blocks = analysis.blocks();
        if (blocks.isEmpty()) {
            return List.of();
        }
        List<NetworkSchemaNaturalLoop> cached = analysis.naturalLoops();
        if (cached != null) {
            return cached;
        }
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators = analysis.dominators();
        LinkedHashMap<PcodeBlockBasic, LinkedHashSet<PcodeBlockBasic>> bodies =
            new LinkedHashMap<>();
        for (PcodeBlockBasic tail : blocks) {
            checkCancelled();
            for (PcodeBlockBasic header : successors(tail)) {
                checkCancelled();
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
            checkCancelled();
            result.add(new NetworkSchemaNaturalLoop(header, bodies.get(header)));
        }
        return analysis.cacheNaturalLoops(result);
    }

    /** True only when the reachable CFG is one unbranched, acyclic path. */
    static boolean isLinear(HighFunction high) {
        if (high == null) {
            return false;
        }
        List<PcodeBlockBasic> blocks = analysis(high).blocks();
        if (blocks.isEmpty() || !naturalLoops(high).isEmpty()) {
            return false;
        }
        int entries = 0;
        for (PcodeBlockBasic block : blocks) {
            checkCancelled();
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
        checkCancelled();
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
            checkCancelled();
            PcodeBlockBasic block = pending.remove(pending.size() - 1);
            for (PcodeBlockBasic predecessor : predecessors(block)) {
                checkCancelled();
                if (body.add(predecessor) && predecessor != header) {
                    pending.add(predecessor);
                }
            }
        }
        return body;
    }

    private static Map<PcodeBlockBasic, Set<PcodeBlockBasic>> computeDominators(
        List<PcodeBlockBasic> blocks) {

        HashSet<PcodeBlockBasic> all = new HashSet<>(blocks);
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators = new HashMap<>();
        for (PcodeBlockBasic block : blocks) {
            checkCancelled();
            dominators.put(
                block,
                block.getInSize() == 0
                    ? new HashSet<>(Set.of(block))
                    : new HashSet<>(all));
        }
        boolean changed;
        do {
            checkCancelled();
            changed = false;
            for (PcodeBlockBasic block : blocks) {
                checkCancelled();
                if (block.getInSize() == 0) {
                    continue;
                }
                Set<PcodeBlockBasic> next = null;
                for (PcodeBlockBasic predecessor : predecessors(block)) {
                    checkCancelled();
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
            checkCancelled();
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
            checkCancelled();
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
            checkCancelled();
            result.add(iterator.next());
        }
        result.sort((left, right) -> left.getSeqnum().compareTo(right.getSeqnum()));
        return result;
    }

    private static ControlFlowAnalysis analysis(HighFunction high) {
        synchronized (ANALYSIS_CACHE) {
            return ANALYSIS_CACHE.computeIfAbsent(high, ControlFlowAnalysis::new);
        }
    }

    private static final class ControlFlowAnalysis {
        private final HighFunction high;
        private final List<PcodeBlockBasic> blocks;
        private Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators;
        private List<PcodeOpAST> orderedOperations;
        private List<PcodeOpAST> operationsWithoutCfg;
        private List<NetworkSchemaNaturalLoop> naturalLoops;

        private ControlFlowAnalysis(HighFunction high) {
            this.high = high;
            ArrayList<PcodeBlockBasic> recovered = high.getBasicBlocks();
            blocks = recovered == null ? List.of() : List.copyOf(recovered);
        }

        private List<PcodeBlockBasic> blocks() {
            return blocks;
        }

        private synchronized Map<PcodeBlockBasic, Set<PcodeBlockBasic>> dominators() {
            if (dominators == null) {
                dominators = computeDominators(blocks);
            }
            return dominators;
        }

        private synchronized List<PcodeOpAST> orderedOperations() {
            return orderedOperations;
        }

        private synchronized List<PcodeOpAST> cacheOrderedOperations(
            List<PcodeOpAST> operations) {

            if (orderedOperations == null) {
                orderedOperations = List.copyOf(operations);
            }
            return orderedOperations;
        }

        private synchronized List<PcodeOpAST> operationsWithoutCfg() {
            if (operationsWithoutCfg == null) {
                operationsWithoutCfg = List.copyOf(
                    NetworkSchemaControlFlow.operationsWithoutCfg(high));
            }
            return operationsWithoutCfg;
        }

        private synchronized List<NetworkSchemaNaturalLoop> naturalLoops() {
            return naturalLoops;
        }

        private synchronized List<NetworkSchemaNaturalLoop> cacheNaturalLoops(
            List<NetworkSchemaNaturalLoop> loops) {

            if (naturalLoops == null) {
                naturalLoops = List.copyOf(loops);
            }
            return naturalLoops;
        }
    }
}
