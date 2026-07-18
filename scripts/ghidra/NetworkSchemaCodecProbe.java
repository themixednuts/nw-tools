// Report structural codec predicates for explicit module-relative functions.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.pcode.HighFunction;

public class NetworkSchemaCodecProbe extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String offsets = requiredEnvironment("NW_NETWORK_PROBE_OFFSETS");
        Path output = Path.of(requiredEnvironment("NW_NETWORK_PROBE_OUT"));
        StringBuilder report = new StringBuilder();

        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        try {
            for (String offset : offsets.split(",")) {
                monitor.checkCancelled();
                Function function = currentProgram.getFunctionManager()
                    .getFunctionContaining(moduleAddress(offset.strip()));
                if (function == null) {
                    report.append(offset).append(" <no function>\n");
                    continue;
                }
                DecompileResults result = decompiler.decompileFunction(function, 120, monitor);
                HighFunction high = result.decompileCompleted() ? result.getHighFunction() : null;
                report.append(formatOffset(function.getEntryPoint()))
                    .append(' ')
                    .append(function.getName(true))
                    .append('\n')
                    .append("  core=")
                    .append(NetworkSchemaCodecClassifier.smallestThreeCoreEvidence(high))
                    .append('\n')
                    .append("  projectedEncode=")
                    .append(NetworkSchemaCodecClassifier.isProjectedVec3SmallestThreeEncodeWrapper(high))
                    .append(" quaternionEncode=")
                    .append(NetworkSchemaCodecClassifier.isQuaternionSmallestThreeEncodeWrapper(high))
                    .append('\n')
                    .append("  projectedDecode=")
                    .append(NetworkSchemaCodecClassifier.isProjectedVec3SmallestThreeDecodeWrapper(high))
                    .append(" quaternionDecode=")
                    .append(NetworkSchemaCodecClassifier.isQuaternionSmallestThreeDecodeWrapper(high))
                    .append("\n\n");
            }
        } finally {
            decompiler.dispose();
        }

        Files.createDirectories(output.toAbsolutePath().getParent());
        Files.writeString(output, report, StandardCharsets.UTF_8);
        println("Wrote codec probe: " + output.toAbsolutePath());
    }

    private String requiredEnvironment(String name) {
        String value = System.getenv(name);
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException(name + " is required");
        }
        return value;
    }

    private Address moduleAddress(String value) {
        String normalized = value;
        int plus = normalized.indexOf('+');
        if (plus >= 0) {
            normalized = normalized.substring(plus + 1);
        }
        normalized = normalized.strip();
        if (normalized.startsWith("0x") || normalized.startsWith("0X")) {
            normalized = normalized.substring(2);
        }
        long offset = Long.parseUnsignedLong(normalized, 16);
        return currentProgram.getImageBase().add(offset);
    }

    private String formatOffset(Address address) {
        long offset = address.subtract(currentProgram.getImageBase());
        return "NewWorld+0x" + Long.toUnsignedString(offset, 16);
    }
}
