mod config;

use clap::{Parser, Subcommand};
use config::Config;

struct AppContext {
    config: Config,
    config_path: std::path::PathBuf,
}

impl AppContext {
    fn load() -> std::io::Result<Self> {
        let (config, path) = Config::load()?;
        Ok(Self { config, config_path: path })
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

fn main() {
    let cli = Cli::parse();
    let mut ctx = match AppContext::load() {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("Failed to load config: {err}");
            std::process::exit(1);
        }
    };

    match cli.command {
        Command::Sync => println!("[todo] sync issues"),
        Command::Init(args) => handle_init(&mut ctx, args),
        Command::Issue { action } => match action {
            IssueAction::List => println!("[todo] list issues"),
            IssueAction::View { number } => println!("[todo] view issue #{number}"),
        },
        Command::Note { action } => match action {
            NoteAction::Add { number, text } => {
                println!("[todo] add note to issue #{number}: {text}")
            }
            NoteAction::List { number } => println!("[todo] list notes for issue #{number}"),
        },
    }
}

fn handle_init(ctx: &mut AppContext, args: InitArgs) {
    if let Some(token) = args.token {
        ctx.config.github_token = Some(token);
    }
    if let Some(repo) = args.repo {
        ctx.config.repo = Some(repo);
    }

    match (&ctx.config.github_token, &ctx.config.repo) {
        (Some(_), Some(_)) => {
            if let Err(err) = ctx.save() {
                eprintln!("Failed to write config: {err}");
                std::process::exit(1);
            }
            println!("Configuration saved to {}", ctx.config_path.display());
        }
        _ => {
            eprintln!("init requires both --token and --repo");
            std::process::exit(1);
        }
    }
}
