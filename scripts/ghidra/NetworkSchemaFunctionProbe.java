// Decompile explicit module-relative functions for network-schema research.
// @category NewWorld

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
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
    private static final String SCRIPT_VERSION = "1.0.0";
    private static final Pattern INTEGER_OR_RANGE = Pattern.compile(
        "\\s*([+-]?(?:0[xX][0-9a-fA-F]+|[0-9]+))" +
        "(?:\\s*-\\s*([+-]?(?:0[xX][0-9a-fA-F]+|[0-9]+)))?\\s*");

    @Override
    protected void run() throws Exception {
        GhidraCli cli = GhidraCli.parse(
            getScriptArgs(),
            Set.of("offsets", "out", "vtable-slots"),
            Set.of("pcode", "cfg", "force", "dry-run"),
            0);
        if (cli.helpRequested()) {
            printHelp();
            return;
        }
        if (cli.versionRequested()) {
            println("NetworkSchemaFunctionProbe " + SCRIPT_VERSION);
            return;
        }
        String rawOffsets = cli.required("offsets", "NW_NETWORK_PROBE_OFFSETS");
        Path output = Path.of(cli.required("out", "NW_NETWORK_PROBE_OUT"));
        boolean includePcode = cli.flag("pcode", "NW_NETWORK_PROBE_PCODE", false);
        boolean includeCfg = cli.flag("cfg", "NW_NETWORK_PROBE_CFG", false);
        boolean force = cli.flag("force", null, false);
        boolean dryRun = cli.flag("dry-run", null, false);
        if (Files.exists(output) && !force && !dryRun) {
            throw new IllegalArgumentException(
                "output already exists: " + output + "; pass --force to replace it");
        }
        List<Integer> vtableSlots = optionalIntegerList(
            cli.value("vtable-slots", "NW_NETWORK_PROBE_VTABLE_SLOTS"),
            "--vtable-slots");
        vtableSlots = vtableSlots.isEmpty() ? List.of(-1) : vtableSlots;
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

        if (dryRun) {
            println("Dry run: would write " + sections.size() + " function section(s) to " +
                output.toAbsolutePath());
        }
        else {
            Files.createDirectories(output.toAbsolutePath().getParent());
            Files.writeString(output, String.join("\n", sections), StandardCharsets.UTF_8);
            println("Wrote function probe: " + output.toAbsolutePath());
        }
    }

    private List<Integer> optionalIntegerList(String value, String source) {
        if (value == null || value.isBlank()) {
            return List.of();
        }
        ArrayList<Integer> result = new ArrayList<>();
        for (String part : value.split(",")) {
            Matcher matcher = INTEGER_OR_RANGE.matcher(part);
            if (!matcher.matches()) {
                throw new IllegalArgumentException(
                    source + " contains an invalid slot or range: " + part.strip());
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
        println("NetworkSchemaFunctionProbe " + SCRIPT_VERSION);
        println("Decompile explicit module-relative functions for network-schema research.");
        println("");
        println("Options:");
        println("  --offsets <LIST>       Comma-separated module offsets [env: NW_NETWORK_PROBE_OFFSETS]");
        println("  --out <FILE>           Output report [env: NW_NETWORK_PROBE_OUT]");
        println("  --pcode[=<BOOL>]       Include SSA p-code [env: NW_NETWORK_PROBE_PCODE]");
        println("  --cfg[=<BOOL>]         Include control-flow graph [env: NW_NETWORK_PROBE_CFG]");
        println("  --vtable-slots <LIST>  Slots or inclusive ranges [env: NW_NETWORK_PROBE_VTABLE_SLOTS]");
        println("  --force                Replace an existing output");
        println("  --dry-run              Analyze without filesystem writes");
        println("  -h, --help             Print help");
        println("  -V, --version          Print version");
    }
}
