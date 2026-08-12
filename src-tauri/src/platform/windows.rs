use super::{FocusOutcome, Platform, RunningInstance};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct Windows;

impl Platform for Windows {
    fn data_root(&self) -> Result<PathBuf> {
        todo!()
    }

    fn default_profile_dir(&self) -> Result<PathBuf> {
        todo!()
    }

    fn claude_binary(&self) -> Result<PathBuf> {
        todo!()
    }

    fn running_instances(&self) -> Result<Vec<RunningInstance>> {
        todo!()
    }

    fn link_shared_config(&self, _profile_dir: &Path, _shared: &Path) -> Result<()> {
        todo!()
    }

    fn focus(&self, _pid: i32, _profile_id: &str) -> Result<FocusOutcome> {
        todo!()
    }

    fn quit(&self, _pid: i32) -> Result<()> {
        todo!()
    }
}
