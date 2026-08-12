# Claude Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tray app for macOS, Windows, and Linux that runs several Claude Desktop instances in parallel, one per account, each with its own Electron user-data directory.

**Architecture:** A Tauri v2 app whose Rust side owns everything. Every OS-specific behaviour sits behind one `Platform` trait implemented three times; no other module branches on the operating system. The core — profile registry, shared-config decisions, process-output parsing, menu rows — is pure logic tested from any machine. Each backend's text parsing is tested against captured fixture output, so all three are testable from a Mac; only spawn, focus, and quit need real hardware, and each backend carries a manual acceptance checklist.

**Tech Stack:** Rust (Tauri v2, serde, anyhow, uuid), TypeScript + Vite (vanilla, no framework), pnpm. Platform crates: `libc` and `objc2-app-kit` (macOS), `libc` (Linux), `windows` (Windows).

## Global Constraints

- App name is **Claude Profiles**. Identifier `com.husniadil.claude-profiles`. The repo directory stays `cc-switcher`; that string must not appear in any user-visible text.
- Never modify the Claude Desktop installation.
- On macOS the binary at `Contents/MacOS/Claude` must be executed directly. Never `open -a Claude` — it activates the existing instance instead of starting a new one.
- The Default profile is the platform's existing Claude user-data directory, used in place. Never moved or copied. It launches with **no** `--user-data-dir` flag.
- The only write inside the stock Claude directory is replacing `claude_desktop_config.json` with a link.
- No email is ever displayed. `lastKnownAccountUuid` is used only to warn about two profiles sharing one account.
- Every failure degrades to a disabled tray row carrying its reason. The tray thread must never panic or unwrap.
- **Never spawn a second process against a user-data directory that already has one.** Claude Desktop takes no single-instance lock (verified 2026-08-12); two processes on one profile corrupt its databases. Liveness is re-checked immediately before every spawn, not inherited from the menu that triggered it.
- Only `platform/macos.rs`, `platform/windows.rs`, `platform/linux.rs` may contain `#[cfg(target_os = ...)]` or OS-specific calls.
- Package manager is `pnpm`. Rust edition 2021.
- Parallel instances are **verified on macOS only**. Tasks 8 and 9 must be accepted on real Windows and Linux hardware before being marked done.

## File Structure

```
src-tauri/src/
  main.rs             # thin binary shim; only calls claude_profiles_lib::run()
  lib.rs              # Tauri setup, tray wiring, command registration, `mod` declarations
  platform/
    mod.rs            # Platform trait, RunningInstance, FocusOutcome, current()
    macos.rs
    windows.rs
    linux.rs
    unix_ps.rs        # `ps` output parsing, shared by macos + linux
    win_proc.rs       # Win32_Process CSV parsing
  paths.rs            # rooted path resolution
  profile_store.rs    # profiles.json registry CRUD
  shared_config.rs    # config link decision logic (delegates linking to Platform)
  account.rs          # lastKnownAccountUuid, duplicate detection
  instance_manager.rs # launch / quit / focus
  tray.rs             # menu row construction + tray rebuild
  commands.rs         # #[tauri::command] functions for the web UI
src/
  index.html, main.ts, styles.css
```

---

### Task 1: Toolchain and scaffold

**Files:**
- Create: project skeleton, `.gitignore`

**Interfaces:**
- Consumes: nothing
- Produces: a buildable Tauri v2 app named Claude Profiles with a tray icon; `cargo test` runs from `src-tauri/`

- [ ] **Step 1: Confirm the Rust toolchain**

Rust is already installed on the development machine, managed by **mise**
(`rust = "latest"` in `~/.config/mise/config.toml`, currently resolving to
1.88.0). Do not install rustup — it would shadow the managed toolchain.

The catch: mise's shims are added to `PATH` by the interactive shell profile, so
a **non-interactive shell sees no `cargo` at all**. Every command in this plan
that calls `cargo` or `pnpm tauri` must run with the shims on `PATH`:

```bash
export PATH="$HOME/.local/share/mise/shims:$PATH"
cargo --version
```

Expected: `cargo 1.88.0` or newer.

If a dependency later refuses to build with an "requires rustc 1.x or newer"
error, refresh the managed toolchain rather than side-loading another one:

```bash
mise use -g rust@latest
```

- [ ] **Step 2: Scaffold**

From the repo root (which already contains `docs/` and a git repo):

```bash
pnpm create tauri-app@latest . --template vanilla-ts --manager pnpm --identifier com.husniadil.claude-profiles --yes
pnpm install
```

If the scaffolder refuses a non-empty directory, scaffold into a temp directory and move `src/`, `src-tauri/`, `package.json`, `pnpm-lock.yaml`, `vite.config.ts`, `tsconfig.json`, `index.html` into the repo root.

Then set the product name to `Claude Profiles` in `src-tauri/tauri.conf.json` (`productName`) and `package.json` (`name` stays kebab-case `claude-profiles`).

- [ ] **Step 3: Dependencies**

Resolve versions at install time rather than pinning numbers from this document —
`objc2` and `windows` move fast and any number written here is stale on arrival:

```bash
cd src-tauri
cargo add serde --features derive
cargo add serde_json anyhow
cargo add uuid --features v4
cargo add --dev tempfile
cargo add tauri --features tray-icon,image-png
```

Then add the platform-gated ones by hand in `Cargo.toml`, taking the current
major versions from `cargo search` or docs.rs:

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "<current>"
objc2-app-kit = { version = "<current>", features = ["NSRunningApplication"] }

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "<current>", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
] }
```

Feature names inside `objc2-app-kit` and `windows` are also version-dependent. If
one does not resolve, open the crate on docs.rs at the version cargo picked and
use the real name. Do not drop a dependency to make the build pass — Tasks 7 and 8
need them.

- [ ] **Step 4: Verify**

```bash
cd src-tauri && cargo build 2>&1 | tail -5 && cargo test 2>&1 | tail -5
```

Expected: build succeeds; `cargo test` reports `0 passed`.

If a platform crate's version or feature name does not resolve, check the crate on docs.rs and use the real names. Do not drop the dependency — Tasks 7 and 8 need them.

- [ ] **Step 5: Generate the app icons from the supplied artwork**

`assets/icons/` holds a favicon pack (the white "cp" mark on orange). Tauri needs
its own set, so generate it rather than copying files across:

```bash
pnpm tauri icon assets/icons/android-chrome-512x512.png
```

This writes `src-tauri/icons/` (`32x32.png`, `128x128.png`, `128x128@2x.png`,
`icon.icns`, `icon.ico`, and the Windows Store sizes) and wires them into
`tauri.conf.json`. The source is 512×512; Tauri prefers 1024×1024 and will warn
about upscaling. Accept the warning — the mark is flat vector-style artwork and
survives it. If the 512 source ever produces a visibly soft `.icns`, regenerate
the artwork at 1024 rather than sharpening the output.

Also copy `assets/icons/favicon-32x32.png` to `src/favicon.png` and reference it
from `index.html`, so the management window carries the same mark.

- [ ] **Step 6: Give the tray a legible icon**

The supplied mark is a solid orange square. In the macOS menu bar that reads as a
coloured block and ignores light/dark menu bars entirely. Produce a monochrome
variant for the tray only:

```bash
mkdir -p src-tauri/icons/tray
sips -s format png --resampleWidth 44 assets/icons/android-chrome-512x512.png \
  --out src-tauri/icons/tray/tray-icon.png
```

Then in `TrayIconBuilder`, set `.icon_as_template(true)` on macOS so the system
recolours it for the current menu bar. A template image must be black-and-alpha
only; if the "cp" mark still renders as a filled square after templating, replace
this file with an alpha-only version of the glyph (white letters become opaque
black, the orange background becomes transparent) — the tray icon is the one
place where the brand colour must be given up for legibility.

On Windows and Linux the tray accepts the colour icon as-is; keep
`icon_as_template` behind `#[cfg(target_os = "macos")]`.

- [ ] **Step 7: Confirm the tray appears**

```bash
pnpm tauri dev
```

Expected: the "cp" icon appears in the menu bar / system tray, legible against
both a light and a dark menu bar. Quit it.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore: scaffold Claude Profiles as a Tauri v2 tray app"
```

---

### Task 2: Platform seam and paths

**Files:**
- Create: `src-tauri/src/platform/mod.rs`, `src-tauri/src/paths.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub struct RunningInstance { pub pid: i32, pub user_data_dir: Option<PathBuf> }`
  - `pub enum FocusOutcome { Focused, Unsupported(String) }`
  - ```rust
    pub trait Platform: Send + Sync {
        fn data_root(&self) -> Result<PathBuf>;
        fn default_profile_dir(&self) -> Result<PathBuf>;
        fn claude_binary(&self) -> Result<PathBuf>;
        fn running_instances(&self) -> Result<Vec<RunningInstance>>;
        fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()>;
        /// `profile_id` is carried alongside `pid` because Linux locates the window
    /// by the `--class` it launched with, not by pid. Other backends ignore it.
    fn focus(&self, pid: i32, profile_id: &str) -> Result<FocusOutcome>;
        fn quit(&self, pid: i32) -> Result<()>;
    }
    ```

    Task 9 later adds three defaulted methods (`extra_launch_args`,
    `register_identity`, `unregister_identity`) for the Linux desktop-identity
    mechanism. They are **not** declared here, because they take a `&Profile` and
    `Profile` is not defined until Task 3. Adding them later costs nothing:
    defaulted trait methods do not disturb existing implementors.
  - `pub fn current() -> Box<dyn Platform>` — compiles to the one backend for this OS
  - `pub fn find_for(instances: &[RunningInstance], profile_dir: &Path, is_default: bool) -> Option<i32>`
  - `pub struct Paths { root: PathBuf }` with `Paths::new(root)`, `profiles_json()`, `profiles_dir()`, `profile_dir(id)`, `shared_config()`
  - `pub const CONFIG_FILENAME: &str = "claude_desktop_config.json";`

`find_for` lives here because it is the one piece of matching logic all three backends share.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/paths.rs`:

```rust
use std::path::PathBuf;

pub const CONFIG_FILENAME: &str = "claude_desktop_config.json";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_rooted_at_the_given_directory() {
        let p = Paths::new("/root");
        assert_eq!(p.profiles_json(), PathBuf::from("/root/profiles.json"));
        assert_eq!(p.profiles_dir(), PathBuf::from("/root/profiles"));
        assert_eq!(p.profile_dir("abc"), PathBuf::from("/root/profiles/abc"));
        assert_eq!(
            p.shared_config(),
            PathBuf::from("/root/shared/claude_desktop_config.json")
        );
    }
}
```

Create `src-tauri/src/platform/mod.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn instances() -> Vec<RunningInstance> {
        vec![
            RunningInstance { pid: 1, user_data_dir: Some(PathBuf::from("/p/work")) },
            RunningInstance { pid: 2, user_data_dir: None },
            RunningInstance { pid: 3, user_data_dir: Some(PathBuf::from("/p/work2")) },
        ]
    }

    #[test]
    fn a_profile_matches_only_its_exact_directory() {
        let i = instances();
        assert_eq!(find_for(&i, &PathBuf::from("/p/work"), false), Some(1));
        assert_eq!(find_for(&i, &PathBuf::from("/p/work2"), false), Some(3));
        assert_eq!(find_for(&i, &PathBuf::from("/p/none"), false), None);
    }

    #[test]
    fn the_default_profile_matches_the_process_with_no_flag() {
        assert_eq!(find_for(&instances(), &PathBuf::from("/ignored"), true), Some(2));
    }

    #[test]
    fn no_flagless_process_means_default_is_not_running() {
        let i = vec![RunningInstance { pid: 9, user_data_dir: Some(PathBuf::from("/p/x")) }];
        assert_eq!(find_for(&i, &PathBuf::from("/ignored"), true), None);
    }
}
```

