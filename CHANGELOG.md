# Changelog

## 0.2.0 — 2026-10-11 (local, unpublished)

- Native px, auto/on/off/status/cmd routing, persistent cwd/environment, and nx alias.
- Quote-aware globs, bounded braces/ranges, tilde, lazy parameter defaults,
  checked integer arithmetic, isolated groups, simple DOSKEY macros and multiline requests.
- Parameter length, no-op colon, command -v and cd --.
- Separate arithmetic, pathname expansion, aliases and parent-session transport modules.
- Atomic completed state snapshots; preserve status through the CMD cwd bridge.
- Receipt-verified upgrades with backups, legacy-loader suppression and reversible disable.
- 89 black-box/differential checks and 20 real Clink checks.
- Streaming devices, full scripts and process-tree cancellation remain future work.

## Initial repository preparation

- Prepare the experimental v0.1 source tree for Git hosting.
- Make the build repository-local; preserve existing local tool configuration.
- Provide self-contained Windows black-box fixtures and optional legacy comparisons.
- Add Windows CI, a portable benchmark driver, contribution guidance and ignore rules.
- Archive the original performance data with machine-local paths removed.
- Propose redirection-scoped virtual devices; implementation is not included.
- Add an opt-in, reversible per-user Clink installation for `nx` testing.

## 0.1.0 prototype — 2026-10-11

- Rust parser and native Windows executor for a bounded interactive subset.
- Quoted arguments, variables, nested command substitution, finite file substitution,
  short-circuit lists, concurrent byte-preserving pipelines and ordered redirects.
- Explicit Clink preview adapter and declarative state-result protocol.
- Whole-line static validation and local limits on input, nesting and capture size.

This is a prototype milestone, not a published or production-ready release.
