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

That covers the platform you are on. The two recipes below cover the other ones from a Mac, and they are worth the trouble: a `-D warnings` failure on a platform you cannot build for is invisible until CI says so, and each of these finds one in minutes.

### Checking the Windows build from macOS

Tauri's build script compiles a Windows resource file, so it needs `llvm-rc`:

```bash
brew install llvm
export PATH="$(brew --prefix llvm)/bin:$PATH"
rustup target add x86_64-pc-windows-msvc

cd src-tauri
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

This type-checks and lints everything, tests included, but it cannot **run** them: there is no linking and no Windows to run on. It catches a Windows-only compile error in seconds instead of after a push.

### Running the Linux gate in a container

Cross-compiling to Linux needs a whole sysroot — GTK, dbus, webkit — so use a container instead and get the real thing: compiled, linted, and the tests actually executed.

```bash
docker run --rm -v "$PWD:/src:ro" ubuntu:22.04 bash -c '
  apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
    libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf \
    build-essential curl wget file libssl-dev libgtk-3-dev libxdo-dev \
    pkg-config ca-certificates >/dev/null
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
    --default-toolchain stable --component rustfmt,clippy >/dev/null
  . "$HOME/.cargo/env"
  # The build script writes inside the source tree, so work on a copy and
  # leave the host checkout alone. It needs ../dist, so run `pnpm build` first.
  mkdir -p /build && cp -a /src/src-tauri /src/dist /build/ && cd /build/src-tauri
  export CARGO_TARGET_DIR=/tmp/target
  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
'
```

Ubuntu 22.04 is the distribution CI pins, and the container is architecture-native, so on Apple Silicon this is an arm64 Linux rather than the amd64 one CI uses. That difference has never mattered for this code, which contains nothing architecture-specific — but it is a difference, and a container is still not a desktop. It proves the code builds and its tests pass on Linux. It proves nothing about the tray, the window, or `xdotool`.

One command, so there is no chance of running a weaker check than CI does: it is the frontend build, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, stopping at the first failure. The build is expected to be warning-free. If your change adds a warning, resolve it rather than leaving it for someone else to wonder about.

## Tests

Changes to behavior come with a test. Prefer testing the decision over the mechanism: much of this codebase is deliberately shaped so the interesting judgement lives in a small pure function that a test can call without a running app or a window server.

## What to be careful about

Claude Desktop has **no single-instance lock**. Two processes pointed at one user-data directory will both stay alive and corrupt its databases. Anything touching launch, process scanning, or profile deletion is guarding against real data loss, so those paths deliberately **fail closed**: when the code cannot tell whether a profile is running, it refuses rather than guesses. Please keep it that way — an `unwrap_or_default()` on a process scan turns "I cannot tell" into "nothing is running", which is precisely the wrong answer.

## Commit messages

Explain *why*, not *what*. The diff already says what changed.
