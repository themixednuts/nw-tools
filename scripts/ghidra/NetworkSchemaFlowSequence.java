import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.PriorityQueue;
import java.util.Set;
import java.util.function.BiPredicate;

import ghidra.program.model.address.Address;
import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeBlock;
import ghidra.program.model.pcode.PcodeBlockBasic;
import ghidra.program.model.pcode.PcodeOp;

/** Exact codec-sequence agreement over a decompiler control-flow graph. */
final class NetworkSchemaFlowSequence {
    enum Status {
        COMPLETE,
        DIVERGENT_PATHS,
        CYCLIC_EVENTS,
        NO_TERMINATING_PATH
    }

    record Result<T>(Status status, List<T> events) {
        Result {
            events = List.copyOf(events);
        }

        boolean complete() {
            return status == Status.COMPLETE;
        }
    }

    record OptionalSuffixResult<T>(
        Status status,
        List<T> requiredEvents,
        List<T> optionalEvents,
        int distinctSequenceCount,
        List<List<T>> observedSequences) {

        OptionalSuffixResult {
            requiredEvents = List.copyOf(requiredEvents);
            optionalEvents = List.copyOf(optionalEvents);
            observedSequences = observedSequences.stream().map(List::copyOf).toList();
        }

        boolean complete() {
            return status == Status.COMPLETE;
        }

        boolean hasOptionalSuffix() {
            return complete() && !optionalEvents.isEmpty();
        }
    }

    private record Component(
        int id,
        List<PcodeBlockBasic> blocks,
        Address order,
        boolean cyclic) {
    }

    private record Outcome<T>(Status status, List<T> events) {
        static <T> Outcome<T> complete(List<T> events) {
            return new Outcome<>(Status.COMPLETE, List.copyOf(events));
        }

        static <T> Outcome<T> failed(Status status) {
            return new Outcome<>(status, List.of());
        }
    }

    private record FlowGraph<T>(
        Status failure,
        Map<Component, List<T>> localEvents,
        Map<Component, Set<Component>> edges,
        List<Component> order,
        List<Component> entries) {

        static <T> FlowGraph<T> failed(Status status) {
            return new FlowGraph<>(status, Map.of(), Map.of(), List.of(), List.of());
        }
    }

    private static final Comparator<PcodeBlockBasic> BLOCK_ORDER = Comparator
        .comparing(NetworkSchemaFlowSequence::startAddress, Comparator.nullsLast(Address::compareTo))
        .thenComparingInt(PcodeBlock::getIndex);

    private NetworkSchemaFlowSequence() {
    }

    static <T> Result<T> analyze(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        BiPredicate<T, T> equivalent) {

        return analyze(high, eventsByBlock, Set.of(), equivalent);
    }

    static <T> Result<T> analyze(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        BiPredicate<T, T> equivalent) {

        return analyze(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            null,
            equivalent);
    }

    static <T> Result<T> analyze(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        BiPredicate<T, T> equivalent) {

        return analyze(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            acceptedTerminalBlocks,
            Set.of(),
            equivalent);
    }

    static <T> Result<T> analyze(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        Set<PcodeBlockBasic> excludedBlocks,
        BiPredicate<T, T> equivalent) {

        return analyze(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            acceptedTerminalBlocks,
            excludedBlocks,
            Map.of(),
            equivalent);
    }

    static <T> Result<T> analyze(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        Set<PcodeBlockBasic> excludedBlocks,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> excludedEdges,
        BiPredicate<T, T> equivalent) {

        if (high == null || equivalent == null) {
            return new Result<>(Status.NO_TERMINATING_PATH, List.of());
        }
        FlowGraph<T> graph = flowGraph(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            excludedBlocks,
            excludedEdges);
        if (graph.failure() != null) {
            return new Result<>(graph.failure(), List.of());
        }
        Map<Component, Outcome<T>> outcomes = new HashMap<>();
        for (int index = graph.order().size() - 1; index >= 0; index--) {
            Component component = graph.order().get(index);
            outcomes.put(
                component,
                componentOutcome(
                    component,
                    graph.localEvents().getOrDefault(component, List.of()),
                    graph.edges().get(component),
                    outcomes,
                    acceptedTerminalBlocks,
                    equivalent));
        }

        Outcome<T> selected = null;
        for (Component component : graph.entries()) {
            Outcome<T> outcome = outcomes.get(component);
            if (outcome == null || outcome.status() == Status.NO_TERMINATING_PATH) {
                continue;
            }
            if (outcome.status() != Status.COMPLETE) {
                return new Result<>(outcome.status(), List.of());
            }
            if (selected != null && !sameSequence(selected.events(), outcome.events(), equivalent)) {
                return new Result<>(Status.DIVERGENT_PATHS, List.of());
            }
            selected = outcome;
        }
        return selected == null
            ? new Result<>(Status.NO_TERMINATING_PATH, List.of())
            : new Result<>(Status.COMPLETE, selected.events());
    }

