// Inspect exact Ghidra data types used by network-schema identity proofs.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Set;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.Structure;

public class NetworkSchemaDataTypeProbe extends GhidraScript {
    private static final String SCRIPT_VERSION = "1.0.0";

    @Override
    protected void run() throws Exception {
        GhidraCli cli = GhidraCli.parse(
            getScriptArgs(),
            Set.of("type-names", "out"),
            Set.of("force", "dry-run"),
            0);
        if (cli.helpRequested()) {
            printHelp();
            return;
        }
        if (cli.versionRequested()) {
            println("NetworkSchemaDataTypeProbe " + SCRIPT_VERSION);
            return;
        }
        String rawNames = cli.required("type-names", "NW_NETWORK_PROBE_TYPE_NAMES");
        Path output = Path.of(cli.required("out", "NW_NETWORK_PROBE_OUT"));
        boolean force = cli.flag("force", null, false);
        boolean dryRun = cli.flag("dry-run", null, false);
        if (Files.exists(output) && !force && !dryRun) {
            throw new IllegalArgumentException(
                "output already exists: " + output + "; pass --force to replace it");
        }
        List<String> requested = List.of(rawNames.split(";"));
        ArrayList<String> lines = new ArrayList<>();

        Iterator<DataType> types = currentProgram.getDataTypeManager().getAllDataTypes();
        while (types.hasNext()) {
            monitor.checkCancelled();
            DataType type = types.next();
            if (!(type instanceof Structure structure)) {
                continue;
            }
            for (String requestedName : requested) {
                String name = requestedName.strip();
                if (name.equals(type.getName()) || type.getPathName().endsWith("/" + name)) {
                    lines.add(name + "\t" + structure.getLength() + "\t" + type.getPathName());
                }
            }
        }

        if (dryRun) {
            println("Dry run: would write " + lines.size() + " data-type row(s) to " +
                output.toAbsolutePath());
        }
        else {
            Files.createDirectories(output.toAbsolutePath().getParent());
            Files.write(output, lines, StandardCharsets.UTF_8);
            println("Wrote data-type probe: " + output.toAbsolutePath());
        }
    }

    private void printHelp() {
        println("NetworkSchemaDataTypeProbe " + SCRIPT_VERSION);
        println("Inspect named network-schema datatypes in the current Ghidra program.");
        println("");
        println("Options:");
        println("  --type-names <LIST>  Semicolon-separated type names [env: NW_NETWORK_PROBE_TYPE_NAMES]");
        println("  --out <FILE>         Output report [env: NW_NETWORK_PROBE_OUT]");
        println("  --force              Replace an existing output");
        println("  --dry-run            Analyze without filesystem writes");
        println("  -h, --help           Print help");
        println("  -V, --version        Print version");
    }
}
