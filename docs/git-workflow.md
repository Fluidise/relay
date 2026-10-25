# Git Workflow & Delivery Policy

This document defines the authoritative Git and GitHub workflow for the `fluidise-relay` repository (`Fluidise/relay`).

---

## 1. Branching Strategy

We follow a **Trunk-Based Development** model with short-lived feature branches:

- **Base branch**: `main` (production-ready baseline).
- **Work branches**:
  - `feat/<short-description>`: New capabilities or functional improvements.
  - `fix/<short-description>`: Bug fixes and issue resolutions.
  - `chore/<short-description>`: Maintenance, refactoring, dependencies, and tooling.
  - `docs/<short-description>`: Documentation updates.

### Branch Freshness

Before creating a new branch or worktree, ensure your local `main` is up to date:

```bash
git checkout main
git pull origin main
```

---

## 2. Commit Message Conventions

Commit messages must strictly follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```text
<type>(<optional scope>): <description>

[optional body]

[optional footer(s)]
```

### Allowed Types
- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `style`: Changes that do not affect the meaning of the code
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `perf`: A code change that improves performance
- `test`: Adding missing tests or correcting existing tests
- `build`: Changes that affect the build system or external dependencies
- `ci`: Changes to our CI configuration files and scripts
- `chore`: Other changes that don't modify src or test files

### Commit Message Validation (Hook)

We validate commit messages via a native Git `commit-msg` hook dogfooding `fluidise-clint`:

```bash
# Install local hooks (one-time setup)
./scripts/install-hooks.sh
```

Every commit is validated against `fluidise-clint` (with regex fallback) before Git records it.

> **Note**: Never include co-author mentions such as `Co-authored-by: Claude` in commit messages.

---

## 3. Pre-Commit Quality Checks

Before committing or pushing, verify code quality locally:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Linter check
cargo clippy --all-targets -- -D warnings

# 3. Test suite
cargo test --all-targets
```

---

## 4. Git Delivery Policy (Pushes & Pull Requests)

1. **Targeted Staging**: Review staged diffs before committing:
   ```bash
   git diff --cached --check
   git diff --cached --stat
   ```
2. **Push Destination**: Always push to GitHub (`origin`):
   ```bash
   git push -u origin <branch-name>
   ```
3. **Pull Requests**:
   - Open Pull Requests against `main` **systematically in Draft mode** (`gh pr create --draft`). Draft PRs prevent premature CI runner consumption and auto-merge execution.
   - When implementation and local verification are complete, mark the PR as ready for review (`gh pr ready`).
   - GitHub Actions automatically runs the full CI pipeline on ready PRs (`.github/workflows/ci.yml`).
   - Upon all checks passing (quality and test matrix), the `auto-merge` job automatically squashes and merges the PR into `main`, deleting the head branch (`delete_branch_on_merge: true`).
4. **Automated PR Triage & Labels**:
   - Pull Requests are automatically triaged by [`.github/workflows/pr-triage.yml`](../.github/workflows/pr-triage.yml).
   - **Auto-assignment**: The PR author is automatically assigned upon opening.
   - **Type labeling**: Labels (`feat`, `fix`, `chore`, `docs`, `ci`, `refactor`, `perf`, `test`, `build`) are applied based on the branch prefix or Conventional Commit title.
   - **Review status**:
     - Draft PRs receive the `work-in-progress` label.
     - Ready PRs receive the `needs-review` label.
   - **Breaking changes**: PRs indicating breaking changes (`!:` or `BREAKING CHANGE:`) receive `breaking-change`.

---

## 5. Parallel Work with Git Worktrees (Situational)

For working on parallel tasks or isolating bug fixes without switching working directories, situational worktrees are supported in `.worktrees/` (ignored by Git):

```bash
# Create a new worktree
./scripts/worktree.sh add feat/my-feature

# List active worktrees
./scripts/worktree.sh list

# Clean up worktree after merge
./scripts/worktree.sh remove feat/my-feature
```

---

## 6. Release Management & Changelog Automation

Release packaging, GitHub release publication, and changelog updates are automated with [`git-cliff`](https://github.com/orhun/git-cliff) via [`.github/workflows/release.yml`](../.github/workflows/release.yml).

### Triggering a Release

Releases are triggered by pushing a SemVer tag (`v*.*.*`):

1. **Direct Tag Push**:
   ```bash
   git checkout main
   git pull origin main
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```
2. **Release via Pull Request**:
   - Merge a preparation PR into `main`.
   - Apply the SemVer tag to the merge commit on `main` and push it to `origin`.

### Automated Pipeline Workflow

Upon detecting a tag push matching `v*`:
1. **GitHub Release**: Extracts release notes for the latest tag using `git-cliff` (`cliff.toml`) and publishes the GitHub Release with markdown formatting and asset attachments if present. Pre-releases (containing `-`, e.g., `v0.1.0-rc.1`) are automatically tagged as pre-releases.
2. **Changelog Sync**: Regenerates the complete historical [`CHANGELOG.md`](../CHANGELOG.md) according to Keep a Changelog specifications and commits it back to `main` with the commit message `chore(release): update CHANGELOG.md for <version> [skip ci]`.

