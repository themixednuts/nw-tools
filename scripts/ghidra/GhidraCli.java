// Shared strict command-line and environment parsing for New World Ghidra scripts.

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

final class GhidraCli {
    private static final String BOOLEAN_VALUES =
        "1, true, yes, on, 0, false, no, or off";

    private final Map<String, String> options;
    private final List<String> positionals;
    private final boolean helpRequested;
    private final boolean versionRequested;

    private GhidraCli(
        Map<String, String> options,
        List<String> positionals,
        boolean helpRequested,
        boolean versionRequested) {

        this.options = Map.copyOf(options);
        this.positionals = List.copyOf(positionals);
        this.helpRequested = helpRequested;
        this.versionRequested = versionRequested;
    }

    static GhidraCli parse(
        String[] arguments,
        Set<String> valueOptions,
        Set<String> booleanOptions,
        int maximumPositionals) {

        Map<String, String> options = new HashMap<>();
        ArrayList<String> positionals = new ArrayList<>();
        boolean help = false;
        boolean version = false;
        boolean positionalOnly = false;
        for (int index = 0; index < arguments.length; index++) {
            String argument = arguments[index];
            if ("--help".equals(argument) || "-h".equals(argument)) {
                help = true;
                continue;
            }
            if ("--version".equals(argument) || "-V".equals(argument)) {
                version = true;
                continue;
            }
            if (positionalOnly || !argument.startsWith("--")) {
                positionals.add(argument);
                continue;
            }
            if ("--".equals(argument)) {
                positionalOnly = true;
                continue;
            }
            String token = argument.substring(2);
            String inlineValue = null;
            int equals = token.indexOf('=');
            if (equals >= 0) {
                inlineValue = token.substring(equals + 1);
                token = token.substring(0, equals);
            }
            if (token.startsWith("no-") && booleanOptions.contains(token.substring(3))) {
                if (inlineValue != null) {
                    throw new IllegalArgumentException("--" + token + " does not take a value");
                }
                options.put(token.substring(3), "false");
            }
            else if (booleanOptions.contains(token)) {
                options.put(token, inlineValue == null ? "true" : inlineValue);
            }
            else if (valueOptions.contains(token)) {
                String value = inlineValue;
                if (value == null) {
                    if (++index >= arguments.length) {
                        throw new IllegalArgumentException("--" + token + " requires a value");
                    }
                    value = arguments[index];
                }
                options.put(token, value);
            }
            else {
                throw new IllegalArgumentException("unknown option: --" + token);
            }
        }
        if (positionals.size() > maximumPositionals) {
            throw new IllegalArgumentException(
                "expected at most " + maximumPositionals + " positional argument(s), got " +
                    positionals.size());
        }
        return new GhidraCli(options, positionals, help, version);
    }

    boolean helpRequested() {
        return helpRequested;
    }

    boolean versionRequested() {
        return versionRequested;
    }

    List<String> positionals() {
        return positionals;
    }

    String value(String option, String environment) {
        String value = options.get(option);
        if (value == null && environment != null) {
            value = System.getenv(environment);
        }
        return value == null || value.isBlank() ? null : value.strip();
    }

    String required(String option, String environment) {
        String value = value(option, environment);
        if (value == null) {
            throw new IllegalArgumentException(
                "--" + option + " is required [env: " + environment + "]");
        }
        return value;
    }

    boolean flag(String option, String environment, boolean defaultValue) {
        String value = value(option, environment);
        return value == null ? defaultValue : parseBoolean(value, "--" + option);
    }

    int nonNegativeInt(String option, String environment, int defaultValue) {
        String value = value(option, environment);
        if (value == null) {
            return defaultValue;
        }
        try {
            int parsed = Integer.parseInt(value);
            if (parsed < 0) {
                throw new NumberFormatException("negative value");
            }
            return parsed;
        }
        catch (NumberFormatException error) {
            throw new IllegalArgumentException(
                "--" + option + " expects a non-negative integer", error);
        }
    }

    static boolean parseBoolean(String value, String source) {
        return switch (value.toLowerCase(Locale.ROOT)) {
            case "1", "true", "yes", "on" -> true;
            case "0", "false", "no", "off" -> false;
            default -> throw new IllegalArgumentException(
                source + " expects one of " + BOOLEAN_VALUES);
        };
    }
}
