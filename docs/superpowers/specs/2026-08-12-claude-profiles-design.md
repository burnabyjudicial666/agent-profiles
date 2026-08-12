# Claude Profiles — Design

Date: 2026-08-12
Status: Approved (revised for cross-platform and rename)

## Problem

Claude Desktop supports exactly one signed-in account. Working across a personal
account and a work account means signing out and back in, losing the running
state of whichever account you left.

## Solution

A menu bar / tray app that runs **multiple Claude Desktop instances in parallel**,
one per account, each with its own Electron user-data directory.

This is not an account switcher that swaps files. Nothing is moved or copied at
switch time. Each profile is a permanently separate user-data directory, and the
app launches, focuses, and quits instances that point at them.

Ships for macOS, Windows, and Linux.

## Naming

The app is **Claude Profiles**. Bundle/app identifier `com.husniadil.claude-profiles`.
The git repository directory remains `cc-switcher`; nothing in the product uses
that name.

## Feasibility

**macOS: verified on 2026-08-12.** Two instances were launched simultaneously with
separate `--user-data-dir` values and both ran independently. Electron enforces
single-instance via a lock file *inside* the user-data directory, so distinct
directories mean distinct locks.

**Windows and Linux: unverified.** The mechanism is Electron's, not macOS's, so it
is expected to hold, but it was not tested — this design was written on a Mac with
no access to the other two platforms. Each platform backend therefore carries a
**manual acceptance check** that must be run on real hardware before that backend
is considered done. If parallel instances turn out to be impossible on a platform,
that backend degrades to launching one instance at a time; the rest of the app is
unaffected.

## Per-platform facts

| | macOS | Windows | Linux |
|---|---|---|---|
| Executable | `/Applications/Claude.app/Contents/MacOS/Claude` | `claude.exe` (MSIX alias in `%LOCALAPPDATA%\Microsoft\WindowsApps`, or `%LOCALAPPDATA%\AnthropicClaude\claude.exe` for the direct installer) | `claude-desktop` on `PATH` |
| Stock user data | `~/Library/Application Support/Claude` | `%APPDATA%\Claude`, **or** the MSIX-virtualized `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude` | `~/.config/Claude` |
| Our data root | `~/Library/Application Support/Claude Profiles` | `%APPDATA%\Claude Profiles` | `~/.config/claude-profiles` |
| Process listing | `ps -axo pid=,args=` | `Get-CimInstance Win32_Process` via PowerShell | `ps -axo pid=,args=` |
| Quit | SIGTERM → SIGKILL | `taskkill /PID` → `/F` | SIGTERM → SIGKILL |
| Focus | `NSRunningApplication` | Win32 `SetForegroundWindow` | best-effort, may be unsupported |
| Shared config | symlink | hardlink | symlink |

Three of these deserve explanation.

**Windows MSIX virtualization.** The official installer is MSIX, which redirects
writes to `%APPDATA%` into a per-package `LocalCache`. The path the app actually
reads may not be the path a user sees. The Windows backend therefore does not
hardcode one location: it probes the candidates and picks the one that exists,
preferring the package path when both do.

**Windows shared config uses hardlinks, not symlinks.** Creating a symlink on
Windows requires either Developer Mode or elevation; creating a hardlink to a file
on the same volume requires neither. Both `%APPDATA%\Claude` and
`%APPDATA%\Claude Profiles` live on the user's volume, so a hardlink works
unprivileged. Its weakness is that a program which rewrites the file by
replace-and-rename breaks the link. This is tolerable because the link is
re-established before every launch, and a broken link is detected by the same
"is it a regular file?" branch that already adopts contents back into the shared
copy.

**Linux focus may be impossible.** Under native Wayland, no application can raise
another application's window without compositor cooperation. The Linux backend
tries `wmctrl` and then `xdotool`, and if neither works it reports focus as
unsupported. The tray then shows the instance as running but the row does nothing
on click, with a tooltip saying so. This is a genuine platform limitation, not a
bug to fix later.

## Scope

In scope: profile management, parallel launch, focus, quit, liveness detection,
shared MCP config, on all three platforms.

Out of scope: syncing chat history between profiles, automating sign-in, modifying
the Claude Desktop installation in any way, packaging/signing/notarizing for
distribution.

## User Stories

- As a user with two accounts, I open the tray and see both profiles, launch each,
  and use both Claude Desktop windows at the same time.
- As a user, I click a profile that is already running and its window comes to the
  front instead of a second copy starting.
- As a user, I edit my MCP server config once and every profile picks it up.
- As a user, I add a third profile, launch it, and sign in to a new account without
  disturbing the other two.
- As a user, I quit Claude Profiles, reopen it, and it still knows which instances
  are running.
- As a Linux user on Wayland, I see clearly that focusing is unavailable on my
  system rather than clicking a row that silently does nothing.

## Architecture

### Stack

Tauri v2. All process and filesystem work lives in Rust. The tray menu is built
natively from Rust. A small web UI window handles profile management (add, rename,
delete) only.

### The platform seam

Everything OS-specific sits behind one trait, implemented three times. Nothing
else in the codebase may branch on the operating system.