Add `mod paths;` and `mod platform;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test`
Expected: FAIL — `cannot find type Paths` / `cannot find function find_for`.

- [ ] **Step 3: Write the implementation**

Add to `paths.rs` above its test module:

```rust
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn profiles_json(&self) -> PathBuf {
        self.root.join("profiles.json")
    }
    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }
    pub fn profile_dir(&self, id: &str) -> PathBuf {
        self.profiles_dir().join(id)
    }
    pub fn shared_config(&self) -> PathBuf {
        self.root.join("shared").join(CONFIG_FILENAME)
    }
}
```

Add to `platform/mod.rs` above its test module:

```rust
use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod unix_ps;
pub mod win_proc;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, PartialEq)]
pub struct RunningInstance {
    pub pid: i32,
    pub user_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FocusOutcome {
    Focused,
    Unsupported(String),
}

pub trait Platform: Send + Sync {
    fn data_root(&self) -> Result<PathBuf>;
    fn default_profile_dir(&self) -> Result<PathBuf>;
    fn claude_binary(&self) -> Result<PathBuf>;
    fn running_instances(&self) -> Result<Vec<RunningInstance>>;
    fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()>;
    /// `profile_id` is carried alongside `pid` because Linux locates the window
    /// by the `--class` it launched with, not by pid. Other backends ignore it.
    fn focus(&self, pid: i32, profile_id: &str) -> Result<FocusOutcome>;
    fn quit(&self, pid: i32) -> Result<()>;
}

pub fn find_for(
    instances: &[RunningInstance],
    profile_dir: &Path,
    is_default: bool,
) -> Option<i32> {
    instances
        .iter()
        .find(|i| match (&i.user_data_dir, is_default) {
            (None, true) => true,
            (Some(dir), false) => dir == profile_dir,
            _ => false,
        })
        .map(|i| i.pid)
}

pub fn current() -> Box<dyn Platform> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOs);
    #[cfg(target_os = "linux")]
    return Box::new(linux::Linux);
    #[cfg(target_os = "windows")]
    return Box::new(windows::Windows);
}
```

Create empty `platform/unix_ps.rs` and `platform/win_proc.rs` for now (Tasks 5 and 6 fill them), and stub backend files that fail to compile only if referenced — simplest is to create each backend file with a unit struct and `todo!()` bodies, replaced in Tasks 7–9.

Note both `unix_ps` and `win_proc` are compiled on **every** platform. That is deliberate: it is what lets the Windows parser be tested from a Mac. Neither module may call OS APIs — they are string-in, data-out.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "feat: add platform trait seam and rooted paths"
```

---

### Task 3: Profile store

**Files:**
- Create: `src-tauri/src/profile_store.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `paths::Paths`
- Produces:
  - `pub struct Profile { pub id: String, pub label: String, pub path: PathBuf, pub is_default: bool, pub last_known_account_uuid: Option<String> }` (derives `Serialize, Deserialize, Clone, Debug, PartialEq`)
  - `ProfileStore::load(paths: &Paths, default_dir: &Path) -> Result<ProfileStore>`
  - `store.save(paths) -> Result<()>`, `list() -> &[Profile]`, `get(id) -> Option<&Profile>`
  - `store.add(label: &str, paths: &Paths) -> Result<Profile>`
  - `store.rename(id, label) -> Result<()>`
  - `store.remove(id, paths) -> Result<()>` — refuses the Default profile
  - `store.set_account_uuid(id, Option<String>)`

`load` takes the Default directory as a parameter rather than asking the platform, so it stays testable with a temp path. A missing `profiles.json` is a first run and seeds the Default profile without creating a backup. If an existing registry cannot be read or parsed, `load` moves it to `profiles.json.corrupt`, replacing any previous backup, before falling back to a freshly seeded store; this keeps profiles whose directories still exist recoverable after the next save.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/profile_store.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    fn fixture() -> (tempfile::TempDir, Paths, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("root"));
        let default_dir = dir.path().join("stock-claude");
        std::fs::create_dir_all(&default_dir).unwrap();
        (dir, paths, default_dir)
    }

    #[test]
    fn first_load_seeds_only_the_default_profile() {
        let (_d, paths, def) = fixture();
        let store = ProfileStore::load(&paths, &def).unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(store.list()[0].is_default);
        assert_eq!(store.list()[0].label, "Default");
        assert_eq!(store.list()[0].path, def);
        assert!(!paths.profiles_json().with_extension("json.corrupt").exists());
    }

    #[test]
    fn a_corrupt_registry_falls_back_to_the_default_profile() {
        let (_d, paths, def) = fixture();
        std::fs::create_dir_all(paths.profiles_json().parent().unwrap()).unwrap();
        std::fs::write(paths.profiles_json(), b"{ not json").unwrap();
        let store = ProfileStore::load(&paths, &def).unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(store.list()[0].is_default);
    }

    #[test]
    fn a_corrupt_registry_is_preserved_before_falling_back_to_default() {
        let (_d, paths, def) = fixture();
        let corrupt_bytes = b"{ not json";
        std::fs::create_dir_all(paths.profiles_json().parent().unwrap()).unwrap();
        std::fs::write(&paths.profiles_json(), corrupt_bytes).unwrap();

        let store = ProfileStore::load(&paths, &def).unwrap();

        assert_eq!(store.list().len(), 1);
        assert!(store.list()[0].is_default);
        assert!(!paths.profiles_json().exists());
        assert_eq!(
            std::fs::read(paths.profiles_json().with_extension("json.corrupt")).unwrap(),
            corrupt_bytes
        );
    }

    #[test]
    fn added_profiles_get_a_directory_and_survive_a_reload() {
        let (_d, paths, def) = fixture();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let created = store.add("Kerja", &paths).unwrap();
        store.save(&paths).unwrap();

        assert!(created.path.is_dir());
        assert!(!created.is_default);
        assert_eq!(created.path, paths.profile_dir(&created.id));

        let reloaded = ProfileStore::load(&paths, &def).unwrap();
        assert_eq!(reloaded.list().len(), 2);
        assert_eq!(reloaded.get(&created.id).unwrap().label, "Kerja");
    }

    #[test]
    fn renaming_changes_only_the_label() {
        let (_d, paths, def) = fixture();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let p = store.add("Kerja", &paths).unwrap();
        store.rename(&p.id, "Kantor").unwrap();
        assert_eq!(store.get(&p.id).unwrap().label, "Kantor");
        assert_eq!(store.get(&p.id).unwrap().path, p.path);
    }

    #[test]
    fn removing_deletes_the_directory() {
        let (_d, paths, def) = fixture();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let p = store.add("Kerja", &paths).unwrap();
        store.remove(&p.id, &paths).unwrap();
        assert!(store.get(&p.id).is_none());
        assert!(!p.path.exists());
    }

    #[test]
    fn the_default_profile_cannot_be_removed() {
        let (_d, paths, def) = fixture();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let id = store.list()[0].id.clone();
        assert!(store.remove(&id, &paths).is_err());
        assert_eq!(store.list().len(), 1);
        assert!(def.exists());
    }
}
```

Add `mod profile_store;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test profile_store`
Expected: FAIL — `cannot find type ProfileStore`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::paths::Paths;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    pub is_default: bool,
    #[serde(default)]
    pub last_known_account_uuid: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ProfileStore {
    profiles: Vec<Profile>,
}

impl ProfileStore {
    pub fn load(paths: &Paths, default_dir: &Path) -> Result<Self> {
        let file = paths.profiles_json();
        let mut store = match std::fs::read(&file) {
            Ok(raw) => match serde_json::from_slice::<ProfileStore>(&raw) {
                Ok(store) => store,
                Err(_) => {
                    preserve_corrupt_registry(&file)?;
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(_) => {
                preserve_corrupt_registry(&file)?;
                Self::default()
            }
        };

        if !store.profiles.iter().any(|p| p.is_default) {
            store.profiles.insert(
                0,
                Profile {
                    id: "default".into(),
                    label: "Default".into(),
                    path: default_dir.to_path_buf(),
                    is_default: true,
                    last_known_account_uuid: None,
                },
            );
        }
        Ok(store)
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        let file = paths.profiles_json();
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn list(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn add(&mut self, label: &str, paths: &Paths) -> Result<Profile> {
        let id = uuid::Uuid::new_v4().to_string();
        let path = paths.profile_dir(&id);
        std::fs::create_dir_all(&path)?;
        let profile = Profile {
            id,
            label: label.to_string(),
            path,
            is_default: false,
            last_known_account_uuid: None,
        };
        self.profiles.push(profile.clone());
        Ok(profile)
    }

    pub fn rename(&mut self, id: &str, label: &str) -> Result<()> {
        let p = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| anyhow!("no profile with id {id}"))?;
        p.label = label.to_string();
        Ok(())
    }

    pub fn remove(&mut self, id: &str, _paths: &Paths) -> Result<()> {
        let idx = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| anyhow!("no profile with id {id}"))?;
        if self.profiles[idx].is_default {
            return Err(anyhow!("the Default profile cannot be removed"));
        }
        let removed = self.profiles.remove(idx);
        if removed.path.exists() {
            std::fs::remove_dir_all(&removed.path)?;
        }
        Ok(())
    }

    pub fn set_account_uuid(&mut self, id: &str, uuid: Option<String>) {
        if let Some(p) = self.profiles.iter_mut().find(|p| p.id == id) {
            p.last_known_account_uuid = uuid;
        }
    }
}

fn preserve_corrupt_registry(file: &Path) -> Result<()> {
    let corrupt = file.with_extension("json.corrupt");
    match std::fs::remove_file(&corrupt) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    std::fs::rename(file, corrupt)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test profile_store`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/profile_store.rs src-tauri/src/lib.rs
git commit -m "feat: add profile registry with Default profile seeding"
```

---

### Task 4: Shared config decision logic

**Files:**
- Create: `src-tauri/src/shared_config.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `paths::CONFIG_FILENAME`, `platform::Platform`
- Produces:
  - `pub enum LinkState { AlreadyLinked, AdoptFile(String), CreateFresh }`
  - `pub fn inspect(link: &Path, shared: &Path, is_linked: bool) -> Result<LinkState>`
  - `pub fn ensure_shared(platform: &dyn Platform, profile_dir: &Path, shared: &Path) -> Result<()>`

