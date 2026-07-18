// List code/data references to explicit module-relative addresses.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class NetworkSchemaReferenceProbe extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String rawOffsets = requiredEnvironment("NW_NETWORK_PROBE_OFFSETS");
        Path output = Path.of(requiredEnvironment("NW_NETWORK_PROBE_OUT"));
        ArrayList<String> lines = new ArrayList<>();

        for (String rawOffset : rawOffsets.split(",")) {
            Address target = moduleAddress(rawOffset.strip());
            lines.add("===== " + formatOffset(target) + " =====");
            lines.add("data\t" + bytesAt(target, 32));
            appendPointerChain(lines, target, 3);
            LinkedHashSet<String> seen = new LinkedHashSet<>();
            ReferenceIterator references = currentProgram.getReferenceManager().getReferencesTo(target);
            while (references.hasNext()) {
                monitor.checkCancelled();
                Reference reference = references.next();
                Address from = reference.getFromAddress();
                Function function = currentProgram.getFunctionManager().getFunctionContaining(from);
                String line = formatOffset(from) + "\t" + reference.getReferenceType() + "\t" +
                    (function == null
                        ? "<no function>"
                        : formatOffset(function.getEntryPoint()) + " " + function.getName(true));
                if (seen.add(line)) {
                    lines.add(line);
                }
            }
        }

        Files.createDirectories(output.toAbsolutePath().getParent());
        Files.write(output, lines, StandardCharsets.UTF_8);
        println("Wrote reference probe: " + output.toAbsolutePath());
    }

    private void appendPointerChain(List<String> lines, Address source, int maxDepth) {
        Memory memory = currentProgram.getMemory();
        AddressSpace addressSpace = currentProgram.getAddressFactory().getDefaultAddressSpace();
        Address cursor = source;
        for (int depth = 0; depth < maxDepth; depth++) {
            try {
                long raw = memory.getLong(cursor);
                Address target = addressSpace.getAddress(raw);
                if (target == null || !memory.contains(target)) {
                    lines.add("pointer[" + depth + "]\t" + Long.toUnsignedString(raw, 16) +
                        "\t<outside program>");
                    return;
                }
                Function function = currentProgram.getFunctionManager().getFunctionAt(target);
                Symbol symbol = currentProgram.getSymbolTable().getPrimarySymbol(target);
                String identity = function != null
                    ? function.getName(true)
                    : symbol != null ? symbol.getName(true) : "<unnamed>";
                lines.add("pointer[" + depth + "]\t" + formatOffset(target) + "\t" + identity +
                    "\tbytes=" + bytesAt(target, 32));
                cursor = target;
            } catch (Exception ignored) {
                lines.add("pointer[" + depth + "]\t<unreadable>");
                return;
            }
        }
    }

    private String bytesAt(Address address, int count) {
        Memory memory = currentProgram.getMemory();
        StringBuilder result = new StringBuilder(count * 3);
        for (int index = 0; index < count; index++) {
            try {
                if (index != 0) {
                    result.append(' ');
                }
                result.append(String.format("%02x", memory.getByte(address.add(index)) & 0xff));
            } catch (Exception ignored) {
                result.append(" ??");
                break;
            }
        }
        return result.toString();
    }

    private String requiredEnvironment(String name) {
        String value = System.getenv(name);
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException(name + " is required");
        }
        return value;
    }

    private Address moduleAddress(String value) {
        int plus = value.indexOf('+');
        String offset = (plus < 0 ? value : value.substring(plus + 1)).strip();
        offset = offset.startsWith("0x") || offset.startsWith("0X")
            ? offset.substring(2)
            : offset;
        return currentProgram.getImageBase().add(Long.parseUnsignedLong(offset, 16));
    }

    private String formatOffset(Address address) {
        return "NewWorld+0x" + Long.toUnsignedString(
            address.subtract(currentProgram.getImageBase()), 16);
    }
}
