use std::path::PathBuf;

pub const CONFIG_FILENAME: &str = "claude_desktop_config.json";

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
