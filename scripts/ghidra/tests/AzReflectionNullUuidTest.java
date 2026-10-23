import java.io.File;
import java.util.LinkedHashMap;
import java.util.Map;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import ghidra.GhidraApplicationLayout;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;

public class AzReflectionNullUuidTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])),
            new ApplicationConfiguration());
        String nullId = "00000000-0000-0000-0000-000000000000";
        String validId = "ABCDEF12-3456-4789-ABCD-0123456789AB";
        AzReflectionRenamer script = new AzReflectionRenamer();
        Map<String, String> names = new LinkedHashMap<>();
        names.put(nullId, "BattleRoyaleData");
        names.put(validId, "MB::Box<Javelin::Payload>");
        var field = AzReflectionRenamer.class.getDeclaredField("datatypeNames");
        field.setAccessible(true);
        field.set(script, names);
        var enrich = AzReflectionRenamer.class.getDeclaredMethod("enrichBehaviorTypeReferences", JsonObject.class);
        enrich.setAccessible(true);
        JsonObject metadata = JsonParser.parseString("""
            {"wrappedTypeId":"%s","nested":[{"m_typeId":" %s "}," %s "]}
            """.formatted(nullId, nullId, validId.toLowerCase(java.util.Locale.ROOT))).getAsJsonObject();
        enrich.invoke(script, metadata);
        JsonObject resolved = metadata.getAsJsonObject("resolvedTypes");
        check(resolved != null && resolved.get(validId).getAsString().equals(names.get(validId)),
            "real type references retain their names");
        check(!resolved.has(nullId), "a null UUID does not mean BattleRoyaleData in BehaviorContext metadata");
        check(metadata.get("wrappedTypeId").getAsString().equals(nullId), "raw null UUID evidence is retained");
        check(names.get(nullId).equals("BattleRoyaleData"), "explicit serialized class identity is retained");
        String once = metadata.toString();
        enrich.invoke(script, metadata);
        check(metadata.toString().equals(once), "reference enrichment is idempotent");
        JsonObject onlyNull = new JsonObject();
        onlyNull.addProperty("wrappedTypeId", nullId);
        enrich.invoke(script, onlyNull);
        check(!onlyNull.has("resolvedTypes"), "null-only metadata has no invented type resolution");
        System.out.println("PASS: null UUID metadata, nested references, valid UUID names, explicit class identity, raw evidence and idempotence");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
