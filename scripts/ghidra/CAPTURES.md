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
| `uuidAnyCreationMap` (5,082) | `typeId` → AnyCreator function | Not used (traces supply `AnyCreator` instead) |
| `rttiOnly` (3,686) | AZ RTTI types the SerializeContext does not reflect: `typeId`, `typeName`, `hierarchy[]`, and the AZ_RTTI block at `vtable` with its `getType`, `getTypeName` and `enumHierarchy` slots | The block's six functions, checked slot by slot; `T::vftable` when code references the block |
| `sharedTypeIds` (12) | UUIDs that distinct native types share (`types[]` with `getType`, `staticTypeId`, `helper`) | Those types' helpers and getters, named by RTTI name rather than by the UUID's class body. UUID-keyed datatypes still cannot tell them apart. |

Serialized enum fields in this capture are always scalars with an `EnumType`
attribute (257 fields, 241 with enumerators). None use the enum UUID as their
`typeId`.

## `behavior-context.json` (BehaviorContext, 482 MB; also `.json.zip` / `.7z`)

Every record appears twice, once under its native `m_*` name and once under a
normalized alias (`m_methods`/`methods`, `m_name`/`name`, ...). The importer
reads each value once.

| Section | Contents | Ghidra destination |
| --- | --- | --- |
| `classes` (611) | `typeId`, `size`, `alignment`, `azRtti`, `baseClasses`, `requestBuses`, allocator/constructor/destructor/cloner/mover pointers, `methods[]`, `properties[]`, `attributes` | Struct size and alignment, `Method<owner::fn>` aliases, class facts in datatype descriptions |
| method record | `m_name` (`Class::Fn`), `m_isConst`, `m_debugDescription`, `m_functionPtr` (implementation), `vtable` (BehaviorMethod adapter), `m_reflectFunction[]`, `virtualDispatchSlotOffset` | Implementation, adapter `vftable` and `Reflect<0xRVA>` aliases |
| `methods` / `properties` (9 / 2,647) | Global methods. Properties are mostly enum constants exposed as getters. | Function names; constant-family Enums under `/AZ/BehaviorContext/Constants` |
| `ebuses` (626) | `events[]` with Broadcast/Event/Queue* function pointers, `idParam`, `virtualProperties` | Bus event function names and comments |
| `typeToClassMap`, `m_onDemandReflection` | UUID → class record; on-demand reflector functions | Joins and `Reflect` names |

Method records carry no argument or result types. The binary is built without
MSVC RTTI, so adapter vtables do not encode signatures either. Typing
parameters beyond `this` would need the dumper to capture each
`BehaviorMethod`'s `BehaviorParameter` array.

## `modules/*.json` (36 files)

A module capture is `{vftable, descriptors[]}`. Each descriptor has
`componentName`, `componentUuid`, `azRtti`, and `vtableSlots[{slot, expected,
address}]`, where `expected` is `CreateComponent`, `GetName`, `GetUuid`,
`Reflect` and so on. The module name is not captured. `AZ::Module` is `AZ_RTTI`,
so vtable slot 1 (`RTTI_GetTypeName`) returns it.