The decision is pure and testable; only creating the link is delegated to the platform. `is_linked` is supplied by the caller because "is this the shared file?" is checked differently for a symlink (compare `read_link`) than for a hardlink (compare inode/file-index).

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/shared_config.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn an_already_linked_config_needs_no_work() {
        let d = tmp();
        let link = d.path().join("cfg.json");
        std::fs::write(&link, "{}").unwrap();
        let shared = d.path().join("shared.json");
        assert_eq!(inspect(&link, &shared, true).unwrap(), LinkState::AlreadyLinked);
    }

    #[test]
    fn an_existing_regular_file_is_adopted_with_its_contents() {
        let d = tmp();
        let link = d.path().join("cfg.json");
        std::fs::write(&link, r#"{"mcpServers":{"a":{}}}"#).unwrap();
        let shared = d.path().join("shared.json");
        assert_eq!(
            inspect(&link, &shared, false).unwrap(),
            LinkState::AdoptFile(r#"{"mcpServers":{"a":{}}}"#.to_string())
        );
    }

    #[test]
    fn a_missing_config_means_create_fresh() {
        let d = tmp();
        let link = d.path().join("nothing.json");
        let shared = d.path().join("shared.json");
        assert_eq!(inspect(&link, &shared, false).unwrap(), LinkState::CreateFresh);
    }

    #[test]
    fn ensure_shared_seeds_an_empty_object_when_there_is_nothing_to_adopt() {
        let d = tmp();
        let profile = d.path().join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        let shared = d.path().join("shared").join(crate::paths::CONFIG_FILENAME);

        ensure_shared(&FakePlatform::default(), &profile, &shared).unwrap();

        assert_eq!(std::fs::read_to_string(&shared).unwrap().trim(), "{}");
    }

    #[test]
    fn ensure_shared_copies_an_existing_config_into_shared_before_linking() {
        let d = tmp();
        let profile = d.path().join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            profile.join(crate::paths::CONFIG_FILENAME),
            r#"{"mcpServers":{"keep":{}}}"#,
        )
        .unwrap();
        let shared = d.path().join("shared").join(crate::paths::CONFIG_FILENAME);

        ensure_shared(&FakePlatform::default(), &profile, &shared).unwrap();

        assert!(std::fs::read_to_string(&shared).unwrap().contains("keep"));
    }
}
```

The stand-in lives in its own module rather than inside `mod tests`, because
Task 11's tests need it too. Add it to `shared_config.rs`:

```rust
#[cfg(test)]
pub mod tests_support {
    use crate::platform::{FocusOutcome, Platform, RunningInstance};
    use std::path::{Path, PathBuf};

    /// Minimal Platform stand-in: real linking is a per-backend concern, so this
    /// one just copies. `running` lets a test stage a live instance.
    #[derive(Default)]
    pub struct FakePlatform {
        running: Vec<RunningInstance>,
    }

    impl FakePlatform {
        pub fn with_running(running: Vec<RunningInstance>) -> Self {
            Self { running }
        }
    }

    impl Platform for FakePlatform {
        fn data_root(&self) -> anyhow::Result<PathBuf> { unimplemented!() }
        fn default_profile_dir(&self) -> anyhow::Result<PathBuf> { unimplemented!() }
        fn claude_binary(&self) -> anyhow::Result<PathBuf> {
            Ok(PathBuf::from("/fake/claude"))
        }
        fn running_instances(&self) -> anyhow::Result<Vec<RunningInstance>> {
            Ok(self.running.clone())
        }
        fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> anyhow::Result<()> {
            std::fs::copy(shared, profile_dir.join(crate::paths::CONFIG_FILENAME))?;
            Ok(())
        }
        fn focus(&self, _pid: i32, _profile_id: &str) -> anyhow::Result<FocusOutcome> { unimplemented!() }
        fn quit(&self, _pid: i32) -> anyhow::Result<()> { unimplemented!() }
    }
}
```

`RunningInstance` must derive `Clone` for this, which Task 2 already specifies.
Import it in the test module with `use crate::shared_config::tests_support::FakePlatform;`.

Add `mod shared_config;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test shared_config`
Expected: FAIL — `cannot find function inspect`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::platform::Platform;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum LinkState {
    AlreadyLinked,
    AdoptFile(String),
    CreateFresh,
}

pub fn inspect(link: &Path, _shared: &Path, is_linked: bool) -> Result<LinkState> {
    if is_linked {
        return Ok(LinkState::AlreadyLinked);
    }
    match std::fs::read_to_string(link) {
        Ok(contents) => Ok(LinkState::AdoptFile(contents)),
        Err(_) => Ok(LinkState::CreateFresh),
    }
}

pub fn ensure_shared(platform: &dyn Platform, profile_dir: &Path, shared: &Path) -> Result<()> {
    let link = profile_dir.join(crate::paths::CONFIG_FILENAME);
    let is_linked = is_same_file(&link, shared);

    match inspect(&link, shared, is_linked)? {
        LinkState::AlreadyLinked => {
            write_shared_if_absent(shared)?;
            return Ok(());
        }
        LinkState::AdoptFile(contents) => {
            create_parent(shared)?;
            // Adopt ONLY into an empty slot. Once a shared config exists it is the
            // single source of truth for every profile, and a newly-added profile
            // carrying its own file must not overwrite it — that would silently
            // destroy the MCP servers every other profile is already using. The
            // displaced file is kept beside the profile so nothing is lost.
            if shared.exists() {
                let displaced = link.with_extension("json.replaced");
                std::fs::rename(&link, &displaced)?;
            } else {
                std::fs::write(shared, contents)?;
            }
        }
        LinkState::CreateFresh => write_shared_if_absent(shared)?,
    }

    if std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }
    platform.link_shared_config(profile_dir, shared)
}

fn write_shared_if_absent(shared: &Path) -> Result<()> {
    create_parent(shared)?;
    if !shared.exists() {
        std::fs::write(shared, "{}")?;
    }
    Ok(())
}

fn create_parent(shared: &Path) -> Result<()> {
    if let Some(parent) = shared.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// True when `link` and `shared` are the same underlying file — covering both the
/// symlink case (macOS, Linux) and the hardlink case (Windows).
fn is_same_file(link: &Path, shared: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(link), std::fs::metadata(shared)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(windows)]
    {
        // Compare the volume serial + file index, which is what actually identifies
        // a file on Windows. Length-and-creation-time was considered and rejected:
        // two freshly written `{}` files share both, so it reports "already linked"
        // for a profile that is not linked at all, leaving it silently unshared.
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        fn identity(path: &Path) -> Option<(u32, u32, u32)> {
            let file = std::fs::File::open(path).ok()?;
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            unsafe {
                GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut info).ok()?;
            }
            Some((
                info.dwVolumeSerialNumber,
                info.nFileIndexHigh,
                info.nFileIndexLow,
            ))
        }

        match (identity(link), identity(shared)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }
}
```

A false negative from `is_same_file` is cheap — it re-creates a link that was already correct. A false positive is not: the profile is reported as linked, the link is never created, and that profile quietly keeps a private config while the user believes it is shared. Both branches therefore compare real file identity (inode on Unix, volume serial + file index on Windows) rather than any metadata heuristic.

The Windows branch needs the `Win32_Storage_FileSystem` feature on the `windows` crate; add it in Task 8 if it is not already present.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test shared_config`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/shared_config.rs src-tauri/src/lib.rs
git commit -m "feat: share claude_desktop_config.json across profiles"
```

---

### Task 5: Unix process parsing

**Files:**
- Modify: `src-tauri/src/platform/unix_ps.rs`

**Interfaces:**
- Consumes: `platform::RunningInstance`
- Produces:
  - `pub fn parse(raw: &str, main_binaries: &[&str]) -> Vec<RunningInstance>`
  - `pub fn scan(main_binaries: &[&str]) -> Result<Vec<RunningInstance>>` — runs `ps -axo pid=,args=`

`main_binaries` differs per OS (`/Applications/Claude.app/Contents/MacOS/Claude` vs `claude-desktop`), so the parser takes it as a parameter and stays shared. Helper and renderer subprocesses carry the same `--user-data-dir` and are excluded by the `--type=` argument every Chromium child process has.

- [ ] **Step 1: Write the failing tests**

In `unix_ps.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const MAC: &str = "/Applications/Claude.app/Contents/MacOS/Claude";

    const MAC_FIXTURE: &str = concat!(
        "  501 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/p/work\n",
        "  502 /Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper --type=gpu-process --user-data-dir=/p/work\n",
        "  503 /Applications/Claude.app/Contents/MacOS/Claude\n",
        "  504 /usr/bin/unrelated --user-data-dir=/p/work\n",
        "  505 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/p/work2\n",
        "  506 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/h/Library/Application Support/Claude Profiles/profiles/abc\n",
    );

    const LINUX_FIXTURE: &str = concat!(
        " 1201 /usr/lib/claude-desktop/claude-desktop --user-data-dir=/home/h/.config/cp/profiles/a\n",
        " 1202 /usr/lib/claude-desktop/claude-desktop --type=renderer --user-data-dir=/home/h/.config/cp/profiles/a\n",
        " 1203 /usr/lib/claude-desktop/claude-desktop\n",
    );

    #[test]
    fn only_main_processes_are_returned() {
        let pids: Vec<i32> = parse(MAC_FIXTURE, &[MAC]).iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![501, 503, 505, 506]);
    }

    #[test]
    fn a_user_data_dir_containing_spaces_is_captured_whole() {
        let found = parse(MAC_FIXTURE, &[MAC]);
        assert_eq!(
            found.last().unwrap().user_data_dir,
            Some(PathBuf::from(
                "/Users/h/Library/Application Support/Claude Profiles/profiles/abc"
            ))
        );
    }

    #[test]
    fn the_user_data_dir_argument_is_extracted() {
        let found = parse(MAC_FIXTURE, &[MAC]);
        assert_eq!(found[0].user_data_dir, Some(PathBuf::from("/p/work")));
        assert_eq!(found[1].user_data_dir, None);
    }

    #[test]
    fn the_linux_binary_name_is_matched_by_substring() {
        let found = parse(LINUX_FIXTURE, &["claude-desktop"]);
        let pids: Vec<i32> = found.iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![1201, 1203]);
    }

    #[test]
    fn an_unrelated_binary_carrying_the_flag_is_ignored() {
        let found = parse(MAC_FIXTURE, &[MAC]);
        assert!(found.iter().all(|i| i.pid != 504));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test unix_ps`
Expected: FAIL — `cannot find function parse`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::platform::RunningInstance;
use anyhow::Result;
use std::path::PathBuf;

const FLAG: &str = "--user-data-dir=";

pub fn parse(raw: &str, main_binaries: &[&str]) -> Vec<RunningInstance> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, args) = line.split_once(' ')?;
            let pid: i32 = pid.trim().parse().ok()?;
            let args = args.trim_start();

            let command = args.split_whitespace().next().unwrap_or("");
            if !main_binaries.iter().any(|b| command.contains(b)) {
                return None;
            }
            if args.contains("--type=") {
                return None;
            }

            // Take the rest of the line, NOT the next whitespace-delimited token.
            // On macOS every profile lives under "Application Support/Claude
            // Profiles", so the path always contains spaces; splitting on
            // whitespace truncates it to "/Users/h/Library/Application" and the
            // liveness match then silently fails — which permits the double
            // launch this whole design exists to prevent. `--user-data-dir` is
            // always the last argument we pass (see `launch`), so the remainder
            // of the line is exactly the path.
            let user_data_dir = args
                .find(FLAG)
                .map(|at| args[at + FLAG.len()..].trim_end())
                .filter(|rest| !rest.is_empty())
                .map(PathBuf::from);

            Some(RunningInstance { pid, user_data_dir })
        })
        .collect()
}

