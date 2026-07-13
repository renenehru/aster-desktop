import { execFileSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const cargo =
  process.env.CARGO ??
  (process.platform === "win32" && process.env.USERPROFILE
    ? path.join(process.env.USERPROFILE, ".cargo", "bin", "cargo.exe")
    : "cargo");
const metadata = JSON.parse(
  execFileSync(
    cargo,
    ["metadata", "--format-version", "1", "--locked", "--manifest-path", "src-tauri/Cargo.toml"],
    { cwd: root, encoding: "utf8", windowsHide: true, maxBuffer: 64 * 1024 * 1024 },
  ),
);

const byId = new Map((metadata.packages ?? []).map((crate) => [crate.id, crate]));
const components = (metadata.packages ?? [])
  .filter((crate) => crate.name !== "aster-desktop")
  .map((crate) => {
    const reference = `pkg:cargo/${encodeURIComponent(crate.name)}@${encodeURIComponent(crate.version)}`;
    return {
      type: "library",
      name: crate.name,
      version: crate.version,
      licenses: crate.license ? [{ expression: crate.license }] : undefined,
      purl: reference,
      "bom-ref": reference,
    };
  })
  .sort((left, right) => left.purl.localeCompare(right.purl));

const referenceForId = (id) => {
  const crate = byId.get(id);
  return crate
    ? `pkg:cargo/${encodeURIComponent(crate.name)}@${encodeURIComponent(crate.version)}`
    : null;
};
const resolveNodes = metadata.resolve?.nodes ?? [];
const dependencies = resolveNodes
  .map((node) => {
    const ref = referenceForId(node.id);
    if (!ref) return null;
    const dependsOn = (node.dependencies ?? [])
      .map(referenceForId)
      .filter((value) => value !== null)
      .sort();
    return { ref, dependsOn: [...new Set(dependsOn)] };
  })
  .filter((value) => value !== null)
  .sort((left, right) => left.ref.localeCompare(right.ref));

const rootPackage = (metadata.packages ?? []).find((crate) => crate.name === "aster-desktop");
if (!rootPackage) throw new Error("The Aster Rust package was not present in Cargo metadata.");
const rootReference = `pkg:cargo/${rootPackage.name}@${rootPackage.version}`;
const sbom = {
  bomFormat: "CycloneDX",
  specVersion: "1.6",
  serialNumber: `urn:uuid:${crypto.randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    component: {
      type: "application",
      name: rootPackage.name,
      version: rootPackage.version,
      purl: rootReference,
      "bom-ref": rootReference,
    },
  },
  components,
  dependencies,
};

await mkdir(path.join(root, "work"), { recursive: true });
const destination = path.join(root, "work", "sbom-rust.cdx.json");
await writeFile(destination, `${JSON.stringify(sbom, null, 2)}\n`, "utf8");
console.log(`CycloneDX SBOM written to ${path.relative(root, destination)}.`);