```rust
pub trait Platform {
    fn data_root(&self) -> Result<PathBuf>;
    fn default_profile_dir(&self) -> Result<PathBuf>;
    fn claude_binary(&self) -> Result<PathBuf>;
    fn running_instances(&self) -> Result<Vec<RunningInstance>>;
    fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()>;
    fn focus(&self, pid: i32) -> Result<FocusOutcome>;
    fn quit(&self, pid: i32) -> Result<()>;
}

pub enum FocusOutcome { Focused, Unsupported(String) }
```

The core — profile registry, shared-config decision logic, process-output parsing,
menu row construction — is platform-independent and unit tested on any machine.
Each backend's own parsing (Windows CSV, Unix `ps`) is tested against captured
fixture strings, so all three backends' logic is testable from a Mac. Only the
live calls (spawn, focus, quit) need real hardware.

### On-disk layout

```
<data_root>/
  profiles.json                        # profile registry
  shared/claude_desktop_config.json    # shared MCP config, source of truth
  profiles/<id>/                       # per-profile Electron user-data dir
      claude_desktop_config.json       # link -> ../../shared/claude_desktop_config.json
```

`profiles.json` holds, per profile: `id` (uuid), `label`, `path`, `is_default`,
and `last_known_account_uuid` (nullable).

### The Default profile

The platform's existing Claude Desktop user-data directory is registered as a
profile named "Default" with `is_default: true`. It is **not** moved, copied, or
modified. It launches with no `--user-data-dir` flag at all. This guarantees the
account already in use keeps working exactly as before and requires no migration.

Its `claude_desktop_config.json` participates in shared config the same way as any
other profile: by link. That is the one write Claude Profiles makes inside the
stock directory, and the user is told before it happens on first run.

### Components

**`platform`** — the trait above plus `macos.rs`, `windows.rs`, `linux.rs`.

**`profile_store`** — reads and writes `profiles.json`; creates and deletes profile
directories. Pure filesystem logic, no process or OS knowledge.

**`shared_config`** — decides what to do with a profile's `claude_desktop_config.json`
and delegates the actual link creation to the platform. Called before every launch:

1. Already a link to the shared file → done.
2. A regular file → copy its contents to `shared/` (so edits Claude Desktop wrote
   are not lost), then replace it with a link.
3. Absent → create the link.
4. `shared/` does not exist → create it, seeded from whatever was adopted, else `{}`.

Case 2 overwrites the shared file. That is the accepted trade-off: the last config
Claude Desktop wrote wins, which matches what a user editing config through the app
expects. On Windows, a broken hardlink presents as case 2 and self-heals.

**`instance_manager`** — launch, quit, focus, composed from the platform backend.
Launch spawns the binary detached, with `--user-data-dir=<path>` for non-default
profiles and no flag for Default.

**Liveness** never trusts stored PIDs. On startup and on every tray open, the app
enumerates running processes and matches each profile by its `--user-data-dir`
argument (and, for Default, a main Claude process carrying no such argument). This
is what lets Claude Profiles restart without losing track of instances. Helper and
renderer subprocesses carry the same argument and must be excluded, or one instance
would look like several.

## Profile identity

Labels are entirely manual. Auto-detecting the account email was investigated and
**dropped**: `config.json` contains no readable email, only `lastKnownAccountUuid`
and an encrypted `oauth:tokenCache`.

`lastKnownAccountUuid` is read from each profile and used for exactly one thing: if
two profiles report the same UUID, both are marked in the menu as signed in to the
same account. No email is ever displayed.

## Error handling

Every failure degrades to a disabled tray row carrying its reason. Nothing crashes
the tray.

- Claude Desktop not found → all rows disabled, one row explains it, naming the
  paths that were probed.
- Profile directory deleted externally → row marked missing, offers to recreate.
- Spawn fails → row shows the OS error.
- Link repair fails → launch is blocked for that profile with the reason shown,
  rather than launching with a silently unshared config.
- Focus unsupported (Linux/Wayland) → row shows the instance as running and states
  that focusing is unavailable on this system.

Deleting a profile always confirms, names the directory, and states how much data
will be destroyed. Deleting is refused while that profile's instance is running.

## Testing

Unit tests, runnable on any of the three platforms regardless of which one they
target:

- `profile_store`: create, rename, delete, load a corrupt registry.
- `shared_config`: all four cases above, including the copy-back path.
- Unix `ps` parsing: fixture lists → correct live/dead verdicts, including the
  no-flag Default case, helper-process exclusion, and a decoy path that is a prefix
  of another.
- Windows `Win32_Process` CSV parsing: the same set of cases against captured
  Windows output, including quoted command lines containing spaces.
- Path probing: given a fake filesystem root, each backend picks the expected
  binary and default-profile directory, including the MSIX-vs-classic choice.

Launch, focus, and quit are verified manually per platform; they cannot be
meaningfully faked. Each backend carries an explicit manual acceptance checklist,
and a backend is not done until someone has run it on that OS.

## Decisions deliberately closed

- Instances share one application icon and are indistinguishable in the OS task
  switcher. Accepted. Generating per-profile app bundles was rejected as
  code-signing risk and breakage on Claude Desktop updates. The tray is the
  navigation surface.
- No hot-swapping of profile data. Parallel instances make it unnecessary.
- No packaging or code signing in this scope. `pnpm tauri build` produces an
  unsigned local build; distribution is a separate project.