    static <T> OptionalSuffixResult<T> analyzeOptionalSuffix(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        BiPredicate<T, T> equivalent) {

        return analyzeOptionalSuffix(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            null,
            equivalent);
    }

    static <T> OptionalSuffixResult<T> analyzeOptionalSuffix(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        BiPredicate<T, T> equivalent) {

        return analyzeOptionalSuffix(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            acceptedTerminalBlocks,
            Set.of(),
            equivalent);
    }

    static <T> OptionalSuffixResult<T> analyzeOptionalSuffix(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        Set<PcodeBlockBasic> excludedBlocks,
        BiPredicate<T, T> equivalent) {

        return analyzeOptionalSuffix(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            acceptedTerminalBlocks,
            excludedBlocks,
            Map.of(),
            equivalent);
    }

    static <T> OptionalSuffixResult<T> analyzeOptionalSuffix(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        Set<PcodeBlockBasic> excludedBlocks,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> excludedEdges,
        BiPredicate<T, T> equivalent) {

        if (high == null || equivalent == null) {
            return optionalSuffixFailure(Status.NO_TERMINATING_PATH);
        }
        FlowGraph<T> graph = flowGraph(
            high,
            eventsByBlock,
            collapsedEventBlocks,
            excludedBlocks,
            excludedEdges);
        if (graph.failure() != null) {
            return optionalSuffixFailure(graph.failure());
        }

        Map<Component, List<List<T>>> sequencesByComponent = new HashMap<>();
        for (int index = graph.order().size() - 1; index >= 0; index--) {
            Component component = graph.order().get(index);
            ArrayList<List<T>> suffixes = new ArrayList<>();
            if (hasTerminatingBlock(component, acceptedTerminalBlocks)) {
                suffixes.add(List.of());
            }
            for (Component successor : graph.edges().get(component)) {
                for (List<T> suffix : sequencesByComponent.getOrDefault(
                        successor,
                        List.of())) {
                    if (!addDistinctSequence(suffixes, suffix, equivalent)) {
                        return optionalSuffixFailure(
                            Status.DIVERGENT_PATHS,
                            4,
                            suffixes);
                    }
                }
            }
            ArrayList<List<T>> sequences = new ArrayList<>();
            List<T> local = graph.localEvents().getOrDefault(component, List.of());
            for (List<T> suffix : suffixes) {
                ArrayList<T> sequence = new ArrayList<>(local.size() + suffix.size());
                sequence.addAll(local);
                sequence.addAll(suffix);
                if (!addDistinctSequence(sequences, sequence, equivalent)) {
                    return optionalSuffixFailure(
                        Status.DIVERGENT_PATHS,
                        4,
                        sequences);
                }
            }
            sequencesByComponent.put(component, List.copyOf(sequences));
        }

        ArrayList<List<T>> entrySequences = new ArrayList<>();
        for (Component entry : graph.entries()) {
            for (List<T> sequence : sequencesByComponent.getOrDefault(entry, List.of())) {
                if (!addDistinctSequence(entrySequences, sequence, equivalent)) {
                    return optionalSuffixFailure(Status.DIVERGENT_PATHS);
                }
            }
        }
        if (entrySequences.isEmpty()) {
            return optionalSuffixFailure(Status.NO_TERMINATING_PATH);
        }
        if (entrySequences.size() > 2) {
            return optionalSuffixFailure(
                Status.DIVERGENT_PATHS,
                entrySequences.size(),
                entrySequences);
        }
        if (entrySequences.size() == 1) {
            return new OptionalSuffixResult<>(
                Status.COMPLETE,
                entrySequences.get(0),
                List.of(),
                1,
                entrySequences);
        }

        List<T> first = entrySequences.get(0);
        List<T> second = entrySequences.get(1);
        List<T> required = first.size() <= second.size() ? first : second;
        List<T> extended = first.size() <= second.size() ? second : first;
        if (required.size() == extended.size() ||
            !isPrefix(required, extended, equivalent)) {
            return optionalSuffixFailure(
                Status.DIVERGENT_PATHS,
                2,
                entrySequences);
        }
        return new OptionalSuffixResult<>(
            Status.COMPLETE,
            required,
            extended.subList(required.size(), extended.size()),
            2,
            entrySequences);
    }

