use std::path::PathBuf;

/// The container every profile directory hangs off, kept to one character on
/// purpose. See [`SOCKET_PATH_LIMIT`] — every byte spent here is a byte an
/// application cannot spend on the socket it puts inside a profile.
const PROFILES_DIR: &str = "p";

/// macOS caps a Unix domain socket path at 104 bytes (`sun_path`), and Linux at
/// 108. Several of the applications we hold profiles for put a socket inside the
/// profile directory — VS Code writes `<version>-main.sock`, the ChatGPT desktop
/// app writes `ipc/ipc.sock` — so the length of a profile path is not cosmetic.
///
/// Measured, not assumed: at a 94-byte socket path VS Code came up with nine
/// processes and created its socket; at 109 bytes exactly one process survived
/// and no socket appeared. ChatGPT is less brittle and merely loses its socket
/// silently, which is worse to diagnose.
///
/// This is why a profile directory is `<root>/p/<short id>` rather than
/// `<root>/profiles/<uuid>`: the latter left a real installation 17 bytes over
/// the limit before the application had written a single byte.
pub const SOCKET_PATH_LIMIT: usize = 104;

/// The longest socket name seen inside a profile, plus its separator: VS Code's
/// `1.13-main.sock`. ChatGPT's `ipc/ipc.sock` is shorter, so budgeting for the
/// longer one covers both.
const SOCKET_NAME_BUDGET: usize = "/1.13-main.sock".len();

/// Whether an application could still create its socket inside this profile.
///
/// The layout is short enough that this holds comfortably for any ordinary home
/// directory, but the home directory is not ours to choose. Refusing to create a
/// profile that cannot work is the same fail-closed choice made when a process
/// scan fails: a profile that half-works is far harder to diagnose than one that
/// was never created.
pub fn leaves_room_for_socket(profile_dir: &std::path::Path) -> bool {
    profile_dir.display().to_string().len() + SOCKET_NAME_BUDGET <= SOCKET_PATH_LIMIT
}

/// Every path belonging to one app's profiles. Rooted at `<data root>/<app id>`,
/// so two apps never share a registry, a profile directory, or a shared config.
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
        self.root.join(PROFILES_DIR)
    }

    pub fn profile_dir(&self, id: &str) -> PathBuf {
        self.profiles_dir().join(id)
    }

    /// The one copy of `filename` that every profile of this app links to.
    pub fn shared_config(&self, filename: &str) -> PathBuf {
        self.root.join("shared").join(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn paths_are_rooted_at_the_given_directory() {
        let p = Paths::new("/root/claude");
        assert_eq!(
            p.profiles_json(),
            PathBuf::from("/root/claude/profiles.json")
        );
        assert_eq!(p.profiles_dir(), PathBuf::from("/root/claude/p"));
        assert_eq!(p.profile_dir("abc"), PathBuf::from("/root/claude/p/abc"));
        assert_eq!(
            p.shared_config("claude_desktop_config.json"),
            PathBuf::from("/root/claude/shared/claude_desktop_config.json")
        );
    }

    #[test]
    fn two_apps_never_collide() {
        // The registries must not overlap: a shared `profiles.json` would make a
        // profile id ambiguous, and the stock-profile entry would be written twice.
        let claude = Paths::new("/root/claude");
        let codex = Paths::new("/root/codex");
        assert_ne!(claude.profiles_json(), codex.profiles_json());
        assert_ne!(claude.profile_dir("abc"), codex.profile_dir("abc"));
        assert_ne!(
            claude.shared_config("config.toml"),
            codex.shared_config("config.toml")
        );
    }

    #[test]
    fn a_profile_directory_can_never_be_mistaken_for_our_own_files() {
        // Profiles keep their own container rather than sitting directly under
        // the root, so no profile id can ever collide with `profiles.json` or
        // `shared` and quietly shadow one of them.
        let p = Paths::new("/root/claude");
        assert_ne!(
            p.profile_dir("shared"),
            p.shared_config("x").parent().unwrap()
        );
        assert_ne!(p.profile_dir("profiles.json"), p.profiles_json());
    }

    /// The real layout on a real machine, against the real limit.
    fn socket_path_len(home: &str, app: &str, id: &str, socket: &str) -> usize {
        let root = Path::new(home)
            .join("Library/Application Support")
            .join("Agent Profiles")
            .join(app);
        Paths::new(root)
            .profile_dir(id)
            .join(socket)
            .display()
            .to_string()
            .len()
    }

    #[test]
    fn a_path_with_no_room_left_is_rejected() {
        let roomy = PathBuf::from(
            "/Users/husni/Library/Application Support/Agent Profiles/code/p/9f3c1a7e",
        );
        assert!(leaves_room_for_socket(&roomy));

        // A home directory long enough to eat the budget: the profile is still a
        // legal path, it simply cannot host the socket an app will want.
        let cramped = PathBuf::from(format!("/Users/{}/x/p/9f3c1a7e", "n".repeat(90)));
        assert!(!leaves_room_for_socket(&cramped));
    }

    #[test]
    fn a_profile_leaves_room_for_the_socket_an_app_puts_inside_it() {
        // VS Code's is the longest of the ones measured.
        let len = socket_path_len("/Users/husni", "code", "9f3c1a7e", "1.13-main.sock");
        assert!(
            len <= SOCKET_PATH_LIMIT,
            "a profile path must leave room for a socket, got {len}"
        );
    }

    #[test]
    fn there_is_headroom_for_a_long_user_name() {
        // The home directory is not ours to choose, so the layout has to survive
        // a name considerably longer than the author's.
        let len = socket_path_len(
            "/Users/christopher.anderson",
            "code",
            "9f3c1a7e",
            "1.13-main.sock",
        );
        assert!(len <= SOCKET_PATH_LIMIT, "no headroom left, got {len}");
    }

    #[test]
    fn the_layout_this_replaced_would_not_have_fitted() {
        // Guards the reason for the short names: a uuid under `profiles/` put a
        // real installation over the limit before the app wrote anything.
        let old = Path::new("/Users/husni")
            .join("Library/Application Support/Agent Profiles/code/profiles")
            .join("9f3c1a7e-4b2d-4c8e-9a1f-2e5d7b6c0a83")
            .join("1.13-main.sock")
            .display()
            .to_string()
            .len();
        assert!(
            old > SOCKET_PATH_LIMIT,
            "the old layout fitted after all: {old}"
        );
    }
}
