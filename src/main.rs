mod config;
mod github;
mod storage;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail, ensure};
use clap::{Args, Parser, Subcommand};
use config::Config;
use github::{GithubClient, RepoSpec};
use storage::{Storage, StoredIssueDetail};

struct AppContext {
    config: Config,
    config_path: PathBuf,
    storage: Storage,
}

impl AppContext {
    fn load() -> Result<Self> {
        let (config, path) = Config::load()?;
        let storage = Storage::open()?;
        Ok(Self {
            config,
            config_path: path,
            storage,
        })
    }

    fn save(&self) -> Result<()> {
        self.config.save(&self.config_path)
    }
}

#[derive(Parser)]
#[command(
    name = "notehub",
    version,
    about = "Interact with GitHub issues as local notes",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Synchronize GitHub issues into the local cache
    Sync(SyncArgs),
    /// Configure GitHub token or repositories
    Init(InitArgs),
    /// Inspect GitHub issues
    Issue {
        #[command(subcommand)]
        action: IssueAction,
    },
    /// Manage configured repositories
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },
    /// Manage local-only notes tied to issues
    Note {
        #[command(subcommand)]
        action: NoteAction,
    },
}

#[derive(Args)]
struct SyncArgs {
    /// Sync only the specified repository (owner/name). May be supplied multiple times.
    #[arg(long, value_name = "owner/name")]
    repo: Vec<String>,
}

#[derive(Args)]
struct InitArgs {
    /// GitHub personal access token used for API calls
    #[arg(long)]
    token: Option<String>,
    /// One or more repositories to add (owner/name). May be repeated.
    #[arg(long, value_name = "owner/name")]
    repo: Vec<String>,
}

#[derive(Subcommand)]
enum IssueAction {
    /// List cached issues, optionally filtering by repository
    List {
        /// Repository to list (owner/name). May be repeated.
        #[arg(long, value_name = "owner/name")]
        repo: Vec<String>,
        /// List cached issues for all configured repositories
        #[arg(long, default_value_t = false)]
        all: bool,
    },
    /// View a single issue by number
    View {
        /// Issue number to display
        number: u64,
        /// Repository to read from (defaults to the active repo)
        #[arg(long, value_name = "owner/name")]
        repo: Option<String>,
    },
}

#[derive(Subcommand)]
enum RepoAction {
    /// Add a repository to the configuration
    Add {
        repo: String,
        /// Also make the added repository the active one
        #[arg(long)]
        set_active: bool,
    },
    /// Add all accessible repositories, optionally excluding some
    AddAll {
        /// Repositories to skip while importing (owner/name).
        #[arg(long, value_name = "owner/name")]
        exclude: Vec<String>,
    },
    /// Remove a repository from the configuration
    Remove { repo: String },
    /// Set the active repository
    Use { repo: String },
    /// Show configured repositories
    List,
}