    static <T> Result<T> analyzeLoopIteration(
        NetworkSchemaNaturalLoop loop,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        BiPredicate<T, T> equivalent) {

        if (loop == null || loop.header() == null || equivalent == null) {
            return new Result<>(Status.CYCLIC_EVENTS, List.of());
        }
        boolean headerCarriesEvents = !eventsByBlock
            .getOrDefault(loop.header(), List.of())
            .isEmpty();
        List<PcodeBlockBasic> entries = headerCarriesEvents
            ? List.of(loop.header())
            : NetworkSchemaControlFlow.successors(loop.header())
                .stream()
                .filter(block -> block != loop.header() && loop.contains(block))
                .toList();
        if (entries.isEmpty()) {
            return new Result<>(Status.NO_TERMINATING_PATH, List.of());
        }

        Set<PcodeBlockBasic> reachable = loopIterationReachable(
            loop,
            entries,
            headerCarriesEvents);
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> edges = new HashMap<>();
        Set<PcodeBlockBasic> terminals = new HashSet<>();
        for (PcodeBlockBasic block : reachable) {
            LinkedHashSet<PcodeBlockBasic> successors = new LinkedHashSet<>();
            for (PcodeBlockBasic successor : NetworkSchemaControlFlow.successors(block)) {
                if (successor == loop.header()) {
                    terminals.add(block);
                }
                else if (reachable.contains(successor)) {
                    successors.add(successor);
                }
                else if (!loop.contains(successor)) {
                    terminals.add(block);
                }
            }
            edges.put(block, Set.copyOf(successors));
        }

        List<PcodeBlockBasic> order = topologicalBlockOrder(reachable, edges);
        if (order.size() != reachable.size()) {
            return new Result<>(Status.CYCLIC_EVENTS, List.of());
        }
        Map<PcodeBlockBasic, Outcome<T>> outcomes = new HashMap<>();
        for (int index = order.size() - 1; index >= 0; index--) {
            PcodeBlockBasic block = order.get(index);
            Outcome<T> suffix = null;
            for (PcodeBlockBasic successor : edges.get(block)) {
                Outcome<T> candidate = outcomes.get(successor);
                if (candidate == null || candidate.status() != Status.COMPLETE) {
                    return new Result<>(Status.DIVERGENT_PATHS, List.of());
                }
                if (suffix != null &&
                    !sameSequence(suffix.events(), candidate.events(), equivalent)) {
                    return new Result<>(Status.DIVERGENT_PATHS, List.of());
                }
                suffix = candidate;
            }
            if (suffix == null && !terminals.contains(block)) {
                return new Result<>(Status.NO_TERMINATING_PATH, List.of());
            }
            ArrayList<T> events = new ArrayList<>(eventsByBlock.getOrDefault(
                block,
                List.of()));
            if (suffix != null) {
                events.addAll(suffix.events());
            }
            outcomes.put(block, Outcome.complete(events));
        }

        Outcome<T> selected = null;
        for (PcodeBlockBasic entry : entries) {
            Outcome<T> candidate = outcomes.get(entry);
            if (candidate == null || candidate.status() != Status.COMPLETE) {
                return new Result<>(Status.NO_TERMINATING_PATH, List.of());
            }
            if (selected != null &&
                !sameSequence(selected.events(), candidate.events(), equivalent)) {
                return new Result<>(Status.DIVERGENT_PATHS, List.of());
            }
            selected = candidate;
        }
        return selected == null
            ? new Result<>(Status.NO_TERMINATING_PATH, List.of())
            : new Result<>(Status.COMPLETE, selected.events());
    }

