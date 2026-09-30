# Leftover XML is searchable as complete facts

Unclaimed pak `.xml` stays searchable. Mannequin and ATL keep their own extract; the generic walk is only for entries they did not claim. Every element name, attribute name, attribute value, and text or CDATA node is a fact; `field` is the element path (`Root/Item/@id` for an attribute). Comments and processing instructions are out. A failed parse still recovers quoted strings and element-ish names and text so a broken file is not a silent miss. Index size is accepted; dropping leftover XML or indexing only some of each file is rejected.
