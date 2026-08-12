# Claude Profiles

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

## Platform status

Verification record as of **2026-08-13**. Every unit test in this repository runs on macOS, including the Windows and Linux ones — they exercise parsing and path logic against fixtures, not a real operating system. **A passing unit test is not acceptance.** An unchecked box below means the behavior has never been observed on real hardware, not that it is known to be broken.

The full Rust suite passes on macOS: **64 tests, 0 failures.**

### macOS — partially verified

- [x] Rust suite passes, including the Claude Desktop binary and path checks
- [x] Two Claude Desktop processes launched directly with distinct `--user-data-dir` values both stayed alive (this is the premise the whole app rests on)
- [ ] Management window opens from the tray, and closing it no longer quits the app
- [ ] Rename and delete work from the management window
- [ ] Tray liveness marker updates after quitting Claude Desktop by hand
- [ ] A newly added profile appears in the tray immediately
- [ ] A second profile launches alongside Default and both stay usable through the app
- [ ] Deletion is refused while that profile is running
- [ ] The delete confirmation shows the directory size

Automated UI driving was not possible: macOS Accessibility permission has to be granted by a human, so the open boxes need a person.

### Windows — never compiled or run

- [x] CSV process parsing and MSIX/classic path-picker logic covered by unit tests (run on macOS)
- [ ] **The Windows target has never been compiled.** `cargo check --target x86_64-pc-windows-msvc` cannot run on this machine (`can't find crate for core`)
- [ ] Real process shape of the installed Claude Desktop
- [ ] MSIX vs classic default-directory selection against a real installation
- [ ] Hardlink creation for the shared MCP config
- [ ] Parallel instances, focus, quit, end-to-end launch

### Linux — never compiled or run

- [x] Desktop-identity helpers, profile-id classes and filenames, `.desktop` metadata, and Wayland detection covered by unit tests (run on macOS)
- [ ] **The Linux target has never been compiled.** `cargo check --target aarch64-unknown-linux-gnu` cannot run on this machine (`can't find crate for std`)
- [ ] Real `claude-desktop` process shape and default data path
- [ ] Per-profile `--class` producing a distinct taskbar identity
- [ ] X11 focus via `xdotool`, and the Wayland limitation path
- [ ] Symlink creation, parallel instances, quit flow

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

The project is not packaged, signed, or notarized for distribution by this repository.