    private static <T> FlowGraph<T> flowGraph(
        HighFunction high,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        Set<PcodeBlockBasic> collapsedEventBlocks,
        Set<PcodeBlockBasic> excludedBlocks,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> excludedEdges) {

        List<PcodeBlockBasic> entryBlocks = entryBlocks(high);
        Set<PcodeBlockBasic> reachable = reachableBlocks(
            entryBlocks,
            excludedBlocks,
            excludedEdges);
        if (reachable.isEmpty()) {
            return FlowGraph.failed(Status.NO_TERMINATING_PATH);
        }

        Map<PcodeBlockBasic, List<PcodeBlockBasic>> successors = filteredSuccessors(
            reachable,
            excludedEdges);
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> predecessors = predecessors(successors);
        List<Component> components = components(reachable, successors, predecessors);
        Map<PcodeBlockBasic, Component> componentByBlock = new HashMap<>();
        Map<Component, List<T>> localEvents = new HashMap<>();
        for (Component component : components) {
            for (PcodeBlockBasic block : component.blocks()) {
                componentByBlock.put(block, component);
            }
            ArrayList<T> componentEvents = new ArrayList<>();
            PcodeBlockBasic eventBlock = null;
            for (PcodeBlockBasic block : component.blocks()) {
                List<T> blockEvents = eventsByBlock.getOrDefault(block, List.of());
                if (!blockEvents.isEmpty()) {
                    if (eventBlock != null && eventBlock != block) {
                        return FlowGraph.failed(Status.CYCLIC_EVENTS);
                    }
                    eventBlock = block;
                    componentEvents.addAll(blockEvents);
                }
            }
            if (component.cyclic() && !componentEvents.isEmpty() &&
                (eventBlock == null || !collapsedEventBlocks.contains(eventBlock))) {
                return FlowGraph.failed(Status.CYCLIC_EVENTS);
            }
            localEvents.put(component, List.copyOf(componentEvents));
        }

        Map<Component, Set<Component>> edges = componentEdges(
            components,
            componentByBlock,
            successors);
        List<Component> order = topologicalOrder(components, edges);
        LinkedHashSet<Component> entries = new LinkedHashSet<>();
        for (PcodeBlockBasic entryBlock : entryBlocks) {
            Component component = componentByBlock.get(entryBlock);
            if (component != null) {
                entries.add(component);
            }
        }
        return new FlowGraph<>(
            null,
            Map.copyOf(localEvents),
            Map.copyOf(edges),
            order,
            List.copyOf(entries));
    }

    private static <T> OptionalSuffixResult<T> optionalSuffixFailure(Status status) {
        return optionalSuffixFailure(status, 0);
    }

    private static <T> OptionalSuffixResult<T> optionalSuffixFailure(
        Status status,
        int distinctSequenceCount) {

        return optionalSuffixFailure(status, distinctSequenceCount, List.of());
    }

    private static <T> OptionalSuffixResult<T> optionalSuffixFailure(
        Status status,
        int distinctSequenceCount,
        List<List<T>> observedSequences) {

        return new OptionalSuffixResult<>(
            status,
            List.of(),
            List.of(),
            distinctSequenceCount,
            observedSequences);
    }

    private static <T> boolean addDistinctSequence(
        List<List<T>> sequences,
        List<T> candidate,
        BiPredicate<T, T> equivalent) {

        for (List<T> sequence : sequences) {
            if (sameSequence(sequence, candidate, equivalent)) {
                return true;
            }
        }
        if (sequences.size() >= 3) {
            return false;
        }
        sequences.add(List.copyOf(candidate));
        return true;
    }

    private static <T> boolean isPrefix(
        List<T> prefix,
        List<T> sequence,
        BiPredicate<T, T> equivalent) {

        if (prefix.size() > sequence.size()) {
            return false;
        }
        for (int index = 0; index < prefix.size(); index++) {
            if (!equivalent.test(prefix.get(index), sequence.get(index))) {
                return false;
            }
        }
        return true;
    }

