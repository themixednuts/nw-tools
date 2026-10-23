# Reflection capture schemas

This is a map of what each capture under `resources/` contains, and where
`AzReflectionRenamer.java` puts it in Ghidra. Counts are from the NewWorld 3-26
captures. Addresses are written as `NewWorld+0x<rva>` for static data, or as
absolute `0x<heap>` for capture-process objects, which never become labels.

## `serialize.json` (SerializeContext, 22 MB)

| Section | Contents | Ghidra destination |
| --- | --- | --- |
| `uuidMap` (4,362) | Class bodies: `name`, `typeId`, `version`, `factory`/`serializer`/`eventHandler`/`persistentId`/`converter` pointers, `containerInfo` (vtable + slots), `azRtti` (`address` of the `RttiHelper<T>` instance, typeName, `hierarchy[]`), `elements[]` | Structures and fields, callback names, container tables, `RttiHelper<T>::s_instance`/`vftable` and its nine slot names |
| `*.hierarchy[]` | `typeId`, `typeName`, `getType` (`AzTypeInfo<T>::Uuid`), `helper` (the type's AZ_RTTI block or `RttiHelper<T>` vftable), `getTypeName` (that helper's name slot) | `AzTypeInfo<T>::Uuid`; `T::GetType`...`AddressOf` for blocks, `RttiHelper<T>` slots for vftables, once the binary confirms the helper |
| `uuidMap.*.elements[]` | `name`, `typeId`, `offset`/`dataSize` (strings), `is_pointer`, `is_base_class`, nested `genericClassInfo`, `attributes[[id, {attributeName, value}]]` | Typed fields. `memberFunction`/`function` attribute values name callbacks and type `this`. An `EnumType` attribute (`value.kind: "Uuid"`) types the field as that enum. |
| `uuidGenericMap` (777) | `[typeId, genericClassInfo]` for template specializations | Template names, container element types |
| `enumTypeIdToUnderlyingTypeIdMap` (132) | Enum UUID → underlying scalar UUID | Enum width and signedness |
| `editContext.enumData` (326) | `[enumId, {name, attributes:[[id, {attributeName: "EnumValue", value: {valueU64, valueU32, valueHighU32, description}}]]}]` | Enum constants. Words are read past narrow storage, so they are masked to the enum width. `valueU64: null` means zero. |
| `classNameToUuid` (7,156) | `[nameCrc, typeId]` pairs | Not used |
| `uuidAnyCreationMap` (5,082) | `typeId` → AnyCreator function (5,030 bodies; one empty-any body serves 50 types) | `AZ::SerializeContext::Class<T>::AnyCreator`, or one alias per type on a shared body. A registration trace for the type takes precedence. |
| `rttiOnly` (3,686) | AZ RTTI types the SerializeContext does not reflect: `typeId`, `typeName`, `hierarchy[]`, and the AZ_RTTI block at `vtable` with its `getType`, `getTypeName` and `enumHierarchy` slots | The block's six functions, checked slot by slot; `T::vftable` when code references the block |
| `sharedTypeIds` (12) | UUIDs that distinct native types share (`types[]` with `getType`, `staticTypeId`, `helper`) | Those types' helpers and getters, named by RTTI name rather than by the UUID's class body. UUID-keyed datatypes still cannot tell them apart. |

Serialized enum fields in this capture are always scalars with an `EnumType`
attribute (257 fields, 241 with enumerators). None use the enum UUID as their
`typeId`.

## `behavior-context.json` (BehaviorContext, 823 MB; also `.json.zip` / `.7z`)

Every record appears twice, once under its native `m_*` name and once under a
normalized alias (`m_methods`/`methods`, `m_name`/`name`, ...). The importer
reads each value once.

| Section | Contents | Ghidra destination |
| --- | --- | --- |
| `classes` (611) | `typeId`, `size`, `alignment`, `azRtti`, `baseClasses`, `requestBuses`, allocator/constructor/destructor/cloner/mover pointers, `methods[]`, `properties[]`, `attributes` | Struct size and alignment, `Method<owner::fn>` aliases, class facts in datatype descriptions |
| method record | `m_name` (`Class::Fn`), `m_isConst`, `m_debugDescription`, `m_functionPtr` (implementation), `vtable` (BehaviorMethod adapter), `m_reflectFunction[]`, `virtualDispatchSlotOffset`, `signature` | Implementation, adapter `vftable` and `Reflect<0xRVA>` aliases; the implementation's prototype |
| `signature` (16,341 methods) | `result` and `arguments[]` as `{name, typeId, azRtti, traits}`, `argumentMetadata[]` (`name`, `toolTip`, `defaultValue`), the decoded `accessors`, `error` when an accessor has another shape | `__fastcall` prototype in the MSVC x64 ABI (see below) |
| `methods` / `properties` (9 / 2,647) | Global methods. Properties are mostly enum constants exposed as getters. | Function names; constant-family Enums under `/AZ/BehaviorContext/Constants` |
| `ebuses` (626) | `events[]` with Broadcast/Event/Queue* function pointers, `idParam`, `virtualProperties` | Bus event function names and comments |
| `typeToClassMap`, `m_onDemandReflection` | UUID → class record; on-demand reflector functions | Joins and `Reflect` names |

`signature` comes from nw-windbg's `behavior-method-signature.ts`, which
decodes each method's `GetNumArguments`, `GetArgument`, `GetArgumentName` and
`GetResult` bodies. `name` is the C++ spelling (`const Vector3&`) and `typeId`
names the stripped type. `traits` are `BehaviorParameter::Traits`: 1 pointer,
2 const, 4 reference, 8 receiver (`this`), 16 string, so pointer and reference
bits give the indirection depth (`T*&` is 5). Argument 0 of a member method
carries the receiver bit.

The importer models what the binary does rather than Ghidra's default
convention. Every by-value class result, including 4- and 8-byte ones such as
`Crc32` and `EntityId`, comes back through a caller buffer named `result`.
That buffer follows the receiver (RCX `this`, RDX `result`) or comes first in a
free function. By-value classes that are not 1, 2, 4 or 8 bytes are passed by
address. Scalars and enums stay in registers. A body that several signatures
share is typed only when they agree; a by-value type without a known size skips
the whole prototype.

The signature describes the script-facing call, which matches the native function
for methods, property getters and setters, and constructors. An EBus event's
Broadcast, Event and Queue variants all point at the bus interface's member, usually
a `mov rax,[rcx]; jmp [rax+slot]` thunk. That member takes the handler as an
unlisted `void *this`, and the Event variants' leading bus id is not one of its
parameters. Other EBus callbacks (`createHandler`, `destroyHandler`,
`getCurrentId`, `queueFunction`) are not typed until their shapes are checked
against the binary.

## `modules/*.json` (36 files)

A module capture is `{vftable, descriptors[]}`. Each descriptor has
`componentName`, `componentUuid`, `azRtti`, and `vtableSlots[{slot, expected,
address}]`, where `expected` is `CreateComponent`, `GetName`, `GetUuid`,
`Reflect` and so on. The module name is not captured. `AZ::Module` is `AZ_RTTI`,
so vtable slot 1 (`RTTI_GetTypeName`) returns it.
