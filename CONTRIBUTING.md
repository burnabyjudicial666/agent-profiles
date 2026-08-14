# Contributing

## The most useful contribution

This project was built and verified entirely on macOS. **Windows and Linux have never been compiled or run on real hardware** — every test for them executes on macOS against fixtures. See the platform checklists in the [README](README.md).

If you run Windows or Linux, checking one of those boxes with a real report is worth more than any further test written on macOS. A bug report saying "the process list looks like this instead" is a genuine contribution, even without a patch.

## Setup

The toolchain is Rust plus `pnpm`. Install them however you like — nothing here depends on a particular version manager.

This repository happens to be developed with [mise](https://mise.jdx.dev/), a tool that installs and pins language runtimes per project. If you use it and its shims are not already on `PATH`, add them:

```bash
export PATH="$HOME/.local/share/mise/shims:$PATH"
```

If you installed Rust with [rustup](https://rustup.rs/) instead, or by any other means, skip that line entirely: `cargo` is already where the commands below expect it.

```bash
pnpm install
pnpm start
```

Start the app with `pnpm start`, not by running the binary from `target/debug`. A development build loads its interface from the Vite dev server, so a bare binary opens a blank management window.

## Before opening a pull request

Run what CI runs:

```bash
pnpm check
```

One command, so there is no chance of running a weaker check than CI does: it is the frontend build, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, stopping at the first failure. The build is expected to be warning-free. If your change adds a warning, resolve it rather than leaving it for someone else to wonder about.

## Tests

Changes to behavior come with a test. Prefer testing the decision over the mechanism: much of this codebase is deliberately shaped so the interesting judgement lives in a small pure function that a test can call without a running app or a window server.

## What to be careful about

Claude Desktop has **no single-instance lock**. Two processes pointed at one user-data directory will both stay alive and corrupt its databases. Anything touching launch, process scanning, or profile deletion is guarding against real data loss, so those paths deliberately **fail closed**: when the code cannot tell whether a profile is running, it refuses rather than guesses. Please keep it that way — an `unwrap_or_default()` on a process scan turns "I cannot tell" into "nothing is running", which is precisely the wrong answer.

## Commit messages

Explain *why*, not *what*. The diff already says what changed.
