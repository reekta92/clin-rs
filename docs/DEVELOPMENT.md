# Development across the Clin repositories

## Maintained repositories

| Repository | Responsibility |
| --- | --- |
| [clin-rs](https://github.com/reekta92/clin-rs) | Application, vault persistence, host integration, UI and acceptance |
| [pinstar](https://github.com/reekta92/pinstar) | Canvas engine and standalone canvas editor |
| [graf](https://github.com/reekta92/graf) | Graph model, physics, rendering and standalone CLI |
| [draw-rs](https://github.com/reekta92/draw-rs) | Drawing model, input, rendering and standalone CLI |
| [.github](https://github.com/reekta92/.github) | Reusable CI, releases and workflow regression tests |
| [merman maintenance branch](https://github.com/reekta92/merman/tree/fix/maintained-yaml-msrv-1.90) | Published `merman-core-clin` parser compatibility package |

The Mermaid fork's main branch tracks upstream; the compatibility branch owns
the parser package used by Clin. Compiler requirements live in each package's
manifest. Exact dependency versions live in committed lockfiles.

## Workspace and handoff

Use sibling checkouts named `clin-rs`, `pinstar`, `graf`, `draw-rs` and `.github`
under one chosen workspace directory, with the origins linked above. Use `git worktree list` to locate canonical checkouts when a relative sibling
path points into another worktree.

Before work, inspect `git status --short --branch`, `git remote -v` and
`git fetch origin --prune` in each affected checkout. Start a branch from fetched
`origin/main`, preserving existing edits and unfinished branches. Do not reset
or switch a colleague's active checkout to synchronize it.

GitHub deletes newly merged branches automatically. For older branches, verify
`git merge-base --is-ancestor <branch> origin/main` and inspect open PRs before
deletion. Inspect worktree status/ownership before cleanup; `git worktree prune
--dry-run --verbose` identifies metadata for missing directories. Preserve
maintained branches, unmerged commits and working files. A closed PR alone does
not prove its branch was integrated.

## Checks and coordinated changes

Run `scripts/check.sh` and `cargo package --locked` in Clin. Library contributors
run formatting, all-target Clippy, tests (all features in Pinstar), declared
minimum Rust and dependency policy. Shared CI additionally tests Windows/macOS
and applies policy to the extracted registry package.

Required checks must pass before merging, with no routine bypass actors.
Administrators can temporarily repair a broken rule, recording the reason in
the repair PR and restoring enforcement afterward.

For a library change, run `python3 scripts/check_upstream.py ../<library>` from
Clin. It tests the committed consumer against the candidate working tree without
changing the consumer manifest or lockfile. Library CI pins that same consumer
revision. Link paired API-change PRs and test a committed compatible Clin
revision before release. After release, update Clin's dependency requirement and
lockfile, then refresh the pinned consumer revision in library CI.

Shared workflow changes must merge into `.github/main` and pass workflow tests
before consumers adopt their immutable SHA. Retarget stacked PRs when the parent
merges. Keep feature, persistence and release changes separately reviewable.
Track candidate-specific acceptance blockers in the release issue.

## Release ownership

Release dependencies first: compatibility parser, Pinstar, then Clin. Graf and
Draw release independently when their contracts pass. Release CI owns version
bumps and immutable tags. Run its dry run before actual publication.

Real candidates and post-release documentation commits pass CI on temporary
branches before pushing main. The workflow reports successful jobs as commit
statuses so GitHub's direct-push rules recognize the verified results; failed
CI or status publication aborts. Callers grant `contents: write`, `actions: write`
and `statuses: write`. Configure required reviewers on `release-approval` if
publication needs interactive GitHub approval.

Complete [release acceptance](RELEASE_CHECKLIST.md) for the actual candidate and
record manual evidence on the release issue. Hosted tests cannot establish
behavior in a real interactive terminal or Obsidian runtime.
