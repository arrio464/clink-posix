# Development and scope

This is an experimental Windows-only executor with an explicitly bounded dialect.
Read [DESIGN.md](DESIGN.md) before changing parsing or process behavior.

## Local validation

Use Windows, Rust 1.98 or newer, and Python 3.10 or newer. No pip or Cargo dependencies are required.

```powershell
cargo fmt --all -- --check
cargo clippy --locked --offline -- -D warnings
./build.ps1
python tests/check.py
python tests/benchmark.py --samples 3 --warmups 1
```

The default tests compile their own native child fixture. They do not need Scoop,
GNU utilities, BusyBox, an existing Clink installation, or the previous Python helper.
These fixture commands are test infrastructure, not replacement Unix utilities.

An optional differential run can compare the common subset with a legacy helper:

```powershell
python tests/check.py --python-reference C:/path/to/native_shell.py
```

The legacy helper must have its own working tool configuration. Its behavior is not
the definition of POSIX correctness; independent expected results remain necessary.

The optional interactive smoke test needs Clink and a real console/PTY:

```powershell
./tests/clink-smoke.ps1 -ClinkExe C:/path/to/clink_x64.exe
```

It starts a separate CMD with isolated profile/scripts, runs seven checks, and exits.
It does not install the Lua adapter globally. CI runs only the self-contained tests;
passing CI does not claim that Ctrl+C or terminal rendering has been verified.

## Changes that need a design decision

- Define quoting, expansion order and unsupported cases before adding syntax.
- Parse the entire input before any execution; never reparse expanded argument values.
- Make new capabilities opt-in and version changes that alter existing interpretation.
- Do not introduce a second parser in Lua or automatic fallback to another shell.
- Keep native platform I/O separate from language semantics and tool configuration.
- Document cancellation, descriptor ownership and bounded resource use for every
  new producer/consumer. Infinite streams must not reuse finite substitution capture.
- Do not expand `/dev/...` strings in arbitrary argv; see the virtual-device RFC.

Keep local manifests, binaries, build caches and unsanitized measurements out of Git.
Historical benchmark files must state their workload and build; do not relabel old
measurements as results for a later binary.

## Repository status

No open-source license has been selected yet. Public hosting alone does not set a
reuse license. Select one deliberately before advertising a licensed release.
The package has `publish = false`; Git hosting and crates.io publication are separate.
