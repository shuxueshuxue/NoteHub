mod config;
mod github;
mod storage;

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
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
        self.config
            .save(&self.config_path)
            .context("failed to write config")
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
    Sync,
    /// Configure the GitHub token and default repo
    Init(InitArgs),
    /// Inspect GitHub issues
    Issue {
        #[command(subcommand)]
        action: IssueAction,
    },
    /// Manage local-only notes tied to issues
    Note {
        #[command(subcommand)]
        action: NoteAction,
    },
}

#[derive(Subcommand)]
enum IssueAction {
    /// List issues currently in the cache
    List,
    /// View a single issue by number
    View {
        /// Issue number to display
        number: u64,
    },
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

#[derive(clap::Args)]
struct InitArgs {
    /// GitHub personal access token used for API calls
    #[arg(long)]
    token: Option<String>,
    /// Default repository to work with (owner/name)
    #[arg(long)]
    repo: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut ctx = AppContext::load().context("failed to initialize application state")?;

    match cli.command {
        Command::Sync => run_sync(&mut ctx).await?,
        Command::Init(args) => handle_init(&mut ctx, args)?,
        Command::Issue { action } => run_issue(&mut ctx, action).await?,
        Command::Note { action } => match action {
            NoteAction::Add { number, text } => {
                println!("[todo] add note to issue #{number}: {text}")
            }
            NoteAction::List { number } => println!("[todo] list notes for issue #{number}"),
        },
    }

    Ok(())
}

fn handle_init(ctx: &mut AppContext, args: InitArgs) -> Result<()> {
    if let Some(token) = args.token {
        ctx.config.github_token = Some(token);
    }
    if let Some(repo) = args.repo {
        ctx.config.repo = Some(repo);
    }

    match (&ctx.config.github_token, &ctx.config.repo) {
        (Some(_), Some(_)) => {
            ctx.save()?;
            println!("Configuration saved to {}", ctx.config_path.display());
        }
        _ => {
            anyhow::bail!("init requires both --token and --repo");
        }
    }

    Ok(())
}

async fn run_sync(ctx: &mut AppContext) -> Result<()> {
    let (token, repo_raw, repo_spec) = resolve_config(&ctx.config)?;
    let client = GithubClient::new(token, repo_spec.clone()).await?;
    let issues = client.list_issues_all().await?;
    for issue in &issues {
        ctx.storage.upsert_issue(&repo_raw, issue)?;
    }
    println!("Fetched {} issue(s)", issues.len());
    Ok(())
}

async fn run_issue(ctx: &mut AppContext, action: IssueAction) -> Result<()> {
    let (token, repo_raw, repo_spec) = resolve_config(&ctx.config)?;

    match action {
        IssueAction::List => {
            let issues = ctx.storage.list_issues(&repo_raw)?;
            if issues.is_empty() {
                println!("No cached issues found. Run `notehub sync` to fetch the latest data.");
            } else {
                for issue in issues {
                    println!("#{:<6} {}", issue.number, issue.title);
                }
            }
        }
        IssueAction::View { number } => {
            if let Some(issue) = ctx.storage.get_issue(&repo_raw, number)? {
                print_issue_detail(issue);
            } else {
                println!("Issue not cached locally. Fetching from GitHub...");
                let client = GithubClient::new(token, repo_spec.clone()).await?;
                let issue = client.get_issue(number).await?;
                ctx.storage.upsert_issue(&repo_raw, &issue)?;
                if let Some(detail) = ctx.storage.get_issue(&repo_raw, number)? {
                    print_issue_detail(detail);
                }
            }
        }
    }

    Ok(())
}

fn print_issue_detail(issue: StoredIssueDetail) {
    println!("#{} - {}", issue.number, issue.title);
    if let Some(body) = issue.body {
        if !body.trim().is_empty() {
            println!("\n{}", body);
        }
    }
    println!("\n(updated {})", issue.updated_at.to_rfc3339());
}

fn resolve_config(config: &Config) -> Result<(&str, String, RepoSpec)> {
    let token = config
        .github_token
        .as_deref()
        .context("GitHub token not configured. Run `notehub init --token ...`")?;
    let repo = config
        .repo
        .as_deref()
        .context("Repository not configured. Run `notehub init --repo ...`")?;
    let spec = RepoSpec::parse(repo)?;
    Ok((token, repo.to_string(), spec))
}
