// Decompiled-text parser and parse-cache owner for NetworkSchemaExtractor.

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

final class NetworkSchemaTextParser {
    private static final Pattern BOOL_POINTER_WRITE_RE =
            Pattern.compile("\\*\\(bool \\*\\)\\s*(?<target>[A-Za-z_][A-Za-z0-9_]*)\\s*=");

    private final int cacheLimit;
    private final Map<String, TextParseCacheEntry> textCache;

    NetworkSchemaTextParser(int cacheLimit) {
        this.cacheLimit = Math.max(0, cacheLimit);
        textCache = boundedLruCache(this.cacheLimit);
    }

    int cacheSize() {
        return textCache.size();
    }

    int cacheLimit() {
        return cacheLimit;
    }

    void clear() {
        textCache.clear();
    }

    boolean hasBoolPointerWrite(String text) {
        return text != null && BOOL_POINTER_WRITE_RE.matcher(text).find();
    }

    List<String> boolPointerWriteTargets(String text) {
        ArrayList<String> targets = new ArrayList<>();
        if (text == null) {
            return targets;
        }
        Matcher matcher = BOOL_POINTER_WRITE_RE.matcher(text);
        while (matcher.find()) {
            targets.add(matcher.group("target"));
        }
        return targets;
    }

    private static <K, V> Map<K, V> boundedLruCache(int maxEntries) {
        if (maxEntries <= 0) {
            return new HashMap<>();
        }
        return new LinkedHashMap<K, V>(Math.min(1024, maxEntries), 0.75f, true) {
            @Override
            protected boolean removeEldestEntry(Map.Entry<K, V> eldest) {
                return size() > maxEntries;
            }
        };
    }

