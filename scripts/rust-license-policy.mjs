import { execFileSync } from "node:child_process";
import path from "node:path";
import process from "node:process";

const cargo =
  process.env.CARGO ??
  (process.platform === "win32" && process.env.USERPROFILE
    ? path.join(process.env.USERPROFILE, ".cargo", "bin", "cargo.exe")
    : "cargo");
const metadata = JSON.parse(
  execFileSync(
    cargo,
    ["metadata", "--format-version", "1", "--locked", "--manifest-path", "src-tauri/Cargo.toml"],
    { encoding: "utf8", windowsHide: true, maxBuffer: 64 * 1024 * 1024 },
  ),
);

const allowedIdentifiers = new Set([
  "0BSD",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "LLVM-exception",
  "MIT",
  "MIT-0",
  "MPL-2.0",
  "CDLA-Permissive-2.0",
  "Unicode-3.0",
  "Unicode-DFS-2016",
  "Unlicense",
  "Zlib",
]);

const rejected = [];
const workspaceRejected = [];
const workspaceMembers = new Set(metadata.workspace_members ?? []);

function isAllowedExpression(expression) {
  // Cargo metadata still contains the legacy SPDX slash form in established
  // crates. Cargo defines `A/B` as the old spelling of `A OR B`; normalize it
  // before evaluating the same allowlist rather than silently accepting an
  // unknown token sequence.
  const normalizedExpression = expression.replace(/\s*\/\s*/g, " OR ");
  const tokens = normalizedExpression.match(/\(|\)|AND|OR|WITH|[A-Za-z0-9.+-]+/g) ?? [];
  let index = 0;
  const primary = () => {
    const token = tokens[index++];
    if (token === "(") {
      const value = orExpression();
      if (tokens[index++] !== ")") return false;
      return value;
    }
    return typeof token === "string" && allowedIdentifiers.has(token);
  };
  const withExpression = () => {
    let value = primary();
    while (tokens[index] === "WITH") {
      index += 1;
      value = primary() && value;
    }
    return value;
  };
  const andExpression = () => {
    let value = withExpression();
    while (tokens[index] === "AND") {
      index += 1;
      value = withExpression() && value;
    }
    return value;
  };
  const orExpression = () => {
    let value = andExpression();
    while (tokens[index] === "OR") {
      index += 1;
      value = andExpression() || value;
    }
    return value;
  };
  const allowed = orExpression();
  return allowed && index === tokens.length;
}

if (
  !isAllowedExpression("MIT/Apache-2.0") ||
  !isAllowedExpression("Apache-2.0 WITH LLVM-exception") ||
  isAllowedExpression("GPL-3.0-only")
) {
  throw new Error("The SPDX policy evaluator failed its built-in allow/deny fixtures.");
}

for (const crate of metadata.packages ?? []) {
  if (workspaceMembers.has(crate.id)) {
    if (crate.license !== "Apache-2.0") {
      workspaceRejected.push(
        `${crate.name}@${crate.version}: expected Apache-2.0, received ${JSON.stringify(crate.license)}`,
      );
    }
    continue;
  }
  if (typeof crate.license !== "string" || !crate.license.trim()) {
    rejected.push(`${crate.name}@${crate.version}: missing SPDX license`);
    continue;
  }
  if (!isAllowedExpression(crate.license)) {
    rejected.push(`${crate.name}@${crate.version}: ${crate.license}`);
  }
}

if (workspaceRejected.length > 0 || rejected.length > 0) {
  if (workspaceRejected.length > 0) {
    console.error(
      "Rust workspace license policy failed:\n" +
        workspaceRejected.map((item) => `- ${item}`).join("\n"),
    );
  }
  if (rejected.length > 0) {
    console.error(
      "Rust dependency license policy failed:\n" + rejected.map((item) => `- ${item}`).join("\n"),
    );
  }
  process.exitCode = 1;
} else {
  console.log(
    `Rust workspace and dependency license policy passed for ${workspaceMembers.size} Apache-2.0 workspace packages and ${metadata.packages.length - workspaceMembers.size} third-party dependency packages.`,
  );
}
