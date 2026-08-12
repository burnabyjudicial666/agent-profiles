# Changelog

Notable changes, newest first. This project follows [Semantic Versioning](https://semver.org).

## [Unreleased]

Nothing yet.

## [0.1.0] — 2026-08-13

First release. Verified on macOS; **Windows and Linux have never been compiled or run on real hardware** — see the platform checklists in the README.

### Added

- Menu bar and system tray app for running multiple Claude Desktop instances in parallel, one profile per account.
- The existing Claude Desktop installation is adopted in place as the **Default** profile, never moved or copied.
- Tray menu showing each profile's live state, with Focus for a running profile and Launch for a stopped one, plus a Quit action per running instance.
- Management window to add, rename, and delete profiles, with the directory size shown before deletion.
- Shared MCP configuration linked into every profile: symbolic links on macOS and Linux, hardlinks on Windows.
- A warning when two profiles appear to be signed in to the same account.
- Opt-in **Launch at login**, off by default and hidden in development builds.
- Per-profile desktop identity on Linux so each profile gets its own taskbar entry.

### Safety

- Claude Desktop has no single-instance lock, so a second process on one user-data directory corrupts its databases. Processes are rescanned immediately before every launch, and a scan that fails **refuses to launch** rather than assuming nothing is running.
- Deletion is refused while that profile is running, and the Default profile can never be deleted.
- A corrupt profile registry is preserved as `profiles.json.corrupt` instead of being overwritten.
- Adopting a profile's existing MCP configuration never overwrites an established shared one; the displaced file is kept alongside the profile.

[Unreleased]: https://github.com/husniadil/claude-profiles/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/husniadil/claude-profiles/releases/tag/v0.1.0
