# Ghidra reflection import

`AzReflectionRenamer.java` is the single entrypoint for AZ/Lumberyard reflection
names, datatypes, layouts, adapter tables and callable APIs in NewWorld 3-26.

Install it with:

```powershell
scripts/ghidra/Sync-AzReflectionRenamer.ps1 -Force
```

The installer bundles the CLI helper and removes the obsolete
`AzSerializeContainers.java`, `AzRepairReflectionNames.java` and
`AzBuildReflectionDatatypes.java` entrypoints from the installed scripts folder.

In CodeBrowser, refresh Script Manager and run `AzReflectionRenamer.java`.
It uses the attached program, or rebinds to the tool's active program. With no
active program, it activates a uniquely matching NewWorld 3-26 already open in
the tool or opens that program from the current project. If the target is missing
or ambiguous, it offers a program chooser. Cancellation or a missing CodeBrowser
manager stops before import edits, with console guidance rather than an exception.
Activation uses Ghidra's script API to rebind state and start the transaction.
Open `E:\Projects\openworld\resources` in the folder picker and click
**Import reflection**. With no child selected, it imports the current directory.
Selecting a nested directory such as `resources/output` also resolves to the
capture root. `--resources` accepts a directory or a file inside that root.
The script first validates all reflection passes without program edits.
It saves `resources/reflection-dry-run.json` with phase results, error stacks,
action counts and samples, and rejected native code-pointer counts. A failed
validation stops before applying changes. When validation passes, the script applies
all passes and automatically writes
`resources/reflection-report.json`, replacing its previous report. Save
the program after reviewing the results.

It discovers these inputs:

- `serialize.json`
- `modules/*.json`
- `behavior-context.json`, or `behavior-context.json.zip`, or `behavior-context.7z`
- Optional `serialize-class-registration.jsonl` and `serialize-field-registration.jsonl`

Each capture is optional individually. Behavior-only and module-only directories
work. Plain BehaviorContext JSON takes precedence over an archive; ZIP and 7z
entries stream without extraction. Input errors stop before program edits,
including unreadable files, malformed registration-trace lines and explicit
`--module-evidence`, `--class-registration` or `--field-registration` paths that
do not exist. Module captures record only the module's vtable; its name comes from
that vtable's `RTTI_GetTypeName` slot in the binary, with a built-in table as
fallback. A module that neither names is counted in `unrecognizedInputs` and
skipped without blocking the import.
Registration-trace callsites and their owning functions are resolved once before
validation, so apply renames exactly what validation checked even after earlier
apply phases create functions.
TypeRegistry and previous output reports are excluded.
BehaviorContext null UUIDs remain raw evidence and are not resolved to a class
name, even when a serialized class explicitly uses that UUID.

For headless runs:

```text
--resources E:/Projects/openworld/resources
--resources E:/Projects/openworld/resources --dry-run
--resources E:/Projects/openworld/resources --apply-renames=false --out tmp/reflection-preview.json
```

`--dry-run` runs only validation and saves its diagnostics without program edits.
`--apply-renames=false` also stops after validation. The previous applied report
is preserved in both modes. `--out` overrides the applied report path; validation
is saved beside it as `<stem>-dry-run.json`, or `reflection-dry-run.json` for the
default report. `--force=false` requires that the applied report does not already
exist when applying changes.
`--serialize-json` still accepts an alternate SerializeContext file and discovers
its sibling captures. Run `--help` for all flags and environment bindings.

Validation exercises SerializeContext RTTI/callbacks, proposed datatype layouts,
containers, registration traces, modules, inferred native RTTI tables,
BehaviorContext APIs/buses/constants, MB names and namespace cleanup. It collects
phase exceptions and action/field errors, continues through independent validation
phases, and blocks application if any error was found. Diagnostics retain the
total error count and up to 100 error samples. Expected unresolved evidence stays
in action reason counts and samples. Zero, unmapped and non-executable native
function pointers are skipped before instruction lookup; sparse RTTI tables still
require enough executable slots and a verified type-name getter.