    private static Set<PcodeBlockBasic> loopIterationReachable(
        NetworkSchemaNaturalLoop loop,
        List<PcodeBlockBasic> entries,
        boolean includeHeader) {

        HashSet<PcodeBlockBasic> reachable = new HashSet<>();
        ArrayDeque<PcodeBlockBasic> pending = new ArrayDeque<>(entries);
        while (!pending.isEmpty()) {
            PcodeBlockBasic block = pending.removeLast();
            if ((!includeHeader && block == loop.header()) ||
                !loop.contains(block) || !reachable.add(block)) {
                continue;
            }
            for (PcodeBlockBasic successor : NetworkSchemaControlFlow.successors(block)) {
                if (successor != loop.header() && loop.contains(successor)) {
                    pending.addLast(successor);
                }
            }
        }
        return Set.copyOf(reachable);
    }

    private static List<PcodeBlockBasic> topologicalBlockOrder(
        Set<PcodeBlockBasic> blocks,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> edges) {

        Map<PcodeBlockBasic, Integer> indegree = new HashMap<>();
        for (PcodeBlockBasic block : blocks) {
            indegree.put(block, 0);
        }
        for (Set<PcodeBlockBasic> successors : edges.values()) {
            for (PcodeBlockBasic successor : successors) {
                indegree.computeIfPresent(successor, (ignored, value) -> value + 1);
            }
        }
        PriorityQueue<PcodeBlockBasic> ready = new PriorityQueue<>(BLOCK_ORDER);
        indegree.forEach((block, degree) -> {
            if (degree == 0) {
                ready.add(block);
            }
        });
        ArrayList<PcodeBlockBasic> result = new ArrayList<>(blocks.size());
        while (!ready.isEmpty()) {
            PcodeBlockBasic block = ready.remove();
            result.add(block);
            for (PcodeBlockBasic successor : edges.get(block)) {
                int degree = indegree.computeIfPresent(
                    successor,
                    (ignored, value) -> value - 1);
                if (degree == 0) {
                    ready.add(successor);
                }
            }
        }
        return List.copyOf(result);
    }

    private static <T> Outcome<T> componentOutcome(
        Component component,
        List<T> local,
        Set<Component> successors,
        Map<Component, Outcome<T>> outcomes,
        Set<PcodeBlockBasic> acceptedTerminalBlocks,
        BiPredicate<T, T> equivalent) {

        Outcome<T> suffix = null;
        for (Component successor : successors) {
            Outcome<T> candidate = outcomes.get(successor);
            if (candidate == null || candidate.status() == Status.NO_TERMINATING_PATH) {
                continue;
            }
            if (candidate.status() != Status.COMPLETE) {
                return candidate;
            }
            if (suffix != null && !sameSequence(suffix.events(), candidate.events(), equivalent)) {
                return Outcome.failed(Status.DIVERGENT_PATHS);
            }
            suffix = candidate;
        }
        if (suffix == null && !hasTerminatingBlock(component, acceptedTerminalBlocks)) {
            return Outcome.failed(Status.NO_TERMINATING_PATH);
        }
        ArrayList<T> events = new ArrayList<>(local.size() +
            (suffix == null ? 0 : suffix.events().size()));
        events.addAll(local);
        if (suffix != null) {
            events.addAll(suffix.events());
        }
        return Outcome.complete(events);
    }

    private static boolean hasTerminatingBlock(
        Component component,
        Set<PcodeBlockBasic> acceptedTerminalBlocks) {

        if (acceptedTerminalBlocks != null) {
            return component.blocks().stream().anyMatch(acceptedTerminalBlocks::contains);
        }
        for (PcodeBlockBasic block : component.blocks()) {
            if (block.getOutSize() == 0) {
                return true;
            }
            var operations = block.getIterator();
            while (operations.hasNext()) {
                if (operations.next().getOpcode() == PcodeOp.RETURN) {
                    return true;
                }
            }
        }
        return false;
    }