pub fn scan(main_binaries: &[&str]) -> Result<Vec<RunningInstance>> {
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,args="])
        .output()?;
    Ok(parse(&String::from_utf8_lossy(&out.stdout), main_binaries))
}
```

Matching by `contains` on the first token, rather than `starts_with` on the whole line, is what makes the same function work for an absolute macOS bundle path and a bare Linux command name. Line 502 of the fixture is excluded because its first token is the Frameworks helper path, which does not contain the main binary path.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test unix_ps`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/platform/unix_ps.rs
git commit -m "feat: parse ps output into running Claude instances"
```

---

### Task 6: Windows process parsing

**Files:**
- Modify: `src-tauri/src/platform/win_proc.rs`

**Interfaces:**
- Consumes: `platform::RunningInstance`
- Produces:
  - `pub fn parse(raw: &str) -> Vec<RunningInstance>` — parses `ProcessId,CommandLine` CSV
  - `pub fn scan() -> Result<Vec<RunningInstance>>` — runs the PowerShell query

The query, run via `powershell -NoProfile -Command`:

```
Get-CimInstance Win32_Process -Filter "Name='claude.exe'" |
  Select-Object ProcessId,CommandLine | ConvertTo-Csv -NoTypeInformation
```

This task is written and tested on the development machine (a Mac) against captured output. `scan()` is compiled everywhere but only ever called by the Windows backend.

- [ ] **Step 1: Write the failing tests**

In `win_proc.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FIXTURE: &str = concat!(
        "\"ProcessId\",\"CommandLine\"\r\n",
        "\"4120\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\" --user-data-dir=C:\\Users\\h\\AppData\\Roaming\\Claude Profiles\\profiles\\a\"\r\n",
        "\"4188\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\" --type=renderer --user-data-dir=C:\\Users\\h\\AppData\\Roaming\\Claude Profiles\\profiles\\a\"\r\n",
        "\"4200\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\"\"\r\n",
    );

    #[test]
    fn helper_processes_are_excluded() {
        let pids: Vec<i32> = parse(FIXTURE).iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![4120, 4200]);
    }

    #[test]
    fn a_user_data_dir_containing_spaces_is_captured_whole() {
        let found = parse(FIXTURE);
        assert_eq!(
            found[0].user_data_dir,
            Some(PathBuf::from(
                r"C:\Users\h\AppData\Roaming\Claude Profiles\profiles\a"
            ))
        );
    }

    #[test]
    fn a_process_without_the_flag_is_the_default_profile() {
        assert_eq!(parse(FIXTURE)[1].user_data_dir, None);
    }

    #[test]
    fn a_blank_or_headers_only_output_yields_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("\"ProcessId\",\"CommandLine\"\r\n").is_empty());
    }
}
```

The path containing a space is the case that breaks a naive `split_whitespace`, which is why `--user-data-dir=` must be read to the end of the command line rather than to the next space. Claude Profiles' own data directory contains a space on Windows (`%APPDATA%\Claude Profiles`), so this is the normal case, not an edge case.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test win_proc`
Expected: FAIL — `cannot find function parse`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::platform::RunningInstance;
use anyhow::Result;
use std::path::PathBuf;

const FLAG: &str = "--user-data-dir=";

pub fn parse(raw: &str) -> Vec<RunningInstance> {
    raw.lines()
        .skip(1) // CSV header
        .filter_map(|line| {
            let (pid_field, rest) = split_csv_field(line.trim_end_matches('\r'))?;
            let pid: i32 = pid_field.parse().ok()?;
            let (command_line, _) = split_csv_field(rest)?;

            if command_line.contains("--type=") {
                return None;
            }

            let user_data_dir = command_line
                .find(FLAG)
                .map(|at| command_line[at + FLAG.len()..].trim().to_string())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);

            Some(RunningInstance { pid, user_data_dir })
        })
        .collect()
}

/// Reads one `"..."` CSV field, unescaping doubled quotes, and returns it with
/// the remainder of the line after the following comma.
fn split_csv_field(line: &str) -> Option<(String, &str)> {
    let body = line.strip_prefix('"')?;
    let mut value = String::new();
    let mut chars = body.char_indices();

    while let Some((i, c)) = chars.next() {
        if c != '"' {
            value.push(c);
            continue;
        }
        match body[i + 1..].chars().next() {
            Some('"') => {
                value.push('"');
                chars.next();
            }
            _ => {
                let rest = body[i + 1..].strip_prefix(',').unwrap_or("");
                return Some((value, rest));
            }
        }
    }
    None
}

pub fn scan() -> Result<Vec<RunningInstance>> {
    let query = "Get-CimInstance Win32_Process -Filter \"Name='claude.exe'\" | \
                 Select-Object ProcessId,CommandLine | ConvertTo-Csv -NoTypeInformation";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", query])
        .output()?;
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}
```

Because `--user-data-dir=` is read to the end of the command line, it must be the **last** argument Claude Profiles passes. It is the only one, so this holds; if another argument is ever added, put it before the flag and revisit this parser.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test win_proc`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/platform/win_proc.rs
git commit -m "feat: parse Win32_Process output into running Claude instances"
```

---

### Task 7: macOS backend

**Files:**
- Modify: `src-tauri/src/platform/macos.rs`

**Interfaces:**
- Consumes: `unix_ps`, the `Platform` trait
- Produces: `pub struct MacOs;` implementing `Platform`

Paths: data root `~/Library/Application Support/Claude Profiles`; default profile `~/Library/Application Support/Claude`; binary `/Applications/Claude.app/Contents/MacOS/Claude`.

- [ ] **Step 1: Write the failing test**

In `macos.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_hang_off_application_support() {
        let root = std::path::Path::new("/Users/h");
        assert_eq!(
            data_root_in(root),
            std::path::PathBuf::from("/Users/h/Library/Application Support/Claude Profiles")
        );
        assert_eq!(
            default_profile_in(root),
            std::path::PathBuf::from("/Users/h/Library/Application Support/Claude")
        );
    }

    #[test]
    fn a_missing_binary_is_rejected_by_name() {
        let err = check_binary(std::path::Path::new("/nope/Claude")).unwrap_err().to_string();
        assert!(err.contains("/nope/Claude"), "the error must name the path, got: {err}");
    }

    #[test]
    fn a_present_but_non_executable_binary_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("Claude");
        std::fs::write(&bin, b"not really a binary").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(check_binary(&bin).is_err());

        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_binary(&bin).is_ok());
    }
}
```

The binary path itself is a constant; asserting it equals its own literal would
test nothing. What is worth testing is the check around it, so `check_binary` is a
free function taking the path rather than logic buried inside `claude_binary`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test macos`
Expected: FAIL — `cannot find function data_root_in`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::platform::{unix_ps, FocusOutcome, Platform, RunningInstance};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub const BINARY: &str = "/Applications/Claude.app/Contents/MacOS/Claude";

pub struct MacOs;

fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?,
    ))
}

fn data_root_in(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join("Claude Profiles")
}

fn default_profile_in(home: &Path) -> PathBuf {
    home.join("Library").join("Application Support").join("Claude")
}

fn check_binary(bin: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(bin)
        .map_err(|_| anyhow!("Claude Desktop was not found at {}", bin.display()))?;
    if meta.permissions().mode() & 0o111 == 0 {
        return Err(anyhow!("{} is not executable", bin.display()));
    }
    Ok(())
}

impl Platform for MacOs {
    fn data_root(&self) -> Result<PathBuf> {
        Ok(data_root_in(&home()?))
    }

    fn default_profile_dir(&self) -> Result<PathBuf> {
        Ok(default_profile_in(&home()?))
    }

    fn claude_binary(&self) -> Result<PathBuf> {
        let bin = PathBuf::from(BINARY);
        check_binary(&bin)?;
        Ok(bin)
    }

    fn running_instances(&self) -> Result<Vec<RunningInstance>> {
        unix_ps::scan(&[BINARY])
    }

    fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()> {
        std::os::unix::fs::symlink(shared, profile_dir.join(crate::paths::CONFIG_FILENAME))?;
        Ok(())
    }

    fn focus(&self, pid: i32, _profile_id: &str) -> Result<FocusOutcome> {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        let app = unsafe { NSRunningApplication::runningApplicationWithProcessIdentifier(pid) }
            .ok_or_else(|| anyhow!("no running application with pid {pid}"))?;
        unsafe { app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows) };
        Ok(FocusOutcome::Focused)
    }

    fn quit(&self, pid: i32) -> Result<()> {
        crate::platform::unix_signal_quit(pid)
    }
}
```

Add the shared Unix quit helper to `platform/mod.rs`:

```rust
#[cfg(unix)]
pub fn unix_signal_quit(pid: i32) -> Result<()> {
    unsafe { libc::kill(pid, libc::SIGTERM) };
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if unsafe { libc::kill(pid, 0) } != 0 {
            return Ok(());
        }
    }
    unsafe { libc::kill(pid, libc::SIGKILL) };
    Ok(())
}
```

It blocks up to 10 seconds, so callers must run it off the tray thread (Task 12 handles that).

If `NSRunningApplication`'s method or enum names do not compile, open `docs.rs/objc2-app-kit` at the resolved version and use the real names — the shape (look up by PID, then activate) does not change. If the crate proves unusable, fall back to:

```rust
let script = format!(
    r#"tell application "System Events" to set frontmost of (first process whose unix id is {pid}) to true"#
);
std::process::Command::new("osascript").args(["-e", &script]).status()?;
```

and note in the README that focusing then requires Accessibility permission.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test macos`
Expected: 3 passed, and `cargo build` succeeds.

- [ ] **Step 5: Manual acceptance (macOS)**

Cannot be automated. With Claude Desktop installed:

1. `ps -axo pid=,args= | grep '[M]acOS/Claude' | grep -v -- "--type="` with the app running prints exactly one line. The bracket expression avoids counting the grep process itself.
2. Launch two instances by hand with distinct `--user-data-dir` values and confirm both stay alive — this is the parallel-instance guarantee.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/platform
git commit -m "feat: macOS platform backend"
```

---

### Task 8: Windows backend

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`

**Interfaces:**
- Consumes: `win_proc`, the `Platform` trait
- Produces:
  - `pub struct Windows;` implementing `Platform`
  - `pub fn pick_default_profile(candidates: &[PathBuf]) -> Option<PathBuf>`
  - `pub fn pick_binary(candidates: &[PathBuf]) -> Option<PathBuf>`

The MSIX installer virtualizes `%APPDATA%`, so the stock user-data directory is one of two places. Both pickers take their candidate list as a parameter so the choice is testable from any machine.

Candidate order for the default profile, first existing wins:
1. `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude` (MSIX)
2. `%APPDATA%\Claude` (direct installer)

Candidate order for the binary:
1. `%LOCALAPPDATA%\AnthropicClaude\claude.exe` (direct installer)
2. `%LOCALAPPDATA%\Microsoft\WindowsApps\claude.exe` (MSIX execution alias)

- [ ] **Step 1: Write the failing tests**

In `windows.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_first_existing_candidate_wins() {
        let d = tempfile::tempdir().unwrap();
        let msix = d.path().join("msix");
        let classic = d.path().join("classic");
        std::fs::create_dir_all(&classic).unwrap();

        assert_eq!(
            pick_default_profile(&[msix.clone(), classic.clone()]),
            Some(classic)
        );

        std::fs::create_dir_all(&msix).unwrap();
        assert_eq!(
            pick_default_profile(&[msix.clone(), d.path().join("classic")]),
            Some(msix)
        );
    }

    #[test]
    fn no_existing_candidate_yields_none() {
        assert_eq!(pick_default_profile(&[PathBuf::from("/nope")]), None);
        assert_eq!(pick_binary(&[PathBuf::from("/nope/claude.exe")]), None);
    }

    #[test]
    fn a_binary_candidate_must_be_a_file_not_a_directory() {
        let d = tempfile::tempdir().unwrap();
        let dir_named_like_exe = d.path().join("claude.exe");
        std::fs::create_dir_all(&dir_named_like_exe).unwrap();
        assert_eq!(pick_binary(&[dir_named_like_exe]), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test windows`
