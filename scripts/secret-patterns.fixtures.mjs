import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const definitions = JSON.parse(
  await readFile(new URL("./secret-patterns.json", import.meta.url), "utf8"),
);
const rules = new Map(
  definitions.map(({ name, pattern, flags = "" }) => [name, new RegExp(pattern, flags)]),
);

const positiveFixtures = new Map([
  ["private key", "-----BEGIN " + "PRIVATE KEY-----"],
  ["GitHub token", "gh" + "p_" + "a".repeat(30)],
  ["AWS access key", "AK" + "IA" + "A1".repeat(8)],
  ["sk-prefixed provider token", "sk-" + "a".repeat(24)],
  ["Google API key", "AI" + "za" + "a".repeat(35)],
  ["NVIDIA API key", "nv" + "api-" + "a".repeat(24)],
  ["assigned credential", "api_" + "key=" + "'" + "a".repeat(24) + "'"],
]);

assert.equal(rules.size, positiveFixtures.size, "Every shared secret rule needs a fixture.");
for (const [name, fixture] of positiveFixtures) {
  assert.match(fixture, rules.get(name), `${name} must detect its synthetic fixture.`);
}

for (const safeFixture of [
  "sk-short",
  "AIza-short",
  "nvapi-short",
  "api_key='fake'",
  "authorization header intentionally omitted",
]) {
  assert.equal(
    [...rules.values()].some((rule) => rule.test(safeFixture)),
    false,
    `Safe fixture must not match: ${safeFixture}`,
  );
}

console.log("Shared secret-pattern fixtures passed.");
