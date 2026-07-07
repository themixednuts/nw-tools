// Shared JSON helpers for NetworkSchemaExtractor source-bundle classes.

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import ghidra.program.model.address.Address;
import java.util.List;

final class NetworkSchemaJson {
    private NetworkSchemaJson() {}

    static void add(JsonObject object, String name, String value) {
        if (value != null) {
            object.addProperty(name, value);
        }
    }

    static void add(JsonObject object, String name, Integer value) {
        if (value != null) {
            object.addProperty(name, value);
        }
    }

    static void addAddress(JsonObject object, String name, Address value,
            NetworkSchemaAddressFormatter formatter) {
        if (value != null) {
            add(object, name, formatter.format(value));
        }
    }

    static JsonArray stringArray(List<String> values) {
        JsonArray array = new JsonArray();
        if (values == null) {
            return array;
        }
        for (String value : values) {
            array.add(value);
        }
        return array;
    }
}
