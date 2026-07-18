import java.util.Set;

import ghidra.program.model.pcode.PcodeBlockBasic;
import ghidra.program.model.pcode.PcodeOpAST;

/** One natural loop recovered from a control-flow back edge. */
record NetworkSchemaNaturalLoop(
    PcodeBlockBasic header,
    Set<PcodeBlockBasic> body) {

    NetworkSchemaNaturalLoop {
        body = Set.copyOf(body);
    }

    boolean contains(PcodeBlockBasic block) {
        return block != null && body.contains(block);
    }

    boolean contains(PcodeOpAST operation) {
        return operation != null && operation.getParent() instanceof PcodeBlockBasic block &&
            contains(block);
    }

    int blockCount() {
        return body.size();
    }
}
