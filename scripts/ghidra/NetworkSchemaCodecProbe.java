// Report structural codec predicates for explicit module-relative functions.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.pcode.HighFunction;

public class NetworkSchemaCodecProbe extends GhidraScript {
    private static final String SCRIPT_VERSION = "1.0.0";

    @Override
    protected void run() throws Exception {
        GhidraCli cli = GhidraCli.parse(
            getScriptArgs(),
            Set.of("offsets", "out"),
            Set.of("force", "dry-run"),
            0);
        if (cli.helpRequested()) {
            printHelp();
            return;
        }
        if (cli.versionRequested()) {
            println("NetworkSchemaCodecProbe " + SCRIPT_VERSION);
            return;
        }
        String offsets = cli.required("offsets", "NW_NETWORK_PROBE_OFFSETS");
        Path output = Path.of(cli.required("out", "NW_NETWORK_PROBE_OUT"));
        boolean force = cli.flag("force", null, false);
        boolean dryRun = cli.flag("dry-run", null, false);
        if (Files.exists(output) && !force && !dryRun) {
            throw new IllegalArgumentException(
                "output already exists: " + output + "; pass --force to replace it");
        }
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

        if (dryRun) {
            println("Dry run: would write codec report to " + output.toAbsolutePath());
        }
        else {
            Files.createDirectories(output.toAbsolutePath().getParent());
            Files.writeString(output, report, StandardCharsets.UTF_8);
            println("Wrote codec probe: " + output.toAbsolutePath());
        }
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

    private void printHelp() {
        println("NetworkSchemaCodecProbe " + SCRIPT_VERSION);
        println("Report structural codec evidence for module-relative functions.");
        println("");
        println("Options:");
        println("  --offsets <LIST>  Comma-separated module offsets [env: NW_NETWORK_PROBE_OFFSETS]");
        println("  --out <FILE>      Output report [env: NW_NETWORK_PROBE_OUT]");
        println("  --force           Replace an existing output");
        println("  --dry-run         Analyze without filesystem writes");
        println("  -h, --help        Print help");
        println("  -V, --version     Print version");
    }
}
