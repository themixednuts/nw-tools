import java.util.LinkedHashMap;
import java.util.Map;

import com.google.gson.JsonObject;

/** Timings and cache counters emitted with every extractor report. */
final class NetworkSchemaRunMetrics {
    private final long runStarted = System.nanoTime();
    private final Map<String, Long> phaseNanos = new LinkedHashMap<>();
    private final Map<String, Long> counters = new LinkedHashMap<>();
    private final Map<String, Map<String, Long>> phaseCounters = new LinkedHashMap<>();
    private Map<String, Long> countersAtPhaseStart = Map.of();

    long startPhase() {
        countersAtPhaseStart = new LinkedHashMap<>(counters);
        return System.nanoTime();
    }

    long finishPhase(String name, long started) {
        long elapsed = System.nanoTime() - started;
        phaseNanos.merge(name, elapsed, Long::sum);
        LinkedHashMap<String, Long> delta = new LinkedHashMap<>();
        counters.forEach((counter, value) -> {
            long difference = value - countersAtPhaseStart.getOrDefault(counter, 0L);
            if (difference != 0L) {
                delta.put(counter, difference);
            }
        });
        phaseCounters.put(name, delta);
        return millis(elapsed);
    }

    void increment(String name) {
        counters.merge(name, 1L, Long::sum);
    }

    long counter(String name) {
        return counters.getOrDefault(name, 0L);
    }

    JsonObject toJson() {
        JsonObject result = new JsonObject();
        result.addProperty("totalMillis", millis(System.nanoTime() - runStarted));

        JsonObject phases = new JsonObject();
        phaseNanos.forEach((name, nanos) -> phases.addProperty(name, millis(nanos)));
        result.add("phaseMillis", phases);

        JsonObject phaseCounts = new JsonObject();
        phaseCounters.forEach((phase, values) -> {
            JsonObject counts = new JsonObject();
            values.forEach(counts::addProperty);
            phaseCounts.add(phase, counts);
        });
        result.add("phaseCounters", phaseCounts);

        JsonObject counts = new JsonObject();
        counters.forEach(counts::addProperty);
        result.add("counters", counts);
        return result;
    }

    private static long millis(long nanos) {
        return Math.round(nanos / 1_000_000.0);
    }
}
