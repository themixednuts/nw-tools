import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

import ghidra.program.model.address.Address;
import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeBlock;
import ghidra.program.model.pcode.PcodeBlockBasic;

/** Bounded enumeration of successful acyclic CFG paths and their wire events. */
final class NetworkSchemaBranchFlow {
    enum Status {
        COMPLETE,
        CYCLIC,
        PATH_LIMIT,
        NO_SUCCESS_PATH
    }

    record Path<T>(List<PcodeBlockBasic> blocks, List<T> events) {
        Path {
            blocks = List.copyOf(blocks);
            events = List.copyOf(events);
        }

        PcodeBlockBasic successorOf(PcodeBlockBasic block) {
            int index = blocks.indexOf(block);
            return index >= 0 && index + 1 < blocks.size() ? blocks.get(index + 1) : null;
        }
    }

    record Result<T>(Status status, List<Path<T>> paths) {
        Result {
            paths = List.copyOf(paths);
        }

        boolean complete() {
            return status == Status.COMPLETE;
        }
    }

    private record Frame<T>(
        PcodeBlockBasic block,
        List<PcodeBlockBasic> blocks,
        List<T> events) {
    }

    private static final Comparator<PcodeBlockBasic> BLOCK_ORDER = Comparator
        .comparing(NetworkSchemaBranchFlow::startAddress, Comparator.nullsLast(Address::compareTo))
        .thenComparingInt(PcodeBlock::getIndex);

    private NetworkSchemaBranchFlow() {
    }

    static <T> Result<T> analyzeFrom(
        PcodeBlockBasic start,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> successfulTerminals,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> excludedEdges,
        int maximumPaths) {

        if (start == null || successfulTerminals == null || successfulTerminals.isEmpty() ||
            maximumPaths <= 0) {
            return new Result<>(Status.NO_SUCCESS_PATH, List.of());
        }

        ArrayDeque<Frame<T>> pending = new ArrayDeque<>();
        pending.addLast(new Frame<>(start, List.of(), List.of()));
        ArrayList<Path<T>> paths = new ArrayList<>();
        while (!pending.isEmpty()) {
            Frame<T> frame = pending.removeLast();
            if (frame.blocks().contains(frame.block())) {
                return new Result<>(Status.CYCLIC, List.of());
            }

            ArrayList<PcodeBlockBasic> blocks = new ArrayList<>(frame.blocks());
            blocks.add(frame.block());
            ArrayList<T> events = new ArrayList<>(frame.events());
            events.addAll(eventsByBlock.getOrDefault(frame.block(), List.of()));
            if (successfulTerminals.contains(frame.block())) {
                paths.add(new Path<>(blocks, events));
                if (paths.size() > maximumPaths) {
                    return new Result<>(Status.PATH_LIMIT, List.of());
                }
                continue;
            }

            ArrayList<PcodeBlockBasic> successors = new ArrayList<>(
                NetworkSchemaControlFlow.successors(frame.block()));
            successors.removeAll(excludedEdges.getOrDefault(frame.block(), Set.of()));
            successors.sort(BLOCK_ORDER.reversed());
            for (PcodeBlockBasic successor : successors) {
                pending.addLast(new Frame<>(successor, blocks, events));
            }
        }

        return paths.isEmpty()
            ? new Result<>(Status.NO_SUCCESS_PATH, List.of())
            : new Result<>(Status.COMPLETE, paths);
    }

    private static Address startAddress(PcodeBlockBasic block) {
        return block == null ? null : block.getStart();
    }
}