Expected: FAIL — `cannot find function pick_default_profile`.

Note these tests run on the development Mac even though the backend targets Windows, because the pickers are pure filesystem probing. Guard the module with `#[cfg(any(target_os = "windows", test))]` in `platform/mod.rs` so it compiles for tests everywhere; the `Platform` impl itself stays `#[cfg(target_os = "windows")]`.

- [ ] **Step 3: Write the implementation**

```rust
use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub fn pick_default_profile(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|c| c.is_dir()).cloned()
}

pub fn pick_binary(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|c| c.is_file()).cloned()
}

fn env_path(var: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var(var).map_err(|_| anyhow!("{var} is not set"))?,
    ))
}

pub fn default_profile_candidates() -> Result<Vec<PathBuf>> {
    let local = env_path("LOCALAPPDATA")?;
    let roaming = env_path("APPDATA")?;
    Ok(vec![
        local
            .join("Packages")
            .join("Claude_pzs8sxrjxfjjc")
            .join("LocalCache")
            .join("Roaming")
            .join("Claude"),
        roaming.join("Claude"),
    ])
}

pub fn binary_candidates() -> Result<Vec<PathBuf>> {
    let local = env_path("LOCALAPPDATA")?;
    Ok(vec![
        local.join("AnthropicClaude").join("claude.exe"),
        local.join("Microsoft").join("WindowsApps").join("claude.exe"),
    ])
}
```

Then the `Platform` impl, gated to Windows:

```rust
#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use crate::platform::{win_proc, FocusOutcome, Platform, RunningInstance};
    use std::path::Path;

    pub struct Windows;

    impl Platform for Windows {
        fn data_root(&self) -> Result<PathBuf> {
            Ok(env_path("APPDATA")?.join("Claude Profiles"))
        }

        fn default_profile_dir(&self) -> Result<PathBuf> {
            let candidates = default_profile_candidates()?;
            pick_default_profile(&candidates).ok_or_else(|| {
                anyhow!(
                    "Claude Desktop's data directory was not found. Looked in: {}",
                    candidates
                        .iter()
                        .map(|c| c.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        }

        fn claude_binary(&self) -> Result<PathBuf> {
            let candidates = binary_candidates()?;
            pick_binary(&candidates).ok_or_else(|| {
                anyhow!(
                    "Claude Desktop was not found. Looked in: {}",
                    candidates
                        .iter()
                        .map(|c| c.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        }

        fn running_instances(&self) -> Result<Vec<RunningInstance>> {
            win_proc::scan()
        }

        fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()> {
            std::fs::hard_link(shared, profile_dir.join(crate::paths::CONFIG_FILENAME))
                .map_err(|e| {
                    anyhow!(
                        "could not link the shared config into {}: {e}. \
                         Both paths must be on the same drive.",
                        profile_dir.display()
                    )
                })?;
            Ok(())
        }

        fn focus(&self, pid: i32, _profile_id: &str) -> Result<FocusOutcome> {
            use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
            };

            struct Search {
                pid: u32,
                found: Option<HWND>,
            }

            unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
                let search = &mut *(lparam.0 as *mut Search);
                let mut owner = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut owner));
                if owner == search.pid && IsWindowVisible(hwnd).as_bool() {
                    search.found = Some(hwnd);
                    return BOOL(0); // stop enumerating
                }
                BOOL(1)
            }

            let mut search = Search { pid: pid as u32, found: None };
            unsafe {
                let _ = EnumWindows(Some(cb), LPARAM(&mut search as *mut _ as isize));
            }

            match search.found {
                Some(hwnd) => {
                    // Windows refuses this when the caller is not already the
                    // foreground process. Report the refusal instead of claiming
                    // success — the tray click usually satisfies it, but a
                    // keyboard-driven or scripted invocation may not.
                    let raised = unsafe { SetForegroundWindow(hwnd) }.as_bool();
                    if raised {
                        Ok(FocusOutcome::Focused)
                    } else {
                        Ok(FocusOutcome::Unsupported(
                            "Windows refused to bring the window forward; \
                             click its taskbar entry instead".into(),
                        ))
                    }
                }
                None => Ok(FocusOutcome::Unsupported(
                    "no visible window was found for this instance".into(),
                )),
            }
        }

        fn quit(&self, pid: i32) -> Result<()> {
            std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string()])
                .status()?;
            std::thread::sleep(std::time::Duration::from_secs(10));
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::Windows;
```

The `windows` crate's exact type names (`BOOL` vs `windows::core::BOOL`) shift between releases. If it does not compile, check `docs.rs/windows` at the resolved version.

`quit` sleeps a flat 10 seconds before the forced kill rather than polling, because checking liveness on Windows needs another API round trip; the tray runs it on a worker thread so the delay is invisible.

Residual risk to confirm on real hardware: because the sleep is unconditional, the forced `taskkill /F` fires even when the process already exited cleanly. Windows recycles process ids aggressively, so in principle that second call could land on an unrelated process that inherited the id within those ten seconds. The acceptance run should check that quitting an instance leaves every other running instance alive. If it turns out to be a real hazard, replace the flat sleep with a poll on `OpenProcess` + `GetExitCodeProcess` and skip the force kill once the process is gone.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test windows`
Expected: 3 passed.

- [ ] **Step 5: Manual acceptance (Windows) — REQUIRED before marking this task done**

On real Windows hardware with Claude Desktop installed:

1. `powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='claude.exe'\" | Select-Object ProcessId,CommandLine | ConvertTo-Csv -NoTypeInformation"` with the app running produces output whose shape matches the fixture in Task 6. **If it does not, fix the fixture and the parser before continuing.**
2. Launch a second instance by hand:
   `& "$env:LOCALAPPDATA\AnthropicClaude\claude.exe" --user-data-dir="$env:TEMP\cp-test"`
   Confirm both instances stay alive. **If the second instance exits immediately or focuses the first, parallel instances are not possible on Windows — record the finding and degrade this backend to single-instance rather than proceeding.**
3. Confirm `std::fs::hard_link` succeeds without elevation from `%APPDATA%\Claude Profiles\shared\` into **whichever default-profile directory step 4 finds**. Test the MSIX path specifically if that is the one in use: writing into a package's `LocalCache` from outside the package is the case most likely to be refused, and it is the case most users will hit. If it fails, switch this backend to copy-on-launch and document that Windows config sharing is one-way.
4. Confirm which default-profile candidate exists on this machine, and that the app picked it.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/platform/windows.rs
git commit -m "feat: Windows platform backend with MSIX-aware path probing"
```

---

### Task 9: Linux backend

**Files:**
- Modify: `src-tauri/src/platform/linux.rs`

**Interfaces:**
- Consumes: `unix_ps`, the `Platform` trait
- Produces:
  - `pub struct Linux;` implementing `Platform`
  - `pub fn wm_class(profile_id: &str) -> String` — `claude-profiles-<id>`
  - `pub fn desktop_entry(profile: &Profile, exec: &str, icon: &str) -> String`
  - `pub fn desktop_file_path(applications_dir: &Path, profile_id: &str) -> PathBuf`
  - `pub fn is_wayland(session_type: Option<&str>) -> bool`

Paths: data root `$XDG_CONFIG_HOME/claude-profiles` (falling back to `~/.config/claude-profiles`); default profile `~/.config/Claude`; binary resolved from `PATH` as `claude-desktop`.

**Linux does not chase windows.** Under native Wayland no application may raise
another's window, so instead of fighting that, each instance is given its own
desktop identity and the desktop environment does the focusing:

- Launch with `--class=claude-profiles-<profile id>`, which Chromium turns into
  `WM_CLASS` on X11 and `app_id` on Wayland.
- Write `~/.local/share/applications/claude-profiles-<profile id>.desktop` with
  `Name=Claude — <label>` and a matching `StartupWMClass`.

The key is the profile's **id**, not a slug of its label. Slugging labels would
let "Work A" and "work a" collide on one desktop entry, and a rename would orphan
the old file. An id is unique and immutable: a rename rewrites the same entry with
a new `Name=`, and only deletion needs cleanup.

Each profile then gets its own taskbar entry, name, and alt-tab slot. Tray focus
becomes a bonus: `xdotool search --class` on X11, an honest `Unsupported` message
pointing at the taskbar entry on Wayland.

`--class` therefore joins `--user-data-dir` in the launch arguments **on Linux
only**, which means `launch_args` in Task 11 can no longer be platform-blind. Add
`fn extra_launch_args(&self, profile: &Profile) -> Vec<String>` to the `Platform`
trait, defaulting to an empty vec, implemented only here; `instance_manager::launch`
appends it. Put `--user-data-dir` last, because the Windows parser in Task 6 reads
it to the end of the command line.

- [ ] **Step 1: Write the failing tests**

In `linux.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, label: &str) -> crate::profile_store::Profile {
        crate::profile_store::Profile {
            id: id.into(),
            label: label.into(),
            path: std::path::PathBuf::from("/p").join(id),
            is_default: false,
            last_known_account_uuid: None,
        }
    }

    #[test]
    fn the_class_is_keyed_on_the_profile_id() {
        assert_eq!(wm_class("a1b2"), "claude-profiles-a1b2");
    }

    #[test]
    fn labels_that_would_slug_identically_still_get_distinct_identities() {
        let a = profile("id-one", "Work A");
        let b = profile("id-two", "work  a");
        assert_ne!(wm_class(&a.id), wm_class(&b.id));

        let dir = std::path::Path::new("/apps");
        assert_ne!(
            desktop_file_path(dir, &a.id),
            desktop_file_path(dir, &b.id)
        );
    }

    #[test]
    fn the_desktop_entry_declares_a_matching_startup_wm_class() {
        let p = profile("a1b2", "Kerja");
        let entry = desktop_entry(&p, "/usr/bin/claude-desktop --class=x", "/i/icon.png");
        assert!(entry.contains("Name=Claude — Kerja"));
        assert!(entry.contains("StartupWMClass=claude-profiles-a1b2"));
        // Metadata only: a launchable entry would start Claude against the
        // Default profile without passing the liveness guard.
        assert!(entry.contains("NoDisplay=true"));
        assert!(entry.contains("Icon=/i/icon.png"));
        assert!(entry.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn the_desktop_file_lands_in_the_applications_directory() {
        assert_eq!(
            desktop_file_path(std::path::Path::new("/home/h/.local/share/applications"), "a1b2"),
            std::path::PathBuf::from(
                "/home/h/.local/share/applications/claude-profiles-a1b2.desktop"
            )
        );
    }

    #[test]
    fn wayland_is_detected_from_the_session_type() {
        assert!(is_wayland(Some("wayland")));
        assert!(!is_wayland(Some("x11")));
        assert!(!is_wayland(None));
    }

    #[test]
    fn the_config_root_honours_xdg_config_home() {
        assert_eq!(
            data_root_from(Some("/xdg"), "/home/h"),
            std::path::PathBuf::from("/xdg/claude-profiles")
        );
        assert_eq!(
            data_root_from(None, "/home/h"),
            std::path::PathBuf::from("/home/h/.config/claude-profiles")
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test linux`
Expected: FAIL — `cannot find function wm_class`.

Guard this module the same way as Task 8 (`#[cfg(any(target_os = "linux", test))]` for the pure helpers) so it is testable from the Mac.

- [ ] **Step 3: Write the implementation**

