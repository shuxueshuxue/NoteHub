use std::{fs, path::PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub github_token: Option<String>,
    #[serde(default)]
    pub repos: Vec<String>,
    pub active_repo: Option<String>,
}

impl Config {
    pub fn load() -> Result<(Self, PathBuf)> {
        let (path, exists) = config_path()?;
        if !exists {
            return Ok((Self::default(), path));
        }

        let raw_text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let mut cfg: Self = toml::from_str(&raw_text)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;
        cfg.deduplicate_repos();
        Ok((cfg, path))
    }

    pub fn save(&self, path: &PathBuf) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
        }

        let raw = toml::to_string_pretty(self).context("failed to encode configuration")?;
        fs::write(path, raw)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }

    pub fn add_repo(&mut self, repo: &str) -> Result<(String, bool)> {
        let normalized = Self::normalize_repo(repo)?;
        if !self.repos.contains(&normalized) {
            self.repos.push(normalized.clone());
            if self.active_repo.is_none() {
                self.active_repo = Some(normalized.clone());
            }
            Ok((normalized, true))
        } else {
            Ok((normalized, false))
        }
    }

    pub fn remove_repo(&mut self, repo: &str) -> Result<(String, bool)> {
        let normalized = Self::normalize_repo(repo)?;
        if let Some(pos) = self.repos.iter().position(|r| r == &normalized) {
            self.repos.remove(pos);
            if self.active_repo.as_ref() == Some(&normalized) {
                self.active_repo = self.repos.first().cloned();
            }
            Ok((normalized, true))
        } else {
            Ok((normalized, false))
        }
    }

    pub fn set_active_repo(&mut self, repo: &str) -> Result<String> {
        let normalized = Self::normalize_repo(repo)?;
        ensure!(
            self.repos.contains(&normalized),
            "repository {normalized} is not configured"
        );
        self.active_repo = Some(normalized.clone());
        Ok(normalized)
    }

    pub fn ensure_active_repo(&mut self) {
        if self
            .active_repo
            .as_ref()
            .map(|r| self.repos.contains(r))
            .unwrap_or(false)
        {
            return;
        }
        self.active_repo = self.repos.first().cloned();
    }

    pub fn normalize_repo(repo: &str) -> Result<String> {
        let trimmed = repo.trim().trim_matches('/');
        ensure!(
            !trimmed.is_empty(),
            "repository must be in the form owner/name"
        );
        let mut parts = trimmed.split('/');
        let owner = parts
            .next()
            .ok_or_else(|| anyhow!("repository must include an owner"))?;
        let name = parts
            .next()
            .ok_or_else(|| anyhow!("repository must include a name"))?;
        ensure!(
            parts.next().is_none(),
            "repository must be in the form owner/name"
        );
        Ok(format!("{owner}/{name}"))
    }

    pub fn repos(&self) -> &[String] {
        &self.repos
    }

    pub fn active_repo(&self) -> Option<&String> {
        self.active_repo.as_ref()
    }

    fn deduplicate_repos(&mut self) {
        let mut unique = Vec::new();
        for repo in &self.repos {
            if !unique.contains(repo) {
                unique.push(repo.clone());
            }
        }
        self.repos = unique;
        self.ensure_active_repo();
    }
}

fn config_path() -> Result<(PathBuf, bool)> {
    let dirs = ProjectDirs::from("com", "LexicalMathical", "NoteHub")
        .ok_or_else(|| anyhow!("unable to determine config directory"))?;
    let path = dirs.config_dir().join(CONFIG_FILE_NAME);
    Ok((path.clone(), path.exists()))
}