Function renames change the symbol name and namespace together through Ghidra's
atomic API. Existing aliases in the old namespace remain intact. An alias matching
the destination name at the same address is merged into the function symbol.
This also handles aliases added by an earlier pass in the same import without
colliding during a temporary rename in the old namespace.
The same qualified function or label name may occur at different addresses,
as Ghidra permits. Separate compiler bodies, RTTI tables and module copies keep
their exact C++ names; `sameQualifiedNameAtOtherAddress` records that reuse.
Legacy namespaces split inside a template argument are repaired from the full
scope, so `AZStd / ranged_enum<Javelin / TaxOrFeeType,_7>` becomes the single
class `AZStd::ranged_enum<Javelin::TaxOrFeeType,_7>`. Correctness checks compare
the local name and actual namespace ancestry: the flattened name alone can look
identical before and after a repair. Class and function IDs are preserved when
moving them in place. If the correct class already exists, member functions and
nested classes move to it where their destinations permit the move. Only empty
malformed fragments are removed; populated conflicts and genuinely truncated
captured names remain intact. Destructor names remain attached to their owner.
Cleanup includes balanced intermediate fragments inside an unbalanced full path,
such as `pair<AZ / Uuid,_AZStd`, even when the complete class was already moved.
Fully qualified class names stored as one global symbol, and redundant namespace
prefixes on class names, are repaired structurally even without an exact reflected
friendly-name match. Concrete container specializations remain classes under
the `AZStd` namespace.
Vtable labels and other aliases under split scopes are repaired as well,
including namespace-only legacy owners. A moved label retains its symbol ID.
If the same name/address already has a canonical label, the duplicate merges into
that label and retains primary status. Aliases for other owners of a shared table
remain intact. This changes symbol ancestry without rewriting native table slots.

