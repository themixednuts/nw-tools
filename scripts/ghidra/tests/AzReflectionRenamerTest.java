import java.io.FileReader;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.regex.Pattern;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.program.flatapi.FlatProgramAPI;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.address.DefaultAddressFactory;
import ghidra.program.model.address.GenericAddressSpace;
import ghidra.program.model.listing.Program;
import ghidra.program.model.lang.Language;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;

/** Integration regressions against the local capture, which is not included in this test source. */
public class AzReflectionRenamerTest {
    private static final Class<?> SCRIPT = AzReflectionRenamer.class;

    public static void main(String[] args) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        JsonElement root;
        try (FileReader reader = new FileReader(args[0])) {
            root = JsonParser.parseReader(reader);
        }
        List<?> containers = (List<?>) call(script, "serializeContainers",
            new Class<?>[]{JsonElement.class}, root);
        check(containers.size() == 440, "capture specialization count");
        Map<String, Object> byId = new HashMap<>();
        for (Object container : containers) {
            byId.put((String) field(container, "typeId"), container);
        }
        checkName(byId, "EDDF6989-9C69-5031-BE72-6493E2FA3A39", "AZStd::vector<EncounterConditionEvent>");
        checkName(byId, "98FF2F25-6160-5D4C-B7C7-34B636703F59", "AZStd::vector<StateFlexCondition*>");
        checkName(byId, "4113BC4E-1027-500F-93A2-545C3DBB824D", "AZStd::unordered_map<unsigned int,AZStd::string>");
        checkName(byId, "6C8F8E52-AB4A-5C1F-8E56-9AC390290B94", "AZStd::unordered_set<AZ::EntityId>");
        long sharedTables = containers.stream().filter(c -> {
            try { return "NewWorld+0x7f22da8".equals(field(c, "vtable")); }
            catch (Exception error) { throw new RuntimeException(error); }
        }).count();
        check(sharedTables == 16, "folded pointer table preserves all owners");
        for (Object container : containers) {
            String name = (String) field(container, "typeName");
            check(name == null || !name.contains("shared_ptr<AZ::Entity*>") , "smart pointer pointee argument");
        }
        String pointer = (String) call(script, "safeTypeName", new Class<?>[]{String.class}, "AZStd::vector<T*>");
        String value = (String) call(script, "safeTypeName", new Class<?>[]{String.class}, "AZStd::vector<T>");
        check(pointer.equals("AZStd::vector<T*>") && !pointer.equals(value), "pointer names stay distinct");
        JsonObject pointerMember = JsonParser.parseString("{\"flags\":1}").getAsJsonObject();
        check("T*".equals(call(script, "memberPointerTypeName", new Class<?>[]{JsonObject.class, String.class},
            pointerMember, "T")), "legacy pointer flag fallback");
        checkCppNames(script, root);

