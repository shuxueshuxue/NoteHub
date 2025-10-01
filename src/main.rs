mod config;
mod github;

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use config::Config;
use github::{GithubClient, RepoSpec};

struct AppContext {
    config: Config,
    config_path: std::path::PathBuf,
}

impl AppContext {
    fn load() -> std::io::Result<Self> {
        let (config, path) = Config::load()?;
        Ok(Self {
            config,
            config_path: path,
        })
    }

    fn save(&self) -> std::io::Result<()> {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut ctx = AppContext::load().context("failed to load config")?;

    match cli.command {
        Command::Sync => run_sync(&mut ctx).await?,
        Command::Init(args) => handle_init(&mut ctx, args)?,
        Command::Issue { action } => run_issue(&ctx, action).await?,
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
            ctx.save().context("failed to write config")?;
            println!("Configuration saved to {}", ctx.config_path.display());
        }
        _ => {
            anyhow::bail!("init requires both --token and --repo");
        }
    }

    Ok(())
}

async fn run_sync(ctx: &mut AppContext) -> Result<()> {
    let client = build_client(&ctx.config).await?;
    let issues = client.list_issues().await?;
    println!("Fetched {} issue(s)", issues.len());
    Ok(())
}

async fn run_issue(ctx: &AppContext, action: IssueAction) -> Result<()> {
    let client = build_client(&ctx.config).await?;

    match action {
        IssueAction::List => {
            let issues = client.list_issues().await?;
            if issues.is_empty() {
                println!("No issues found");
            } else {
                for issue in issues {
                    println!("#{:<6} {}", issue.number, issue.title);
                }
            }
        }
        IssueAction::View { number } => {
            let issue = client.get_issue(number).await?;
            println!("#{number} - {}", issue.title);
            if let Some(body) = issue.body {
                if !body.trim().is_empty() {
                    println!("\n{}", body);
                }
            }
        }
    }

    Ok(())
}

async fn build_client(config: &Config) -> Result<GithubClient> {
    let token = config
        .github_token
        .as_deref()
        .context("GitHub token not configured. Run `notehub init --token ...`")?;
    let repo = config
        .repo
        .as_deref()
        .context("Repository not configured. Run `notehub init --repo ...`")?;

    let repo_spec = RepoSpec::parse(repo)?;
    GithubClient::new(token, repo_spec).await
}
