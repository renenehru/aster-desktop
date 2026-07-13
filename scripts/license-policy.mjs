import { execFileSync } from "node:child_process";
import process from "node:process";

const pnpmEntrypoint = process.env.npm_execpath;
if (!pnpmEntrypoint) {
  throw new Error("Run this check through the pnpm license:frontend script.");
}

const report = JSON.parse(
  execFileSync(process.execPath, [pnpmEntrypoint, "licenses", "list", "--prod", "--json"], {
    encoding: "utf8",
    windowsHide: true,
  }),
);

const allowedLicenses = new Set([
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 OR MIT",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "MIT",
  "MIT OR Apache-2.0",
  "MPL-2.0",
  "Unicode-3.0",
  "Zlib",
]);
const rejected = Object.keys(report).filter((license) => !allowedLicenses.has(license));

if (rejected.length > 0) {
  console.error(`Frontend license policy failed for: ${rejected.join(", ")}`);
  process.exitCode = 1;
} else {
  console.log(
    `Frontend license policy passed for ${Object.keys(report).length} license expressions.`,
  );
}
