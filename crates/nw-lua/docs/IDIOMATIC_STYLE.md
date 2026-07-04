# Idiomatic Style Decision for P9b

This document drives the P9b clean-code emitter described in
`crates/nw-lua/ARCHITECTURE.md` section 5. P9b is an AST-to-AST pass over
`decompile::ast::Block`, applied before formatting. Its job is to make correct
decompiled Lua look like New World Lua without changing Lua 5.1 behavior.

Hard rule: P9b must not rename recovered debug identifiers. It may style only
synthetic identifiers introduced by `nw-lua` itself, and only when the binding
being renamed is known and all uses of that binding are rewritten together.

## Tooling Survey

| Tool/source | Whitespace formatting | Identifier naming conventions | Structural rewrites | Auto-renames identifiers? | Notes for P9b |
| --- | --- | --- | --- | --- | --- |
| [StyLua](https://github.com/JohnnyMorganz/StyLua) | Yes. It is a deterministic Lua formatter that parses code and prints it back out with a consistent style. Options cover indentation, line width, quote style, call parentheses, simple-statement collapsing, etc. | No naming policy. It mainly follows Roblox formatting style, but it does not enforce identifier case. | Formatting-level only. It can normalize layout, collapse simple statements, optionally sort requires, and alter quote/call-parentheses style. It does not rewrite `M.f = function` into declaration sugar or synthesize method/module structure. | No. | Keep StyLua as the final printer/formatter. Do not rely on it for P9b's AST sugar or naming decisions. |
| [luacheck](https://github.com/mpeterv/luacheck) / [warnings](https://luacheck.readthedocs.io/en/stable/warnings.html) | Minimal formatting diagnostics only, such as whitespace/line length warnings. It is not a formatter. | No case policy. It can warn about globals/locals, but not naming case. | Static analysis only: undefined globals, unused variables/values, uninitialized access, shadowing/redefinition, unreachable code, unbalanced assignments, empty blocks/statements. | No. | Useful as a later validation signal, not as a source transformation tool. |
| [Lua Language Server (LuaLS)](https://luals.github.io/) | Yes. LuaLS has a built-in formatter backed by EmmyLuaCodeStyle and configurable through `.editorconfig` or `Lua.format.defaultConfig`. | Yes. It has `name-style-check`, configurable by `Lua.nameStyle.config`. | Diagnostics, semantic analysis, and formatting. No automatic semantics-preserving decompiler cleanup. | No general auto-renamer. It diagnoses style; user/editor refactors are separate. | Confirms that naming style is a policy layer, separate from formatting. |
| [EmmyLuaCodeStyle](https://github.com/CppCXY/EmmyLuaCodeStyle) | Yes. It provides document/range formatting, format checking, and editorconfig-based options. | Yes. Its name-style checker supports `snake_case`, `camel_case`, `pascal_case`, `upper_snake_case`, and regex/pattern rules. Defaults are mostly `snake_case`, with mixed allowances for globals, requires, classes, and constants. | Formatting and style diagnostics. It explicitly says it is not a Lua formatting specification. | No. | Good vocabulary for describing case policies. Not evidence that P9b should rename real symbols. |
| [LuaRocks style guide](https://github.com/luarocks/lua-style-guide) | Human style guide, not a formatter. | Dominant pure-Lua guidance: variables/functions/methods use `snake_case`; classes use `CamelCase`; constants may use `UPPER_CASE` sparingly. | Advises function declaration syntax, module table returns, external function declarations, method notation for OOP. | No. | Useful generic Lua baseline, but it does not match New World's case style. |
| [Olivine Labs style guide](https://github.com/Olivine-Labs/lua-style-guide) | Human style guide, not a formatter. | `snake_case` for objects/functions/instances; `PascalCase` for factories. | Prefers `local function` over `local f = function`, module returns, and self-named table methods. | No. | Confirms common Lua preference for function declaration sugar. Case policy differs from New World. |
| [Roblox Lua style guide](https://roblox.github.io/lua-style-guide/) | Human style guide; StyLua mainly follows this formatting family. | `PascalCase` for classes/API-like objects, `camelCase` for locals/member values/functions, `LOUD_SNAKE_CASE` for local constants, `_camelCase` for private members. | Uses `local Module = {}; function Module.go(); return Module` examples. | No. | Much closer to New World than LuaRocks/Olivine on case: PascalCase exported objects, camelCase locals/fields. |
| [lua-users style guide](https://lua-users.org/wiki/LuaStyleGuide) and related module guides | Human guidance, no formatter. | Community style is intentionally not uniform. Commonly cited guides vary between joined lowercase, snake_case, camelCase, and PascalCase for class-like tables. | Common post-`module()` idiom is `local module_table = {}; function module_table.public(); return module_table`; OOP methods use `:`. | No. | Reinforces that P9b should match target corpus, not invent a universal Lua style. |
| [Hisham's post-module() module guide](https://hisham.hm/2014/01/02/how-to-write-lua-modules-in-a-post-module-world/) | Human guidance. | Lowercase module locals for generic Lua; `LikeThis` class tables for OOP. | Public functions on module table with dot syntax; class/object methods with colon syntax; always return the module table. | No. | Useful for module-shape reasoning. New World uses the same table-return and colon-method shape, with different case. |
| [Lua 5.1 reference manual](https://gensoft.pasteur.fr/docs/lua/5.1.4/manual.html#2.5.9) | Language semantics, not style. | None. | Defines exact function/method/local-function sugar. | No. | This is the source of truth for whether a P9b rewrite is semantics-preserving. |

## Idioms To Emit

### Module table pattern

Generic Lua frequently uses:

```lua
local M = {}

function M.f(value)
  return value + 1
end

return M
```

New World usually uses a named PascalCase table instead of `M`:

```lua
local TimeHelperFunctions = {
  secondsInMinute = 60,
}

function TimeHelperFunctions:ConvertSecondsToDaysHoursMinutesSeconds(seconds)
  return days, hours, minutes, seconds
end

return TimeHelperFunctions
```

Prefer `function M.f()` or `function M:Method()` over
`M.f = function() ... end` when the function assignment is a standalone
statement. Keep `field = function() ... end` inside table constructors and
metatable literals unless a later pass proves a broader reshaping is safe.

### Function field assignment vs declaration sugar

Lua defines this as exact sugar:

```lua
M.f = function(a, b)
  return a + b
end
```

can become:

```lua
function M.f(a, b)
  return a + b
end
```

Use the declaration form for module/class table functions because it matches
Lua style guides and New World source.

### Method sugar

Lua defines this:

```lua
function M.method(self, value)
  return self.count + value
end
```

as equivalent to:

```lua
function M:method(value)
  return self.count + value
end
```

For New World output, the method name should normally keep its recovered case.
If the method name is synthetic and table/class-like, use PascalCase:

```lua
function CrestTab:SetVisualElements()
  self.ButtonSave:SetText("@ui_save")
end
```

The `:` form is safe to synthesize only when the first parameter is exactly the
receiver binding named `self`, or when the first parameter is synthetic and P9b
renames that same binding to `self` throughout the function body.

### Local function declaration

Prefer:

```lua
local function f(value)
  return value + 1
end
```

over:

```lua
local f = function(value)
  return value + 1
end
```

There is a recursion caveat. Lua 5.1 says:

```lua
local function f() body end
```

translates to:

```lua
local f
f = function() body end
```

not:

```lua
local f = function() body end
```

So converting `local f = function() ... f() ... end` to `local function f()`
can change which `f` the function body captures. P9b must require binding
proof, not just syntax.

### Naming case conventions

There is no single idiomatic Lua naming case. The common pure-Lua baseline from
LuaRocks/Olivine is `snake_case` for functions and locals, `PascalCase` for
class-like factories, and `UPPER_CASE` for constants. Roblox/Luau prefers
PascalCase classes/API-like objects, camelCase locals/members/functions, and
LOUD_SNAKE_CASE constants.

New World's observed convention is closer to Roblox/Lumberyard C++ API style:

| Kind | New World observed style | P9b policy for synthetic names |
| --- | --- | --- |
| Returned script table / component table / class-like table | PascalCase, often matching the file/module concept: `Logger`, `UIStyle`, `CrestTab`, `TimeHelperFunctions`, `ActivateEntity` | PascalCase only when P9b has a recognized returned module table and a reliable chunk/file/module name. Otherwise keep deterministic fallback. |
| Methods on those tables | PascalCase after `:`: `OnInit`, `SetVisualElements`, `ConvertSecondsToHrsMinSecString` | PascalCase for synthetic table member names only. Do not recase recovered field names. |
| Global helper functions in NW scripts | PascalCase: `RequireScript`, `IsAttackRelatedToLocalPlayer`, `FindLocalPlayerEntityId` | Do not create new globals for style. Do not rename recovered globals. |
| Local variables and fields | lowerCamel: `channelName`, `sendToConsole`, `effectGroupName`, `frameLineAlpha`, `localPlayerEntityId` | lowerCamel for synthetic locals when there is a semantic role. Keep opaque register names (`v0`, `v1_2`) when no role is known. |
| Constants / enum-like fields | `UPPER_SNAKE_CASE` or engine enum names: `SEND_ALL`, `TEXT_CASING_NORMAL`, `eUiTextCaseSetting_Normal` | `UPPER_SNAKE_CASE` only for synthetic constant fields when proven constant-like. Do not infer constant-ness from uppercase recovered names. |
| RequireScript imports | Mixed. PascalCase for class/data handlers, lowerCamel for helpers: `BaseElement`, `EntitlementsDataHandler`, `styleHelpers`, `crestTabCommon` | Do not style imports in P9b v1 unless a later name pass has a reliable target-specific rule. |

## New World Observed Conventions

Representative files inspected:

- `E:\Projects\az-rs\resources\fixtures\lua\good-lua\scripts\_common\logger.lua`
- `E:\Projects\az-rs\resources\fixtures\lua\good-lua\lyshineui\_common\uistyle.lua`
- `E:\Projects\az-rs\resources\fixtures\lua\good-lua\scripts\combatimpact\impactcommon.lua`
- `E:\Projects\DEMOJSON\scripts\weaponeffects\weaponeffectbase.lua`
- `E:\Projects\DEMOJSON\scripts\gameplay\env\area_trigger.lua`
- `E:\Projects\DEMOJSON\lyshineui\guildmenu\cresttab.lua`
- `E:\Projects\DEMOJSON\lyshineui\_common\timehelperfunctions.lua`
- `E:\Projects\LuaDecompiler\examples\abilitiescommon.decompiled.lua`
- `crates\nw-lua\tests\phase8_closures.rs`
- `crates\nw-lua\tests\phase9_naming.rs`

Lightweight scan over `E:\Projects\DEMOJSON` plus
`E:\Projects\az-rs\resources\fixtures\lua\good-lua`:

- 1296 Lua files scanned.
- 17800 `function Table:Name(...)` declarations.
- 41 `function Table.Name(...)` declarations.
- 391 plain `function Name(...)` declarations.
- Among colon methods, 17734 started with an uppercase character, 63 looked
  lower-camel, and 3 contained underscores.
- 3144 lines contained `RequireScript(`.
- 2912 lines returned a bare identifier.

The count is intentionally simple regex evidence, not a parser. It is still
strong enough to decide the target style: colon methods dominate, and method
names are overwhelmingly PascalCase.

### Table-return module/component shape

`E:\Projects\az-rs\resources\fixtures\lua\good-lua\scripts\_common\logger.lua`:

```lua
local Logger = {
  SEND_ALL = 1,
  SEND_UNIQUE = 2,
  defaultChannelName = "Anonymous",
  channels = {},
  listeners = {}
}
function Logger:CreateChannel(name, sendToConsole)
  local channel = {
    name = name,
    sendToConsole = sendToConsole or self.SEND_NOTHING,
    messages = RingBuffer:new()
  }
  return channel
end
return Logger
```

Observed:

- Returned table is PascalCase.
- Methods use `function Logger:CreateChannel(...)`.
- Constants are upper snake.
- Locals and ordinary fields are lowerCamel.

`E:\Projects\DEMOJSON\scripts\gameplay\env\area_trigger.lua`:

```lua
local ActivateEntity = {
  Properties = {
    EnteringAreaEvent = {
      default = EventData()
    }
  }
}
function ActivateEntity:OnActivate()
  self.triggerAreaHandler = TriggerAreaNotificationBus.Connect(self, self.entityId)
end
return ActivateEntity
```

Observed:

- Entity/component table is local PascalCase and returned.
- Lifecycle methods are PascalCase colon methods.
- `Properties` and property keys are PascalCase because they bind engine/editor
  properties; do not recase them.

### LyShine UI modules

`E:\Projects\DEMOJSON\lyshineui\guildmenu\cresttab.lua`:

```lua
local CrestTab = {
  Properties = {
    CrestBack = {
      default = EntityId()
    }
  },
  spawnTickets = {},
  crestData = GuildIconData(),
  TWITCH_ENTITLEMENT_IMAGE = "lyshineui/images/entitlements/icon_entitlement_twitchprime.png"
}
local BaseElement = RequireScript("LyShineUI._Common.BaseElement")
BaseElement:CreateNewElement(CrestTab)
local crestTabCommon = RequireScript("LyShineUI.GuildMenu.CrestTabCommon")
function CrestTab:OnInit()
  BaseElement.OnInit(self)
end
function CrestTab:SetVisualElements()
  local frameLineAlpha = 0.5
end
return CrestTab
```

Observed:

- The main table is PascalCase and returned.
- Most methods are PascalCase colon methods.
- `RequireScript(...)` locals are mixed: `BaseElement` and handlers are
  PascalCase, while helpers such as `crestTabCommon` are lowerCamel.
- Locals are lowerCamel.

`E:\Projects\az-rs\resources\fixtures\lua\good-lua\lyshineui\_common\uistyle.lua`:

```lua
local styleHelpers = RequireScript("LyShineUI._Common.StyleHelpers")
local UIStyle = {}
function UIStyle:Init()
  self.TEXT_CASING_NORMAL = eUiTextCaseSetting_Normal
  self.COLOR_WHITE = ColorRgba(255, 255, 255, 1)
end
```

Observed:

- Main module table can preserve acronym capitalization (`UIStyle`).
- Imported helper local can be lowerCamel even when module segment is
  PascalCase.
- Constant-like fields on `self` use upper snake.

### Global/module hybrid scripts

`E:\Projects\DEMOJSON\scripts\weaponeffects\weaponeffectbase.lua`:

```lua
local dataLayer = RequireScript("LyShineUI.UiDataLayer")
WeaponEffectBase = {
  EffectTypes = {Particle = 1, Audio = 2},
  effectGroups = {},
  isDeactivating = false
}
function WeaponEffectBase:OnActivate()
  self.effectEventBusHandler = WeaponEffectEventBus.Connect(self, self.entityId)
end
return WeaponEffectBase
```

Observed:

- Some NW scripts intentionally assign the main table globally before returning
  it. P9b must not "fix" this by inserting `local`; that changes global
  side-effects and module API behavior.
- Methods still use PascalCase colon declarations.

`E:\Projects\az-rs\resources\fixtures\lua\good-lua\scripts\combatimpact\impactcommon.lua`:

```lua
function IsAttackRelatedToLocalPlayer(attackerEntityId, targetEntityId)
  return PlayerComponentRequestsBus.Event.IsLocalPlayer(attackerEntityId) == true
end
function ImpactTable:PlayImpactSoundAtPosition(soundName, impactPos, attackerEntityId, targetEntityId)
  local playerEntityId = FindLocalPlayerEntityId(targetEntityId, attackerEntityId)
end
```

Observed:

- Plain globals exist and are PascalCase.
- Existing globals and existing table names must be preserved. P9b is not a
  global-localizing cleanup pass.

### Current decompiler examples

`E:\Projects\LuaDecompiler\examples\abilitiescommon.decompiled.lua`:

```lua
return {backgroundPathByCategory = {...}, defaultBackgroundPath = "...", GetBackgroundPath = function(a0, a1)
  return a0.backgroundPathByCategory[a1] or a0.defaultBackgroundPath
end, ShowAbilityTooltip = function(a0, a1, a2)
  if not a1 or not a2 then
    return
  end
end}
```

Observed:

- Older decompiler output is correct but not NW-idiomatic: it returns a table
  literal with `Field = function(...)` values and synthetic parameters like
  `a0`.
- A tempting cleanup would be to synthesize a module table and colon methods,
  but that is a broad module reshaping transform. P9b v1 should recognize this
  pattern for future work but not rewrite it unless a stricter equivalence
  proof is added.

`crates\nw-lua\tests\phase8_closures.rs` already accepts or expects idiomatic
forms:

```rust
assert!(
    decompiled.contains("local function fib(n)"),
    "expected idiomatic local function form:\n{decompiled}"
);
let has_method_form = decompiled.contains("function o:get()");
let has_assignment_form = decompiled.contains("o.get = function(self)");
```

`crates\nw-lua\src\decompile\naming.rs` currently uses deterministic synthetic
fallback names:

```rust
Name::from(format!("v{reg}"))
Name::from(format!("arg{}", u16::from(reg) + 1))
Name::from(format!("up{idx}"))
```

Those are synthetic and eligible for P9b styling only under the rules below.

## Ordered P9b Transform List

The order matters: module recognition supplies context for naming and method
decisions, but should not itself rewrite executable structure in v1.

| Order | Transform | Status | Preconditions | Output policy |
| --- | --- | --- | --- | --- |
| 1 | Recognize module/component table pattern | SAFE | Pattern is one local or global table binding assigned a table value, followed by functions/fields on that same table, with a final `return SameTable`. Recognition must be metadata-only unless a later listed transform applies. | Tag the table as `module_like`/`component_like`. Do not reorder statements, localize globals, or create a table binding from a `return { ... }` literal. |
| 2 | Style synthetic returned module table name | SAFE-WITH-PRECONDITION | The table name is synthetic (`vN`, `vN_M`, or equivalent internal marker), the module pattern from order 1 is recognized, a reliable chunk/file/module basename exists, the generated PascalCase name is a valid Lua identifier, and no collision occurs in scope. | Rename the binding and all local uses to PascalCase, e.g. `shopcommon` -> `ShopCommon`, `timehelperfunctions` -> `TimeHelperFunctions`. If any condition fails, keep `vN`. |
| 3 | Standalone table function assignment sugar | SAFE-WITH-PRECONDITION | Statement is a single-target assignment whose RHS is exactly one function expression: `T.f = function(...) ... end`. Target must be representable as a Lua function name (`Name(.Name)*.Name`), or `["field"]` only when the string key is a valid non-keyword identifier. The assignment must not be part of multi-assignment. | Emit `function T.f(...) ... end`. Preserve the recovered table/member names exactly. |
| 4 | Standalone global function assignment sugar | SAFE-WITH-PRECONDITION | Same as order 3, but target is a global name: `F = function(...) ... end`. Do not apply when `F` is a recovered local binding or when assignment order is part of a multi-assignment. | Emit `function F(...) ... end`. This matches NW globals like `RequireScript` and `FindLocalPlayerEntityId`, but P9b must not invent new globals. |
| 5 | Local function sugar from `local f = function` | SAFE-WITH-PRECONDITION | Statement is `local f = function(...) ... end`, `f` is a single local binding, and binding analysis proves the function body has no free reference to `f` that would change from an outer/global binding to this new local. Simpler v1 rule: require no reference to `f` inside the function body. | Emit `local function f(...) ... end`. If the function is recursive, prefer order 6's `local f; f = function` pattern instead. |
| 6 | Local recursive function sugar from declaration plus assignment | SAFE-WITH-PRECONDITION | Adjacent statements are exactly `local f` followed by `f = function(...) ... end`; both refer to the same local binding; there is no intervening read/write; the assignment RHS is only the function expression. | Emit `local function f(...) ... end`. This is the Lua 5.1-defined recursive local-function shape. |
| 7 | Method declaration sugar | SAFE-WITH-PRECONDITION | Input after orders 3/4 is `function T.method(self, ...) ... end` or equivalent assignment form. The first parameter binding is named `self`, or it is synthetic and P9b can rename that parameter binding and all uses to `self` without collision. | Emit `function T:method(...) ... end`. For recovered method names, preserve case. For synthetic table member names only, prefer PascalCase in NW-targeted mode. |
| 8 | Style synthetic receiver parameter to `self` | SAFE-WITH-PRECONDITION | Only inside a function selected for method sugar. First parameter is synthetic (`arg1` or equivalent), all reads resolve to that parameter binding, and no nested local named `self` collision would be introduced in the same scope. | Rename that binding to `self` and then apply order 7. Do not rename a recovered non-`self` first parameter. |
| 9 | Style synthetic local variables with semantic roles | SAFE-WITH-PRECONDITION | The name is synthetic and role evidence is strong: receiver `self`, numeric loop index `i`/`j`, generic unused slot `_`, recognized module table name, or a future semantic role assigned by P9. All uses must be binding-aware rewrites. | Use NW case: lowerCamel for locals/parameters, PascalCase for returned module/class tables, upper snake only for proven constant-like fields. Keep opaque `vN` names when no role is known. |
| 10 | Convert `T["field"] = function` to declaration sugar | SAFE-WITH-PRECONDITION | Same as order 3, plus the string key is a valid Lua identifier and not a keyword. | Emit `function T.field(...) ... end`; if first parameter qualifies, order 7 may emit `function T:field(...)`. |
| 11 | Reparse gate after P9b and after StyLua | SAFE | Always. | P9b output must parse before formatting, and formatted output must parse again. This catches invalid sugaring even when the transform was locally plausible. |

## Explicit Unsafe Skips

| Candidate | Status | Why P9b v1 should skip |
| --- | --- | --- |
| Rename recovered debug locals/upvalues/params | UNSAFE-SKIP | Violates fidelity. Also changes debug-library local-name observations. |
| Recase recovered table fields, method names, globals, or string keys | UNSAFE-SKIP | These are source/API names. In NW they often encode engine contracts and editor property bindings. |
| Localize globals (`WeaponEffectBase = {}` -> `local WeaponEffectBase = {}`) | UNSAFE-SKIP | Changes global side effects and external visibility. NW uses some global module tables intentionally. |
| Reshape `return { f = function(...) ... end }` into `local M = {}; function M:f(...) ...; return M` | UNSAFE-SKIP for v1 | Broadly changes structure, evaluation order around mixed table fields, duplicate-key behavior risk, closure upvalue shape, and debug behavior. It may be possible later under a much stricter proof, but not for P9b v1. |
| Move functions out of arbitrary table constructors | UNSAFE-SKIP for v1 | Table constructor field evaluation order and duplicate fields can matter. Metatable literals are often clearer with inline functions. |
| Convert `local f = function() ... f ... end` to `local function f` without binding proof | UNSAFE-SKIP | Lua 5.1 scoping differs; the function body may currently capture an outer/global `f`. |
| Convert non-`self` first parameters to method sugar | UNSAFE-SKIP unless synthetic and binding-renamed | `function T.method(this, x)` -> `function T:method(x)` would make the body's `this` unresolved unless all binding uses are rewritten. Never do this for recovered `this`, `a0`, etc. |
| Convert call sites `obj.method(obj, x)` to `obj:method(x)` | UNSAFE-SKIP for v1 | It is safe only under strict receiver identity/evaluation rules and preferably opcode evidence. P9b v1 focuses on declarations, not calls. |
| Sort or regroup statements for style | UNSAFE-SKIP | Statement order is executable in Lua. Do not move `RequireScript`, `BaseElement:CreateNewElement`, table initialization, bus connects, or field assignments. |
| Synthesize imports or rename `RequireScript` locals from path heuristics | UNSAFE-SKIP for v1 | NW import local case is mixed and context-dependent (`BaseElement` vs `styleHelpers` vs `crestTabCommon`). |

## Rename Rules

Never rename recovered identifiers:

- `Proto.loc_vars` names that are valid Lua identifiers.
- `Proto.upvalues` names that are valid Lua identifiers.
- Parameter names recovered from debug local records.
- Global names recovered from constants, such as `RequireScript`,
  `PlayerComponentRequestsBus`, or `WeaponEffectBase`.
- Table field names recovered from constants or source keys, such as
  `OnInit`, `Properties`, `ButtonSave`, `TEXT_CASING_NORMAL`, or
  `backgroundPathByCategory`.

P9b may style only synthetic names:

- Register fallbacks currently shaped like `v0`, `v1`, `v1_2`.
- Synthetic parameter fallbacks currently shaped like `arg1`, `arg2`.
- Synthetic upvalue fallbacks currently shaped like `up0`.
- Synthetic constant fallbacks currently shaped like `k0`, if ever emitted as
  identifiers.
- New names P9b itself introduces for a recognized module table or receiver
  parameter.

Even for synthetic names, P9b must be binding-aware. A text replacement is not
acceptable. The pass must know the declaration and every use of that local,
parameter, or upvalue binding.

## Recommended Naming Policy

For NW-targeted clean-code emission:

1. Preserve every recovered identifier exactly.
2. For recognized returned module/component tables with synthetic names, use
   PascalCase from the chunk/file stem when reliable.
3. For synthetic methods on recognized module/component tables, use PascalCase
   only if the member name itself is synthetic. Most member names are recovered
   from constants and must not be recased.
4. For method receiver parameters, use `self` when the binding is synthetic or
   already recovered as `self`.
5. For ordinary synthetic locals and parameters, prefer lowerCamel only when P9
   or P9b has semantic role evidence. Otherwise keep deterministic fallback
   names (`vN`, `argN`) rather than inventing misleading names.
6. For constants, use upper snake only when the field/local is synthetic and
   proven constant-like. Do not infer constness from value alone in v1.

Verdict: New World Lua is not snake_case. It is PascalCase for exported
component/module/class tables and methods, lowerCamel for ordinary locals and
fields, and UPPER_SNAKE_CASE for constant-like fields.

