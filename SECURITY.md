# Security Policy

## Reporting a vulnerability

Please report privately rather than in a public issue: open a [security advisory](https://github.com/husniadil/claude-profiles/security/advisories/new), or email husni.adil@gmail.com.

This is a personal project maintained in spare time. Expect a first response within a week; please do not read silence as dismissal.

## What this app touches

Worth knowing when judging whether something is a security issue:

- **Profile directories** are Claude Desktop user-data directories. They hold session state and credentials for a signed-in account. Anything that leaks a path, reads their contents, or deletes the wrong one is a security concern, not just a bug.
- **`delete_profile` removes a directory tree recursively.** Its guards — the Default profile can never be removed, and a running profile refuses deletion — are what stand between a mistake and real data loss.
- **The shared MCP configuration** is linked into every profile. A change that writes to it affects every profile at once, and MCP config can contain server credentials.
- **The account UUID** is read from each profile's `config.json` only to warn when two profiles look signed in to the same account. Email addresses are never read or displayed, and nothing is sent anywhere: this app makes no network requests of its own.

## Release integrity

Release binaries are **unsigned**. This means the operating system cannot verify they came from this project, and you should only trust artifacts downloaded from this repository's Releases page. If you obtained a "Claude Profiles" build anywhere else, do not trust it.
