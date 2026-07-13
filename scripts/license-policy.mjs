import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import process from "node:process";

const projectPackage = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const projectLicenseValid = projectPackage.license === "Apache-2.0";

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

if (!projectLicenseValid || rejected.length > 0) {
  if (!projectLicenseValid) {
    console.error(
      `Frontend project license must be Apache-2.0; received ${JSON.stringify(projectPackage.license)}.`,
    );
  }
  if (rejected.length > 0) {
    console.error(`Frontend dependency license policy failed for: ${rejected.join(", ")}`);
  }
  process.exitCode = 1;
} else {
  console.log(
    `Frontend project and dependency license policy passed for Apache-2.0 and ${Object.keys(report).length} dependency license expressions.`,
  );
}