#[derive(Subcommand)]
enum NoteAction {
    /// Attach a note to an issue
    Add {
        /// Target issue number
        number: u64,
        /// Text for the note
        text: String,
    },
    /// List notes for an issue
    List {
        /// Target issue number
        number: u64,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut ctx = AppContext::load().context("failed to initialize application state")?;

    match cli.command {
        Command::Sync(args) => run_sync(&mut ctx, args).await?,
        Command::Init(args) => handle_init(&mut ctx, args)?,
        Command::Issue { action } => run_issue(&mut ctx, action).await?,
        Command::Repo { action } => run_repo(&mut ctx, action).await?,
        Command::Note { action } => match action {
            NoteAction::Add { number, text } => {
                println!("[todo] add note to issue #{number}: {text}");
            }
            NoteAction::List { number } => println!("[todo] list notes for issue #{number}"),
        },
    }

    Ok(())
}

fn handle_init(ctx: &mut AppContext, args: InitArgs) -> Result<()> {
    let mut changed = false;

    if let Some(token) = args.token {
        ctx.config.github_token = Some(token);
        changed = true;
    }

    for repo in args.repo {
        let (normalized, added) = ctx.config.add_repo(&repo)?;
        if added {
            println!("Configured repository {normalized}");
        } else {
            println!("Repository {normalized} already configured");
        }
        changed = changed || added;
    }

    ctx.config.ensure_active_repo();

    if changed {
        ctx.save()?;
        if let Some(active) = ctx.config.active_repo() {
            println!(
                "Configuration saved to {}. Active repository: {}",
                ctx.config_path.display(),
                active
            );
        } else {
            println!("Configuration saved to {}", ctx.config_path.display());
        }
    } else {
        println!("No changes applied. Use --token or --repo to update configuration.");
    }

    if ctx.config.github_token.is_none() {
        println!("Warning: GitHub token not configured");
    }
    if ctx.config.repos().is_empty() {
        println!("Warning: no repositories configured");
    }

    Ok(())
}

async fn run_sync(ctx: &mut AppContext, args: SyncArgs) -> Result<()> {
    let token = get_token(&ctx.config)?;
    let repos = resolve_repos(&ctx.config, &args.repo, false, args.repo.is_empty())?;

    for repo in repos {
        println!("Syncing {repo}...");
        let spec = RepoSpec::parse(&repo)?;
        let client = GithubClient::new(token, spec).await?;
        let issues = client.list_issues_all().await?;
        for issue in &issues {
            ctx.storage.upsert_issue(&repo, issue)?;
        }
        println!("  cached {} issues", issues.len());
    }

    Ok(())
}

async fn run_issue(ctx: &mut AppContext, action: IssueAction) -> Result<()> {
    let token = get_token(&ctx.config)?;

    match action {
        IssueAction::List { repo, all } => {
            let repos = resolve_repos(&ctx.config, &repo, repo.is_empty() && !all, all)?;
            for (idx, repo_name) in repos.iter().enumerate() {
                let issues = ctx.storage.list_issues(repo_name)?;
                if repos.len() > 1 {
                    if idx > 0 {
                        println!();
                    }
                    println!("Repository: {repo_name}");
                }
                if issues.is_empty() {
                    println!("  (no cached issues)");
                } else {
                    for issue in issues {
                        println!("#{:<6} {}", issue.number, issue.title);
                    }
                }
            }
        }
        IssueAction::View { number, repo } => {
            let repo_name = resolve_single_repo(&ctx.config, repo.as_deref())?;
            if let Some(issue) = ctx.storage.get_issue(&repo_name, number)? {
                print_issue_detail(issue);
            } else {
                println!("Issue not cached locally. Fetching from GitHub...");
                let spec = RepoSpec::parse(&repo_name)?;
                let client = GithubClient::new(token, spec).await?;
                let issue = client.get_issue(number).await?;
                ctx.storage.upsert_issue(&repo_name, &issue)?;
                if let Some(detail) = ctx.storage.get_issue(&repo_name, number)? {
                    print_issue_detail(detail);
                }
            }
        }
    }

    Ok(())
}

async fn run_repo(ctx: &mut AppContext, action: RepoAction) -> Result<()> {
    match action {
        RepoAction::List => {
            if ctx.config.repos().is_empty() {
                println!(
                    "No repositories configured. Use `notehub repo add owner/name` to add one."
                );
            } else {
                let active = ctx.config.active_repo();
                for repo in ctx.config.repos() {
                    if Some(repo) == active {
                        println!("* {repo} (active)");
                    } else {
                        println!("  {repo}");
                    }
                }
            }
        }
        RepoAction::Add { repo, set_active } => {
            let (normalized, added) = ctx.config.add_repo(&repo)?;
            if added {
                println!("Added {normalized}");
            } else {
                println!("Repository {normalized} already exists");
            }
            if set_active || ctx.config.active_repo().is_none() {
                let active = ctx.config.set_active_repo(&normalized)?;
                println!("Active repository: {active}");
            }
            ctx.save()?;
        }
        RepoAction::AddAll { exclude } => {
            let token = get_token(&ctx.config)?;
            let mut exclude_set = HashSet::new();
            for repo in exclude {
                let normalized = Config::normalize_repo(&repo)?;
                exclude_set.insert(normalized);
            }

            let repos = github::list_authenticated_repos(token).await?;
            let mut added = 0usize;
            let mut skipped_existing = 0usize;
            let mut skipped_excluded = 0usize;

            for repo in repos {
                let normalized = Config::normalize_repo(&repo)?;
                if exclude_set.contains(&normalized) {
                    skipped_excluded += 1;
                    continue;
                }
                let (_, was_added) = ctx.config.add_repo(&normalized)?;
                if was_added {
                    added += 1;
                } else {
                    skipped_existing += 1;
                }
            }

            ctx.config.ensure_active_repo();
            ctx.save()?;

            println!("Imported {added} repository(ies)");
            if skipped_existing > 0 {
                println!("Skipped {skipped_existing} already configured repositories");
            }
            if skipped_excluded > 0 {
                println!("Skipped {skipped_excluded} excluded repositories");
            }
            if let Some(active) = ctx.config.active_repo() {
                println!("Active repository: {active}");
            }
        }
        RepoAction::Remove { repo } => {
            let (normalized, removed) = ctx.config.remove_repo(&repo)?;
            if removed {
                println!("Removed {normalized}");
                ctx.save()?;
            } else {
                println!("Repository {normalized} not configured");
            }
        }
        RepoAction::Use { repo } => {
            let active = ctx.config.set_active_repo(&repo)?;
            ctx.save()?;
            println!("Active repository: {active}");
        }
    }
    Ok(())
}

fn print_issue_detail(issue: StoredIssueDetail) {
    println!("#{} - {}", issue.number, issue.title);
    if let Some(body) = issue.body {
        if !body.trim().is_empty() {
            println!(
                "
{}",
                body
            );
        }
    }
    println!(
        "
(updated {})",
        issue.updated_at.to_rfc3339()
    );
}

fn get_token<'a>(config: &'a Config) -> Result<&'a str> {
    config
        .github_token
        .as_deref()
        .context("GitHub token not configured. Run `notehub init --token ...`.")
}

fn resolve_single_repo(config: &Config, requested: Option<&str>) -> Result<String> {
    let repos = resolve_repos(
        config,
        &requested.map(|s| vec![s.to_string()]).unwrap_or_default(),
        requested.is_none(),
        false,
    )?;

    ensure!(repos.len() == 1, "expected exactly one repository");
    Ok(repos.into_iter().next().unwrap())
}

fn resolve_repos(
    config: &Config,
    requested: &[String],
    use_active: bool,
    all: bool,
) -> Result<Vec<String>> {
    if all {
        let repos = config.repos().to_vec();
        if repos.is_empty() {
            bail!("No repositories configured. Add one with `notehub repo add owner/name`.");
        }
        return Ok(repos);
    }

    if !requested.is_empty() {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for repo in requested {
            let normalized = Config::normalize_repo(repo)?;
            ensure!(
                config.repos().contains(&normalized),
                "repository {normalized} is not configured"
            );
            if seen.insert(normalized.clone()) {
                result.push(normalized);
            }
        }
        return Ok(result);
    }

    if use_active {
        if let Some(active) = config.active_repo() {
            return Ok(vec![active.clone()]);
        }
        bail!("No active repository configured. Use `notehub repo use owner/name` to set one.");
    }

    bail!("No repositories specified");
}
