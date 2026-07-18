// Inspect exact Ghidra data types used by network-schema identity proofs.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.Structure;

public class NetworkSchemaDataTypeProbe extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String rawNames = requiredEnvironment("NW_NETWORK_PROBE_TYPE_NAMES");
        Path output = Path.of(requiredEnvironment("NW_NETWORK_PROBE_OUT"));
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

        Files.createDirectories(output.toAbsolutePath().getParent());
        Files.write(output, lines, StandardCharsets.UTF_8);
        println("Wrote data-type probe: " + output.toAbsolutePath());
    }

    private String requiredEnvironment(String name) {
        String value = System.getenv(name);
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException(name + " is required");
        }
        return value;
    }
}