```rust
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub const BINARY_NAME: &str = "claude-desktop";

pub fn wm_class(profile_id: &str) -> String {
    format!("claude-profiles-{profile_id}")
}

pub fn desktop_file_path(applications_dir: &Path, profile_id: &str) -> PathBuf {
    applications_dir.join(format!("{}.desktop", wm_class(profile_id)))
}

pub fn desktop_entry(profile: &Profile, exec: &str, icon: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Claude — {label}\n\
         Comment=Claude Desktop, {label} profile\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         NoDisplay=true\n\
         StartupWMClass={class}\n",
        label = profile.label,
        class = wm_class(&profile.id)
    )
}

pub fn is_wayland(session_type: Option<&str>) -> bool {
    matches!(session_type, Some(s) if s.eq_ignore_ascii_case("wayland"))
}
```

`NoDisplay=true` is load-bearing, not cosmetic. These entries exist so the desktop
can match a window's `app_id`/`WM_CLASS` to a name and icon — they are metadata,
not launchers. Their `Exec` carries `--class` but deliberately no
`--user-data-dir`, so anything that actually ran them would start Claude against
the **Default** profile's directory while wearing another profile's identity — and
it would do so behind the tray's back, skipping the liveness guard entirely. Since
Claude Desktop takes no single-instance lock, that is a direct route to the
database corruption this design exists to prevent. Hiding the entries from menus
and launchers keeps every real launch funnelled through `launch`, which checks
first.

```rust

pub fn data_root_from(xdg_config_home: Option<&str>, home: &str) -> PathBuf {
    match xdg_config_home {
        Some(x) if !x.is_empty() => PathBuf::from(x).join("claude-profiles"),
        _ => PathBuf::from(home).join(".config").join("claude-profiles"),
    }
}

fn which(tool: &str) -> bool {
    std::process::Command::new("which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

Then the `Platform` impl, gated to Linux. `claude_binary` resolves via `which claude-desktop` and errors with the apt install hint when absent:

```rust
fn claude_binary(&self) -> Result<PathBuf> {
    let out = std::process::Command::new("which").arg(BINARY_NAME).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "Claude Desktop was not found on PATH as `{BINARY_NAME}`. \
             Install it with: sudo apt install claude-desktop"
        ));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}
```

`running_instances` calls `unix_ps::scan(&[BINARY_NAME])`. `link_shared_config` uses `std::os::unix::fs::symlink`, identical to macOS. `quit` calls `crate::platform::unix_signal_quit`. `default_profile_dir` returns `$HOME/.config/Claude`. `data_root` calls `data_root_from` with the real environment.

`extra_launch_args` supplies the class, and is the only backend that overrides it:

```rust
fn extra_launch_args(&self, profile: &Profile) -> Vec<String> {
    vec![format!("--class={}", wm_class(&profile.id))]
}
```

`focus` tries X11 and is honest on Wayland:

```rust
fn focus(&self, _pid: i32, profile_id: &str) -> Result<FocusOutcome> {
    if is_wayland(std::env::var("XDG_SESSION_TYPE").ok().as_deref()) {
        return Ok(FocusOutcome::Unsupported(
            "Wayland does not let one app raise another's window — \
             use this profile's own entry in your taskbar or alt-tab".into(),
        ));
    }
    if !which("xdotool") {
        return Ok(FocusOutcome::Unsupported(
            "install xdotool to focus from here, or use this profile's taskbar entry".into(),
        ));
    }
    let class = wm_class(profile_id);
    let status = std::process::Command::new("xdotool")
        .args(["search", "--class", &class, "windowactivate"])
        .status()?;
    if status.success() {
        Ok(FocusOutcome::Focused)
    } else {
        Ok(FocusOutcome::Unsupported(format!(
            "xdotool found no window with class {class}"
        )))
    }
}
```

Two deliberate choices here. `wmctrl` is absent because its `-ia` flag takes a
window id, not a pid — it would have been a call that never worked. And the search
is by class, not by `--pid`: `_NET_WM_PID` is not reliably set, whereas the class
is one we passed at launch ourselves. That is also why `focus` takes the profile id
alongside the pid; macOS and Windows ignore the id and use the pid.

The desktop entries are written and removed alongside profiles. Add to this module:

```rust
pub fn applications_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".local").join("share").join("applications"))
}

pub fn write_desktop_entry(profile: &Profile, binary: &Path, icon: &Path) -> Result<()> {
    let dir = applications_dir()?;
    std::fs::create_dir_all(&dir)?;
    let exec = format!("{} --class={}", binary.display(), wm_class(&profile.id));
    std::fs::write(
        desktop_file_path(&dir, &profile.id),
        desktop_entry(profile, &exec, &icon.display().to_string()),
    )?;
    Ok(())
}

pub fn remove_desktop_entry(profile_id: &str) -> Result<()> {
    let path = desktop_file_path(&applications_dir()?, profile_id);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
```

The `Exec` line intentionally omits `--user-data-dir`: launching from the taskbar
entry should start the *Default* profile rather than silently spawning a second
copy of a profile that may already be running, which Task 11's guard exists to
prevent and which the desktop entry cannot check. The entry's job is identity and
focus, not launching.

Two trait methods carry this: add `fn register_identity(&self, profile: &Profile) -> Result<()>`
and `fn unregister_identity(&self, profile: &Profile) -> Result<()>` to `Platform`,
both defaulting to `Ok(())`, implemented only here. `commands::add_profile` and
`rename_profile` call register; `delete_profile` calls unregister before removing
the directory. Renaming must unregister the old label first, or a stale entry is
left behind pointing at a class nothing uses.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test linux`
Expected: 6 passed.

- [ ] **Step 5: Manual acceptance (Linux) — REQUIRED before marking this task done**

On real Linux hardware with `claude-desktop` installed from Anthropic's apt repository:

1. `ps -axo pid=,args= | grep claude-desktop | grep -v -- "--type="` with the app running prints exactly one line. **If the process name or path differs from the Task 5 fixture, fix the fixture and the parser.**
2. Confirm `~/.config/Claude` is where the app actually stores its data. If it differs, correct `default_profile_dir`.
3. `claude-desktop --user-data-dir=/tmp/cp-test` while another instance runs: confirm both stay alive. **If not, record the finding and degrade this backend to single-instance.**
4. Note whether the session is X11 or Wayland (`echo $XDG_SESSION_TYPE`) and confirm the focus behaviour matches — focused on X11 with xdotool present, a clear "unsupported" message otherwise.
5. **Confirm `--class` actually takes effect.** Launch a profile, then read the window's class:
   - X11: `xprop WM_CLASS` and click the window — expect `claude-profiles-<profile id>`, the id being the uuid shown for that profile in the management window.
   - Wayland (GNOME): `gdbus call --session -d org.gnome.Shell -o /org/gnome/Shell -m org.gnome.Shell.Eval 'global.get_window_actors().map(w => w.meta_window.get_wm_class())'`, or simply check whether the taskbar shows a separate, correctly-named entry.

   **If `--class` is ignored, this whole approach collapses** — the instances stay indistinguishable and there is no focus story on Wayland at all. Record that finding plainly, keep the xdotool path for X11, and state the Wayland limitation in the README rather than papering over it.
6. Confirm the generated `.desktop` file makes the taskbar show "Claude — <label>" with the Claude Profiles icon, and that deleting the profile removes the entry.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/platform/linux.rs
git commit -m "feat: Linux platform backend with honest focus degradation"
```

---

### Task 10: Account UUID

**Files:**
- Create: `src-tauri/src/account.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `profile_store::Profile`
- Produces:
  - `pub fn read_account_uuid(profile_dir: &Path) -> Option<String>`
  - `pub fn duplicate_uuids(profiles: &[Profile]) -> HashSet<String>`

Every failure is silent and yields `None`. This is a cosmetic hint, never a blocker.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_store::Profile;
    use std::path::PathBuf;

    fn profile(id: &str, uuid: Option<&str>) -> Profile {
        Profile {
            id: id.into(),
            label: id.into(),
            path: PathBuf::from("/tmp").join(id),
            is_default: false,
            last_known_account_uuid: uuid.map(str::to_string),
        }
    }

    #[test]
    fn reads_the_account_uuid_from_config_json() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("config.json"),
            r#"{"darkMode":"true","lastKnownAccountUuid":"abc-123"}"#,
        )
        .unwrap();
        assert_eq!(read_account_uuid(d.path()), Some("abc-123".to_string()));
    }

    #[test]
    fn a_missing_or_unreadable_config_yields_none() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_account_uuid(d.path()), None);
        std::fs::write(d.path().join("config.json"), b"not json").unwrap();
        assert_eq!(read_account_uuid(d.path()), None);
    }

    #[test]
    fn duplicates_are_the_uuids_claimed_more_than_once() {
        let profiles = vec![
            profile("a", Some("same")),
            profile("b", Some("same")),
            profile("c", Some("other")),
            profile("d", None),
        ];
        let dupes = duplicate_uuids(&profiles);
        assert_eq!(dupes.len(), 1);
        assert!(dupes.contains("same"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test account`
Expected: FAIL — `cannot find function read_account_uuid`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::profile_store::Profile;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn read_account_uuid(profile_dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(profile_dir.join("config.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value.get("lastKnownAccountUuid")?.as_str().map(str::to_string)
}

pub fn duplicate_uuids(profiles: &[Profile]) -> HashSet<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for uuid in profiles.iter().filter_map(|p| p.last_known_account_uuid.as_deref()) {
        *counts.entry(uuid).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(uuid, _)| uuid.to_string())
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test account`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/account.rs src-tauri/src/lib.rs
git commit -m "feat: flag profiles signed in to the same account"
```

---

### Task 11: Instance manager

**Files:**
- Create: `src-tauri/src/instance_manager.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `platform::Platform`, `profile_store::Profile`, `paths::Paths`, `shared_config`
- Produces:
  - `pub fn launch_args(profile: &Profile) -> Vec<String>`
  - `pub fn prepare(platform: &dyn Platform, profile: &Profile, paths: &Paths) -> Result<()>`
  - `pub fn launch(platform: &dyn Platform, profile: &Profile, paths: &Paths) -> Result<i32>`

Focus and quit are called on the platform directly; only launch needs composing.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_store::Profile;
    use crate::shared_config::tests_support::FakePlatform;
    use std::path::PathBuf;

    fn profile(is_default: bool) -> Profile {
        Profile {
            id: "x".into(),
            label: "X".into(),
            path: PathBuf::from("/p/x"),
            is_default,
            last_known_account_uuid: None,
        }
    }

    #[test]
    fn a_normal_profile_launches_with_its_user_data_dir() {
        assert_eq!(launch_args(&profile(false)), vec!["--user-data-dir=/p/x"]);
    }

    #[test]
    fn the_default_profile_launches_with_no_arguments() {
        assert!(launch_args(&profile(true)).is_empty());
    }

    #[test]
    fn preparing_a_missing_profile_directory_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(d.path());
        let p = Profile { path: d.path().join("gone"), ..profile(false) };
        assert!(prepare(&FakePlatform::default(), &p, &paths).is_err());
    }

    #[test]
    fn launching_a_profile_that_is_already_running_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(d.path());
        let p = Profile { path: d.path().join("live"), ..profile(false) };
        std::fs::create_dir_all(&p.path).unwrap();

        let platform = FakePlatform::with_running(vec![crate::platform::RunningInstance {
            pid: 4242,
            user_data_dir: Some(p.path.clone()),
        }]);

        let err = launch(&platform, &p, &paths).unwrap_err().to_string();
        assert!(err.contains("already running"), "got: {err}");
        assert!(err.contains("4242"), "the error must name the pid, got: {err}");
    }

    #[test]
    fn launching_is_refused_when_liveness_cannot_be_determined() {
        let d = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(d.path());
        let p = Profile { path: d.path().join("live"), ..profile(false) };
        std::fs::create_dir_all(&p.path).unwrap();

        let err = launch(&FakePlatform::failing_scan(), &p, &paths).unwrap_err().to_string();
        assert!(err.contains("could not check"), "got: {err}");
    }
}
```

`FakePlatform` comes from Task 4's `tests_support` module and needs one addition: a
`failing_scan()` constructor plus a `scan_fails: bool` field, so `running_instances`
can return `Err` on demand. Everything else about the fixture stays as it is.

The last test only passes if `launch` checks liveness **before** anything that can
fail for an unrelated reason. Order matters and is asserted by the test: the guard
runs first, so a live profile reports "already running" rather than whatever the
binary probe happens to say. That is also the better message for a user.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test instance_manager`
Expected: FAIL — `cannot find function launch_args`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::paths::Paths;
use crate::platform::{find_for, Platform};
use crate::profile_store::Profile;
use anyhow::{anyhow, Result};

