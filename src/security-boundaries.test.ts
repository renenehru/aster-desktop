import capability from "../src-tauri/capabilities/main.json";

const productionModules = import.meta.glob<string>(
  ["./**/*.{ts,tsx,js,jsx}", "!./**/*.{test,spec}.{ts,tsx,js,jsx}", "!./test/**"],
  { eager: true, query: "?raw", import: "default" },
);

const productionFrontendSource = () => Object.values(productionModules).join("\n");

describe("static renderer security boundaries", () => {
  it("keeps credential values and diagnostic logging out of production renderer source", () => {
    const source = productionFrontendSource();

    expect(source).not.toMatch(/\btype\s*=\s*["']password["']/i);
    expect(source).not.toMatch(/\bapiKey\b/);
    expect(source).not.toMatch(/\bstore_api_key\b/);
    expect(source).not.toMatch(/\bconsole\.(?:log|error|warn|info|debug)\s*\(/);
    expect(source).toMatch(
      /\binvokeDesktop\s*\(\s*["']prompt_store_api_key["']\s*,\s*\{\s*\}\s*\)/,
    );
    expect(source.match(/\binvoke(?:\s*<[^>\n]+>)?\s*\(/g) ?? []).toHaveLength(1);
    expect(source).toMatch(/return\s+invoke<unknown>\s*\(\s*command\s*,\s*payload\s*\)/);
  });

  it("grants only the native prompt credential permission", () => {
    const permissions = capability.permissions;

    expect(permissions).toContain("allow-prompt-store-api-key");
    expect(permissions).not.toContain("allow-store-api-key");
  });
});
