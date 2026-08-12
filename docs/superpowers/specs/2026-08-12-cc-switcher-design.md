# cc-switcher — Design

Date: 2026-08-12
Status: Approved

## Problem

Claude Desktop on macOS supports exactly one signed-in account. Working across a
personal account and a work account means signing out and back in, losing the
running state of whichever account you left.

## Solution

A macOS menu bar app that runs **multiple Claude Desktop instances in parallel**,
one per account, each with its own Electron user-data directory.

This is not an account switcher that swaps files. Nothing is moved or copied at
switch time. Each profile is a permanently separate user-data directory, and the
app launches, focuses, and quits instances that point at them.

### Feasibility (verified 2026-08-12)

Two instances were launched simultaneously with separate `--user-data-dir` values
and both ran independently. Electron enforces single-instance via a lock file
*inside* the user-data directory, so distinct directories mean distinct locks.

Two constraints follow from the verification:

- The binary at `/Applications/Claude.app/Contents/MacOS/Claude` must be executed
  directly. `open -a Claude` activates the existing instance instead of starting
  a new one.
- Each fresh user-data directory is populated by Claude Desktop on first launch,
  including its own `claude_desktop_config.json`.

## Scope

In scope: profile management, parallel launch, focus, quit, liveness detection,
shared MCP config.

Out of scope: cross-platform support, syncing chat history between profiles,
automating sign-in, modifying `Claude.app` in any way.

## User Stories

- As a user with two accounts, I open the menu bar and see both profiles, launch
  each, and use both Claude Desktop windows at the same time.
- As a user, I click a profile that is already running and its window comes to
  the front instead of a second copy starting.
- As a user, I edit my MCP server config once and every profile picks it up.
- As a user, I add a third profile, launch it, and sign in to a new account
  without disturbing the other two.
- As a user, I quit cc-switcher, reopen it, and it still knows which instances
  are running.

## Architecture

### Stack

Tauri v2. All process and filesystem work lives in Rust. The tray menu is built
natively from Rust. A small web UI window handles profile management (add,
rename, delete) only.

### On-disk layout

```
~/Library/Application Support/cc-switcher/
  profiles.json                        # profile registry
  shared/claude_desktop_config.json    # shared MCP config, source of truth
  profiles/<id>/                       # per-profile Electron user-data dir
      claude_desktop_config.json       # symlink -> ../../shared/claude_desktop_config.json
```

`profiles.json` holds, per profile: `id` (uuid), `label`, `path`, `is_default`,
and `last_known_account_uuid` (nullable).

### The Default profile

The existing `~/Library/Application Support/Claude` directory is registered as a
profile named "Default" with `is_default: true`. It is **not** moved, copied, or
modified. It launches with no `--user-data-dir` flag at all. This guarantees the
account already in use keeps working exactly as before and requires no migration.

Because it is not under our directory, the Default profile's
`claude_desktop_config.json` participates in shared config the same way as any
other: by symlink (see below). This is the one write cc-switcher makes inside the
stock directory, and on first run the user is told before it happens.

### Components

**`profile_store`** — reads and writes `profiles.json`; creates and deletes
profile directories. Pure filesystem logic, no process knowledge. Unit tested
against a temp directory.

**`shared_config`** — guarantees a profile's `claude_desktop_config.json` is a
symlink to `shared/`. Called before every launch. Logic:

1. If it is already a symlink pointing at the shared file, done.
2. If it is a regular file, copy its contents to `shared/` (so edits Claude
   Desktop wrote are not lost), then replace it with the symlink.
3. If it is absent, create the symlink.
4. If `shared/` does not exist yet, seed it from the Default profile's config.

Step 2 overwrites the shared file. That is the accepted trade-off: the last
config Claude Desktop wrote wins over the previous shared content, which matches
what a user editing config through the app would expect.

**`instance_manager`** — launch, focus, quit, and liveness.

- *Launch*: spawn the Claude binary detached, with `--user-data-dir=<path>` for
  non-default profiles and no flag for Default. Record the PID.
- *Focus*: `NSRunningApplication::runningApplicationWithProcessIdentifier` on the
  recorded PID, then activate.
- *Quit*: `SIGTERM` to the PID, which Electron handles as a clean quit. If the
  process is still alive after 10 seconds, `SIGKILL`.
- *Liveness*: never trusts stored PIDs alone. On startup and on every menu open,
  it enumerates running processes and matches each profile by the
  `--user-data-dir=<path>` argument (and, for Default, a Claude main process with
  no such argument). This is what lets cc-switcher restart without losing track
  of instances someone else started. The argument-matching function is pure and
  unit tested against captured process-listing fixtures.

### Data flow

Menu opens → `instance_manager` rescans processes → tray menu is rebuilt with a
live/dead marker per profile. Clicking a dead profile calls `shared_config` then
launches. Clicking a live profile focuses it.

## Profile identity

Labels are entirely manual. Auto-detecting the account email was investigated and
**dropped**: `config.json` contains no readable email, only `lastKnownAccountUuid`
and an encrypted `oauth:tokenCache`.

`lastKnownAccountUuid` is read from each profile and used for exactly one thing:
if two profiles report the same UUID, both are marked in the menu with a note
that they are signed in to the same account. No email is ever displayed.

## Error handling

Every failure degrades to a greyed-out menu row carrying its reason. Nothing
crashes the tray.

- `Claude.app` missing or binary not executable → all rows disabled, one row
  explains it.
- Profile directory deleted externally → row marked missing, offers to recreate.
- Spawn fails → row shows the OS error.
- Symlink repair fails (permissions) → launch is blocked for that profile with
  the reason shown, rather than launching with a silently unshared config.

Deleting a profile always confirms, names the directory, and states how much data
will be destroyed.

## Testing

Rust unit tests, runnable without Claude Desktop installed:

- `profile_store`: create, rename, delete, load a corrupt registry.
- `shared_config`: all four symlink cases above, including the copy-back path.
- process matching: parse fixture process lists into correct live/dead verdicts,
  including the no-flag Default case and a decoy path that is a prefix of another.

Launch, focus, and quit are verified manually against the real app; they cannot
be meaningfully faked.

## Open decisions deliberately closed

- Instances share one Dock icon and are indistinguishable in ⌘-Tab. Accepted.
  Generating per-profile `.app` wrappers was rejected as code-signing risk and
  breakage on Claude Desktop updates. The menu bar is the navigation surface.
- No hot-swapping of profile data. Parallel instances make it unnecessary.
