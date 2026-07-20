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
const buildEngineering = await read("scripts/build-engineering.ps1");
const buildIdentityModule = await read("scripts/Aster.BuildIdentity.psm1");
const buildIdentityFixtures = await read("scripts/build-identity.fixtures.ps1");
const packagingModule = await read("scripts/Aster.Packaging.psm1");
const packageAuditFixtures = await read("scripts/package-audit.fixtures.ps1");
const packageProvenanceFixtures = await read("scripts/package-provenance.fixtures.ps1");
const packageEngineering = await read("scripts/package-engineering.ps1");
const packageAudit = await read("scripts/package-audit.ps1");
const verificationScript = await read("scripts/verify.ps1");
const secretScanner = await read("scripts/secret-scan.mjs");
const secretPatternFixtureTest = await read("scripts/secret-patterns.fixtures.mjs");
const sharedSecretPatterns = JSON.parse(await read("scripts/secret-patterns.json"));
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

const shippedProviderOrigins = [
  "https://api.z.ai",
  "https://api.deepseek.com",
  "https://dashscope-us.aliyuncs.com",
  "https://generativelanguage.googleapis.com",
  "https://integrate.api.nvidia.com",
];
const shippedAccountOrigins = [
  "https://z.ai",
  "https://platform.deepseek.com",
  "https://modelstudio.console.alibabacloud.com",
  "https://usercenter2-intl.console.alibabacloud.com",
  "https://billing-cost-intl.aliyun.com",
  "https://aistudio.google.com",
  "https://build.nvidia.com",
];

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
  packageManifest.version === "0.2.0" && tauri.version === packageManifest.version,
  "The npm and Tauri manifests must identify the same MVP v2 version.",
);
assert(
  /^version\s*=\s*"0\.2\.0"$/m.test(mainCargo) && /^version\s*=\s*"0\.2\.0"$/m.test(helperCargo),
  "Every first-party Rust package must identify MVP v2 as version 0.2.0.",
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
  packageEngineering.includes("[string]$VerifierIdentity") &&
    packageEngineering.includes("Invoke-AsterEngineeringPackage") &&
    !packageEngineering.includes("Identity: local Codex verification session"),
  "The public packaging wrapper must require a validated caller-supplied identity and delegate to the reviewed module.",
);
assert(
  buildEngineering.includes("Import-Module $identityModule -Force") &&
    packagingModule.includes("Import-Module $identityModule -Force") &&
    buildIdentityModule.includes("function Get-AsterFileIdentity") &&
    buildIdentityModule.includes("function Get-AsterDirectoryDigest") &&
    buildIdentityModule.includes("function Assert-AsterNoReparseAncestors") &&
    buildIdentityModule.includes("function Remove-AsterSafeDirectoryTree"),
  "Build and package scripts must share reparse-safe identity and filesystem primitives.",
);
assert(
  buildIdentityFixtures.includes("The directory digest did not detect a changed frontend asset") &&
    buildIdentityFixtures.includes(
      "depends on the current culture instead of ordinal path order",
    ) &&
    buildIdentityFixtures.includes("Windows junction") &&
    buildIdentityFixtures.includes("accepted a file outside the repository root") &&
    packageManifest.scripts?.["security:build-identity"] ===
      "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-identity.fixtures.ps1" &&
    packageManifest.scripts?.check?.includes("pnpm security:build-identity") &&
    ciWorkflow.includes("run: pnpm security:build-identity"),
  "Local and CI gates must exercise clean-revision build-identity positive and negative fixtures.",
);
assert(
  buildEngineering.includes("Engineering builds require a completely clean Git working tree") &&
    buildEngineering.includes("The tracked source changed during the engineering build") &&
    buildEngineering.includes('Join-Path $work "build-identity.json"') &&
    buildEngineering.includes("releaseBinary = Get-AsterFileIdentity") &&
    buildEngineering.includes("installer = Get-AsterFileIdentity") &&
    buildEngineering.includes("frontend = Get-AsterFileIdentity") &&
    buildEngineering.includes("rust = Get-AsterFileIdentity") &&
    buildEngineering.includes("sha256 = Get-AsterDirectoryDigest"),
  "Engineering builds must bind executable, installer, SBOMs, and dist to one clean revision.",
);
assert(
  packagingModule.includes(
    "work/evidence/YYYY-MM-DD-<full-source-revision>-engineering-build.md",
  ) &&
    packagingModule.includes("Get-AsterEvidenceText") &&
    packagingModule.includes("Get-AsterStrictBuildIdentity") &&
    packagingModule.includes("exact duplicate-free schema") &&
    packagingModule.includes("Assert-AsterZipInventory") &&
    packagingModule.includes("source-inventory.json") &&
    packagingModule.includes("work\\package-staging") &&
    packagingModule.includes("Self-declared verifier identity") &&
    packagingModule.includes("verification-evidence.log") &&
    packagingModule.includes("A PASS result must identify the canonical retained evidence log") &&
    packagingModule.includes("exactly one canonical unsigned engineering classification") &&
    packagingModule.includes("EvidenceLog = $evidenceLog") &&
    packagingModule.includes("$validatedEvidenceLog") &&
    packagingModule.includes("[System.IO.Directory]::Move($staging, $output)") &&
    !packagingModule.includes("FixtureAfterMoveHook") &&
    !packagingModule.includes("--exclude=") &&
    !packagingModule.includes("tar.exe"),
  "Engineering packaging must stage, revalidate, inventory, and safely publish one exact clean revision.",
);
assert(
  packageManifest.scripts?.["security:package-audit"] ===
    "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-audit.fixtures.ps1" &&
    packageManifest.scripts?.check?.includes("pnpm security:package-audit") &&
    ciWorkflow.includes("run: pnpm security:package-audit") &&
    packageAudit.includes("foreach ($offset in @(0, 1))") &&
    packageAuditFixtures.includes("UTF-16LE offset 1") &&
    packageAuditFixtures.includes("UTF-16BE offset 1") &&
    packageAuditFixtures.includes("Binary package-audit alignment fixtures passed"),
  "Local and CI gates must scan UTF-16LE/BE binary secrets at both byte alignments.",
);
assert(
  buildIdentityModule.includes("[System.Security.Cryptography.SHA256]::Create()") &&
    !buildIdentityModule.includes("Get-FileHash") &&
    !packagingModule.includes("Get-FileHash") &&
    packagingModule.includes('return "NOT RUN (verifier unavailable)"') &&
    packagingModule.includes('$securityModule.ExportedCommands["Get-AuthenticodeSignature"]') &&
    packagingModule.includes("The Authenticode verifier contract is invalid.") &&
    packagingModule.includes("The Authenticode verifier returned an invalid status.") &&
    packagingModule.includes("it is not signature") &&
    packageProvenanceFixtures.includes("The unavailable Authenticode verifier fixture") &&
    packageProvenanceFixtures.includes('"NOT RUN (verifier unavailable)"'),
  "Build and packaging identity must not depend on the optional Get-FileHash cmdlet.",
);
assert(
  packageManifest.scripts?.["security:package-provenance"] ===
    "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-provenance.fixtures.ps1" &&
    packageManifest.scripts?.check?.includes("pnpm security:package-provenance") &&
    ciWorkflow.includes("run: pnpm security:package-provenance") &&
    packageProvenanceFixtures.includes("Package-provenance fixtures passed") &&
    packageProvenanceFixtures.includes("source-archive inventory fixture") &&
    packageProvenanceFixtures.includes("unsafe archive-directory fixture") &&
    packageProvenanceFixtures.includes("regular-to-symlink archive fixture") &&
    packageProvenanceFixtures.includes("missing ZIP parent-directory fixture") &&
    packageProvenanceFixtures.includes("invalid ZIP directory-mode fixture") &&
    packageProvenanceFixtures.includes("JSON-escaped user path") &&
    packageProvenanceFixtures.includes("Unicode slash-escaped user path") &&
    packageProvenanceFixtures.includes("duplicate JSON key") &&
    packageProvenanceFixtures.includes("unsafe-range JSON integer") &&
    packageProvenanceFixtures.includes("missing procedure identity") &&
    packageProvenanceFixtures.includes("duplicate engineering classification") &&
    packageProvenanceFixtures.includes("production classification") &&
    packageProvenanceFixtures.includes("unbackticked result row") &&
    packageProvenanceFixtures.includes("indented malformed result row") &&
    packageProvenanceFixtures.includes("duplicate conflicting gate result") &&
    packageProvenanceFixtures.includes("whitespace-padded gate alias") &&
    packageProvenanceFixtures.includes("PASS without reviewable evidence") &&
    packageProvenanceFixtures.includes("missing retained evidence-log fixture") &&
    packageProvenanceFixtures.includes("invalid retained evidence-log UTF-8 fixture") &&
    packageProvenanceFixtures.includes("oversized retained evidence-log fixture") &&
    packageProvenanceFixtures.includes("personal-path retained evidence-log fixture") &&
    packageProvenanceFixtures.includes("credential-header retained evidence-log fixture") &&
    packageProvenanceFixtures.includes("shared-secret retained evidence-log fixture") &&
    packageProvenanceFixtures.includes("post-copy mismatch fixture") &&
    packageProvenanceFixtures.includes("post-copy evidence-log mismatch fixture") &&
    packageProvenanceFixtures.includes("atomic publication-race fixture") &&
    packageProvenanceFixtures.includes("locked publication-child fixture") &&
    packageProvenanceFixtures.includes("unpublished candidate received the final output name") &&
    packageProvenanceFixtures.includes("final-package mismatch fixture") &&
    packageProvenanceFixtures.includes("final-state mismatch fixture") &&
    packageProvenanceFixtures.includes("package-junction fixture"),
  "Local and CI gates must execute the temp-repository package-provenance abuse suite.",
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
  packagingModule.includes(
    "Copy-AsterFileSafely -RepositoryRoot $root -Source $license -Destination $paths.licenseOutput",
  ) &&
    packagingModule.includes(
      "Copy-AsterFileSafely -RepositoryRoot $root -Source $notice -Destination $paths.noticeOutput",
    ),
  "Engineering handoffs must copy LICENSE and NOTICE beside distributable artifacts.",
);
assert(
  /\$identityTargets\s*=\s*@\([\s\S]*?\$paths\.licenseOutput[\s\S]*?\$paths\.noticeOutput[\s\S]*?\)/.test(
    packagingModule,
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
const requiredSecretPatternNames = [
  "AWS access key",
  "GitHub token",
  "Google API key",
  "NVIDIA API key",
  "assigned credential",
  "private key",
  "sk-prefixed provider token",
];
assert(
  Array.isArray(sharedSecretPatterns) &&
    JSON.stringify(sharedSecretPatterns.map(({ name }) => name).sort()) ===
      JSON.stringify(requiredSecretPatternNames),
  "The shared credential scanner must retain the reviewed provider and generic secret rules.",
);
assert(
  secretScanner.includes('scripts", "secret-patterns.json') &&
    packageAudit.includes('Join-Path $PSScriptRoot "secret-patterns.json"') &&
    secretPatternFixtureTest.includes("Every shared secret rule needs a fixture.") &&
    packageManifest.scripts?.["security:patterns"] ===
      "node scripts/secret-patterns.fixtures.mjs" &&
    packageManifest.scripts?.check?.includes("pnpm security:patterns") &&
    ciWorkflow.includes("run: pnpm security:patterns"),
  "Repository and packaged-artifact scans must share tested credential patterns.",
);
assert(
  packageManifest.scripts?.verify ===
    "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1" &&
    verificationScript.includes("were NOT RUN because -SkipRust was selected") &&
    verificationScript.includes("Full frontend and Rust verification completed successfully."),
  "The verify command must run the full orchestrator and label skipped Rust gates as NOT RUN.",
);

const approvedCiActionPins = new Set([
  "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0",
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
const expectedApplicationPermissions = [
  "allow-acknowledge-external-processing",
  "allow-app-status",
  "allow-cancel-generation",
  "allow-create-conversation",
  "allow-deepseek-balance-status",
  "allow-delete-api-key",
  "allow-delete-conversation",
  "allow-export-conversation",
  "allow-get-conversation",
  "allow-import-conversations",
  "allow-list-conversations",
  "allow-model-catalog",
  "allow-open-external-url",
  "allow-open-provider-account",
  "allow-prompt-store-api-key",
  "allow-provider-statuses",
  "allow-refresh-deepseek-balance",
  "allow-rename-conversation",
  "allow-send-message",
  "allow-set-usage-budget",
  "allow-update-conversation-selection",
  "allow-usage-summary",
];
const actualApplicationPermissions = permissions
  .filter((permission) => String(permission).startsWith("allow-"))
  .map(String)
  .sort();
assert(
  JSON.stringify(actualApplicationPermissions) === JSON.stringify(expectedApplicationPermissions),
  "The main window must expose exactly the specified MVP v2 application commands.",
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
assert(
  !permissions.includes("allow-credential-status"),
  "Provider-scoped credential status must replace the retired global credential command.",
);

const frontendForbidden = [
  ["dangerouslySetInnerHTML", "The renderer must not use dangerouslySetInnerHTML."],
  ["localStorage", "The renderer must not use localStorage."],
  ["sessionStorage", "The renderer must not use sessionStorage."],
];
for (const [needle, message] of frontendForbidden) {
  assert(!frontend.includes(needle), message);
}
for (const origin of [...shippedProviderOrigins, ...shippedAccountOrigins]) {
  assert(!frontend.includes(origin), `The renderer must not contain the fixed origin ${origin}.`);
}
for (const origin of shippedProviderOrigins) {
  assert(
    applicationRust.includes(origin),
    `The Rust-owned registry must contain the verified provider origin ${origin}.`,
  );
}
for (const origin of shippedAccountOrigins) {
  assert(
    applicationRust.includes(origin),
    `The Rust-owned account-action map must contain the fixed origin ${origin}.`,
  );
}
const reqwestClientBuilders = applicationRust.match(/\bClient::builder\(\)/g) ?? [];
const disabledReqwestRetryPolicies =
  applicationRust.match(/\.retry\(reqwest::retry::never\(\)\)/g) ?? [];
const sharedProviderBuilderUses =
  applicationRust.match(/let client = provider_client_builder\(\)/g) ?? [];
assert(
  reqwestClientBuilders.length === 1 &&
    disabledReqwestRetryPolicies.length === 1 &&
    sharedProviderBuilderUses.length === 2,
  "Every production and controlled provider client must use the one shared reqwest builder with retries explicitly disabled.",
);
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
  /\binvokeDesktop\s*\(\s*["']prompt_store_api_key["']\s*,\s*\{\s*providerId\s*\}\s*\)/.test(
    frontend,
  ),
  "The renderer must invoke the native credential prompt with only the selected provider ID.",
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
