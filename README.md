# Claude Profiles

> **Unofficial.** This is a third-party tool with no affiliation to, endorsement by, or support from Anthropic. "Claude" and "Claude Desktop" are trademarks of Anthropic. This project only launches the Claude Desktop you already installed, with a different user-data directory.

Claude Profiles is a menu bar and system tray app for running multiple Claude Desktop instances in parallel, with one profile per account. Each profile has its own permanently separate Electron user-data directory, so using one account does not require signing out of another.

The **Default** profile is the Claude Desktop installation that already exists on the machine. Claude Profiles uses that profile in place: it does not move or copy the directory, and launches it without a `--user-data-dir` argument. Additional profiles live below the Claude Profiles data root and receive their own user-data directories.

## Important safety behavior

Claude Desktop does not provide a single-instance lock for a user-data directory. Starting two processes against the same directory can corrupt its databases. Claude Profiles therefore rescans processes immediately before every launch. A profile that is already running gets a Focus action instead of a second launch, and an unreadable process scan fails closed.

Profile labels are manual. Account email addresses are not read from disk or displayed. The stored `lastKnownAccountUuid` value is used only to warn when two profiles appear to be signed in to the same account.

## Shared MCP configuration

The MCP configuration is shared across every profile. Claude Profiles keeps one source-of-truth file at its data root and links each profile's `claude_desktop_config.json` to it before launch:

- **macOS and Linux:** symbolic links.
- **Windows:** hardlinks, so Developer Mode or elevation is not required. The files must be on the same drive.

If a profile has an existing regular configuration file, its contents are adopted into the shared configuration when there is no shared file yet. When a shared configuration already exists, the displaced profile file is retained rather than silently overwriting the configuration used by the other profiles.

## Launch at login

The management window offers an opt-in **Launch at login** toggle. It is off until you turn it on, and it starts only the tray: no profile is opened for you.

The operating system owns this setting — a login item on macOS, a registry entry on Windows, an autostart desktop entry on Linux. Claude Profiles keeps no copy of it and reads the real value each time the window opens, so turning it off in your system settings is reflected here rather than contradicted.

The toggle is hidden in development builds. A login item registered from `pnpm tauri dev` would point at a `target/debug` binary that moves, gets rebuilt, and disappears on `cargo clean`, leaving an entry that fails silently at every boot.

## Platform status

Verification record as of **2026-08-13**. Every unit test in this repository runs on macOS, including the Windows and Linux ones — they exercise parsing and path logic against fixtures, not a real operating system. **A passing unit test is not acceptance.** An unchecked box below means the behavior has never been observed on real hardware, not that it is known to be broken.

The full Rust suite passes on macOS: **64 tests, 0 failures.**

### macOS — verified, except the newest feature

- [x] Rust suite passes, including the Claude Desktop binary and path checks
- [x] Two Claude Desktop processes launched directly with distinct `--user-data-dir` values both stayed alive (this is the premise the whole app rests on)
- [x] The tray menu opens and lists the profiles
- [x] Management window opens from the tray, and closing it hides the window instead of quitting the app — the tray survives, and Manage Profiles opens it again
- [x] Renaming a profile works from the management window
- [x] Blank and duplicate labels are refused
- [x] Deleting a profile works from the management window
- [x] Tray liveness marker updates after quitting Claude Desktop by hand
- [x] A newly added profile appears in the tray immediately
- [x] A second profile launches alongside Default and both stay usable through the app — two Claude Desktop instances running in parallel, which is the whole point of this app
- [x] Deletion is refused while that profile is running
- [x] The delete confirmation shows the directory size
- [x] The window refuses to be resized below its usable minimum
- [ ] The Launch at login toggle actually registers and removes a login item, and survives a reboot

One box is open because the feature is newer than the acceptance run, not because it is suspected broken. The other checked boxes were confirmed by a human against the unsigned release build (`0.1.0`, Apple Silicon) on 2026-08-13, not inferred from tests. Automated UI driving was attempted and abandoned: macOS attributes Accessibility to the responsible process, and a headless agent session has no grantable one, so the open boxes still need a person.

### Windows — never compiled or run

