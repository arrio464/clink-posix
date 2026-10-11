# Local validation record

Date: 2026-10-11, Windows 11 x64, Rust 1.98.1 (GNU target), Python 3.x, Clink 1.9.34.

This record describes this local checkout. It is not a claim that GitHub Actions has run; the
remote repository has not been created or pushed by the preparing agent.

| Check | Result | Scope |
|---|---|---|
| `cargo fmt --all -- --check` | Pass | Source formatting |
| `cargo clippy --locked --offline -- -D warnings` | Pass | Static lint, Windows GNU |
| `./build.ps1` | Pass | Offline release build, no Cargo dependencies |
| `python tests/check.py` | 47 pass | Self-contained native fixture; no Scoop, BusyBox or old Python helper |
| `python tests/check.py --python-reference PATH` | 47 pass | Optional common-subset differential with installed legacy helper on this machine |
| `./tests/clink-smoke.ps1 -ClinkExe PATH` | 7 pass | Isolated real Clink/CMD session, no global installation |
| `python tests/benchmark.py --samples 3 --warmups 1` | Pass | Portable benchmark smoke run with fixture tools; output ignored by Git |

The historical 25-sample measurements in [PERFORMANCE.md](../PERFORMANCE.md) predate the
repository portability edits. They use a different external-tool workload. This record does
not relabel those numbers as a new measurement.

The current installed Clink loader and Python helper were not replaced or modified. The
repository's binary, machine-local tool manifest, benchmark raw output, smoke logs and build
artifacts are ignored; `git ls-files` is the authoritative list of intended publication content.

Outstanding production gates: persistent CMD parent-state synchronization, Ctrl+C and descendant
process cleanup, total resource budgets, terminal code-page behavior, and comprehensive
external-program argv compatibility. Virtual devices beyond redirection `/dev/null` remain a
proposal, not implementation.
