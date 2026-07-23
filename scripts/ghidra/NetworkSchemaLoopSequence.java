import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.BiPredicate;

import ghidra.program.model.pcode.PcodeBlockBasic;

/** Exact path sequences for one natural-loop iteration. */
final class NetworkSchemaLoopSequence {
    enum Status {
        COMPLETE,
        DIVERGENT_PATHS,
        CYCLIC_EVENTS,
        NO_TERMINATING_PATH
    }

    record OptionalSuffixResult<T>(
        Status status,
        List<T> requiredEvents,
        List<T> optionalEvents,
        int distinctSequenceCount) {

        OptionalSuffixResult {
            requiredEvents = List.copyOf(requiredEvents);
            optionalEvents = List.copyOf(optionalEvents);
        }

        boolean hasOptionalSuffix() {
            return status == Status.COMPLETE && !optionalEvents.isEmpty();
        }
    }

    private record PathSequences<T>(Status status, List<List<T>> sequences) {
        PathSequences {
            sequences = sequences.stream().map(List::copyOf).toList();
        }
    }

    private NetworkSchemaLoopSequence() {
    }

    static <T> OptionalSuffixResult<T> analyzeOptionalSuffix(
        NetworkSchemaNaturalLoop loop,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        BiPredicate<T, T> equivalent) {

        if (loop == null || loop.header() == null || equivalent == null) {
            return failed(Status.NO_TERMINATING_PATH);
        }
        boolean headerCarriesEvents =
            !eventsByBlock.getOrDefault(loop.header(), List.of()).isEmpty();
        List<PcodeBlockBasic> entries = headerCarriesEvents
            ? List.of(loop.header())
            : NetworkSchemaControlFlow.successors(loop.header()).stream()
                .filter(block -> block != loop.header() && loop.contains(block))
                .toList();
        if (entries.isEmpty()) {
            return failed(Status.NO_TERMINATING_PATH);
        }

        Map<PcodeBlockBasic, PathSequences<T>> memo = new HashMap<>();
        ArrayList<List<T>> sequences = new ArrayList<>();
        for (PcodeBlockBasic entry : entries) {
            PathSequences<T> paths = sequencesFrom(
                loop,
                entry,
                eventsByBlock,
                equivalent,
                memo,
                new HashSet<>());
            if (paths.status() != Status.COMPLETE) {
                return failed(paths.status());
            }
            for (List<T> sequence : paths.sequences()) {
                if (!addDistinctSequence(sequences, sequence, equivalent)) {
                    return failed(Status.DIVERGENT_PATHS, sequences.size());
                }
            }
        }
        if (sequences.isEmpty()) {
            return failed(Status.NO_TERMINATING_PATH);
        }
        if (sequences.size() == 1) {
            return new OptionalSuffixResult<>(
                Status.COMPLETE,
                sequences.get(0),
                List.of(),
                1);
        }
        if (sequences.size() != 2) {
            return failed(Status.DIVERGENT_PATHS, sequences.size());
        }

        List<T> first = sequences.get(0);
        List<T> second = sequences.get(1);
        List<T> required = first.size() <= second.size() ? first : second;
        List<T> extended = first.size() <= second.size() ? second : first;
        if (required.size() == extended.size() ||
            !isPrefix(required, extended, equivalent)) {
            return failed(Status.DIVERGENT_PATHS, 2);
        }
        return new OptionalSuffixResult<>(
            Status.COMPLETE,
            required,
            extended.subList(required.size(), extended.size()),
            2);
    }

    private static <T> PathSequences<T> sequencesFrom(
        NetworkSchemaNaturalLoop loop,
        PcodeBlockBasic block,
        Map<PcodeBlockBasic, List<T>> eventsByBlock,
        BiPredicate<T, T> equivalent,
        Map<PcodeBlockBasic, PathSequences<T>> memo,
        Set<PcodeBlockBasic> active) {

        PathSequences<T> cached = memo.get(block);
        if (cached != null) {
            return cached;
        }
        if (!active.add(block)) {
            return new PathSequences<>(Status.CYCLIC_EVENTS, List.of());
        }
        try {
            ArrayList<List<T>> suffixes = new ArrayList<>();
            for (PcodeBlockBasic successor : NetworkSchemaControlFlow.successors(block)) {
                if (successor == loop.header() || !loop.contains(successor)) {
                    if (!addDistinctSequence(suffixes, List.of(), equivalent)) {
                        return new PathSequences<>(Status.DIVERGENT_PATHS, suffixes);
                    }
                    continue;
                }
                PathSequences<T> child = sequencesFrom(
                    loop,
                    successor,
                    eventsByBlock,
                    equivalent,
                    memo,
                    active);
                if (child.status() != Status.COMPLETE) {
                    return child;
                }
                for (List<T> suffix : child.sequences()) {
                    if (!addDistinctSequence(suffixes, suffix, equivalent)) {
                        return new PathSequences<>(Status.DIVERGENT_PATHS, suffixes);
                    }
                }
            }
            if (suffixes.isEmpty()) {
                return new PathSequences<>(Status.NO_TERMINATING_PATH, List.of());
            }

            List<T> local = eventsByBlock.getOrDefault(block, List.of());
            ArrayList<List<T>> sequences = new ArrayList<>(suffixes.size());
            for (List<T> suffix : suffixes) {
                ArrayList<T> sequence = new ArrayList<>(local.size() + suffix.size());
                sequence.addAll(local);
                sequence.addAll(suffix);
                if (!addDistinctSequence(sequences, sequence, equivalent)) {
                    return new PathSequences<>(Status.DIVERGENT_PATHS, sequences);
                }
            }
            PathSequences<T> result = new PathSequences<>(Status.COMPLETE, sequences);
            memo.put(block, result);
            return result;
        }
        finally {
            active.remove(block);
        }
    }

    private static <T> boolean addDistinctSequence(
        List<List<T>> sequences,
        List<T> candidate,
        BiPredicate<T, T> equivalent) {

        if (sequences.stream().anyMatch(existing ->
                sameSequence(existing, candidate, equivalent))) {
            return true;
        }
        if (sequences.size() >= 3) {
            return false;
        }
        sequences.add(List.copyOf(candidate));
        return true;
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

    private static <T> OptionalSuffixResult<T> failed(Status status) {
        return failed(status, 0);
    }

    private static <T> OptionalSuffixResult<T> failed(
        Status status,
        int distinctSequenceCount) {

        return new OptionalSuffixResult<>(
            status,
            List.of(),
            List.of(),
            distinctSequenceCount);
    }
}
