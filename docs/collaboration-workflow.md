# GitHub Collaboration Workflow

This guide describes the recommended Git and GitHub workflow for working on Aster Desktop from multiple Windows laptops and with trusted collaborators. It complements [GOVERNANCE.md](../GOVERNANCE.md), [CONTRIBUTING.md](../CONTRIBUTING.md), and [AGENTS.md](../AGENTS.md).

## Repository policy

- Keep the repository private until the owner has selected a license and explicitly approved public distribution.
- Use `main` as the protected integration branch.
- Make changes on short-lived branches and merge through pull requests.
- Give normal collaborators **Write** access, not **Admin** access.
- Require the frontend and Rust CI jobs, conversation resolution, and at least one independent approval before merge when the GitHub plan supports those controls.
- Never upload unsigned engineering binaries as a production GitHub Release.

## Configure a new laptop

Install Git for Windows, GitHub CLI, the toolchain listed in [development.md](development.md), and Microsoft Edge WebView2 Runtime. Authenticate GitHub through the browser or a credential manager; do not place a token in a remote URL, source file, shell history, or repository configuration.

Configure your contributor identity using the name and email you want recorded in commits:

```powershell
git config --global user.name "Your Name"
git config --global user.email "your-verified-email@example.com"
```

Clone the private repository and bootstrap the locked dependencies:

```powershell
gh repo clone renenehru/aster-desktop
Set-Location aster-desktop
pnpm install --frozen-lockfile
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

A fresh clone intentionally excludes the API key, SQLite history, conversation exports, build output, coverage, SBOM scratch files, and unsigned installers.

## Start a change

Synchronize `main` without creating an accidental merge commit:

```powershell
git switch main
git fetch --prune origin
git pull --ff-only origin main
```

Create a focused branch:

```powershell
git switch -c feature/short-description
```

Follow the specification-driven workflow before implementation. Use `fix/`, `security/`, `docs/`, or `maintenance/` prefixes when they better describe the work.

## Review before committing

Inspect the exact change and publication set:

```powershell
git status --short
git diff --check
git diff
```

Run the applicable gates. For a complete local source verification:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Stage intentionally, then inspect the staged manifest and diff:

```powershell
git add --all
git status --short
git diff --cached --check
git diff --cached
```

Do not stage `node_modules/`, `src-tauri/target/`, `dist/`, `coverage/`, `work/`, `outputs/`, databases, exports, environment files, logs, signing material, or credentials. If any appears, stop and correct the ignore or staging problem before committing.

## Commit and publish a branch

Use a focused English commit message:

```powershell
git commit -m "feat: describe the user-visible outcome"
git push --set-upstream origin HEAD
```

Open a pull request:

```powershell
gh pr create --fill
```

Complete every section of the pull-request template. Link requirement IDs, threat IDs, tests, evidence, residual risks, and required `NOT RUN` outcomes.

## Continue work on another laptop

Before switching laptops, commit and push the branch from the current machine. On the next machine:

```powershell
git fetch --prune origin
git switch feature/short-description
git pull --ff-only
pnpm install --frozen-lockfile
```

Do not copy the working directory through cloud storage or removable media as a synchronization strategy. A clone plus locked dependency installation avoids copying credentials, local databases, generated artifacts, stale build caches, or machine-specific state.

If work is not ready to share, create a private draft commit on the feature branch rather than copying untracked files between machines. Never commit a secret merely to move it.

## Update a branch from `main`

Fetch the latest source and rebase a private feature branch when the team has agreed to that workflow:

```powershell
git fetch origin
git rebase origin/main
```

If the branch is already shared, coordinate before rewriting it. Never force-push `main`. Use `git push --force-with-lease` only on your own reviewed feature branch and only when collaborators have confirmed that rewriting it is safe.

## Review and merge

Reviewers should verify:

- controlling requirements and non-goals;
- trust-boundary, permission, data-flow, migration, and dependency impact;
- normal, boundary, failure, cancellation, and abuse coverage;
- English UI/configuration/documentation;
- scoped `PASS`, `FAIL`, and `NOT RUN` evidence;
- absence of secrets, user data, generated artifacts, and unsigned release claims.

Prefer squash merge for a noisy branch or a normal merge when preserving a deliberately structured commit series is useful. Delete the merged feature branch after confirming `main` and CI are healthy.

## Invite collaborators safely

From the repository settings, invite each trusted engineer by their exact GitHub username and grant **Write** access. Ask collaborators to enable two-factor authentication and protect their GitHub account. Review access periodically and remove accounts that no longer need it.

Do not share one GitHub account, personal access token, SSH private key, signing key, or Z.AI API key among collaborators. Each contributor uses their own identity and local credential store.

## Source archives

GitHub clones and pull requests are the normal source-distribution mechanism. When an explicit source handoff ZIP is required, use the engineering packaging script only from a clean identified commit:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-engineering.ps1 `
  -EvidenceRecord "docs\evidence\YYYY-MM-DD-<revision>-engineering-build.md"
```

The script uses `git archive` on `HEAD`; ignored and untracked workspace files are excluded by construction. Do not zip the working directory or upload the existing `outputs/` source snapshot to GitHub.

## Recovery rules

- If a secret is committed, stop, revoke it outside the repository, preserve safe evidence, and follow [SECURITY.md](../SECURITY.md). Deleting the latest file alone does not remove it from Git history.
- Do not use `git reset --hard`, destructive clean operations, or force pushes as routine recovery.
- Before resolving a conflict, identify the controlling requirement and preserve unrelated work.
- If branch protection or CI is unavailable, record that limitation and enforce the same review policy manually.