- [x] CSV process parsing and MSIX/classic path-picker logic covered by unit tests (run on macOS)
- [ ] **The Windows target has never been compiled.** `cargo check --target x86_64-pc-windows-msvc` cannot run on this machine (`can't find crate for core`)
- [ ] Real process shape of the installed Claude Desktop
- [ ] MSIX vs classic default-directory selection against a real installation
- [ ] Hardlink creation for the shared MCP config
- [ ] Parallel instances, focus, quit, end-to-end launch
- [ ] Launch at login writes and removes its registry entry

### Linux — never compiled or run

- [x] Desktop-identity helpers, profile-id classes and filenames, `.desktop` metadata, and Wayland detection covered by unit tests (run on macOS)
- [ ] **The Linux target has never been compiled.** `cargo check --target aarch64-unknown-linux-gnu` cannot run on this machine (`can't find crate for std`)
- [ ] Real `claude-desktop` process shape and default data path
- [ ] Per-profile `--class` producing a distinct taskbar identity
- [ ] X11 focus via `xdotool`, and the Wayland limitation path
- [ ] Symlink creation, parallel instances, quit flow
- [ ] Launch at login writes and removes its autostart desktop entry

Contributions running Windows or Linux are especially welcome — checking one of those boxes with a real report is more valuable than any further test written on macOS.

## Linux and Wayland focus limitation

Native Wayland does not allow one application to raise another application's window. On Wayland, the tray's Focus action reports that limitation and points the user to the profile's taskbar entry or Alt-Tab. On X11, the app can use `xdotool` when it is installed. The generated Linux desktop identity is keyed by the profile's immutable id so renaming a label rewrites the same identity instead of creating a stale entry.

## Task-switcher icons

On macOS and Windows, all Claude Desktop instances intentionally share one application icon in the operating system's task switcher. Claude Profiles does not create per-profile app bundles, because doing so would add code-signing and update-maintenance risk. The tray is the navigation surface on those platforms. Linux is designed differently: each profile receives its own desktop identity and taskbar entry, but that behavior still awaits live Linux acceptance.

## Windows MSIX caveat

The official Windows installation may be an MSIX package. Windows can virtualize its writes, so Claude Desktop's effective data directory may be:

```text
%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude
```

rather than:

```text
%APPDATA%\Claude
```

Claude Profiles probes both locations and prefers the MSIX package path when both exist. The Windows acceptance run must confirm which path the installed build actually uses. The binary may likewise come from the direct-install location or the WindowsApps execution alias.

## Build

This repository uses `pnpm` and a Rust toolchain managed by mise. If the mise shims are not already on `PATH`, prefix Rust commands as follows:

```bash
export PATH="$HOME/.local/share/mise/shims:$PATH"
```

Install frontend dependencies:

```bash
pnpm install
```

Run the unsigned local development app:

```bash
pnpm tauri dev
```

Create an unsigned local bundle for the current platform:

```bash
pnpm tauri build
```

Run the same gates CI runs, before opening a pull request:

```bash
pnpm build
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The build is expected to be warning-free. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Releases

Tagging `v*` builds on three runners and attaches the artifacts to a draft GitHub Release: one universal macOS `.dmg` covering both Intel and Apple Silicon, Windows `.msi`/`.exe`, and Linux `.AppImage`/`.deb`. The Linux runner is pinned to Ubuntu 22.04 on purpose — a binary linked against a newer glibc refuses to start on older distributions, and the error it produces blames the wrong thing.

## Installing a release build

Releases are **unsigned**, because code-signing certificates cost money this project does not have. The operating system will therefore object, and the objection is misleading in both cases:

- **macOS** claims the app "is damaged and can't be opened". It is not damaged; it is merely unsigned. Right-click the app and choose **Open**, then confirm. If macOS still refuses, clear the quarantine flag: `xattr -d com.apple.quarantine "/Applications/Claude Profiles.app"`
- **Windows** shows a SmartScreen warning about an unknown publisher. Choose **More info → Run anyway**.

Only do this for a build you obtained from this project's Releases page. If either warning appears for a download from anywhere else, it deserves your suspicion.

## License

MIT — see [LICENSE](LICENSE).
