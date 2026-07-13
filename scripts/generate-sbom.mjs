import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const packageJson = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
if (packageJson.license !== "Apache-2.0") {
  throw new Error("The frontend project license must be Apache-2.0 before SBOM generation.");
}
const pnpmEntrypoint = process.env.npm_execpath;
if (!pnpmEntrypoint) {
  throw new Error("Run this generator through the pnpm sbom:frontend script.");
}
const rawTree = execFileSync(
  process.execPath,
  [pnpmEntrypoint, "list", "--prod", "--depth", "Infinity", "--json"],
  { cwd: root, encoding: "utf8", windowsHide: true },
);
const [tree] = JSON.parse(rawTree);
const licenseReport = JSON.parse(
  execFileSync(process.execPath, [pnpmEntrypoint, "licenses", "list", "--prod", "--json"], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  }),
);
const licenseByPackageVersion = new Map();
for (const [expression, packages] of Object.entries(licenseReport)) {
  for (const packageEntry of packages ?? []) {
    for (const version of packageEntry.versions ?? []) {
      const key = `${packageEntry.name}@${version}`;
      const previous = licenseByPackageVersion.get(key);
      if (previous && previous !== expression) {
        throw new Error(`Conflicting license expressions for ${key}: ${previous}, ${expression}`);
      }
      licenseByPackageVersion.set(key, expression);
    }
  }
}

const components = new Map();
const dependencies = new Map();

function visit(name, node) {
  if (!node || typeof node !== "object" || typeof node.version !== "string") return null;
  const reference = `pkg:npm/${encodeURIComponent(name)}@${encodeURIComponent(node.version)}`;
  const licenseExpression = licenseByPackageVersion.get(`${name}@${node.version}`);
  if (!licenseExpression) {
    throw new Error(`No dependency license expression was found for ${name}@${node.version}.`);
  }
  if (!components.has(reference)) {
    components.set(reference, {
      type: "library",
      name,
      version: node.version,
      licenses: [{ expression: licenseExpression }],
      purl: reference,
      "bom-ref": reference,
    });
  }
  const children = [];
  for (const [childName, child] of Object.entries(node.dependencies ?? {})) {
    const childReference = visit(childName, child);
    if (childReference) children.push(childReference);
  }
  dependencies.set(reference, [...new Set(children)].sort());
  return reference;
}

const rootReference = `pkg:npm/${encodeURIComponent(packageJson.name)}@${encodeURIComponent(packageJson.version)}`;
const rootChildren = [];
for (const [name, dependency] of Object.entries(tree?.dependencies ?? {})) {
  const reference = visit(name, dependency);
  if (reference) rootChildren.push(reference);
}

const sbom = {
  bomFormat: "CycloneDX",
  specVersion: "1.6",
  serialNumber: `urn:uuid:${crypto.randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    tools: {
      components: [
        {
          type: "application",
          name: "Aster repository SBOM generator",
          version: packageJson.version,
        },
      ],
    },
    component: {
      type: "application",
      name: packageJson.name,
      version: packageJson.version,
      licenses: [{ expression: packageJson.license }],
      purl: rootReference,
      "bom-ref": rootReference,
    },
  },
  components: [...components.values()].sort((a, b) => a.purl.localeCompare(b.purl)),
  dependencies: [
    { ref: rootReference, dependsOn: [...new Set(rootChildren)].sort() },
    ...[...dependencies.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([ref, dependsOn]) => ({ ref, dependsOn })),
  ],
};

await mkdir(path.join(root, "work"), { recursive: true });
const destination = path.join(root, "work", "sbom-frontend.cdx.json");
await writeFile(destination, `${JSON.stringify(sbom, null, 2)}\n`, "utf8");
console.log(`CycloneDX SBOM written to ${path.relative(root, destination)}.`);