pub fn launch_args(profile: &Profile) -> Vec<String> {
    if profile.is_default {
        Vec::new()
    } else {
        vec![format!("--user-data-dir={}", profile.path.display())]
    }
}

pub fn prepare(platform: &dyn Platform, profile: &Profile, paths: &Paths) -> Result<()> {
    if !profile.path.is_dir() {
        return Err(anyhow!(
            "profile directory {} is missing",
            profile.path.display()
        ));
    }
    crate::shared_config::ensure_shared(platform, &profile.path, &paths.shared_config())
}

/// Refuses to spawn a second process against a user-data directory that already
/// has one. Claude Desktop has NO single-instance lock (verified 2026-08-12): two
/// processes on one profile both stay alive and corrupt its databases. The tray
/// already offers Focus instead of Launch for a live profile, but a menu built
/// seconds ago can be stale, so the check is repeated here, closest to the spawn.
pub fn launch(platform: &dyn Platform, profile: &Profile, paths: &Paths) -> Result<i32> {
    // Fail CLOSED. If the process scan itself fails we do not know whether this
    // profile is live, and `unwrap_or_default()` would turn "I cannot tell" into
    // "nothing is running" — launching straight into the corruption this guard
    // exists to prevent. Refusing costs the user one retry; guessing costs them
    // their profile.
    let running = platform.running_instances().map_err(|error| {
        anyhow!(
            "could not check whether {} is already running ({error}); \
             refusing to launch, because a second copy would corrupt the profile",
            profile.label
        )
    })?;
    if let Some(pid) = find_for(&running, &profile.path, profile.is_default) {
        return Err(anyhow!(
            "{} is already running as pid {pid}; focus it instead of launching a second copy",
            profile.label
        ));
    }

    let binary = platform.claude_binary()?;
    prepare(platform, profile, paths)?;
    let mut args = platform.extra_launch_args(profile);
    args.extend(launch_args(profile)); // --user-data-dir stays last

    let child = std::process::Command::new(binary)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(child.id() as i32)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test instance_manager`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/instance_manager.rs src-tauri/src/shared_config.rs src-tauri/src/lib.rs
git commit -m "feat: compose launch from platform backend and shared config"
```

---

### Task 12: Tray menu

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: everything above
- Produces:
  - `pub struct AppState { pub platform: Box<dyn Platform>, pub paths: Paths, pub store: Mutex<ProfileStore> }`
  - `pub struct MenuRow { pub id: String, pub text: String, pub enabled: bool, pub pid: Option<i32> }`
  - `pub fn menu_rows(store: &ProfileStore, instances: &[RunningInstance], binary_error: Option<&str>) -> Vec<MenuRow>`
  - `pub fn rebuild(app: &tauri::AppHandle) -> Result<()>`

Menu item ids encode the action: `launch:<id>`, `focus:<id>`, `quit:<id>`, plus fixed `manage` and `quit_app`.

A running profile produces **two** rows: the focus row, and an indented quit row
directly beneath it. Without the second row there is no way to stop an instance
from the tray — and since Task 13 refuses to delete a profile while it runs, a user
would have no in-app path to deleting one at all. A stopped profile produces only
its launch row.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use crate::platform::RunningInstance;
    use std::path::PathBuf;

    fn store_with_one_extra() -> (tempfile::TempDir, ProfileStore) {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        store.add("Kerja", &paths).unwrap();
        (d, store)
    }

    #[test]
    fn a_running_profile_gets_a_marker_and_a_pid() {
        let (_d, store) = store_with_one_extra();
        let kerja = store.list()[1].clone();
        let instances = vec![RunningInstance {
            pid: 777,
            user_data_dir: Some(kerja.path.clone()),
        }];
        let rows = menu_rows(&store, &instances, None);
        let row = rows.iter().find(|r| r.id == format!("focus:{}", kerja.id)).unwrap();
        assert_eq!(row.pid, Some(777));
        assert!(row.text.starts_with("● "));
        assert!(row.enabled);
    }

    #[test]
    fn a_running_profile_also_offers_a_quit_row_right_after_it() {
        let (_d, store) = store_with_one_extra();
        let kerja = store.list()[1].clone();
        let instances = vec![RunningInstance {
            pid: 777,
            user_data_dir: Some(kerja.path.clone()),
        }];
        let rows = menu_rows(&store, &instances, None);

        let focus_at = rows.iter().position(|r| r.id == format!("focus:{}", kerja.id)).unwrap();
        let quit = &rows[focus_at + 1];
        assert_eq!(quit.id, format!("quit:{}", kerja.id));
        assert_eq!(quit.pid, Some(777));
        assert!(quit.text.contains("Quit"));
    }

    #[test]
    fn a_stopped_profile_offers_launch_and_no_quit_row() {
        let (_d, store) = store_with_one_extra();
        let kerja = store.list()[1].clone();
        let rows = menu_rows(&store, &[], None);
        let row = rows.iter().find(|r| r.id == format!("launch:{}", kerja.id)).unwrap();
        assert_eq!(row.pid, None);
        assert!(row.text.starts_with("○ "));
        assert!(!rows.iter().any(|r| r.id.starts_with("quit:")));
    }

    #[test]
    fn a_missing_binary_disables_every_row_and_adds_an_explanation() {
        let (_d, store) = store_with_one_extra();
        let rows = menu_rows(&store, &[], Some("Claude Desktop was not found at /x"));
        assert!(rows.iter().filter(|r| r.id != "error").all(|r| !r.enabled));
        assert!(rows.iter().any(|r| r.text.contains("not found at /x")));
    }

    #[test]
    fn profiles_sharing_an_account_are_marked() {
        let (d, mut store) = store_with_one_extra();
        let _ = d;
        let a = store.list()[0].id.clone();
        let b = store.list()[1].id.clone();
        store.set_account_uuid(&a, Some("same".into()));
        store.set_account_uuid(&b, Some("same".into()));
        let rows = menu_rows(&store, &[], None);
        assert_eq!(rows.iter().filter(|r| r.text.contains("same account")).count(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test tray`
Expected: FAIL — `cannot find function menu_rows`.

- [ ] **Step 3: Write the pure part**

```rust
use crate::paths::Paths;
use crate::platform::{find_for, Platform, RunningInstance};
use crate::profile_store::ProfileStore;
use std::sync::Mutex;

pub struct AppState {
    pub platform: Box<dyn Platform>,
    pub paths: Paths,
    pub store: Mutex<ProfileStore>,
}

pub struct MenuRow {
    pub id: String,
    pub text: String,
    pub enabled: bool,
    pub pid: Option<i32>,
}

pub fn menu_rows(
    store: &ProfileStore,
    instances: &[RunningInstance],
    binary_error: Option<&str>,
) -> Vec<MenuRow> {
    let dupes = crate::account::duplicate_uuids(store.list());
    let mut rows: Vec<MenuRow> = store
        .list()
        .iter()
        .flat_map(|p| {
            let pid = find_for(instances, &p.path, p.is_default);
            let marker = if pid.is_some() { "●" } else { "○" };
            let shared_account = p
                .last_known_account_uuid
                .as_deref()
                .map(|u| dupes.contains(u))
                .unwrap_or(false);
            let suffix = if shared_account { "  (same account)" } else { "" };
            let action = if pid.is_some() { "focus" } else { "launch" };

            let mut out = vec![MenuRow {
                id: format!("{action}:{}", p.id),
                text: format!("{marker} {}{suffix}", p.label),
                enabled: binary_error.is_none(),
                pid,
            }];

            if pid.is_some() {
                out.push(MenuRow {
                    id: format!("quit:{}", p.id),
                    text: format!("      Quit {}", p.label),
                    enabled: binary_error.is_none(),
                    pid,
                });
            }
            out
        })
        .collect();

    if let Some(message) = binary_error {
        rows.push(MenuRow {
            id: "error".into(),
            text: message.to_string(),
            enabled: false,
            pid: None,
        });
    }
    rows
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test tray`
Expected: 5 passed.

- [ ] **Step 5: Wire the rows into a real tray**

Append `rebuild(app: &tauri::AppHandle) -> Result<()>`. It must:

1. Read `AppState` from `app.state()`.
2. Refresh account uuids: for each profile, `account::read_account_uuid(&p.path)` → `store.set_account_uuid(...)`, then `store.save(&paths)`, ignoring save errors.
3. `platform.running_instances()` — on error, use an empty list so rebuild can continue, and add a disabled visible reason row explaining the scan failure.
4. `platform.claude_binary()` — capture `Err(e)` as `Some(e.to_string())` for `binary_error`; all runtime errors are likewise surfaced as disabled visible reason rows during the rebuild.
5. Build a `tauri::menu::Menu` from `menu_rows` with `MenuItem::with_id(app, &row.id, &row.text, row.enabled, None::<&str>)`; append a separator, then `manage` / "Manage Profiles…" and `quit_app` / "Quit Claude Profiles".
6. Attach with `TrayIconBuilder::with_id("main").menu(&menu).build(app)` on first construction, or `tray.set_menu(Some(menu))` on later rebuilds.

In `lib.rs`, register `on_menu_event`. Split the id on `:` and dispatch:

- `launch:<id>` → `instance_manager::launch`, then `rebuild`
- `focus:<id>` → re-scan, `find_for`, `platform.focus(pid, &profile.id)`; on `FocusOutcome::Unsupported(msg)` do not fail — log it and rebuild with a disabled visible reason row carrying the message
- `quit:<id>` → re-scan, then use fallible `std::thread::Builder::spawn` for a worker running `platform.quit(pid)` followed by `rebuild` (it blocks up to 10s and must not stall the tray); log thread-creation failures and best-effort rebuild with a disabled visible reason row
- `manage` → show the management window (Task 13)
- `quit_app` → `app.exit(0)`

Every handler wraps its work in a closure returning `Result<()>`; on `Err`, log and rebuild with a disabled visible reason row rather than panicking. The pure `menu_rows` contract remains unchanged; rebuild-only runtime and scan errors are supplied as an additional reason to the rebuild layer.

Also call `rebuild` from a `TrayIconEvent` handler so the menu refreshes each time it opens, and once during `setup`. **Match on the event and rebuild only for `TrayIconEvent::Click`** — the handler also receives `Enter`, `Move` and `Leave`, and `Move` fires continuously while the pointer crosses the icon. Rebuilding on every one of those would run a full process scan and rewrite `profiles.json` dozens of times per second, for a menu nobody opened.

For the same reason `rebuild` must not write the registry unconditionally. It refreshes each profile's account uuid before drawing, but should call `store.save` only when a uuid actually changed; otherwise every glance at the tray becomes a disk write.

Add one more function beside it, and call it from `setup` **before** the first
rebuild:

```rust
/// Re-asserts every profile's desktop identity. A no-op everywhere but Linux.
pub fn sync_identities(state: &AppState) {
    let Ok(store) = state.store.lock() else { return };
    for p in store.list() {
        let _ = state.platform.register_identity(p);
    }
}
```

Without this, the only profiles that would ever get a `.desktop` entry are ones
added through the management window during this run: the Default profile — which
is seeded, never added — would never get one, and neither would profiles carried
over from a previous run or whose entry was deleted by hand. Running it at startup
makes the entries self-healing. Errors are swallowed on purpose; a missing desktop
entry must never stop the tray from coming up.

- [ ] **Step 6: Verify end to end (development platform)**

```bash
pnpm tauri dev
```

Checks, in order:
1. Menu shows "○ Default".
2. Click it → Claude Desktop starts; reopen the menu → "● Default".
3. Click it again → the existing window comes to the front, no second copy.
4. Quit Claude Desktop by hand; reopen the menu → back to "○ Default".
5. Kill Claude Profiles while Claude Desktop runs, restart it → still shows "●".

Check 5 proves process scanning beats stored PIDs. Do not skip it.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src
git commit -m "feat: tray menu with live instance state"
```

---

### Task 13: Management window

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`, `src/index.html`, `src/main.ts`, `src/styles.css`, `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: `AppState`, `profile_store`, `account`, `platform`
- Produces these `#[tauri::command]` functions, all returning `Result<T, String>`:
  - `list_profiles(state) -> Vec<ProfileView>` where `ProfileView { id, label, path, is_default, shares_account }`
  - `add_profile(app, state, label: String) -> ProfileView`
  - `rename_profile(app, state, id, label) -> ()`
  - `delete_profile(app, state, id) -> ()`
  - `profile_size_bytes(state, id) -> u64`
  - `pub fn to_views(store: &ProfileStore) -> Vec<ProfileView>`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use crate::profile_store::ProfileStore;

    #[test]
    fn profiles_sharing_an_account_are_flagged_in_the_view() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let a = store.add("A", &paths).unwrap();
        let b = store.add("B", &paths).unwrap();
        store.set_account_uuid(&a.id, Some("same".into()));
        store.set_account_uuid(&b.id, Some("same".into()));

        let views = to_views(&store);

        assert!(views.iter().find(|v| v.id == a.id).unwrap().shares_account);
        assert!(views.iter().find(|v| v.id == b.id).unwrap().shares_account);
        assert!(!views[0].shares_account); // Default has no uuid
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test commands`
Expected: FAIL — `cannot find function to_views`.

- [ ] **Step 3: Write the implementation**

```rust
use crate::profile_store::ProfileStore;
use crate::tray::AppState;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct ProfileView {
    pub id: String,
    pub label: String,
    pub path: String,
    pub is_default: bool,
    pub shares_account: bool,
}

pub fn to_views(store: &ProfileStore) -> Vec<ProfileView> {
    let dupes = crate::account::duplicate_uuids(store.list());
    store
        .list()
        .iter()
        .map(|p| ProfileView {
            id: p.id.clone(),
            label: p.label.clone(),
            path: p.path.display().to_string(),
            is_default: p.is_default,
            shares_account: p
                .last_known_account_uuid
                .as_deref()
                .map(|u| dupes.contains(u))
                .unwrap_or(false),
        })
        .collect()
}

#[tauri::command]
pub fn list_profiles(state: tauri::State<AppState>) -> Result<Vec<ProfileView>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(to_views(&store))
}

#[tauri::command]
pub fn add_profile(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    label: String,
) -> Result<ProfileView, String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    let created = store.add(&label, &state.paths).map_err(|e| e.to_string())?;
    store.save(&state.paths).map_err(|e| e.to_string())?;
    let view = to_views(&store)
        .into_iter()
        .find(|v| v.id == created.id)
        .ok_or("profile vanished after creation")?;
    let _ = state.platform.register_identity(&created);
    drop(store);
    let _ = crate::tray::rebuild(&app);
    Ok(view)
}
```

Write `rename_profile`, `delete_profile`, and `profile_size_bytes` in the same shape: lock, mutate, `save`, drop the lock, `rebuild`, return. `profile_size_bytes` walks the directory recursively summing `metadata.len()`.

Each mutation also maintains the profile's desktop identity, which is what makes
the Linux taskbar entry follow the profile:

- `add_profile` → `register_identity` on the new profile, as above.
- `rename_profile` → `register_identity` again after the label changes, so the
  entry's `Name=` follows. No cleanup is needed: the entry is keyed on the profile
  id, so the rewrite lands on the same file.
- `delete_profile` → `unregister_identity` **before** removing the directory, so a
  failure to delete the data does not leave a dangling launcher entry behind.

All three ignore the result. A desktop entry that could not be written is a
cosmetic loss, never a reason to fail the operation the user asked for.

`delete_profile` must refuse while that profile's instance is running: call `state.platform.running_instances()` and `find_for` first, and if a pid comes back, return `Err("quit this profile's Claude Desktop before deleting it".into())`. The tray's quit row is the user's way to satisfy that.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test commands`
Expected: 1 passed.

- [ ] **Step 5: Build the window UI**

`src/index.html`: a heading "Claude Profiles", a list container `<ul id="profiles">`, an `<input id="new-label">` with an "Add profile" button, and an empty `<div id="error">`.

`src/main.ts`: on load call `invoke("list_profiles")` and render each row with its label, its path in small grey text, a "same account" badge when `shares_account`, and Rename / Delete buttons (both hidden when `is_default`). Rename uses `prompt()`. Delete calls `profile_size_bytes` first and confirms with the human-readable size in the message. Re-render after every mutation. Show any rejected promise's message in `#error` rather than swallowing it.

`src-tauri/tauri.conf.json`: set the main window `"visible": false` so the app starts with no window. The `manage` tray item calls `window.show()` then `window.set_focus()`.

To keep the app off the macOS Dock, add to `setup`:

```rust
#[cfg(target_os = "macos")]
app.set_activation_policy(tauri::ActivationPolicy::Accessory);
```

- [ ] **Step 6: Verify**

```bash
pnpm tauri dev
```

Checks: no Dock icon on macOS; "Manage Profiles…" opens the window; adding "Kerja" makes a new tray row immediately; launching Kerja starts a **second** Claude Desktop alongside Default; both stay usable at once; deleting Kerja while it runs is refused with the message; quitting it then deleting works and asks for confirmation with a size.

A fresh profile shows a signed-out Claude Desktop — that is correct. Sign in with the second account to complete the check.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: profile management window"
```

---

### Task 14: Cross-platform verification and README

**Files:**
- Create: `README.md`
- Modify: whatever the acceptance runs in Tasks 8 and 9 turn up

**Interfaces:**
- Consumes: everything
- Produces: no new API

- [ ] **Step 1: Confirm the whole suite passes on the development machine**

```bash
cd src-tauri && cargo test
```

Expected: every test from Tasks 2–13 passes. Report the actual count.

- [ ] **Step 2: Cross-compile check**

On a development machine where the targets are already installed, run:

```bash
export PATH="$HOME/.local/share/mise/shims:$PATH"
cd src-tauri
cargo check --target x86_64-pc-windows-msvc
cargo check --target aarch64-unknown-linux-gnu
```

Do not install rustup, Rust targets, or platform toolchains as part of this task. On a Mac without those targets, `cargo check` is still attempted and its missing-target error is recorded in the README. Linking will not succeed without each platform's toolchain, but an available target check catches the type errors that matter — wrong `windows` crate names, missing `cfg` guards, and a backend that does not satisfy the trait. Do not silently skip an attempted check.

- [ ] **Step 3: Run the Windows acceptance checklist**

Execute every step of Task 8 Step 5 on real Windows hardware when it is available, and record the outcomes in the README's "Platform status" table. If no Windows machine is available for this run, record the backend as unverified and identify the human-run checklist that remains. If parallel instances turn out to be impossible, say so plainly there.

- [ ] **Step 4: Run the Linux acceptance checklist**

Execute every step of Task 9 Step 5 on real Linux hardware when it is available, and record the outcomes, including whether the session was X11 or Wayland and whether focusing worked. If no Linux machine is available for this run, record the backend as unverified and identify the human-run checklist that remains.

- [ ] **Step 5: Verify the missing-binary path**

Do not temporarily move the installed Claude Desktop application from `/Applications` in an automated run: that modifies the user's installation and requires privileged access. The existing macOS unit test covers rejection of a missing or non-executable binary. A human may run the original move-and-restore check on a disposable installation if they want the tray presentation verified; record it separately from the unit-test result.

- [ ] **Step 6: Verify the duplicate-account warning**

This check requires two real signed-in profiles and interactive account access. Do not launch the real Default profile or sign in on behalf of the user in an automated run. A human may add a second profile, sign in to the same account, confirm both rows carry "(same account)", and sign the second one out afterwards; until then, record the end-to-end warning as unverified.

- [ ] **Step 7: Write the README**

Cover: what it does (parallel Claude Desktop instances, one per account); that the Default profile is the existing installation used in place; that MCP config is shared across every profile, by symlink on macOS/Linux and hardlink on Windows; a **Platform status** table stating what has actually been verified on each OS and what has not; the Windows MSIX caveat; the Linux/Wayland focus limitation; that instances share one icon in the OS task switcher by design; and how to build (`pnpm install`, `pnpm tauri dev`, `pnpm tauri build`).

Be accurate about verification status. Do not describe an untested platform as working.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "docs: README with honest per-platform verification status"
```

---

## Self-Review Notes

Spec coverage against `2026-08-12-claude-profiles-design.md`:

| Spec requirement | Task |
|---|---|
| Platform trait seam, no OS branching elsewhere | 2 |
| On-disk layout | 2, 3 |
| Default profile used in place, no flag | 3, 11 |
| Shared config, all four cases | 4 |
| Windows hardlink instead of symlink | 4, 8 |
| macOS paths, focus, quit | 7 |
| Windows MSIX path probing, focus, quit | 8 |
| Linux paths, focus degradation, quit | 9 |
| Linux per-profile `--class` + `.desktop` identity | 9, 11, 12, 13 |
| Desktop entries cover Default and heal themselves | 12 (`sync_identities`) |
| Launch refused when the profile is already running | 11 |
| Quitting an instance from the tray | 12 |
| Unix `ps` parsing incl. helper exclusion | 5 |
| Windows CSV parsing incl. spaces in paths | 6 |
| Liveness survives an app restart | 5, 6, 12 (check 5) |
| Manual labels only, no email | 3, 13 |
| `lastKnownAccountUuid` duplicate warning | 10, 12 |
| Errors degrade to disabled rows | 8, 9, 12 |
| Focus unsupported reported honestly | 2, 9, 12 |
| Delete confirms, states size, refuses while running | 13 |
| Manual acceptance per platform | 7, 8, 9, 14 |

Known risk carried deliberately: parallel instances are proven only on macOS. Tasks 8 and 9 each contain an explicit branch for the outcome where they are impossible, so discovering that during acceptance does not invalidate the plan.
