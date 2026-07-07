// Address formatter callback shared by NetworkSchemaExtractor helper classes.

import ghidra.program.model.address.Address;

@FunctionalInterface
interface NetworkSchemaAddressFormatter {
    String format(Address address);
}