    private static <T> boolean sameSequence(
        List<T> left,
        List<T> right,
        BiPredicate<T, T> equivalent) {

        if (left.size() != right.size()) {
            return false;
        }
        for (int index = 0; index < left.size(); index++) {
            if (!equivalent.test(left.get(index), right.get(index))) {
                return false;
            }
        }
        return true;
    }

    private static List<PcodeBlockBasic> entryBlocks(HighFunction high) {
        ArrayList<PcodeBlockBasic> blocks = high.getBasicBlocks();
        ArrayList<PcodeBlockBasic> entries = new ArrayList<>();
        for (PcodeBlockBasic block : blocks) {
            if (block.getInSize() == 0) {
                entries.add(block);
            }
        }
        if (entries.isEmpty() && !blocks.isEmpty()) {
            entries.add(blocks.stream().min(BLOCK_ORDER).orElseThrow());
        }
        entries.sort(BLOCK_ORDER);
        return List.copyOf(entries);
    }

    private static Set<PcodeBlockBasic> reachableBlocks(
        List<PcodeBlockBasic> entries,
        Set<PcodeBlockBasic> excludedBlocks,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> excludedEdges) {

        HashSet<PcodeBlockBasic> reachable = new HashSet<>();
        ArrayDeque<PcodeBlockBasic> pending = new ArrayDeque<>(entries);
        while (!pending.isEmpty()) {
            PcodeBlockBasic block = pending.removeLast();
            if (excludedBlocks.contains(block) || !reachable.add(block)) {
                continue;
            }
            for (PcodeBlockBasic successor : NetworkSchemaControlFlow.successors(block)) {
                if (excludedEdges.getOrDefault(block, Set.of()).contains(successor)) {
                    continue;
                }
                pending.addLast(successor);
            }
        }
        return Set.copyOf(reachable);
    }

    private static Map<PcodeBlockBasic, List<PcodeBlockBasic>> filteredSuccessors(
        Set<PcodeBlockBasic> reachable,
        Map<PcodeBlockBasic, Set<PcodeBlockBasic>> excludedEdges) {

        Map<PcodeBlockBasic, List<PcodeBlockBasic>> result = new HashMap<>();
        for (PcodeBlockBasic block : reachable) {
            List<PcodeBlockBasic> successors = NetworkSchemaControlFlow.successors(block)
                .stream()
                .filter(reachable::contains)
                .filter(successor ->
                    !excludedEdges.getOrDefault(block, Set.of()).contains(successor))
                .toList();
            result.put(block, successors);
        }
        return Map.copyOf(result);
    }

    private static Map<PcodeBlockBasic, List<PcodeBlockBasic>> predecessors(
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> successors) {

        Map<PcodeBlockBasic, ArrayList<PcodeBlockBasic>> mutable = new HashMap<>();
        successors.keySet().forEach(block -> mutable.put(block, new ArrayList<>()));
        successors.forEach((block, targets) -> targets.forEach(target ->
            mutable.computeIfAbsent(target, ignored -> new ArrayList<>()).add(block)));
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> result = new HashMap<>();
        mutable.forEach((block, sources) -> {
            sources.sort(BLOCK_ORDER);
            result.put(block, List.copyOf(sources));
        });
        return Map.copyOf(result);
    }

    private static List<Component> components(
        Set<PcodeBlockBasic> reachable,
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> successors,
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> predecessors) {

        List<PcodeBlockBasic> finishOrder = finishOrder(reachable, successors);
        HashSet<PcodeBlockBasic> assigned = new HashSet<>();
        ArrayList<Component> components = new ArrayList<>();
        for (int index = finishOrder.size() - 1; index >= 0; index--) {
            PcodeBlockBasic root = finishOrder.get(index);
            if (!assigned.add(root)) {
                continue;
            }
            ArrayList<PcodeBlockBasic> blocks = new ArrayList<>();
            ArrayDeque<PcodeBlockBasic> pending = new ArrayDeque<>();
            pending.add(root);
            while (!pending.isEmpty()) {
                PcodeBlockBasic block = pending.removeLast();
                blocks.add(block);
                for (PcodeBlockBasic predecessor : predecessors.getOrDefault(
                        block,
                        List.of())) {
                    if (reachable.contains(predecessor) && assigned.add(predecessor)) {
                        pending.addLast(predecessor);
                    }
                }
            }
            blocks.sort(BLOCK_ORDER);
            boolean selfEdge = blocks.size() == 1 &&
                successors.getOrDefault(blocks.get(0), List.of()).contains(blocks.get(0));
            components.add(new Component(
                components.size(),
                List.copyOf(blocks),
                startAddress(blocks.get(0)),
                blocks.size() > 1 || selfEdge));
        }
        return List.copyOf(components);
    }

