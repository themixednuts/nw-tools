// Deterministic text utilities used by NetworkSchemaExtractor.

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

final class NetworkSchemaText {
    private NetworkSchemaText() {}

    static String sourceTypeLeaf(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        String trimmed = value.trim();
        while (trimmed.endsWith("*") || trimmed.endsWith("&")) {
            trimmed = trimmed.substring(0, trimmed.length() - 1).trim();
        }
        int namespace = trimmed.lastIndexOf("::");
        return namespace >= 0 ? trimmed.substring(namespace + 2) : trimmed;
    }

    static boolean isCodecHelperOwnerName(String value) {
        String leaf = sourceTypeLeaf(value);
        if (leaf == null || leaf.isEmpty()) {
            return false;
        }
        String lower = leaf.toLowerCase(Locale.ROOT);
        return lower.endsWith("marshaler") ||
                lower.endsWith("marshaller") ||
                lower.endsWith("compressor") ||
                lower.endsWith("compressorbase");
    }

    static String normalizedExpression(String value) {
        if (value == null) {
            return null;
        }
        String result = value.trim();
        boolean changed;
        do {
            changed = false;
            while (result.startsWith("&")) {
                result = result.substring(1).trim();
                changed = true;
            }
            while (result.startsWith("(")) {
                int end = matchingIndex(result, 0, '(', ')');
                if (end <= 0) {
                    break;
                }
                String inner = result.substring(1, end).trim();
                if (isLikelyCastType(inner)) {
                    result = result.substring(end + 1).trim();
                    changed = true;
                    continue;
                }
                if (end == result.length() - 1) {
                    result = inner;
                    changed = true;
                    continue;
                }
                break;
            }
        } while (changed);
        return result.replaceAll("\\s+", " ");
    }

    static boolean isLikelyCastType(String value) {
        if (value == null || value.isEmpty()) {
            return false;
        }
        return value.contains("*") || value.contains("string") || value.contains("unordered_map")
                || value.startsWith("undefined") || value.startsWith("byte")
                || value.startsWith("bool") || value.startsWith("longlong")
                || value.startsWith("ulonglong");
    }

    static int matchingIndex(String text, int start, char open, char close) {
        if (text == null || start < 0 || start >= text.length() || text.charAt(start) != open) {
            return -1;
        }
        int depth = 0;
        for (int i = start; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c == open) {
                depth++;
            } else if (c == close) {
                depth--;
                if (depth == 0) {
                    return i;
                }
            }
        }
        return -1;
    }

    static List<String> splitTopLevel(String value) {
        ArrayList<String> result = new ArrayList<>();
        if (value == null || value.isEmpty()) {
            return result;
        }
        int angleDepth = 0;
        int parenDepth = 0;
        int bracketDepth = 0;
        int start = 0;
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '<') {
                angleDepth++;
            } else if (c == '>') {
                angleDepth = Math.max(0, angleDepth - 1);
            } else if (c == '(') {
                parenDepth++;
            } else if (c == ')') {
                parenDepth = Math.max(0, parenDepth - 1);
            } else if (c == '[') {
                bracketDepth++;
            } else if (c == ']') {
                bracketDepth = Math.max(0, bracketDepth - 1);
            } else if (c == ',' && angleDepth == 0 && parenDepth == 0 && bracketDepth == 0) {
                String part = value.substring(start, i).trim();
                if (!part.isEmpty()) {
                    result.add(part);
                }
                start = i + 1;
            }
        }
        String tail = value.substring(start).trim();
        if (!tail.isEmpty()) {
            result.add(tail);
        }
        return result;
    }

    static Long parseIntegerLiteral(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim().replace("_", "");
        try {
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                return Long.parseUnsignedLong(trimmed.substring(2), 16);
            }
            return Long.parseLong(trimmed);
        } catch (NumberFormatException ignored) {
            return null;
        }
    }

    static Long parseSignedIntegerLiteral(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim().replace("_", "");
        if (trimmed.isEmpty()) {
            return null;
        }

        boolean negative = trimmed.startsWith("-");
        if (negative) {
            trimmed = trimmed.substring(1);
        }
        Long parsed = parseIntegerLiteral(trimmed);
        if (parsed == null) {
            return null;
        }
        if (!negative) {
            return parsed;
        }
        if (parsed == Long.MIN_VALUE) {
            return Long.MIN_VALUE;
        }
        if (parsed < 0) {
            return null;
        }
        return -parsed;
    }
}
