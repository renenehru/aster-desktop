import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const excludedDirectories = new Set([
  ".git",
  "coverage",
  "node_modules",
  "outputs",
  "target",
  "work",
]);
const excludedFiles = new Set(["pnpm-lock.yaml", "Cargo.lock", "secret-scan.mjs"]);
const textExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".md",
  ".mjs",
  ".ps1",
  ".rs",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yaml",
  ".yml",
]);

const rules = [
  { name: "private key", pattern: /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/ },
  { name: "GitHub token", pattern: /\bgh[pousr]_[A-Za-z0-9_]{30,}\b/ },
  { name: "AWS access key", pattern: /\bAKIA[0-9A-Z]{16}\b/ },
  { name: "OpenAI-style token", pattern: /\bsk-[A-Za-z0-9_-]{20,}\b/ },
  {
    name: "assigned credential",
    pattern:
      /(?:api[_-]?key|access[_-]?token|authorization|client[_-]?secret)\s*[:=]\s*["'][A-Za-z0-9_./+=-]{16,}["']/i,
  },
];

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (entry.isDirectory() && excludedDirectories.has(entry.name)) continue;
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(absolute)));
    } else if (
      entry.isFile() &&
      !excludedFiles.has(entry.name) &&
      textExtensions.has(path.extname(entry.name).toLowerCase())
    ) {
      files.push(absolute);
    }
  }
  return files;
}

const findings = [];
for (const file of await collectFiles(root)) {
  const content = await readFile(file, "utf8");
  for (const rule of rules) {
    if (rule.pattern.test(content)) {
      findings.push(`${path.relative(root, file)}: possible ${rule.name}`);
    }
  }
}

if (findings.length > 0) {
  console.error("Secret scan failed:\n" + findings.map((finding) => `- ${finding}`).join("\n"));
  process.exitCode = 1;
} else {
  console.log("Secret scan passed: no credential patterns were detected in repository text files.");
}
