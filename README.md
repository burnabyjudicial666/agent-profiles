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

Status below is a verification record as of **2026-08-13**. A source-level test is not a substitute for live acceptance on the target operating system.

| Platform | Verified | Not yet verified / owner of follow-up |
| --- | --- | --- |
| **macOS** | The Rust suite passes on macOS (59 tests). The Claude Desktop binary/path checks and macOS process-parser tests pass. Parallel Claude Desktop processes were launched directly twice with distinct `--user-data-dir` values and both stayed alive. | The Claude Profiles UI/tray acceptance is still awaiting a human-run macOS check. In particular, the Manage Profiles tray item showing and focusing the hidden window, a new profile appearing in the tray immediately, launching a second profile alongside Default and keeping both usable through the app, refusing deletion while that profile runs, and showing the directory size in the delete confirmation remain unverified. The prior attempt could not drive the UI because macOS Accessibility/Assistive Access permission must be granted by a human. |
| **Windows** | Windows CSV process parsing and MSIX/classic path-picker logic are covered by unit tests run on macOS. The Windows target check was attempted. | Live Windows acceptance is unverified: process shape, parallel instances, MSIX default-directory selection, hardlink creation, focus, quit, and end-to-end launch behavior all need a human-run check on Windows. `cargo check --target x86_64-pc-windows-msvc` could not run to completion because the target is not installed on this Mac (`can't find crate for core`). |
| **Linux** | Linux desktop-identity helpers, profile-id-based classes and filenames, `.desktop` metadata, and Wayland detection are covered by unit tests run on macOS. The Linux target check was attempted. | Live Linux acceptance is unverified: the real `claude-desktop` process shape, default data path, parallel instances, `--class` behavior, generated taskbar identity, X11 focus, Wayland behavior, symlink creation, and quit flow all need a human-run check on Linux. `cargo check --target aarch64-unknown-linux-gnu` could not run to completion because the target is not installed on this Mac (`can't find crate for std`). |

The cross-target checks were attempted without installing rustup or any Rust targets. The live Windows and Linux checklists remain open rather than being inferred from the unit tests.

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
