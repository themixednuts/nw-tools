// Structured wire-shape parsing and evidence comparison.

import java.util.ArrayList;
import java.util.List;

final class NetworkSchemaWireShape {
    private NetworkSchemaWireShape() {}

    sealed interface Node permits Atom, Application {}

    record Atom(String value) implements Node {}

    record Application(String name, List<Node> arguments) implements Node {
        Application {
            arguments = List.copyOf(arguments);
        }
    }

    static boolean equivalentBySemanticOrLayout(
        String expectedShape,
        String expectedLayout,
        String observedShape,
        String observedLayout) {

        Node expectedSemantic = parse(
            expectedShape == null ? expectedLayout : expectedShape);
        Node expectedPhysical = parse(expectedLayout);
        Node observedSemantic = parse(
            observedShape == null ? observedLayout : observedShape);
        Node observedPhysical = parse(observedLayout);
        return expectedSemantic != null && expectedPhysical != null &&
            observedSemantic != null && observedPhysical != null &&
            equivalent(
                expectedSemantic,
                expectedPhysical,
                observedSemantic,
                observedPhysical);
    }

    private static boolean equivalent(
        Node expectedSemantic,
        Node expectedPhysical,
        Node observedSemantic,
        Node observedPhysical) {

        if (expectedSemantic instanceof Application expected &&
            observedSemantic instanceof Application observed &&
            expected.name().equals(observed.name()) &&
            expected.arguments().size() == observed.arguments().size()) {

            List<Node> expectedLayouts = alignedArguments(expectedPhysical, expected);
            List<Node> observedLayouts = alignedArguments(observedPhysical, observed);
            for (int index = 0; index < expected.arguments().size(); index++) {
                if (!equivalent(
                        expected.arguments().get(index),
                        expectedLayouts.get(index),
                        observed.arguments().get(index),
                        observedLayouts.get(index))) {
                    return false;
                }
            }
            return true;
        }
        return semanticLeafEquivalent(expectedSemantic, observedSemantic) ||
            expectedPhysical.equals(observedPhysical);
    }

    private static List<Node> alignedArguments(Node layout, Application semantic) {
        if (layout instanceof Application application &&
            application.name().equals(semantic.name()) &&
            application.arguments().size() == semantic.arguments().size()) {
            return application.arguments();
        }
        return semantic.arguments();
    }

    private static boolean semanticLeafEquivalent(Node expected, Node observed) {
        if (expected.equals(observed)) {
            return true;
        }
        if (!(expected instanceof Atom expectedAtom) ||
            !(observed instanceof Atom observedAtom)) {
            return false;
        }
        return isBooleanByte(expectedAtom.value()) &&
            isBooleanByte(observedAtom.value());
    }

    private static boolean isBooleanByte(String value) {
        return "bool".equals(value) || "u8".equals(value);
    }

    static Node parse(String value) {
        if (value == null) {
            return null;
        }
        Parser parser = new Parser(value);
        Node node = parser.parseNode();
        parser.skipWhitespace();
        return node != null && parser.atEnd() ? node : null;
    }

    private static final class Parser {
        private final String input;
        private int position;

        Parser(String input) {
            this.input = input;
        }

        Node parseNode() {
            skipWhitespace();
            String name = parseName();
            if (name == null) {
                return null;
            }
            skipWhitespace();
            if (atEnd() || input.charAt(position) != '<') {
                return new Atom(name);
            }
            position++;
            ArrayList<Node> arguments = new ArrayList<>();
            while (true) {
                Node argument = parseNode();
                if (argument == null) {
                    return null;
                }
                arguments.add(argument);
                skipWhitespace();
                if (atEnd()) {
                    return null;
                }
                char delimiter = input.charAt(position++);
                if (delimiter == '>') {
                    return new Application(name, arguments);
                }
                if (delimiter != ',') {
                    return null;
                }
            }
        }

        String parseName() {
            int start = position;
            while (!atEnd()) {
                char current = input.charAt(position);
                if (current == '<' || current == '>' || current == ',' ||
                    Character.isWhitespace(current)) {
                    break;
                }
                position++;
            }
            return start == position ? null : input.substring(start, position);
        }

        void skipWhitespace() {
            while (!atEnd() && Character.isWhitespace(input.charAt(position))) {
                position++;
            }
        }

        boolean atEnd() {
            return position == input.length();
        }
    }
}
