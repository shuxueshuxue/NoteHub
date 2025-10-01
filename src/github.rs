use anyhow::{Context, Result};
use octocrab::Octocrab;

#[derive(Clone, Debug)]
pub struct RepoSpec {
    pub owner: String,
    pub name: String,
}

impl RepoSpec {
    pub fn parse(repo: &str) -> Result<Self> {
        let mut parts = repo.splitn(2, '/');
        let owner = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .context("repository must be in the form owner/name")?;
        let name = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .context("repository must be in the form owner/name")?;
        Ok(Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }
}

pub struct GithubClient {
    inner: Octocrab,
    repo: RepoSpec,
}

impl GithubClient {
    pub async fn new(token: &str, repo: RepoSpec) -> Result<Self> {
        let inner = Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .context("failed to build GitHub client")?;
        Ok(Self { inner, repo })
    }

    pub async fn list_issues(&self) -> Result<Vec<octocrab::models::issues::Issue>> {
        let page = self
            .inner
            .issues(&self.repo.owner, &self.repo.name)
            .list()
            .per_page(50)
            .send()
            .await
            .context("failed to fetch issues")?;
        Ok(page.items)
    }

    pub async fn get_issue(&self, number: u64) -> Result<octocrab::models::issues::Issue> {
        self.inner
            .issues(&self.repo.owner, &self.repo.name)
            .get(number)
            .await
            .with_context(|| format!("failed to fetch issue #{number}"))
    }
}