    Set<Integer> boolParameterIndices(String decompiledText) {
        LinkedHashSet<Integer> result = new LinkedHashSet<>();
        if (decompiledText == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(decompiledText);
        if (entry.boolParameterIndices != null) {
            return entry.boolParameterIndices;
        }
        List<String> parameterNames = parameterNamesFromDecompiledFunction(decompiledText);
        if (parameterNames.isEmpty()) {
            entry.boolParameterIndices = immutableIntegerSet(result);
            return entry.boolParameterIndices;
        }
        HashMap<String, Integer> parameterIndex = new HashMap<>();
        for (int i = 0; i < parameterNames.size(); i++) {
            parameterIndex.put(parameterNames.get(i), i);
        }

        Matcher matcher = BOOL_POINTER_WRITE_RE.matcher(decompiledText);
        while (matcher.find()) {
            Integer index = parameterIndex.get(matcher.group("target"));
            if (index != null) {
                result.add(index);
            }
        }
        entry.boolParameterIndices = immutableIntegerSet(result);
        return entry.boolParameterIndices;
    }

    List<String> parameterNamesFromDecompiledFunction(String decompiledText) {
        ArrayList<String> result = new ArrayList<>();
        if (decompiledText == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(decompiledText);
        if (entry.parameterNames != null) {
            return entry.parameterNames;
        }
        int bodyStart = decompiledText.indexOf('{');
        int argsEnd = bodyStart < 0 ? decompiledText.lastIndexOf(')')
                                    : decompiledText.lastIndexOf(')', bodyStart);
        if (argsEnd < 0) {
            entry.parameterNames = immutableStringList(result);
            return entry.parameterNames;
        }
        int argsStart = decompiledText.lastIndexOf('(', argsEnd);
        if (argsStart < 0) {
            entry.parameterNames = immutableStringList(result);
            return entry.parameterNames;
        }
        for (String parameter :
                NetworkSchemaText.splitTopLevel(decompiledText.substring(argsStart + 1, argsEnd))) {
            String name = parameterName(parameter);
            if (name != null) {
                result.add(name);
            }
        }
        entry.parameterNames = immutableStringList(result);
        return entry.parameterNames;
    }

    private String parameterName(String parameter) {
        if (parameter == null) {
            return null;
        }
        String trimmed = parameter.trim();
        if (trimmed.isEmpty() || "void".equals(trimmed)) {
            return null;
        }
        int index = trimmed.length() - 1;
        while (index >= 0
                && (Character.isLetterOrDigit(trimmed.charAt(index))
                        || trimmed.charAt(index) == '_')) {
            index--;
        }
        if (index == trimmed.length() - 1) {
            return null;
        }
        return trimmed.substring(index + 1);
    }

    private TextParseCacheEntry cacheEntry(String text) {
        return textCache.computeIfAbsent(text, ignored -> new TextParseCacheEntry());
    }

    private List<String> immutableStringList(ArrayList<String> value) {
        return Collections.unmodifiableList(new ArrayList<>(value));
    }

    private Set<Integer> immutableIntegerSet(LinkedHashSet<Integer> value) {
        return Collections.unmodifiableSet(new LinkedHashSet<>(value));
    }

    private List<ParsedUnmarshalCall> immutableParsedUnmarshalCalls(
            ArrayList<ParsedUnmarshalCall> value) {
        return Collections.unmodifiableList(new ArrayList<>(value));
    }

    List<ParsedUnmarshalCall> parseUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(text);
        if (entry.unmarshalCalls != null) {
            return entry.unmarshalCalls;
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf("Unmarshal<", search);
            if (nameIndex < 0) {
                break;
            }
            int templateStart = text.indexOf('<', nameIndex);
            int templateEnd = NetworkSchemaText.matchingIndex(text, templateStart, '<', '>');
            int argsStart = text.indexOf('(', templateEnd);
            int argsEnd = NetworkSchemaText.matchingIndex(text, argsStart, '(', ')');
            if (templateStart < 0 || templateEnd < 0 || argsStart < 0 || argsEnd < 0) {
                search = nameIndex + "Unmarshal<".length();
                continue;
            }
            String templateType = text.substring(templateStart + 1, templateEnd).trim();
            result.add(new ParsedUnmarshalCall(templateType, null, nameIndex,
                    NetworkSchemaText.splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            search = argsEnd + 1;
        }
        entry.unmarshalCalls = immutableParsedUnmarshalCalls(result);
        return entry.unmarshalCalls;
    }

    List<ParsedUnmarshalCall> parseMarshalerUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(text);
        if (entry.marshalerUnmarshalCalls != null) {
            return entry.marshalerUnmarshalCalls;
        }
        int search = 0;
        while (search < text.length()) {
            int marshalerIndex = text.indexOf("Marshaler<", search);
            int marshallerIndex = text.indexOf("Marshaller<", search);
            if (marshalerIndex < 0 && marshallerIndex < 0) {
                break;
            }
            boolean useMarshaller = marshalerIndex < 0
                    || (marshallerIndex >= 0 && marshallerIndex < marshalerIndex);
            int nameIndex = useMarshaller ? marshallerIndex : marshalerIndex;
            String marker = useMarshaller ? "Marshaller<" : "Marshaler<";
            int templateStart = text.indexOf('<', nameIndex);
            int templateEnd = NetworkSchemaText.matchingIndex(text, templateStart, '<', '>');
            int unmarshalIndex = templateEnd < 0 ? -1 : text.indexOf("::Unmarshal", templateEnd);
            int argsStart = unmarshalIndex < 0 ? -1 : text.indexOf('(', unmarshalIndex);
            int argsEnd = NetworkSchemaText.matchingIndex(text, argsStart, '(', ')');
            if (templateStart < 0 || templateEnd < 0 || unmarshalIndex < 0 || argsStart < 0
                    || argsEnd < 0) {
                search = nameIndex + marker.length();
                continue;
            }

            String templateType = text.substring(templateStart + 1, templateEnd).trim();
            int ownerStart = directCallOwnerStart(text, unmarshalIndex);
            String functionName = ownerStart < 0
                    ? marker.substring(0, marker.length() - 1) + "<" + templateType + ">::Unmarshal"
                    : text.substring(ownerStart, unmarshalIndex).trim() + "::Unmarshal";
            result.add(new ParsedUnmarshalCall(templateType, functionName, nameIndex,
                    NetworkSchemaText.splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            search = argsEnd + 1;
        }
        entry.marshalerUnmarshalCalls = immutableParsedUnmarshalCalls(result);
        return entry.marshalerUnmarshalCalls;
    }

    List<ParsedUnmarshalCall> parseDirectTypeUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(text);
        if (entry.directTypeUnmarshalCalls != null) {
            return entry.directTypeUnmarshalCalls;
        }
        int search = 0;
        while (search < text.length()) {
            int unmarshalIndex = text.indexOf("::Unmarshal(", search);
            if (unmarshalIndex < 0) {
                break;
            }
            int ownerStart = directCallOwnerStart(text, unmarshalIndex);
            int argsStart = unmarshalIndex + "::Unmarshal".length();
            int argsEnd = NetworkSchemaText.matchingIndex(text, argsStart, '(', ')');
            if (ownerStart < 0 || argsEnd < 0) {
                search = unmarshalIndex + "::Unmarshal(".length();
                continue;
            }

            String owner = text.substring(ownerStart, unmarshalIndex).trim();
            if (owner.isEmpty() || owner.contains("Marshaler<") ||
                    owner.contains("Marshaller<") ||
                    NetworkSchemaText.isCodecHelperOwnerName(owner)) {
                search = argsEnd + 1;
                continue;
            }

            result.add(new ParsedUnmarshalCall(NetworkSchemaText.sourceTypeLeaf(owner),
                    owner + "::Unmarshal", unmarshalIndex,
                    NetworkSchemaText.splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            search = argsEnd + 1;
        }
        entry.directTypeUnmarshalCalls = immutableParsedUnmarshalCalls(result);
        return entry.directTypeUnmarshalCalls;
    }

    List<ParsedUnmarshalCall> parseCodecUnmarshalCalls(String text) {
        ArrayList<ParsedUnmarshalCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(text);
        if (entry.codecUnmarshalCalls != null) {
            return entry.codecUnmarshalCalls;
        }
        int search = 0;
        while (search < text.length()) {
            int unmarshalIndex = text.indexOf("::Unmarshal(", search);
            if (unmarshalIndex < 0) {
                break;
            }
            int ownerStart = directCallOwnerStart(text, unmarshalIndex);
            int argsStart = unmarshalIndex + "::Unmarshal".length();
            int argsEnd = NetworkSchemaText.matchingIndex(text, argsStart, '(', ')');
            if (ownerStart < 0 || argsEnd < 0) {
                search = unmarshalIndex + "::Unmarshal(".length();
                continue;
            }

            String owner = text.substring(ownerStart, unmarshalIndex).trim();
            if (NetworkSchemaText.isCodecHelperOwnerName(owner)) {
                result.add(new ParsedUnmarshalCall(
                    NetworkSchemaText.sourceTypeLeaf(owner),
                    owner + "::Unmarshal",
                    unmarshalIndex,
                    NetworkSchemaText.splitTopLevel(text.substring(argsStart + 1, argsEnd))));
            }
            search = argsEnd + 1;
        }
        entry.codecUnmarshalCalls = immutableParsedUnmarshalCalls(result);
        return entry.codecUnmarshalCalls;
    }

    List<ParsedReadRawCall> parseReadRawCalls(String text) {
        ArrayList<ParsedReadRawCall> result = new ArrayList<>();
        if (text == null) {
            return result;
        }
        TextParseCacheEntry entry = cacheEntry(text);
        if (entry.readRawCalls != null) {
            return entry.readRawCalls;
        }
        int search = 0;
        while (search < text.length()) {
            int nameIndex = text.indexOf("ReadRaw", search);
            if (nameIndex < 0) {
                break;
            }
            int afterName = nameIndex + "ReadRaw".length();
            if (afterName < text.length()) {
                char next = text.charAt(afterName);
                if (Character.isLetterOrDigit(next) || next == '_') {
                    search = afterName;
                    continue;
                }
            }
            int argsStart = text.indexOf('(', afterName);
            int argsEnd = NetworkSchemaText.matchingIndex(text, argsStart, '(', ')');
            if (argsStart < 0 || argsEnd < 0) {
                search = afterName;
                continue;
            }

            List<String> args =
                    NetworkSchemaText.splitTopLevel(text.substring(argsStart + 1, argsEnd));
            int storageIndex = args.size() >= 4 ? 1 : 0;
            int lengthIndex = storageIndex + 1;
            if (lengthIndex < args.size()) {
                Integer byteLength = readRawByteLength(args.get(lengthIndex));
                if (byteLength != null && byteLength > 0) {
                    result.add(
                            new ParsedReadRawCall(args.get(storageIndex), byteLength, nameIndex));
                }
            }
            search = argsEnd + 1;
        }

        entry.readRawCalls = Collections.unmodifiableList(new ArrayList<>(result));
        return entry.readRawCalls;
    }

    private Integer readRawByteLength(String expression) {
        if (expression == null) {
            return null;
        }
        String value = NetworkSchemaText.normalizedExpression(expression);
        if (value == null) {
            return null;
        }
        value = value.replaceAll("(?i)[uUlL]+$", "");
        Long parsed = NetworkSchemaText.parseIntegerLiteral(value);
        if (parsed == null || parsed <= 0 || parsed > Integer.MAX_VALUE) {
            return null;
        }
        return parsed.intValue();
    }

    private int directCallOwnerStart(String text, int unmarshalIndex) {
        int index = unmarshalIndex - 1;
        while (index >= 0) {
            char c = text.charAt(index);
            if (Character.isLetterOrDigit(c) || c == '_' || c == ':' || c == '<' || c == '>'
                    || c == ',' || Character.isWhitespace(c)) {
                index--;
                continue;
            }
            break;
        }
        return index + 1;
    }

    String storageArgumentForMarshalerCall(ParsedUnmarshalCall call) {
        if (call == null || call.args.size() < 3) {
            return null;
        }
        if (call.functionName != null && call.functionName.contains("Marshaller<")
                && call.args.size() == 3) {
            return call.args.get(2);
        }
        return call.args.get(call.args.size() - 2);
    }

    String storageArgumentForDirectUnmarshalCall(ParsedUnmarshalCall call) {
        if (call == null || call.args.size() < 2) {
            return null;
        }
        return call.args.get(call.args.size() - 2);
    }

    ParsedUnmarshalFieldsCall parseUnmarshalFieldsCall(String text) {
        if (text == null) {
            return null;
        }
        int nameIndex = text.indexOf("UnmarshalFields<");
        if (nameIndex < 0) {
            return null;
        }
        int templateStart = text.indexOf('<', nameIndex);
        int templateEnd = NetworkSchemaText.matchingIndex(text, templateStart, '<', '>');
        if (templateStart < 0 || templateEnd < 0) {
            return null;
        }
        int argsStart = text.indexOf('(', templateEnd);
        int argsEnd = NetworkSchemaText.matchingIndex(text, argsStart, '(', ')');
        if (argsStart < 0 || argsEnd < 0) {
            return null;
        }

        ParsedUnmarshalFieldsCall call = new ParsedUnmarshalFieldsCall();
        call.templateTypes.addAll(
                NetworkSchemaText.splitTopLevel(text.substring(templateStart + 1, templateEnd)));
        List<String> args = NetworkSchemaText.splitTopLevel(text.substring(argsStart + 1, argsEnd));
        for (int i = 2; i < args.size(); i++) {
            call.fieldArgs.add(parseArgument(args.get(i)));
        }
        return call.fieldArgs.isEmpty() ? null : call;
    }

    ParsedArgument parseArgument(String value) {
        ParsedArgument result = new ParsedArgument();
        String trimmed = value == null ? "" : value.trim();
        if (trimmed.startsWith("(")) {
            int end = NetworkSchemaText.matchingIndex(trimmed, 0, '(', ')');
            if (end > 0) {
                String cast = trimmed.substring(1, end).trim();
                if (NetworkSchemaText.isLikelyCastType(cast)) {
                    result.castType = cast;
                    result.expression = trimmed.substring(end + 1).trim();
                    return result;
                }
            }
        }
        result.expression = trimmed;
        return result;
    }

    private static final class TextParseCacheEntry {
        List<String> parameterNames;
        Set<Integer> boolParameterIndices;
        List<ParsedUnmarshalCall> unmarshalCalls;
        List<ParsedUnmarshalCall> marshalerUnmarshalCalls;
        List<ParsedUnmarshalCall> directTypeUnmarshalCalls;
        List<ParsedUnmarshalCall> codecUnmarshalCalls;
        List<ParsedReadRawCall> readRawCalls;
    }
}
