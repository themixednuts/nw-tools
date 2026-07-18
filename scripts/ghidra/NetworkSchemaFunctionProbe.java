// Decompile explicit module-relative functions for network-schema research.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.pcode.HighFunction;
import ghidra.program.model.pcode.PcodeBlock;
import ghidra.program.model.pcode.PcodeBlockBasic;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.PcodeOpAST;

public class NetworkSchemaFunctionProbe extends GhidraScript {
    private static final Pattern INTEGER_OR_RANGE = Pattern.compile(
        "\\s*([+-]?(?:0[xX][0-9a-fA-F]+|[0-9]+))" +
        "(?:\\s*-\\s*([+-]?(?:0[xX][0-9a-fA-F]+|[0-9]+)))?\\s*");

    @Override
    protected void run() throws Exception {
        String rawOffsets = requiredEnvironment("NW_NETWORK_PROBE_OFFSETS");
        Path output = Path.of(requiredEnvironment("NW_NETWORK_PROBE_OUT"));
        boolean includePcode = Boolean.parseBoolean(
            System.getenv().getOrDefault("NW_NETWORK_PROBE_PCODE", "false"));
        boolean includeCfg = Boolean.parseBoolean(
            System.getenv().getOrDefault("NW_NETWORK_PROBE_CFG", "false"));
        List<Integer> vtableSlots = optionalIntegerListEnvironment(
            "NW_NETWORK_PROBE_VTABLE_SLOTS");
        if (vtableSlots.isEmpty()) {
            Integer slot = optionalIntegerEnvironment("NW_NETWORK_PROBE_VTABLE_SLOT");
            vtableSlots = slot == null ? List.of(-1) : List.of(slot);
        }
        List<String> sections = new ArrayList<>();

        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        try {
            for (String rawOffset : rawOffsets.split(",")) {
                monitor.checkCancelled();
                Address source = moduleAddress(rawOffset.trim());
                for (int vtableSlot : vtableSlots) {
                    Address address = vtableSlot < 0
                        ? source
                        : pointerAt(source.add(Math.multiplyExact(vtableSlot, 8L)));
                    Function function = currentProgram.getFunctionManager()
                        .getFunctionContaining(address);
                    String slotLabel = vtableSlot < 0 ? "" : " vtable-slot=" + vtableSlot;
                    if (function == null) {
                        sections.add("===== " + rawOffset + slotLabel + " <no function> =====\n");
                        continue;
                    }
                    DecompileResults result = decompiler.decompileFunction(function, 120, monitor);
                    String body = result.decompileCompleted()
                        ? result.getDecompiledFunction().getC()
                        : "<decompile failed: " + result.getErrorMessage() + ">";
                    String pcode = result.decompileCompleted() && includePcode
                        ? renderPcode(result.getHighFunction())
                        : "";
                    String cfg = result.decompileCompleted() && includeCfg
                        ? renderCfg(result.getHighFunction())
                        : "";
                    sections.add(
                        "===== " + formatOffset(function.getEntryPoint()) + slotLabel + " " +
                        function.getName(true) + " =====\n" + body + pcode + cfg + "\n");
                }
            }
        } finally {
            decompiler.dispose();
        }

        Files.createDirectories(output.toAbsolutePath().getParent());
        Files.writeString(output, String.join("\n", sections), StandardCharsets.UTF_8);
        println("Wrote function probe: " + output.toAbsolutePath());
    }

    private Integer optionalIntegerEnvironment(String name) {
        String value = System.getenv(name);
        return value == null || value.isBlank() ? null : Integer.decode(value.strip());
    }

    private List<Integer> optionalIntegerListEnvironment(String name) {
        String value = System.getenv(name);
        if (value == null || value.isBlank()) {
            return List.of();
        }
        ArrayList<Integer> result = new ArrayList<>();
        for (String part : value.split(",")) {
            Matcher matcher = INTEGER_OR_RANGE.matcher(part);
            if (!matcher.matches()) {
                throw new IllegalArgumentException(
                    name + " contains an invalid slot or range: " + part.strip());
            }
            int first = Integer.decode(matcher.group(1));
            int last = matcher.group(2) == null ? first : Integer.decode(matcher.group(2));
            int step = first <= last ? 1 : -1;
            for (int slot = first;; slot += step) {
                result.add(slot);
                if (slot == last) {
                    break;
                }
            }
        }
        return List.copyOf(result);
    }

    private Address pointerAt(Address address) throws Exception {
        long pointer = getLong(address);
        return currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(pointer);
    }

    private String renderPcode(HighFunction highFunction) {
        StringBuilder output = new StringBuilder("\n----- SSA p-code -----\n");
        var operations = highFunction.getPcodeOps();
        while (operations.hasNext()) {
            PcodeOpAST operation = operations.next();
            output.append(formatOffset(operation.getSeqnum().getTarget()))
                .append(' ')
                .append(operation)
                .append('\n');
        }
        return output.toString();
    }

    private String renderCfg(HighFunction highFunction) {
        StringBuilder output = new StringBuilder("\n----- High p-code CFG -----\n");
        ArrayList<PcodeBlockBasic> blocks = highFunction.getBasicBlocks();
        blocks.sort((left, right) -> left.getStart().compareTo(right.getStart()));
        for (PcodeBlockBasic block : blocks) {
            output.append("block ")
                .append(block.getIndex())
                .append(" start=")
                .append(formatOffset(block.getStart()))
                .append(" in=")
                .append(blockIndices(block, false))
                .append(" out=")
                .append(blockIndices(block, true))
                .append('\n');
            var operations = block.getIterator();
            while (operations.hasNext()) {
                PcodeOp operation = operations.next();
                output.append("  ")
                    .append(formatOffset(operation.getSeqnum().getTarget()))
                    .append(' ')
                    .append(operation.getMnemonic())
                    .append('\n');
            }
        }
        return output.toString();
    }

    private List<Integer> blockIndices(PcodeBlockBasic block, boolean successors) {
        int count = successors ? block.getOutSize() : block.getInSize();
        ArrayList<Integer> result = new ArrayList<>(count);
        for (int index = 0; index < count; index++) {
            PcodeBlock edge = successors ? block.getOut(index) : block.getIn(index);
            PcodeBlock leaf = edge == null ? null : edge.getFrontLeaf();
            if (leaf instanceof PcodeBlockBasic basic) {
                result.add(basic.getIndex());
            }
        }
        return List.copyOf(result);
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
