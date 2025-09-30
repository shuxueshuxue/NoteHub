use std::{fs, io, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Personal access token for GitHub API requests
    pub github_token: Option<String>,
    /// Single repo identifier in the form owner/name for MVP
    pub repo: Option<String>,
}

impl Config {
    pub fn load() -> io::Result<(Self, PathBuf)> {
        let (path, exists) = config_path()?;
        if !exists {
            return Ok((Self::default(), path));
        }
        let raw = fs::read_to_string(&path)?;
        let cfg = toml::from_str(&raw).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok((cfg, path))
    }

    pub fn save(&self, path: &PathBuf) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let raw = toml::to_string_pretty(self).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(path, raw)
    }
}

fn config_path() -> io::Result<(PathBuf, bool)> {
    let dirs = ProjectDirs::from("com", "LexicalMathical", "NoteHub")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unable to determine config directory"))?;
    let path = dirs.config_dir().join(CONFIG_FILE_NAME);
    Ok((path.clone(), path.exists()))
}