    private static List<PcodeBlockBasic> finishOrder(
        Set<PcodeBlockBasic> reachable,
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> successors) {
        ArrayList<PcodeBlockBasic> roots = new ArrayList<>(reachable);
        roots.sort(BLOCK_ORDER);
        HashSet<PcodeBlockBasic> visited = new HashSet<>();
        ArrayList<PcodeBlockBasic> order = new ArrayList<>(reachable.size());
        for (PcodeBlockBasic root : roots) {
            if (!visited.add(root)) {
                continue;
            }
            ArrayDeque<DfsFrame> stack = new ArrayDeque<>();
            stack.addLast(new DfsFrame(root, successors.getOrDefault(root, List.of())));
            while (!stack.isEmpty()) {
                DfsFrame frame = stack.peekLast();
                PcodeBlockBasic successor = frame.nextSuccessor();
                if (successor != null) {
                    if (visited.add(successor)) {
                        stack.addLast(new DfsFrame(
                            successor,
                            successors.getOrDefault(successor, List.of())));
                    }
                    continue;
                }
                order.add(frame.block);
                stack.removeLast();
            }
        }
        return List.copyOf(order);
    }

    private static Map<Component, Set<Component>> componentEdges(
        List<Component> components,
        Map<PcodeBlockBasic, Component> componentByBlock,
        Map<PcodeBlockBasic, List<PcodeBlockBasic>> blockSuccessors) {

        Map<Component, Set<Component>> edges = new HashMap<>();
        for (Component component : components) {
            LinkedHashSet<Component> successors = new LinkedHashSet<>();
            for (PcodeBlockBasic block : component.blocks()) {
                for (PcodeBlockBasic successor : blockSuccessors.getOrDefault(
                        block,
                        List.of())) {
                    Component target = componentByBlock.get(successor);
                    if (target != null && target != component) {
                        successors.add(target);
                    }
                }
            }
            edges.put(component, Set.copyOf(successors));
        }
        return edges;
    }

    private static List<Component> topologicalOrder(
        List<Component> components,
        Map<Component, Set<Component>> edges) {

        Map<Component, Integer> indegree = new HashMap<>();
        for (Component component : components) {
            indegree.put(component, 0);
        }
        for (Set<Component> successors : edges.values()) {
            for (Component successor : successors) {
                indegree.computeIfPresent(successor, (ignored, value) -> value + 1);
            }
        }
        Comparator<Component> order = Comparator
            .comparing(Component::order, Comparator.nullsLast(Address::compareTo))
            .thenComparingInt(Component::id);
        PriorityQueue<Component> ready = new PriorityQueue<>(order);
        indegree.forEach((component, degree) -> {
            if (degree == 0) {
                ready.add(component);
            }
        });
        ArrayList<Component> result = new ArrayList<>(components.size());
        while (!ready.isEmpty()) {
            Component component = ready.remove();
            result.add(component);
            for (Component successor : edges.get(component)) {
                int degree = indegree.computeIfPresent(
                    successor,
                    (ignored, value) -> value - 1);
                if (degree == 0) {
                    ready.add(successor);
                }
            }
        }
        return List.copyOf(result);
    }

    private static Address startAddress(PcodeBlockBasic block) {
        return block == null ? null : block.getStart();
    }

    private static final class DfsFrame {
        private final PcodeBlockBasic block;
        private final List<PcodeBlockBasic> successors;
        private int index;

        private DfsFrame(
            PcodeBlockBasic block,
            List<PcodeBlockBasic> successors) {

            this.block = block;
            this.successors = successors;
        }

        private PcodeBlockBasic nextSuccessor() {
            while (index < successors.size()) {
                PcodeBlockBasic successor = successors.get(index++);
                return successor;
            }
            return null;
        }
    }
}