Captured SerializeContext `factory` objects also expose `IObjectFactory` dispatch
entries. The importer validates the x64 destructor/Create/Destroy pointer slots
and adds `AZ::SerializeContext::IObjectFactory::Create` and `Destroy` aliases.
Existing primary function names and signatures stay intact. These aliases describe
the interface; they do not infer an `InstanceFactory<T>` concrete implementation
or create ordinary container methods. Shared null/no-op callbacks remain shared.
Zero, non-code and interior-function slots are rejected. Source contract:
[Lumberyard SerializeContext.h](https://github.com/aws/lumberyard/blob/master/dev/Code/Framework/AzCore/AzCore/Serialization/SerializeContext.h).

Datatypes and layouts are built before function signatures. Empty structures
with Ghidra's exact `PlaceHolder Class Structure` marker are filled from captured
layout evidence in place, retaining their IDs and existing pointers. Structures
with manual fields or a different description keep their existing layouts.

Signature validation plans parameter names, types and storage against the existing
function. Automatic `this` and hidden return parameters retain their ABI slots;
explicit receivers also remain ahead of the reflection context. A name collision
with a trailing parameter or local uses a stable suffix such as `reflection_2`
without renaming the existing variable. Trailing parameter types, comments and
valid custom storage are retained. Return and parameter changes use one update.
For captured member callbacks, a receiver tied to an adapter namespace can become
an explicit `this` pointer to the reflected owner at its existing register. Hidden
or indirect ABI cases without enough evidence are reported and left unchanged.
A receiver is not placed behind a hidden return pointer: Ghidra's `__fastcall`
model puts that pointer in RCX, where an MSVC member receives `this`, so the
function records `signature-receiver-hidden-return-order-unresolved` instead.
Owners whose structure is sized but has no serialized fields, such as
BehaviorContext-sized opaque classes, still type the receiver. Empty Ghidra
placeholders and zero-length declarations do not.

Storage repair is limited to recomputing unassigned or invalid dynamic parameter
storage through the existing calling convention and compiler specification.
The importer keeps custom register/stack storage, checks planned stack slots
against locals and never forces deletion of conflicting locals. Ambiguous storage
records a `signature-*` skip reason. It does not reconstruct arbitrary stack
frames, stack depth or local-variable offsets. Validation diagnostics include
`checkedSignatureCount`, `signatureReasonCounts` and bounded `signaturePlanSamples`.
Applied action records retain the plans and any name or storage adjustments.

The dry run checks evidence analysis, proposed layouts and signature/storage plans. Database writes and
apply-only operations run after it. An unexpected application failure rolls back
the script transaction and updates the diagnostics with the failed phase and
stack; it still surfaces the exception for debugging. Input failures and
cancellation also save diagnostics when storage permits. Both reports are replaced
atomically after successful writing. Validation summaries stay bounded, and the
automatic validation pass reuses the loaded captures rather than loading them twice.

Native string evidence is indexed once per import and reused across validation and
application. Non-string data is rejected through Ghidra's value-class check before
reading its value, avoiding unnecessary length calculations and database queries.
ASCII, Unicode, character arrays and typedef strings retain their addresses.
The value index keeps the existing 512-character bound; longer MB signatures and
core RTTI UUID evidence are parsed separately. MB signatures accept `class`,
`struct`, `enum` and unqualified template arguments. Cancellation propagates,
and an incomplete string index is never cached.

Only `NewWorld+0x...` and `NewWorld.exe+0x...` module-relative addresses, matched
case-insensitively, rebase to this program's image base. External DLL/API-set
offsets are preserved in capture evidence and skipped as outside the program.
They cannot create functions, aliases or implementation labels at a coinciding
NewWorld offset. Absolute addresses still require loaded program memory.

Validation does not exercise Swing event delivery or background Auto Analysis.
A Swing timeout in `ToolTaskManager.taskCompleted` can occur separately from the
import. Check the importer completion message and the timestamp of its applied
report before rerunning; that timeout alone does not establish import failure.

`tests/AzReflectionValidationTest.java` reproduces the null vtable slot through
the actual inferred RTTI scan in a disposable Ghidra database. It also checks
unmapped/non-code slots, valid sparse tables, decoded this-adjust/direct-jump
thunks, saved previews, injected validation/apply failures, cancellation,
rollback, previous-report preservation and successful retries.
`tests/AzReflectionSymbolRenameTest.java` replays native RTTI imports with source,
destination and both aliases, aliases created earlier in the import, namespace-only
moves, unchanged names and nested generics. It checks preserved function and source
alias identities, read-only previews and repeated imports.
`tests/AzReflectionParametersTest.java` reproduces the reported
`TeleportContributorsAction::Reflect` failure through the full importer at its
original address. It checks datatype/parameter identity, named fields behind
automatic receivers, explicit receivers, local/trailing name collisions, hidden
returns, indirect values, custom register/stack storage, return/parameter updates,
stack conflict preservation and injected missing dynamic stack storage. Preview
and repeated application use the same plans. Pass the Ghidra installation and
a writable fixture directory to its main method.
`tests/AzReflectionStringScanTest.java` checks non-string read avoidance, one data
scan across validation/application, Unicode/array/typedef strings, duplicate
addresses, long generic and UUID evidence, and cancellation followed by a complete
retry. Optional `guard`, `variants`, `scan-count` and `cancellation` arguments
isolate its cases. These database tests do not reproduce a Swing timeout.
`tests/AzReflectionModuleAddressTest.java` reproduces `ucrtbase+0xab9a0` landing
inside a NewWorld `SETNZ` instruction. It checks skipped DLL/API-set actions,
preserved manual symbols/comments and instruction boundaries, accepted main-image
addresses, preview and retry.
`tests/AzReflectionResourcesPickerTest.java` uses Ghidra's real chooser approval
rule without showing a dialog, and verifies that empty selection imports the
current directory. Run it with GUI-capable AWT, not headless AWT. `legacy` reproduces
the old files-and-directories mode failure. `tests/AzReflectionFactoryTest.java`
checks factory interface aliases, shared manual primary names, missing functions,
invalid/interior slots, unchanged table/object bytes, preview and retry.

The report joins UUIDs, qualified names, datatype paths, defined fields and
evidence sources in `reflectedTypes`. Full action evidence is included by default.
Inventories and action records stream through a per-run scratch directory beside
the output report rather than Java's default temporary directory or a second
in-memory report tree. Report format 2 stores original symbol names once per
address in `symbolNameSnapshots`. An action's `oldNamesRef` resolves to a snapshot's
`address`; its `names` array records the names at the first observation of that
address. Later actions record newly added names without copying the growing list.
Comments are merged once per address and comment type,
including shared implementations, adapters and source reflectors. This avoids
repeated large comment-history diffs while retaining every owner's metadata.
Temporary files are removed after success, cancellation or an input error; the
final report replaces the previous file only after writing completes.

Before applying changes, the importer checks that the report and Ghidra database
temporary volumes each have at least 4 GiB free. An existing project volume must
retain at least the larger of 4 GiB or the current program file size plus 512 MiB,
leaving room for a subsequent program save. While writing, it checks a 512 MiB
reserve on the report and temporary volumes and retains the project reserve.
A storage failure aborts the script's transaction, preserves the
previous report and prints the affected path and reason in the script console.
It stops the import rather than dropping action or reflection records.

Ghidra's database temporary directory is configured at startup. On this workstation,
`D:\.ghidra\support\launch.properties` sets `application.tempdir` and `java.io.tmpdir`
to `E:/Projects/openworld/tmp/ghidra`. Restart Ghidra to apply those settings.
The previous launcher settings are backed up in
`E:/Projects/openworld/tmp/az-containers/launch.properties.before-storage-fix`.

If an earlier run exhausted the Java heap and reported locked checkpoint buffers,
restart Ghidra and reopen the last saved program before running the updated script.
That failed session cannot be relied on to save or end its transaction cleanly.

BehaviorContext native sizes and alignments supplement serialized fields; exposed properties
supply getter/setter names, not object member offsets. Behavior-only classes
remain opaque storage of the captured size. BehaviorContext aliases retain their
source spelling in the report; native RTTI identities keep their existing names.
Module-only classes with unknown sizes become incomplete declarations.
Conflicting native size evidence is
reported, and manual layouts are preserved.

## BehaviorContext coverage

The importer reads global methods and properties, constructors, class callbacks,
attributes, base UUIDs, request/notification buses, wrapped types, the UUID-to-class
map and on-demand reflection registrations. It joins type-map copies by UUID and
reads native `m_*` members or their normalized aliases once. Captured qualifiers,
deprecated names, descriptions, attributes and member-function pointer words are
retained exactly. Missing native signatures and undecoded attribute values are
counted explicitly in `behaviorContextEvidence.unresolvedMetadata`.

Every EBus event supplies its captured Broadcast, Event, QueueBroadcast and
QueueEvent implementations. The normalized `event` dispatch member is
distinguished from a map wrapper. Bus ID UUIDs/traits and virtual-property links
appear in function comments and the complete inventory. Virtual properties join
their getter/setter events through captured addresses; those heap addresses never
become static labels. Shared implementations receive aliases for every owner
and metadata comments. A common virtual thunk keeps its slot identity alongside
all its bus/event aliases.

Class metadata is added to generated datatype descriptions. Captured method
adapter vtables and reflection source functions receive analysis aliases under
`AZ::BehaviorContext::Method<owner::function>`. These identify BehaviorMethod
adapters and do not establish the exposed object's native vtable layout.
Source function aliases use the capture RVA, for example `Reflect<0x170>`.
Every captured body also gets an `Implementation<0xRVA>` alias in this scope,
so overloaded constructors and same-named methods remain distinguishable even
when the capture omits their argument signatures. The complete `functionUses`
inventory maps canonical owners, roles and aliases to implementation addresses.

Enum constants in this capture are exposed as global read-only properties.
The importer recovers integer return bits only from verified x64 MOV-immediate
or XOR-zero/RET stubs in the loaded binary. It builds Ghidra Enum datatypes under
`/AZ/BehaviorContext/Constants`, grouped by observed property prefix and return
register width. These are script constant families: their descriptions explicitly
state that a prefix and return register do not establish a native C++ enum UUID
or storage size. Dynamic getters remain unresolved; values are never inferred
from property order or names. Existing manual enum types remain intact.

`behaviorContextEvidence.inventory` retains all classes, global methods and
properties, EBuses, type-map links, on-demand registrations and constant recovery
evidence. It removes duplicate alias copies while preserving unique fields and
unknown attributes. Captured API properties and bases without object offsets do
not supply invented structure fields or method signatures.

`tests/AzBehaviorContextTest.java` verifies applied symbols, datatype descriptions,
constant values, exact uint64 attributes, all EBus dispatch variants, virtual
properties, shared owners, normalized records, map-only classes, manual conflicts,
previews and repeated runs in a disposable ProgramDB. The full-capture import
test additionally checks JSON/ZIP agreement: 611 classes, 2,647 global properties,
626 EBuses, 4,464 events, 472 resolved virtual-property links and 168 on-demand
reflectors.

## Qualified names and datatype repair

The renamer uses Ghidra's `SymbolPathParser` for RTTI types, datatype categories,
class owners and qualified function names. It splits `::` outside templates and
function signatures. For example, `AZStd::array<AZ::Vector3,2>` becomes category
`/AZStd` and type `array<AZ::Vector3,2>`. `MB::` and `Javelin::` remain namespaces;
class and function template arguments retain their own qualified names. The MB
`GetTypeName<T>` signature matcher accepts nested templates. Operator punctuation,
pointers, arrays and negative template arguments remain distinct in names.

Every unified run repairs names produced by older versions. It moves
generated datatypes and matching class namespaces in place, preserving managed
type IDs, field and pointer references, nested classes and member symbols. It
leaves manual datatype layouts and destination conflicts intact and records
conflicts in the report. It applies this repair before rebuilding reflected structures.

The repair also corrects existing `AZ`, `AZStd`, `std`, `MB` and `Javelin` roots
that older analysis marked as classes. Ghidra lacks a reverse class conversion,
so it replaces the root symbol and moves its immediate children in place. Child
symbol IDs, function addresses and descendant ownership remain intact. The
report records these changes as `namespace_kind_migration` with both root IDs.

Some runtime RTTI names in the capture are truncated. Those names stay atomic
and appear in the report's `unparsedTypeNames`; the parser does not split their
incomplete template arguments into namespaces or invent a missing suffix.

## Datatype fields and native layouts

The unified entrypoint builds named, typed reflected members at their captured
offsets,
including bases, nested templates, pointers and enums. A serialized enum field is
either typed by the enum's UUID or, as in this capture, typed by its underlying
scalar with an `EnumType` attribute naming the enum UUID. Both become a Ghidra
Enum at the field's width, filled from `editContext.enumData` enumerators.
Each constant is masked to the enum's width and sign-extended for signed
underlying types, because the capture reads a full word past narrow enum
storage. Enums without captured enumerators stay empty, named Enums.
Earlier generated typedefs are replaced in place, and manual types are kept.
`long` and `unsigned long` follow the program's data organization: 4 bytes on
MSVC x64. BehaviorContext constant families are imported separately.
Pointer fields reference their pointee instead of allocating a pointee structure
with the pointer's size. Generated field placeholders can grow in place, keeping
existing datatype IDs and references. Hand-written layouts are preserved.

The NewWorld 3-26 x64 profile fills these native layouts:

| Type | Native fields |
| --- | --- |
| `AZ::Uuid` | 16-byte `data` array |
| `AZ::Component` | vptr, entity pointer at +8, reflected `Id` at +16 |
| `AZ::EntityId`, `AZ::Crc32` | Typed scalar members |
| `AZ::Vector3`, `AZ::Vector4`, `AZ::Quaternion` | Four float SIMD lanes |
| `AZStd::vector<T>` | Typed `m_start`, `m_last`, `m_end`, allocator; 32 bytes |
| `AZStd::unordered_map<K,V>`, `unordered_set<K>` | Allocator, list, bucket vector, bucket pointer/count, load factor and start bucket; 144 bytes |
| `AZStd::list<T>`, `array<T,N>` | Typed list nodes or inline array elements |
| `AZStd::string` | Small-buffer union, size, capacity and allocator; 40 bytes |
| Smart pointers | Typed pointee pointer and shared ownership pointer when present |
| `AZStd::pair<A,B>` | `first`/`second` offsets recovered from the live EnumElements callback |

Helpers such as `node`, `bucket`, `list_storage` and `bucket_vector` are analysis
datatypes beneath the owning specialization's category. Their names describe
storage; they are not new claims about native C++ class names. Map/set predicate
and hasher storage remains undefined. Container objects have no vptr.

The profile checks bytes in the hash Size callback at `NewWorld+0xd5ce20` and
vector Size callback at `NewWorld+0xdb0880`. Pair offsets additionally require a
matching live adapter slot and p-code evidence. Generic `elements` describe
serialized values, not object fields: sentinel offsets are ignored. Their
by-value sizes establish element strides, including 120-byte
`EncounterConditionEvent`. Rust ports supply names and behavior context, not a
C++ memory layout.

The report distinguishes complete supported profiles, reflected subsets, opaque
custom-serializer types, unknown sizes, overlaps and unresolved ownership. When
distinct UUIDs share a qualified C++ name, each datatype uses a separate category
under its namespace: `/AZStd/__typeid__/<UUID>/vector<int>`, for example. The C++
class/template name stays unchanged. Fields and `this` signatures select their
datatype by UUID, even when the captured sizes differ. A field type without a
class body does not reuse a generated type that records a different UUID at its
canonical path; it uses its own `__typeid__/<UUID>` category instead. The report retains the
ambiguous source name and adds `canonicalDatatypePath` and
`identityResolution: uuid-specific-datatype`. Generated types with a known UUID
move in place, preserving IDs and existing references. Manual canonical layouts
without a captured UUID remain intact.

Reflected pointer display names already include the outer `*`. The importer
resolves the pointee separately before adding that pointer, preventing accidental
`void**` fields for captured `T*` members. Qualified
RTTI names take precedence over short serialization names, so `Vector3`'s known
UUID resolves to `/AZ/Vector3`. Old generated unqualified types move in place.
Empty same-size placeholders can redirect their references to an existing native
layout; only empty flattened duplicate classes are removed.

`tests/AzReflectionDatatypesTest.java` builds the full local capture in a disposable
Ghidra ProgramDB. It checks pointer/UUID repair, referenced datatype identities,
native container fields, pair alignment, SIMD arrays, MB enum identity, Javelin
field offsets, previews, manual layouts, repeated runs and ABI mismatch rejection.
`tests/AzReflectionDatatypeIdentityTest.java` checks distinct same-name UUID
layouts, by-value and pointer consumers, receiver signatures, migration IDs,
manual canonical types, report coverage and repeated runs. Its arguments are the
Ghidra installation directory and a temporary output directory.
`tests/AzReflectionRepeatedNamesTest.java` checks repeated native RTTI tables and
bodies, MB generic functions and namespace repair at separate addresses, with the
same two arguments.
`tests/AzReflectionTemplateScopeRepairTest.java` reproduces the live ranged_enum
hierarchy, an existing canonical class, nested classes, destructors and stale
empty fragments. It also checks uncaptured balanced template names, genuinely
truncated names, vtable primary labels and shared aliases, unchanged native slots,
symbol identities, previews and retries, with the same arguments.

`tests/AzReflectionRenamerTest.java` checks the local capture's RTTI names and
resolved generic specializations, including MB compiler signatures. The
`AzReflectionNameRepairTest.java` companion runs migration checks against a
disposable Ghidra database, with the Ghidra installation directory as its first
argument. It checks reference preservation, preview mode, conflicts and repeated
runs without opening or changing the game database.

The sync script validates nonempty sources and stages complete output files
before replacing installed copies. An interrupted write to a staged file leaves
the existing installed script intact.

## Serialized AZStd containers

The unified script's container pass resolves nested specializations and pointer
elements, reads
`classData.containerInfo.vtable`, and compares every primary slot against live
program memory before editing that table. Captured `container` addresses are
heap objects from the capture process and cannot locate tables in a static
program. This profile uses `NewWorld 3-26`'s little endian, 64 bit ABI.
The program's editable name does not identify its build. Pointer layout and
the captured table entries determine whether each table can be annotated.
Run the entrypoint from CodeBrowser's Script Manager with an active program;
a Script Manager without an active program cannot attach the pass to a binary.
Its primary container interface has 23 slots, ending at `ElementsUpdated`.
The dump's `slotCount` can include adjacent `GenericClassInfo` tables; slots
23 and later are excluded. Several flag and operation names remain unresolved
and receive comments rather than guessed source names.

Names use the renamer's analysis namespace, for example
`AZ::SerializeContext::ClassData<AZStd::vector<StateFlexCondition*>>::container::Size`.
`ClassData<T>` identifies the reflected owner; it is not a C++ template recovered
from RTTI. These tables belong to serialization adapters. AZStd container
instances themselves have no vptr. Shared tables and folded callbacks get
specialization aliases and comments listing their owners. Existing user function
names remain intact. Inlined operations have no independent address to rename.

`tests/AzReflectionRenamerTest.java` exercises identities from the local capture,
pointer naming, shared ownership, rebasing, overread boundaries and rejection of
mismatching or incomplete slot records. Compile it with the renamer and
`GhidraCli.java` against the installed Ghidra jars, then pass the local
`serialize.json` path to its main method. The capture is not included in tests.

`tests/AzReflectionImportTest.java` exercises the complete entrypoint in a
disposable Ghidra database. It checks UUID joins, class and function templates,
BehaviorContext size/alignment, string callback addresses, API property handling,
module-only declarations, ZIP streaming, missing SerializeContext, previews,
malformed inputs, TypeRegistry exclusion and repeated runs. Pass the Ghidra
installation directory and the local resources directory to its main method.
The datatype suite also accepts a third argument to load the module and
BehaviorContext evidence before checking the full capture's native layouts.

`tests/AzReflectionMemoryTest.java` applies 1,000 owners sharing an implementation,
adapter and reflector with `-Xmx256m`, then repeats the import. It checks one
comment-history entry per address, an unchanged rerun and temporary-file cleanup.
Pass the Ghidra installation directory and `1000` to its main method.
`tests/AzReflectionCaptureMemoryTest.java` applies the actual BehaviorContext ZIP
to a disposable ProgramDB with RET stubs at all captured implementation addresses.
With `-Xmx512m`, it verifies 21,154 function uses, every inventory record, full
action output and one comment write per address. Pass the Ghidra installation
directory and resources directory. The stubs test import behavior and memory use;
they do not validate the game's native function bodies.

`tests/AzReflectionStorageTest.java` checks 30,000 aliases sharing one address with
`-Xmx256m`. It verifies one original-name snapshot, report-local scratch files and
linear action output size. It injects low-space preflight, mid-write low-space,
spool-write and final-report-write failures without filling a disk, then checks
program rollback, preservation of the prior report, cleanup and a successful rerun.
Pass the Ghidra installation directory, a writable fixture directory and `30000`.
When running the suites directly with Java, use the same temporary-directory
VM options as Ghidra's launcher so the preflight checks the configured volume.

`tests/AzReflectionStartupTest.java` exercises attached and tool-active programs,
open targets, nested project targets, ambiguity, chooser cancellation and a
missing tool. A GUI adapter substitute delegates to real Ghidra state rebinding
and transactions in a disposable ProgramDB, checking that import targets the
selected program and releases the chooser consumer. Pass the Ghidra installation
directory to its main method.

## Network Schema Extractor

`NetworkSchemaExtractor.java` builds a static JSON report for network type and
field registration evidence. The file in Ghidra's script directory is generated;
do not edit `$HOME/ghidra_scripts/NetworkSchemaExtractor.java` directly. Edit
the small source files in this directory instead, then run the sync script.

Run the generated script against the loaded `NewWorld 3-26` program and point it
at:

```text
resources/typeregistry.json
```

The extractor source is split across helper files and ordered fragments for
maintainability, but Ghidra should run a single bundled
`NetworkSchemaExtractor.java`. Use:

```powershell
scripts/ghidra/Sync-NetworkSchemaExtractor.ps1
```

The sync script writes the bundled file to `$HOME/ghidra_scripts` by default and
removes copied helper `.java` files from that output directory. Keeping helper
`.java` files in the Ghidra script root makes the script manager treat them as
standalone scripts, which can prevent `NetworkSchemaExtractor` from loading.
The sync script also rejects network-schema source modules over 1000 lines.

Compile the bundled script without running binary analysis:

```powershell
scripts/ghidra/Test-NetworkSchemaExtractor.ps1 -GhidraHome D:\.ghidra
```

`-GhidraHome` defaults from `NW_GHIDRA_HOME`. Both PowerShell entrypoints use
advanced-function help (`Get-Help ... -Full`), support `-Version`, and the sync
entrypoint provides `-Force` and `-DryRun` for its generated output.

- `NetworkSchemaAddressFormatter.java` — address formatter callback shared by models.
- `NetworkSchemaModels.java` — leaf model/data holders.
- `NetworkSchemaPcode.java` — p-code evidence model holders.
- `NetworkSchemaStack.java` — stack-state and constructor evidence models.
- `NetworkSchemaText.java` — deterministic text/literal helpers.
- `NetworkSchemaTypeModels.java` — nested type and container-shape model holders.
- `NetworkSchemaX86.java` — x86 operand, register, memory, and offset helpers.
- `NetworkSchemaJson.java` — JSON helpers.
- `network_schema_extractor/NetworkSchemaExtractor.*.javafrag` — ordered
  fragments for the generated Ghidra script entrypoint.

Interactive runs present an analysis-mode dialog. `Full schema` remains the default.
Focused type, handler-vtable, and function-trace runs ask for their addresses or type
indices directly and write distinct report files, so they cannot replace the complete
schema report.

Headless runs use the same modes plus explicit, discoverable options:

```text
NetworkSchemaExtractor full --typeregistry resources/typeregistry.json \
  --out resources/network-schema.static.json --force
NetworkSchemaExtractor types 0x12,0x34 \
  --typeregistry resources/typeregistry.json --dry-run
```

Run `NetworkSchemaExtractor --help` for its complete surface. The standalone
datatype, function, codec, and reference probes also implement `--help`,
`--version`, `--out`, `--force`, and `--dry-run`; their `NW_NETWORK_*`
environment bindings are documented alongside the corresponding flags.

The script emits every `typeregistry.json` row, recovers
`MB::ReplicatedState::RegisterField` callers from Ghidra where available, and
adds constructor field order, groups, handler offsets, instance vtables, and
decoded AZ RTTI provider evidence to rows that can be statically mapped. Native
type names are recovered from actual AZ/Hub registration helper tables or AZ RTTI
providers; TypeRegistry names remain the raw TypeRegistry/debug-name field.

For types with no replicated-state field registration, the extractor follows their
unmarshal call graph and records only field, storage, codec, and nested-type evidence
proven by P-code data flow. Delegation to a polymorphic fragment codec remains an
explicit schema operation instead of being flattened into synthetic fields.
