// x86 instruction/operand helpers for stack-based NetworkSchemaExtractor
// analysis.

import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.scalar.Scalar;
import java.util.ArrayList;
import java.util.Locale;

final class NetworkSchemaX86 {
    private NetworkSchemaX86() {}

    static Integer addOffsets(int base, long delta) {
        long value = (long) base + delta;
        if (value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) {
            return null;
        }
        return (int) value;
    }

    static Long immediateValue(Instruction instruction, int operandIndex) {
        for (Object object : operandObjects(instruction, operandIndex)) {
            if (object instanceof Scalar scalar) {
                return scalar.getUnsignedValue();
            }
        }
        return null;
    }

    static Long signedImmediateValue(Instruction instruction, int operandIndex) {
        for (Object object : operandObjects(instruction, operandIndex)) {
            if (object instanceof Scalar scalar) {
                return scalar.getSignedValue();
            }
        }
        return null;
    }

    static String operandText(Instruction instruction, int operandIndex) {
        try {
            return instruction.getDefaultOperandRepresentation(operandIndex);
        } catch (Exception ignored) {
            return null;
        }
    }

    static Object[] operandObjects(Instruction instruction, int operandIndex) {
        try {
            return instruction.getOpObjects(operandIndex);
        } catch (Exception ignored) {
            return new Object[0];
        }
    }

    static MemoryReference memoryReference(Instruction instruction, int operandIndex) {
        MemoryAddress memory = memoryAddress(instruction, operandIndex);
        if (memory == null || memory.terms.size() != 1) {
            return null;
        }
        MemoryTerm term = memory.terms.get(0);
        return term.scale == 1 ? new MemoryReference(term.register, memory.displacement) : null;
    }

    static MemoryAddress memoryAddress(Instruction instruction, int operandIndex) {
        String text = operandText(instruction, operandIndex);
        if (text == null || !text.contains("[") || !text.contains("]")) {
            return null;
        }

        int start = text.indexOf('[');
        int end = text.lastIndexOf(']');
        if (end <= start) {
            return null;
        }

        ArrayList<MemoryTerm> terms = new ArrayList<>();
        int displacement = 0;
        String inside = text.substring(start + 1, end).replace(" ", "").replace("-", "+-");
        for (String token : inside.split("\\+")) {
            if (token == null || token.isEmpty()) {
                continue;
            }

            MemoryTerm term = parseMemoryTerm(token);
            if (term != null) {
                terms.add(term);
                continue;
            }

            Long value = NetworkSchemaText.parseSignedIntegerLiteral(token);
            if (value == null || value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) {
                return null;
            }
            Integer nextDisplacement = addOffsets(displacement, value);
            if (nextDisplacement == null) {
                return null;
            }
            displacement = nextDisplacement;
        }

        return terms.isEmpty() ? null : new MemoryAddress(terms, displacement);
    }

    private static MemoryTerm parseMemoryTerm(String token) {
        String[] parts = token.split("\\*", -1);
        if (parts.length == 0 || parts.length > 2) {
            return null;
        }

        String register = canonicalRegisterName(parts[0]);
        if (!isKnownRegisterName(register)) {
            return null;
        }

        int scale = 1;
        if (parts.length == 2) {
            Long parsedScale = NetworkSchemaText.parseSignedIntegerLiteral(parts[1]);
            if (parsedScale == null
                    || (parsedScale != 1L && parsedScale != 2L && parsedScale != 4L
                            && parsedScale != 8L)) {
                return null;
            }
            scale = parsedScale.intValue();
        }
        return new MemoryTerm(register, scale);
    }

    private static boolean isKnownRegisterName(String register) {
        return register != null
                && ("RAX".equals(register) || "RBX".equals(register) || "RCX".equals(register)
                        || "RDX".equals(register) || "RSI".equals(register)
                        || "RDI".equals(register) || "RBP".equals(register)
                        || "RSP".equals(register) || "RIP".equals(register)
                        || register.matches("R(?:[8-9]|1[0-5])"));
    }

    static String registerOperand(Instruction instruction, int operandIndex) {
        String operandText = operandText(instruction, operandIndex);
        if (operandText != null && operandText.contains("[")) {
            return null;
        }
        Object[] objects = operandObjects(instruction, operandIndex);
        if (objects.length != 1 || !(objects[0] instanceof Register register)) {
            return null;
        }
        return canonicalRegisterName(register.getName());
    }

    static String canonicalRegisterName(String name) {
        if (name == null) {
            return null;
        }
        String upper = name.toUpperCase(Locale.ROOT);
        if (upper.length() == 2 && upper.charAt(1) == 'X') {
            return "R" + upper;
        }
        if (upper.length() == 3 && upper.charAt(0) == 'E') {
            return "R" + upper.substring(1);
        }
        if (upper.startsWith("R") && upper.endsWith("D")) {
            return upper.substring(0, upper.length() - 1);
        }
        if (upper.matches("R(?:[8-9]|1[0-5])[BW]")) {
            return upper.substring(0, upper.length() - 1);
        }
        if (upper.matches("R(?:[8-9]|1[0-5])W")) {
            return upper.substring(0, upper.length() - 1);
        }
        if ("AL".equals(upper) || "AH".equals(upper) || "AX".equals(upper) || "EAX".equals(upper)) {
            return "RAX";
        }
        if ("BL".equals(upper) || "BH".equals(upper) || "BX".equals(upper) || "EBX".equals(upper)) {
            return "RBX";
        }
        if ("CL".equals(upper) || "CH".equals(upper) || "CX".equals(upper) || "ECX".equals(upper)) {
            return "RCX";
        }
        if ("DL".equals(upper) || "DH".equals(upper) || "DX".equals(upper) || "EDX".equals(upper)) {
            return "RDX";
        }
        if ("DIL".equals(upper) || "EDI".equals(upper)) {
            return "RDI";
        }
        if ("SIL".equals(upper) || "ESI".equals(upper)) {
            return "RSI";
        }
        if ("BPL".equals(upper) || "EBP".equals(upper)) {
            return "RBP";
        }
        if ("SPL".equals(upper) || "ESP".equals(upper)) {
            return "RSP";
        }
        return upper;
    }

    static String upperMnemonic(Instruction instruction) {
        String mnemonic = instruction.getMnemonicString();
        return mnemonic == null ? null : mnemonic.toUpperCase(Locale.ROOT);
    }
}
