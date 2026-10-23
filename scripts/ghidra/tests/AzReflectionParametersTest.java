import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraState;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.LongLongDataType;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.data.Undefined8DataType;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.LocalVariableImpl;
import ghidra.program.model.listing.Parameter;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.listing.Variable;
import ghidra.program.model.listing.VariableStorage;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.task.TaskMonitor;

/** Replays the game's automatic-this / reflection signature through the complete import. */
public class AzReflectionParametersTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB program = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory(Path.of(args[1]), "az-parameters-test-");
        try {
            var base = program.getAddressFactory().getDefaultAddressSpace().getAddress(0x7ff600000000L);
            var entry = base.add(0x2731ac0);
            Function function;
            int setup = program.startTransaction("existing member Reflect signature");
            try {
                program.setImageBase(base, false);
                program.getMemory().createInitializedBlock("code", entry, 0x100, (byte) 0xc3,
                    TaskMonitor.DUMMY, false).setExecute(true);
                program.getMemory().setBytes(entry, new byte[]{0x48, (byte) 0x8b, (byte) 0xca, (byte) 0xc3});
                check(new DisassembleCommand(entry, null, true).applyTo(program, TaskMonitor.DUMMY), "decode fixture");
                var symbols = program.getSymbolTable();
                var cls = symbols.createClass(program.getGlobalNamespace(), "TeleportContributorsAction", SourceType.USER_DEFINED);
                var contextPlaceholder = new StructureDataType(new CategoryPath("/AZ"), "ReflectContext", 0);
                contextPlaceholder.setDescription("PlaceHolder Class Structure");
                var context = program.getDataTypeManager().addDataType(contextPlaceholder, null);
                var opaquePlaceholder = new StructureDataType(CategoryPath.ROOT, "OpaqueOwner", 0);
                opaquePlaceholder.setDescription("PlaceHolder Class Structure");
                program.getDataTypeManager().addDataType(opaquePlaceholder, null);
                // A stale empty generated name beside the filled canonical layout it should redirect to.
                var canonicalMoved = new StructureDataType(new CategoryPath("/Javelin"), "Moved", 8);
                canonicalMoved.replaceAtOffset(0, IntegerDataType.dataType, 4, "m_kept", null);
                program.getDataTypeManager().addDataType(canonicalMoved, null);
                var staleMoved = new StructureDataType(CategoryPath.ROOT, "Moved", 8);
                staleMoved.setDescription("AZ SerializeContext reflected type: Moved (9B3E2C41-0D5A-4E8F-A1C7-3F6B2D9E8A10)");
                var stale = program.getDataTypeManager().addDataType(staleMoved, null);
                var holder = new StructureDataType(CategoryPath.ROOT, "MovedHolder", 0);
                holder.add(new PointerDataType(stale), 8, "m_moved", null);
                program.getDataTypeManager().addDataType(holder, null);
                function = program.getFunctionManager().createFunction("Reflect", cls, entry,
                    new AddressSet(entry, entry.add(3)), SourceType.USER_DEFINED);
                function.setCallingConvention("__thiscall");
                function.addParameter(new ParameterImpl("reflection", new PointerDataType(context), program), SourceType.USER_DEFINED);
                program.getFunctionManager().createFunction("Apply", cls, entry.add(0x20),
                    new AddressSet(entry.add(0x20)), SourceType.USER_DEFINED);
            }
            finally { program.endTransaction(setup, true); }
            Files.writeString(resources.resolve("behavior-context.json"), """
                {"classes":[{"name":"TeleportContributorsAction",
                "typeId":"E2AE1B42-7DEC-4AE8-B3E0-B263C2602E78","size":"32","alignment":"8",
                "m_methods":[{"m_name":"Apply","m_functionPtr":"NewWorld+0x2731ae0",
                "m_reflectFunction":["NewWorld+0x2731ac0"]}]},
                {"name":"OpaqueOwner","typeId":"6C1D6B30-5A6F-4D2B-9C55-0E8F5B7A1D24","size":"24","alignment":"8"},
                {"name":"Javelin::Moved","typeId":"9B3E2C41-0D5A-4E8F-A1C7-3F6B2D9E8A10","size":"8","alignment":"4"}]}
                """);
            Files.writeString(resources.resolve("serialize.json"), """
                {"uuidMap":{"9B3E2C41-0D5A-4E8F-A1C7-3F6B2D9E8A10":{
                 "name":"Moved","typeId":"9B3E2C41-0D5A-4E8F-A1C7-3F6B2D9E8A10","version":"1","attributes":[],
                 "azRtti":{"typeId":"9B3E2C41-0D5A-4E8F-A1C7-3F6B2D9E8A10","typeName":"Javelin::Moved"},
                 "elements":[{"name":"m_kept","typeId":"72039442-EB38-4D42-A1AD-CB68F7E0EEF6",
                   "offset":"0","dataSize":"4","is_pointer":false,"is_base_class":false,"attributes":[]}]},
                 "6C1D6B30-5A6F-4D2B-9C55-0E8F5B7A1D24":{
                 "name":"OpaqueOwner","typeId":"6C1D6B30-5A6F-4D2B-9C55-0E8F5B7A1D24","version":"1","attributes":[],"elements":[]},
                 "E2AE1B42-7DEC-4AE8-B3E0-B263C2602E78":{
                 "name":"TeleportContributorsAction","typeId":"E2AE1B42-7DEC-4AE8-B3E0-B263C2602E78","version":"1","attributes":[],
                 "azRtti":{"typeId":"E2AE1B42-7DEC-4AE8-B3E0-B263C2602E78",
                 "typeName":"TeleportContributorsAction","address":"NewWorld+0x2731ac1"},
                 "elements":[{"name":"m_value","typeId":"43DA906B-7DEF-4CA8-9790-854106D3F983",
                   "offset":"0","dataSize":"4","is_pointer":false,"is_base_class":false,"attributes":[]}]}}}
                """);
            long reflectionId = function.getParameters()[1].getSymbol().getID();
            Structure oldThisType = (Structure) ((ghidra.program.model.data.Pointer) function.getParameters()[0].getDataType()).getDataType();
            long thisTypeId = program.getDataTypeManager().getID(oldThisType);
            String before = function.getPrototypeString(true, true);
            var coreContext = program.getDataTypeManager().getDataType(new CategoryPath("/AZ"), "ReflectContext");
            long coreContextId = program.getDataTypeManager().getID(coreContext);
            run(program, resources, true);
            check(function.getPrototypeString(true, true).equals(before) && coreContext.getLength() == 1, "preview changed the signature or core placeholder");
            JsonObject validation = JsonParser.parseString(Files.readString(resources.resolve("reflection-dry-run.json"))).getAsJsonObject();
            check(validation.get("checkedSignatureCount").getAsInt() > 0 &&
                validation.getAsJsonObject("signaturePlanSamples").toString().contains("RDX:8"), "dry run did not inspect the real signature/storage plan");
            run(program, resources, false);
            var parameters = function.getParameters();
            check(parameters.length == 2 && parameters[0].isAutoParameter() && parameters[0].getName().equals("this") &&
                parameters[1].getName().equals("reflection"), "automatic this was overwritten or duplicated");
            check(parameters[1].getRegister().getName().equals("RDX"), "context moved out of its proven native register");
            check(parameters[1].getSymbol().getID() == reflectionId, "existing reflection symbol replaced");
            check(coreContext.getLength() == 16 && ((Structure) coreContext).getComponentAt(0).getFieldName().equals("vftable") &&
                program.getDataTypeManager().getID(coreContext) == coreContextId, "core placeholder was replaced or left empty");
            Structure thisType = (Structure) ((ghidra.program.model.data.Pointer) parameters[0].getDataType()).getDataType();
            check(thisType.getLength() == 32 && thisType.getComponentAt(0).getFieldName().equals("m_value") &&
                program.getDataTypeManager().getID(thisType) == thisTypeId, "automatic this still points at an empty class placeholder");
            DataType opaque = program.getDataTypeManager().getDataType(CategoryPath.ROOT, "OpaqueOwner");
            check(opaque.getLength() == 24 && !opaque.isZeroLength(), "zero-length placeholder grew short of its native size: " + opaque.getLength());
            var manager = program.getDataTypeManager();
            DataType moved = manager.getDataType(new CategoryPath("/Javelin"), "Moved");
            Structure movedHolder = (Structure) manager.getDataType(CategoryPath.ROOT, "MovedHolder");
            check(moved instanceof Structure && ((Structure) moved).getComponentAt(0).getFieldName().equals("m_kept") &&
                manager.getDataType(CategoryPath.ROOT, "Moved") == null &&
                ((ghidra.program.model.data.Pointer) movedHolder.getComponent(0).getDataType()).getDataType().isEquivalent(moved) &&
                ((ghidra.program.model.data.Pointer) movedHolder.getComponent(0).getDataType()).getDataType().getCategoryPath().getPath().equals("/Javelin"),
                "placeholder redirect moved the canonical layout out of its namespace");
            run(program, resources, false);
            JsonObject report = JsonParser.parseString(Files.readString(resources.resolve("reflection-report.json"))).getAsJsonObject();
            check(report.get("appliedCount").getAsInt() == 0, "parameter repair is not idempotent");
            checkEdgeCases(program);
            System.out.println("PASS: exact TeleportContributorsAction failure, datatype-before-signature ordering, placeholder/parameter IDs, " +
                "automatic and explicit this, collision-safe names, hidden returns, indirect values, custom storage, " +
                "atomic return/parameter plans, stack conflict preservation, bounded dynamic stack repair, preview and retry");
        }
        finally {
            program.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void checkEdgeCases(ProgramDB program) throws Exception {
        int transaction = program.startTransaction("parameter/storage edge cases");
        try {
            var manager = program.getDataTypeManager();
            StructureDataType ownerType = new StructureDataType("ParameterOwner", 16);
            ownerType.replaceAtOffset(0, IntegerDataType.dataType, 4, "m_value", null);
            Structure owner = (Structure) manager.addDataType(ownerType, null);
            DataType thisPointer = new PointerDataType(owner, manager);
            DataType context = new PointerDataType(manager.getDataType(new CategoryPath("/AZ"), "ReflectContext"), manager);
            DataType value = LongLongDataType.dataType;
            var cls = program.getSymbolTable().createClass(program.getGlobalNamespace(), "ParameterOwner", SourceType.USER_DEFINED);
            var start = program.getImageBase().add(0x3000000);
            program.getMemory().createInitializedBlock("parameter-edge-code", start, 0x1000, (byte) 0xc3,
                TaskMonitor.DUMMY, false).setExecute(true);
            int offset = 0;

            Function member = function(program, cls, start.add(offset++ * 16), "member", "__thiscall");
            member.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            long memberParamId = member.getParameters()[1].getSymbol().getID();
            checkSignature(program, member, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(member.getParameters().length == 2 && member.getParameters()[0].isAutoParameter() &&
                member.getParameters()[1].getRegister().getName().equals("RDX") &&
                member.getParameters()[1].getSymbol().getID() == memberParamId, "member receiver/storage was lost");
            checkSignature(program, member, new String[]{"this"}, new DataType[]{thisPointer}, null, false, null);

            var adapterClass = program.getSymbolTable().createClass(program.getGlobalNamespace(), "ReflectionAdapter", SourceType.USER_DEFINED);
            Function adapterMember = function(program, adapterClass, start.add(offset++ * 16), "wrongAutoType", "__thiscall");
            adapterMember.addParameter(new ParameterImpl("payload", value, program), SourceType.USER_DEFINED);
            long payloadId = adapterMember.getParameters()[1].getSymbol().getID();
            JsonObject typedReceiver = checkSignature(program, adapterMember, new String[]{"this"}, new DataType[]{thisPointer}, null, true, null);
            check(adapterMember.getParameters()[0].getName().equals("this") && adapterMember.getParameters()[0].getDataType().isEquivalent(thisPointer) &&
                adapterMember.getParameters()[0].getRegister().getName().equals("RCX") &&
                adapterMember.getParameters()[1].getRegister().getName().equals("RDX") &&
                adapterMember.getParameters()[1].getSymbol().getID() == payloadId &&
                typedReceiver.get("materializedAutomaticThis").getAsBoolean(), "known native owner was replaced by an adapter's placeholder this type");

            Function staticReflect = function(program, cls, start.add(offset++ * 16), "staticReflect", "__fastcall");
            staticReflect.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            checkSignature(program, staticReflect, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(staticReflect.getParameters().length == 1 && staticReflect.getParameters()[0].getRegister().getName().equals("RCX"),
                "static context shifted into a receiver slot");

            Function explicit = function(program, cls, start.add(offset++ * 16), "explicitThis", "__fastcall");
            explicit.addParameter(new ParameterImpl("this", thisPointer, program), SourceType.USER_DEFINED);
            explicit.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            checkSignature(program, explicit, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(explicit.getParameters()[0].getName().equals("this") &&
                explicit.getParameters()[1].getRegister().getName().equals("RDX"), "explicit receiver overwritten");

            Function collision = function(program, cls, start.add(offset++ * 16), "collision", "__fastcall");
            for (String name : new String[]{"old_context", "reflection", "reflection_2"}) {
                collision.addParameter(new ParameterImpl(name, value, program), SourceType.USER_DEFINED);
            }
            collision.getParameters()[1].setComment("keep this trailing parameter");
            long trailingId = collision.getParameters()[1].getSymbol().getID();
            JsonObject collisionAction = checkSignature(program, collision, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(collision.getParameters()[0].getName().equals("reflection_3") &&
                collision.getParameters()[1].getName().equals("reflection") &&
                collision.getParameters()[1].getComment().equals("keep this trailing parameter") &&
                collision.getParameters()[1].getSymbol().getID() == trailingId &&
                collisionAction.getAsJsonArray("parameterNameAdjustments").size() == 1, "trailing collision damaged existing parameters");

            Function localCollision = function(program, cls, start.add(offset++ * 16), "localCollision", "__fastcall");
            localCollision.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            localCollision.addLocalVariable(new LocalVariableImpl("reflection", value, -16, program), SourceType.USER_DEFINED);
            localCollision.addLocalVariable(new LocalVariableImpl("reflection_2", value, -32, program), SourceType.USER_DEFINED);
            long localId = localCollision.getLocalVariables()[0].getSymbol().getID();
            checkSignature(program, localCollision, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(localCollision.getParameters()[0].getName().equals("reflection_3") &&
                localCollision.getLocalVariables()[0].getSymbol().getID() == localId, "local collision renamed/deleted a local");

            Function descriptor = function(program, cls, start.add(offset++ * 16), "descriptor", "__thiscall");
            descriptor.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            checkSignature(program, descriptor, new String[]{"this", "reflection"}, new DataType[]{thisPointer, context}, null, true, null);
            check(descriptor.getParameters().length == 2 && descriptor.getParameters()[0].isAutoParameter(), "descriptor got a second receiver");

            Function callback = function(program, cls, start.add(offset++ * 16), "callback", "__fastcall");
            callback.addParameter(new ParameterImpl("old_receiver", value, program), SourceType.ANALYSIS);
            callback.addParameter(new ParameterImpl("payload", value, program), SourceType.USER_DEFINED);
            checkSignature(program, callback, new String[]{"this"}, new DataType[]{thisPointer}, null, true, null);
            check(callback.getParameters()[0].getName().equals("this") && callback.getParameters()[0].getDataType().isEquivalent(thisPointer) &&
                callback.getParameters()[1].getName().equals("payload"), "member callback has no correctly typed receiver");

            Structure large = (Structure) manager.addDataType(new StructureDataType("LargeReturn", 24), null);
            Function hidden = function(program, cls, start.add(offset++ * 16), "hiddenReturn", "__thiscall");
            hidden.setReturnType(large, SourceType.USER_DEFINED);
            hidden.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            hidden.addParameter(new ParameterImpl("payload", large, program), SourceType.USER_DEFINED);
            Parameter[] hiddenBefore = hidden.getParameters();
            check(Arrays.stream(hiddenBefore).filter(Parameter::isAutoParameter).count() == 2, "fixture lacks hidden return and receiver");
            String[] automaticStorage = Arrays.stream(hiddenBefore).filter(Parameter::isAutoParameter)
                .map(parameter -> parameter.getVariableStorage().toString()).toArray(String[]::new);
            checkSignature(program, hidden, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(Arrays.equals(automaticStorage, Arrays.stream(hidden.getParameters()).filter(Parameter::isAutoParameter)
                .map(parameter -> parameter.getVariableStorage().toString()).toArray(String[]::new)) &&
                hidden.getReturn().getFormalDataType().isEquivalent(large) &&
                hidden.getParameters()[3].getFormalDataType().isEquivalent(large) && hidden.getParameters()[3].isForcedIndirect(),
                "hidden return or indirect value ABI was flattened");
            checkSignature(program, hidden, new String[]{"this"}, new DataType[]{context}, null, false,
                "signature-auto-this-type-unresolved");

            // __fastcall puts the hidden return pointer in RCX, but a member receiver owns RCX.
            Function hiddenCallback = function(program, cls, start.add(offset++ * 16), "hiddenReturnCallback", "__fastcall");
            hiddenCallback.setReturnType(large, SourceType.USER_DEFINED);
            hiddenCallback.addParameter(new ParameterImpl("old_receiver", value, program), SourceType.ANALYSIS);
            checkSignature(program, hiddenCallback, new String[]{"this"}, new DataType[]{thisPointer}, null, false,
                "signature-receiver-hidden-return-order-unresolved");

            Function custom = function(program, cls, start.add(offset++ * 16), "custom", "__fastcall");
            custom.setCustomVariableStorage(true);
            custom.addParameter(new ParameterImpl("old_context", value, program.getRegister("RDX"), program), SourceType.USER_DEFINED);
            custom.addParameter(new ParameterImpl("payload", value, 0x38, program), SourceType.USER_DEFINED);
            String customStorage = custom.getParameters()[1].getVariableStorage().toString();
            checkSignature(program, custom, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(custom.hasCustomVariableStorage() && custom.getParameters()[0].getRegister().getName().equals("RDX") &&
                custom.getParameters()[1].getVariableStorage().toString().equals(customStorage), "valid custom register/stack storage overwritten");
            LocalVariableImpl registerConflict = new LocalVariableImpl("claimed_register", 0, value, program.getRegister("RDX"), program);
            checkSignature(program, view(custom, custom.getParameters(), new Variable[]{registerConflict}),
                new String[]{"context"}, new DataType[]{context}, null, false, "signature-custom-local-storage-conflict");

            Function small = function(program, cls, start.add(offset++ * 16), "smallCustom", "__fastcall");
            small.setCustomVariableStorage(true);
            small.addParameter(new ParameterImpl("old_context", IntegerDataType.dataType, program.getRegister("ECX"), program), SourceType.USER_DEFINED);
            checkSignature(program, small, new String[]{"reflection"}, new DataType[]{context}, null, false, "signature-custom-storage-unresolved");
            Function emptyCustom = function(program, cls, start.add(offset++ * 16), "emptyCustom", "__fastcall");
            emptyCustom.setCustomVariableStorage(true);
            checkSignature(program, emptyCustom, new String[]{"reflection"}, new DataType[]{context}, null, false, "signature-custom-storage-unresolved");

            Function customReturn = function(program, cls, start.add(offset++ * 16), "customReturn", "__fastcall");
            customReturn.setReturnType(IntegerDataType.dataType, SourceType.USER_DEFINED);
            customReturn.setCustomVariableStorage(true);
            customReturn.addParameter(new ParameterImpl("old_context", value, program.getRegister("RCX"), program), SourceType.USER_DEFINED);
            checkSignature(program, customReturn, new String[]{"context"}, new DataType[]{context}, context, false,
                "signature-custom-return-storage-unresolved");
            check(customReturn.getParameters()[0].getName().equals("old_context") &&
                customReturn.getReturnType().isEquivalent(IntegerDataType.dataType), "blocked return update partially edited parameters");

            Function dynamicReturn = function(program, cls, start.add(offset++ * 16), "dynamicReturn", "__fastcall");
            dynamicReturn.setReturnType(IntegerDataType.dataType, SourceType.USER_DEFINED);
            dynamicReturn.addParameter(new ParameterImpl("old_context", value, program), SourceType.ANALYSIS);
            checkSignature(program, dynamicReturn, new String[]{"context"}, new DataType[]{context}, context, true, null);
            check(dynamicReturn.getReturnType().isEquivalent(context) &&
                dynamicReturn.getReturn().getRegister().getName().equals("RAX") &&
                dynamicReturn.getParameters()[0].getRegister().getName().equals("RCX"), "RTTI cast return/context registers were not typed together");

            Function stacked = function(program, cls, start.add(offset++ * 16), "stacked", "__fastcall");
            for (int i = 0; i < 5; i++) stacked.addParameter(new ParameterImpl(i == 0 ? "reflection" : "value_" + i,
                i == 0 ? context : value, program), SourceType.USER_DEFINED);
            Parameter[] actual = stacked.getParameters();
            check(actual[4].hasStackStorage(), "fixture has no stack parameter");
            LocalVariableImpl conflictingLocal = new LocalVariableImpl("claimed_stack_slot", value, actual[4].getStackOffset(), program);
            Function conflictView = view(stacked, actual, new Variable[]{conflictingLocal});
            checkSignature(program, conflictView, new String[]{"context"}, new DataType[]{context}, null, false,
                "signature-stack-storage-conflict");
            check(stacked.getParameters()[0].getName().equals("reflection"), "ambiguous stack overlap was forcibly edited");

            Parameter[] missingStack = actual.clone();
            missingStack[4] = new ParameterImpl(actual[4].getName(), actual[4].getDataType(), program);
            Function staleView = view(stacked, missingStack, stacked.getLocalVariables());
            JsonObject repaired = checkSignature(program, staleView, new String[]{"reflection"}, new DataType[]{context}, null, true, null);
            check(repaired.get("storageRepair").getAsString().equals("recompute-invalid-dynamic-parameter-storage") &&
                stacked.getParameters()[4].getStackOffset() == actual[4].getStackOffset(), "invalid dynamic stack storage did not use the compiler ABI");
        }
        finally { program.endTransaction(transaction, false); }
    }

    private static Function function(ProgramDB program, ghidra.program.model.symbol.Namespace namespace,
        ghidra.program.model.address.Address entry, String name, String convention) throws Exception {
        Function function = program.getFunctionManager().createFunction(name, namespace, entry, new AddressSet(entry), SourceType.USER_DEFINED);
        function.setCallingConvention(convention);
        return function;
    }

    private static Function view(Function actual, Parameter[] parameters, Variable[] locals) {
        return (Function) Proxy.newProxyInstance(Function.class.getClassLoader(), new Class<?>[]{Function.class},
            (proxy, method, args) -> {
                if (method.getName().equals("getParameters")) return parameters;
                if (method.getName().equals("getLocalVariables")) return locals;
                try { return method.invoke(actual, args); }
                catch (InvocationTargetException error) { throw error.getCause(); }
            });
    }

    private static JsonObject checkSignature(ProgramDB program, Function function, String[] names, DataType[] types,
        DataType returnType, boolean expectedChanged, String skipReason) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY, new PrintWriter(new StringWriter()));
        var symbols = AzReflectionRenamer.class.getDeclaredField("symbols");
        symbols.setAccessible(true);
        symbols.set(script, program.getSymbolTable());
        var apply = AzReflectionRenamer.class.getDeclaredField("applyRenames");
        apply.setAccessible(true);
        var update = AzReflectionRenamer.class.getDeclaredMethod("updateReflectionSignature", Function.class,
            String[].class, DataType[].class, DataType.class, JsonObject.class);
        update.setAccessible(true);
        JsonObject preview = new JsonObject();
        String before = function.getPrototypeString(true, true);
        int datatypeCount = program.getDataTypeManager().getDataTypeCount(true);
        update.invoke(script, function, names, types, returnType, preview);
        check(preview.get("signatureChecked").getAsBoolean() && !preview.get("applied").getAsBoolean() &&
            function.getPrototypeString(true, true).equals(before) && program.getDataTypeManager().getDataTypeCount(true) == datatypeCount,
            "signature preview changed the program: " + function.getName());
        if (skipReason != null) check(preview.get("reason").getAsString().equals(skipReason), "preview missed storage conflict: " + function.getName());
        apply.set(script, true);
        JsonObject action = new JsonObject();
        boolean changed = (boolean) update.invoke(script, function, names, types, returnType, action);
        check(changed == expectedChanged && action.get("applied").getAsBoolean() == expectedChanged,
            "unexpected signature change: " + function.getName() + " " + action);
        if (skipReason != null) check(action.get("reason").getAsString().equals(skipReason) &&
            function.getPrototypeString(true, true).equals(before), "blocked signature partially changed: " + function.getName());
        if (!Proxy.isProxyClass(function.getClass())) {
            JsonObject retry = new JsonObject();
            check(!(boolean) update.invoke(script, function, names, types, returnType, retry),
                "signature retry changed an existing plan: " + function.getName() + " " + retry);
        }
        return action;
    }

    private static void run(ProgramDB program, Path resources, boolean preview) throws Exception {
        AzReflectionRenamer script = new AzReflectionRenamer();
        script.set(new GhidraState(null, null, program, null, null, null), TaskMonitor.DUMMY,
            new PrintWriter(new StringWriter()));
        script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
        script.setScriptArgs(preview ? new String[]{"--resources", resources.toString(), "--dry-run"} :
            new String[]{"--resources", resources.toString()});
        script.start();
        try { script.run(); }
        finally { script.end(true); }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
