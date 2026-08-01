import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const root = process.cwd();

async function read(relativePath) {
  return readFile(path.join(root, relativePath), "utf8");
}

async function readOptional(relativePath) {
  try {
    return await read(relativePath);
  } catch (error) {
    if (error?.code === "ENOENT") return "";
    throw error;
  }
}

async function matchingFiles(directory, matches) {
  const entries = await readdir(path.join(root, directory), { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (entry.isDirectory() && entry.name === "target") continue;
    const relative = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await matchingFiles(relative, matches)));
    else if (matches(relative)) files.push(relative);
  }
  return files;
}

async function combine(files) {
  const contents = await Promise.all(files.map((file) => read(file)));
  return contents.map((content, index) => `// FILE: ${files[index]}\n${content}`).join("\n");
}

function tomlSection(source, name) {
  const marker = `[${name}]`;
  const start = source.indexOf(marker);
  if (start === -1) return "";
  const remainder = source.slice(start + marker.length);
  const nextSection = remainder.search(/^\s*\[/m);
  return nextSection === -1 ? remainder : remainder.slice(0, nextSection);
}

function normalized(relativePath) {
  return relativePath.split(path.sep).join("/");
}

const failures = [];
const assert = (condition, message) => {
  if (!condition) failures.push(message);
};

const tauriRaw = await read("src-tauri/tauri.conf.json");
const tauri = JSON.parse(tauriRaw);
const tauriDev = JSON.parse(await read("src-tauri/tauri.dev.conf.json"));
const capability = JSON.parse(await read("src-tauri/capabilities/main.json"));
const vite = await read("vite.config.ts");
const indexHtml = await read("index.html");
const packageRaw = await read("package.json");
const packageManifest = JSON.parse(packageRaw);
const projectLicense = await readOptional("LICENSE");
const projectNotice = await readOptional("NOTICE");
const licensingDocumentation = await combine([
  "README.md",
  "CONTRIBUTING.md",
  "docs/collaboration-workflow.md",
  "docs/roadmap.md",
]);
const environmentExample = await read(".env.example");
const packageEngineering = await read("scripts/package-engineering.ps1");
const frontendSbomGenerator = await read("scripts/generate-sbom.mjs");
const rustSbomGenerator = await read("scripts/generate-rust-sbom.mjs");
const ciWorkflow = await read(".github/workflows/ci.yml");
const mainCargo = await read("src-tauri/Cargo.toml");
const helperCargo = await read("src-tauri/credential-prompt/Cargo.toml");
const frontendFiles = await matchingFiles(
  "src",
  (file) =>
    /\.(?:ts|tsx|js|jsx)$/.test(file) &&
    !/\.(?:test|spec)\.[^.]+$/.test(file) &&
    !normalized(file).includes("/test/"),
);
const frontend = await combine(frontendFiles);
const rustFiles = await matchingFiles("src-tauri", (file) => file.endsWith(".rs"));
const helperRustFiles = rustFiles.filter((file) =>
  normalized(file).startsWith("src-tauri/credential-prompt/"),
);
const applicationRustFiles = rustFiles.filter((file) => !helperRustFiles.includes(file));
const helperRust = await combine(helperRustFiles);
const applicationRust = await combine(applicationRustFiles);
const rustEntry = await read("src-tauri/src/lib.rs");
const rustMain = await read("src-tauri/src/main.rs");

const productionCsp = String(tauri.app?.security?.csp ?? "");
assert(productionCsp.includes("default-src 'self'"), "Tauri CSP must default to self.");
assert(productionCsp.includes("script-src 'self'"), "Tauri CSP must restrict scripts to self.");
assert(productionCsp.includes("object-src 'none'"), "Tauri CSP must disable objects.");
const cspWithoutTauriIpc = productionCsp.replaceAll("http://ipc.localhost", "");
assert(
  !/https?:\/\//.test(cspWithoutTauriIpc),
  "Production CSP must not contain a remote HTTP origin.",
);
assert(indexHtml.includes("script-src 'self'"), "HTML CSP must restrict scripts to self.");
assert(/sourcemap:\s*false/.test(vite), "Production source maps must remain disabled.");
assert(
  tauri.app?.security?.freezePrototype === true,
  "Tauri must freeze the JavaScript prototype.",
);
assert(
  tauri.build?.devUrl === undefined,
  "The production Tauri configuration must not embed a development endpoint.",
);
assert(
  tauri.app?.security?.devCsp === undefined,
  "The production Tauri configuration must not embed a development CSP.",
);
assert(
  tauriDev.build?.devUrl === "http://127.0.0.1:1420" &&
    String(tauriDev.app?.security?.devCsp ?? "").includes("ws://127.0.0.1:1420") &&
    !/https:\/\//.test(String(tauriDev.app?.security?.devCsp ?? "")),
  "The explicit development overlay must remain loopback-only and contain its development CSP.",
);
assert(
  /&\s*\$git\s+-C\s+\$root\s+status\s+--porcelain=v1\s+--untracked-files=normal/.test(
    packageEngineering,
  ),
  "Source packaging must reject a dirty Git working tree.",
);
assert(
  /&\s*\$git\s+-C\s+\$root\s+archive\s+--format=zip\s+--output=\$sourceArchive\s+HEAD/.test(
    packageEngineering,
  ),
  "Source packaging must archive tracked files from the identified HEAD commit.",
);
assert(
  !packageEngineering.includes("--exclude=") && !packageEngineering.includes("tar.exe"),
  "Source packaging must not archive the working directory through a hard-coded denylist.",
);
assert(
  createHash("sha256").update(projectLicense).digest("hex") ===
    "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
  "LICENSE must be the unmodified canonical Apache License 2.0 text.",
);
assert(
  packageManifest.license === "Apache-2.0" && packageManifest.private === true,
  "The npm package must declare Apache-2.0 while retaining the accidental-publication guard.",
);
assert(
  /^license\s*=\s*"Apache-2\.0"$/m.test(mainCargo) &&
    /^license\s*=\s*"Apache-2\.0"$/m.test(helperCargo),
  "Every Rust workspace package must declare Apache-2.0.",
);
assert(
  tauri.bundle?.license === "Apache-2.0" &&
    tauri.bundle?.licenseFile === "../LICENSE" &&
    tauri.bundle?.resources?.["../LICENSE"] === "LICENSE" &&
    tauri.bundle?.resources?.["../NOTICE"] === "NOTICE",
  "Tauri bundles must declare Apache-2.0 and install the canonical license and notice files.",
);
assert(
  projectNotice.startsWith("Aster Desktop\n") &&
    projectNotice.includes("Contributor Covenant, version 2.1") &&
    projectNotice.includes("https://creativecommons.org/licenses/by/4.0/") &&
    projectNotice.includes("https://github.com/mozilla/inclusion"),
  "NOTICE must preserve the project attribution, Contributor Covenant CC BY 4.0 notice, and Mozilla attribution.",
);
assert(
  /Copy-Item\s+-LiteralPath\s+\$license\s+-Destination\s+\$licenseOutput/.test(
    packageEngineering,
  ) &&
    /Copy-Item\s+-LiteralPath\s+\$notice\s+-Destination\s+\$noticeOutput/.test(packageEngineering),
  "Engineering handoffs must copy LICENSE and NOTICE beside distributable artifacts.",
);
assert(
  /\$identityTargets\s*=\s*@\([\s\S]*?\$licenseOutput[\s\S]*?\$noticeOutput[\s\S]*?\)/.test(
    packageEngineering,
  ),
  "Engineering artifact identity and checksums must cover LICENSE and NOTICE.",
);
assert(
  frontendSbomGenerator.includes("licenses: [{ expression: packageJson.license }]") &&
    rustSbomGenerator.includes("licenses: [{ expression: rootPackage.license }]") &&
    frontendSbomGenerator.includes('packageJson.license !== "Apache-2.0"') &&
    rustSbomGenerator.includes('rootPackage.license !== "Apache-2.0"'),
  "Both SBOM root components must carry the verified Apache-2.0 expression.",
);
assert(
  rustSbomGenerator.includes("const normalizeCargoLicenseExpression = (expression) =>") &&
    rustSbomGenerator.includes('expression.replace(/\\s*\\/\\s*/g, " OR ")') &&
    rustSbomGenerator.includes(
      "licenses: crate.license\n        ? [{ expression: normalizeCargoLicenseExpression(crate.license) }]",
    ) &&
    rustSbomGenerator.includes(
      'normalizeCargoLicenseExpression("MIT/Apache-2.0") !== "MIT OR Apache-2.0"',
    ),
  "Rust SBOM dependency licenses must normalize Cargo legacy slash alternatives to SPDX OR expressions.",
);
assert(
  !/no open-source license has been selected|repository currently has no open-source license|keep the repository private until|do not redistribute project source/i.test(
    licensingDocumentation,
  ),
  "Current contributor documentation must not retain the pre-license distribution restrictions.",
);
assert(
  !/^\s*[A-Z][A-Z0-9_]*\s*=/m.test(environmentExample),
  "The environment example must not advertise unsupported secret, endpoint, or diagnostic configuration.",
);

const approvedCiActionPins = new Set([
  "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
  "actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e",
  "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
  "pnpm/action-setup@0ebf47130e4866e96fce0953f49152a61190b271",
  "Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4",
]);
const ciActionReferences = [...ciWorkflow.matchAll(/^\s*uses:\s*([^\s#]+)/gm)].map(
  (match) => match[1],
);
const uniqueCiActionReferences = new Set(ciActionReferences);
assert(
  ciActionReferences.length > 0 &&
    ciActionReferences.every((reference) => approvedCiActionPins.has(reference)) &&
    [...approvedCiActionPins].every((reference) => uniqueCiActionReferences.has(reference)),
  "CI actions must use the reviewed full-SHA pins whose upstream runtimes were approved for the hosted runner.",
);
const rustJobMarker = ciWorkflow.indexOf("\n  rust:\n");
const rustJob = rustJobMarker === -1 ? "" : ciWorkflow.slice(rustJobMarker);
const rustClippyMarker = rustJob.indexOf("      - name: Run Clippy");
const lockedFrontendInstallMarker = rustJob.indexOf("pnpm install --frozen-lockfile");
const frontendBuildMarker = rustJob.indexOf("pnpm build");
const cargoAuditInstallMarker = rustJob.indexOf(
  "cargo install cargo-audit --version 0.22.2 --locked",
);
const cargoAuditRunMarker = rustJob.indexOf("cargo audit --file src-tauri/Cargo.lock");
assert(
  rustClippyMarker > 0 &&
    lockedFrontendInstallMarker > 0 &&
    lockedFrontendInstallMarker < rustClippyMarker &&
    frontendBuildMarker > lockedFrontendInstallMarker &&
    frontendBuildMarker < rustClippyMarker,
  "Rust CI must build locked frontend assets before Tauri macros run under Clippy.",
);
assert(
  cargoAuditInstallMarker > 0 &&
    cargoAuditRunMarker > cargoAuditInstallMarker &&
    !rustJob.includes("rustsec/audit-check@"),
  "Rust CI must run the reviewed locked cargo-audit version without a token-bearing action.",
);

const permissions = Array.isArray(capability.permissions) ? capability.permissions : [];
const forbiddenPermission =
  /(?:^|:)(?:shell|process|fs|http|opener)(?::|$)|window:allow-create|\*/i;
assert(
  permissions.every((permission) => !forbiddenPermission.test(String(permission))),
  "The main capability contains a wildcard or out-of-scope privileged permission.",
);
assert(
  Array.isArray(capability.windows) &&
    capability.windows.length === 1 &&
    capability.windows[0] === "main",
  "Privileged commands must be scoped only to the main window.",
);
const expectedWindowPermissions = [
  "core:window:allow-close",
  "core:window:allow-minimize",
  "core:window:allow-start-dragging",
  "core:window:allow-toggle-maximize",
];
const actualWindowPermissions = permissions
  .filter((permission) => String(permission).startsWith("core:window:"))
  .map(String)
  .sort();
assert(
  JSON.stringify(actualWindowPermissions) === JSON.stringify(expectedWindowPermissions),
  "The custom title bar must have exactly the four approved core window permissions.",
);
assert(
  tauri.app?.windows?.[0]?.decorations === false,
  "The custom title bar requires native decorations to be disabled.",
);
assert(
  permissions.includes("allow-open-external-url"),
  "Safe Markdown links require the typed Rust external URL command.",
);
assert(
  permissions.includes("allow-prompt-store-api-key"),
  "ADR-0008 requires the native prompt-only credential permission.",
);
assert(
  !permissions.includes("allow-store-api-key"),
  "ADR-0008 forbids the retired renderer-to-Rust credential value permission.",
);

const frontendForbidden = [
  ["dangerouslySetInnerHTML", "The renderer must not use dangerouslySetInnerHTML."],
  ["localStorage", "The renderer must not use localStorage."],
  ["sessionStorage", "The renderer must not use sessionStorage."],
  ["https://api.z.ai", "The renderer must not contain the provider endpoint."],
];
for (const [needle, message] of frontendForbidden) {
  assert(!frontend.includes(needle), message);
}
assert(!/\bfetch\s*\(/.test(frontend), "The renderer must not make direct fetch calls.");
assert(!/\bWebSocket\s*\(/.test(frontend), "The renderer must not open WebSockets.");
assert(
  !/\btype\s*=\s*["']password["']/i.test(frontend),
  "ADR-0008 forbids password inputs in production renderer source.",
);
assert(
  !/\bapiKey\b/.test(frontend),
  "ADR-0008 forbids API-key values in production renderer source.",
);
assert(
  !/\bstore_api_key\b/.test(frontend),
  "ADR-0008 forbids the retired secret-bearing renderer IPC command.",
);
assert(
  /\binvokeDesktop\s*\(\s*["']prompt_store_api_key["']\s*,\s*\{\s*\}\s*\)/.test(frontend),
  "The renderer must invoke the native credential prompt through the raw-body helper with no arguments.",
);
const directInvokeSites = frontend.match(/\binvoke(?:\s*<[^>\n]+>)?\s*\(/g) ?? [];
assert(
  directInvokeSites.length === 1 &&
    /return\s+invoke<unknown>\s*\(\s*command\s*,\s*payload\s*\)/.test(frontend),
  "Every desktop command must use the single raw-body IPC helper.",
);
assert(
  /new\s+TextEncoder\(\)\.encode\(JSON\.stringify\(argumentsObject\)\)/.test(frontend) &&
    /payload\.byteLength\s*>\s*MAX_IPC_BODY_BYTES/.test(frontend) &&
    /MAX_IPC_BODY_BYTES\s*=\s*320\s*\*\s*1_024/.test(frontend),
  "The raw-body IPC helper must UTF-8 encode JSON and enforce the 320 KiB renderer limit.",
);
assert(
  !/\bconsole\.(?:log|error|warn|info|debug)\s*\(/.test(frontend),
  "ADR-0009 forbids production renderer console logging.",
);

assert(
  !rustEntry.includes(".plugin(tauri_plugin_opener"),
  "The opener plugin must not register raw renderer IPC handlers.",
);

assert(
  /\baster-credential-prompt\s*=\s*\{[^}\n]*\bpath\s*=\s*"credential-prompt"[^}\n]*\}/.test(
    mainCargo,
  ),
  "ADR-0008 requires the exact local credential prompt helper dependency.",
);
assert(
  /\[lints\.rust\][\s\S]*?\bunsafe_code\s*=\s*"forbid"/.test(mainCargo),
  "The main Rust crate must forbid unsafe code in Cargo configuration.",
);
assert(
  rustEntry.trimStart().startsWith("#![forbid(unsafe_code)]") &&
    rustMain.trimStart().startsWith("#![forbid(unsafe_code)]"),
  "Both main Rust crate entry points must forbid unsafe code.",
);
assert(
  /\[lints\.rust\][\s\S]*?\bunsafe_code\s*=\s*"deny"/.test(helperCargo) &&
    helperRust.includes("#![deny(unsafe_code)]"),
  "The credential helper must deny unsafe code by default.",
);

const unsafeConstruct = /\bunsafe\s*(?:\{|fn\b|extern\b|impl\b|trait\b)/;
const unsafeFiles = [];
for (const file of rustFiles) {
  const source = await read(file);
  if (unsafeConstruct.test(source)) unsafeFiles.push(file);
}
assert(unsafeFiles.length > 0, "The reviewed native credential FFI site must remain explicit.");
assert(
  unsafeFiles.every((file) => normalized(file).startsWith("src-tauri/credential-prompt/")),
  "Unsafe Rust is permitted only inside the exact local credential prompt helper crate.",
);
assert(
  !unsafeConstruct.test(applicationRust) && !applicationRust.includes("#[allow(unsafe_code)]"),
  "The main Rust crate and build source must contain no unsafe exception.",
);
assert(
  (helperRust.match(/\bunsafe\s*\{/g) ?? []).length === 1 &&
    (helperRust.match(/#\[allow\(unsafe_code\)\]/g) ?? []).length === 1,
  "ADR-0008 permits exactly one narrowly allowed credential-helper FFI block.",
);

const forbiddenRustDependency =
  /^(?:log|tracing|tracing-subscriber|env_logger|flexi_logger|slog|fern|tauri-plugin-log|sentry|opentelemetry)\s*=/m;
assert(
  !forbiddenRustDependency.test(tomlSection(mainCargo, "dependencies")) &&
    !forbiddenRustDependency.test(tomlSection(helperCargo, "dependencies")),
  "ADR-0009 forbids direct Rust logging or telemetry dependencies.",
);
const runtimePackageNames = Object.keys({
  ...(packageManifest.dependencies ?? {}),
  ...(packageManifest.optionalDependencies ?? {}),
});
const forbiddenFrontendDependency =
  /^(?:@sentry\/|loglevel$|pino$|winston$|bunyan$|analytics$|mixpanel-browser$|@opentelemetry\/)/i;
assert(
  runtimePackageNames.every((name) => !forbiddenFrontendDependency.test(name)),
  "ADR-0009 forbids frontend logging or telemetry runtime dependencies.",
);
const rustLoggingMacro = /\b(?:println|eprintln|dbg|trace|debug|info|warn|error|log)!\s*\(/;
assert(
  !rustLoggingMacro.test(`${applicationRust}\n${helperRust}`),
  "ADR-0009 forbids application logging macros in Aster Rust source.",
);
assert(
  !/\.plugin\s*\(\s*(?:tauri_plugin_log|sentry|opentelemetry)/i.test(applicationRust),
  "ADR-0009 forbids logging or telemetry plugins.",
);
const telemetryEndpoint =
  /https?:\/\/(?:[^/\s@]+\.)?(?:sentry\.io|datadoghq\.com|segment\.io|mixpanel\.com|amplitude\.com|newrelic\.com|appcenter\.ms|applicationinsights\.azure\.com)|\b(?:telemetry|analytics|crashReport|crash_report)[_-]?(?:endpoint|url|dsn)\b/i;
assert(
  !telemetryEndpoint.test(
    `${frontend}\n${applicationRust}\n${helperRust}\n${tauriRaw}\n${vite}\n${packageRaw}\n${mainCargo}\n${helperCargo}`,
  ),
  "ADR-0009 forbids telemetry, analytics, and crash-reporting endpoints.",
);

if (failures.length > 0) {
  console.error(
    "Security configuration audit failed:\n" + failures.map((item) => `- ${item}`).join("\n"),
  );
  process.exitCode = 1;
} else {
  console.log("Security configuration audit passed.");
}
