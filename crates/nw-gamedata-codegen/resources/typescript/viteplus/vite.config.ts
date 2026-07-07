import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    exports: true{{PACK_ENTRY}}
  },
  lint: {
    ignorePatterns: ["dist/**", "node_modules/**"],
    options: {
      typeAware: true,
      typeCheck: true
    }
  },
  fmt: {
    ignorePatterns: ["dist/**", "node_modules/**"],
    printWidth: 320,
    sortPackageJson: false,
    trailingComma: "none"
  }
});