        Object container = byId.get("EDDF6989-9C69-5031-BE72-6493E2FA3A39");
        GenericAddressSpace space = new GenericAddressSpace("ram", 64, AddressSpace.TYPE_RAM, 0);
        DefaultAddressFactory factory = new DefaultAddressFactory(new AddressSpace[]{space});
        Address base = space.getAddress(0x140000000L);
        JsonArray slots = (JsonArray) field(container, "slots");
        Map<Address, Long> pointers = new HashMap<>();
        Address table = base.add(0x7f9fa70);
        for (int i = 0; i < 23; i++) {
            String captured = slots.get(i).getAsJsonObject().get("address").getAsString();
            pointers.put(table.add(i * 8L), base.add(Long.parseUnsignedLong(captured.split("0x")[1], 16)).getOffset());
        }
        MemoryBlock block = (MemoryBlock) Proxy.newProxyInstance(SCRIPT.getClassLoader(), new Class<?>[]{MemoryBlock.class},
            (proxy, method, values) -> method.getName().equals("isExecute") ? true : null);
        Memory memory = (Memory) Proxy.newProxyInstance(SCRIPT.getClassLoader(), new Class<?>[]{Memory.class},
            (proxy, method, values) -> switch (method.getName()) {
                case "contains" -> true;
                case "getBlock" -> block;
                case "getLong" -> pointers.get((Address) values[0]);
                default -> throw new UnsupportedOperationException(method.getName());
            });
        boolean[] bigEndian = {false};
        int[] pointerSize = {8};
        Language language = (Language) Proxy.newProxyInstance(SCRIPT.getClassLoader(), new Class<?>[]{Language.class},
            (proxy, method, values) -> method.getName().equals("isBigEndian") ? bigEndian[0] : null);
        Program program = (Program) Proxy.newProxyInstance(SCRIPT.getClassLoader(), new Class<?>[]{Program.class},
            (proxy, method, values) -> switch (method.getName()) {
                case "getName" -> "NewWorld.exe";
                case "getDefaultPointerSize" -> pointerSize[0];
                case "getLanguage" -> language;
                case "getImageBase" -> base;
                case "getAddressFactory" -> factory;
                case "getMemory" -> memory;
                default -> throw new UnsupportedOperationException(method.getName());
            });
        setField(FlatProgramAPI.class, script, "currentProgram", program);
        check(Boolean.TRUE.equals(call(script, "supportsContainerPointerLayout", new Class<?>[]{})),
            "renamed program is accepted before live pointer validation");
        bigEndian[0] = true;
        check(Boolean.FALSE.equals(call(script, "supportsContainerPointerLayout", new Class<?>[]{})),
            "big-endian program is rejected");
        bigEndian[0] = false;
        pointerSize[0] = 4;
        check(Boolean.FALSE.equals(call(script, "supportsContainerPointerLayout", new Class<?>[]{})),
            "32-bit pointer layout is rejected");
        pointerSize[0] = 8;
        AzReflectionRenamer entrypoint = new AzReflectionRenamer();
        setField(FlatProgramAPI.class, entrypoint, "currentProgram", program);
        Method requireOpenProgram = SCRIPT.getDeclaredMethod("requireOpenProgram");
        requireOpenProgram.setAccessible(true);
        requireOpenProgram.invoke(entrypoint);
        setField(FlatProgramAPI.class, entrypoint, "currentProgram", null);
        java.io.StringWriter startupConsole = new java.io.StringWriter();
        entrypoint.set(new ghidra.app.script.GhidraState(null, null, null, null, null, null),
            ghidra.util.task.TaskMonitor.DUMMY, new java.io.PrintWriter(startupConsole));
        check(Boolean.FALSE.equals(requireOpenProgram.invoke(entrypoint)),
            "missing GUI program stops gracefully before input dialogs");
        check(startupConsole.toString().contains("CodeBrowser"), "missing program identifies the correct window");
        setField(SCRIPT, script, "pointerReadCache", new HashMap<>());
        setField(SCRIPT, script, "executableAddressCache", new HashMap<>());
        Class<?> containerClass = container.getClass();
        check(call(script, "validateContainerTable", new Class<?>[]{containerClass}, container) == null,
            "module-relative addresses rebase and validate");
        JsonArray mutated = slots.deepCopy();
        mutated.get(23).getAsJsonObject().addProperty("address", "NewWorld+0xdeadbeef");
        setField(containerClass, container, "slots", mutated);
        check(call(script, "validateContainerTable", new Class<?>[]{containerClass}, container) == null,
            "overread slots beyond index 22 are ignored");
        mutated.get(1).getAsJsonObject().addProperty("address", "NewWorld+0xdeadbeef");
        check("live-slot-mismatch-1".equals(call(script, "validateContainerTable", new Class<?>[]{containerClass}, container)),
            "mismatching capture is rejected before mutations");
        mutated = slots.deepCopy();
        mutated.get(2).getAsJsonObject().addProperty("index", 1);
        setField(containerClass, container, "slots", mutated);
        check("invalid-slot-index-2".equals(call(script, "validateContainerTable", new Class<?>[]{containerClass}, container)),
            "slot ordering is validated");
        JsonArray truncated = new JsonArray();
        for (int i = 0; i < 22; i++) truncated.add(slots.get(i));
        setField(containerClass, container, "slots", truncated);
        check("incomplete-container-slots".equals(call(script, "validateContainerTable", new Class<?>[]{containerClass}, container)),
            "truncated table is rejected");
        System.out.println("PASS: capture identities, pointers, shared tables, rebasing, boundaries, " +
            "mismatch rejection, renamed programs and missing-program diagnostics");
    }

    private static void checkName(Map<String, Object> byId, String id, String expected) throws Exception {
        check(expected.equals(field(byId.get(id), "typeName")), expected + " got " + field(byId.get(id), "typeName"));
    }

    @SuppressWarnings("unchecked")
    private static void checkCppNames(AzReflectionRenamer script, JsonElement root) throws Exception {
        Map<String, List<String>> examples = new java.util.LinkedHashMap<>();
        examples.put("AZStd::vector<AZ::EntityId>", List.of("AZStd", "vector<AZ::EntityId>"));
        examples.put("::class AZStd::array<AZ::Vector3,2>", List.of("AZStd", "array<AZ::Vector3,2>"));
        examples.put("Javelin::ClientMessages::Payload<MB::Replica<AZStd::pair<AZ::EntityId,float>>>",
            List.of("Javelin", "ClientMessages", "Payload<MB::Replica<AZStd::pair<AZ::EntityId,float>>>"));
        examples.put("MB::Replica<AZStd::vector<Javelin::ClientMessages::Entry*>>::Encode<AZ::Uuid>",
            List.of("MB", "Replica<AZStd::vector<Javelin::ClientMessages::Entry*>>", "Encode<AZ::Uuid>"));
        examples.put("AZStd::integral_constant<int,-1>", List.of("AZStd", "integral_constant<int,-1>"));
        examples.put("N::Holder<void(*)(AZ::EntityId,MB::Result)>::Visit<Javelin::Message>",
            List.of("N", "Holder<void(*)(AZ::EntityId,MB::Result)>", "Visit<Javelin::Message>"));
        examples.put("MB::Holder<AZ::Uuid>::operator<", List.of("MB", "Holder<AZ::Uuid>", "operator<"));
        examples.put("MB::Holder<AZ::Uuid>::operator<<", List.of("MB", "Holder<AZ::Uuid>", "operator<<"));
        examples.put("MB::Holder<AZ::Uuid>::operator()", List.of("MB", "Holder<AZ::Uuid>", "operator()"));
        examples.put("AZStd::vector<class Javelin::Payload<struct MB::Key>>",
            List.of("AZStd", "vector<Javelin::Payload<MB::Key>>"));
        examples.put("LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::Component>(void",
            List.of("LocalComponentRef<InterfaceType><const_char_*_cdecl_MB::GetTypeName<Javelin::Component>(void"));
        for (Map.Entry<String, List<String>> example : examples.entrySet()) {
            Object result = call(script, "cppScopeParts", new Class<?>[]{String.class}, example.getKey());
            check(example.getValue().equals(result), "C++ scope parts for " + example.getKey() + " got " + result);
        }
        List<String> typeScope = (List<String>) call(script, "reflectedDatatypeScope",
            new Class<?>[]{String.class, String.class}, null, "AZStd::array<AZ::Vector3,2>");
        check(typeScope.equals(List.of("AZStd", "array<AZ::Vector3,2>")), "datatype leaf and parent category");
        Object category = call(script, "datatypeCategoryPath", new Class<?>[]{List.class}, typeScope);
        check("/AZStd".equals(category.toString()), "qualified datatype belongs to /AZStd");
        check(Boolean.FALSE.equals(call(script, "isClassScope", new Class<?>[]{List.class, int.class}, typeScope, 0)),
            "AZStd is a namespace");
        check(Boolean.TRUE.equals(call(script, "isClassScope", new Class<?>[]{List.class, int.class}, typeScope, 1)),
            "array specialization is a class");
        for (String namespace : List.of("MB", "Javelin")) {
            check(Boolean.FALSE.equals(call(script, "isClassScope", new Class<?>[]{List.class, int.class},
                List.of(namespace), 0)), namespace + " is a namespace even for free template functions");
        }
        List<String> nested = (List<String>) call(script, "behaviorQualifiedScope",
            new Class<?>[]{String.class, String.class}, "Javelin::Outer<MB::Key>::Inner", "Fallback");
        check(Boolean.TRUE.equals(call(script, "isClassScope", new Class<?>[]{List.class, int.class}, nested, 1)),
            "templated outer owner is a class");
        JsonObject method = new JsonObject();
        method.addProperty("name", "MB::Owner<AZ::Uuid>::Convert<AZStd::vector<Javelin::Entry>>");
        check("Convert<AZStd::vector<Javelin::Entry>>".equals(call(script, "behaviorLocalName",
            new Class<?>[]{JsonObject.class, String.class}, method, "Fallback")), "qualified template function leaf");
        Field signature = SCRIPT.getDeclaredField("MB_GET_TYPE_NAME_SIGNATURE_RE");
        signature.setAccessible(true);
        Pattern pattern = (Pattern) signature.get(null);
        String mbType = "AZStd::vector<Javelin::Payload<MB::Replica<AZ::EntityId>>>";
        var matcher = pattern.matcher("const char *__cdecl MB::GetTypeName<class " + mbType + ">(void)");
        check(matcher.find() && mbType.equals(matcher.group("type")), "MB nested template GetTypeName signature");
        matcher = pattern.matcher("const char *__cdecl MB::GetTypeName<unsigned int>(void)");
        check(matcher.find() && "unsigned int".equals(matcher.group("type")), "MB scalar template specialization");
        List<JsonObject> objects = (List<JsonObject>) call(script, "collectObjects", new Class<?>[]{JsonElement.class}, root);
        Map<String, Integer> namespaces = new TreeMap<>();
        int checked = 0;
        int malformed = 0;
        for (JsonObject object : objects) {
            JsonElement value = object.get("typeName");
            if (value == null || value.isJsonNull() || !value.isJsonPrimitive()) continue;
            String name = value.getAsString();
            if (name.isBlank() || name.contains("Unsourced name:")) continue;
            List<String> parts = (List<String>) call(script, "cppScopeParts", new Class<?>[]{String.class}, name);
            String normalized = name.replaceAll("\\b(?:class|struct|enum)\\s+", "").replaceFirst("^::", "");
            String expected = (String) call(script, "safeTypeName", new Class<?>[]{String.class}, normalized);
            check(expected.equals(String.join("::", parts)), "capture type spelling survives parsing: " + name);
            if (parts.size() > 1) namespaces.merge(parts.get(0), 1, Integer::sum);
            if (!Boolean.TRUE.equals(call(script, "balancedCppDelimiters", new Class<?>[]{String.class}, normalized))) {
                check(parts.size() == 1, "truncated RTTI name stays atomic: " + name);
                malformed++;
            }
            matcher = pattern.matcher(name);
            if (matcher.find()) {
                List<String> function = (List<String>) call(script, "cppScopeParts", new Class<?>[]{String.class},
                    "MB::GetTypeName<" + matcher.group("type") + ">");
                check(function.size() == 2 && function.get(0).equals("MB"), "MB signature in captured RTTI");
                namespaces.merge("MB", 1, Integer::sum);
            }
            checked++;
        }
        Map<String, String> rawNames = (Map<String, String>) call(script, "indexRawTypeNames",
            new Class<?>[]{List.class}, objects);
        Map<String, JsonObject> generics = (Map<String, JsonObject>) call(script, "indexGenericInfo",
            new Class<?>[]{JsonElement.class, List.class, Map.class}, root, objects, field(script, "jsonObjectsById"));
        int specializations = 0;
        for (Map.Entry<String, JsonObject> entry : generics.entrySet()) {
            String name = (String) call(script, "displayTypeName",
                new Class<?>[]{String.class, String.class, Map.class, Map.class, java.util.Set.class},
                entry.getKey(), rawNames.get(entry.getKey()), generics, rawNames, new java.util.HashSet<>());
            if (name == null) continue;
            List<String> parts = (List<String>) call(script, "cppScopeParts", new Class<?>[]{String.class}, name);
            check(parts.size() == 2 && parts.get(0).equals("AZStd") || !name.startsWith("AZStd::"),
                "generic owner namespace is separated from template arguments: " + name);
            if (parts.size() > 1) namespaces.merge(parts.get(0), 1, Integer::sum);
            specializations++;
        }
        check(namespaces.containsKey("Javelin") && namespaces.containsKey("MB") && namespaces.containsKey("AZStd"),
            "capture coverage includes engine and game namespaces: " + namespaces);
        System.out.println("PASS: " + checked + " RTTI names, " + specializations + " resolved generics, " +
            malformed + " truncated name records preserved; namespace roots " + namespaces.keySet());
    }

    private static Object field(Object instance, String name) throws Exception {
        Field field = instance.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(instance);
    }

    private static void setField(Class<?> type, Object instance, String name, Object value) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        field.set(instance, value);
    }

    private static Object call(Object instance, String name, Class<?>[] types, Object... args) throws Exception {
        Method method = SCRIPT.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return method.invoke(instance, args);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
