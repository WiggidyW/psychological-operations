pub mod error;
pub mod config;
pub mod db;
pub mod psyop;
pub mod input;
pub mod publish;
pub mod playwright;
pub mod score;
pub mod notifications;
pub mod agent;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "psychological-operations")]
#[command(about = "Agentic X scraper with ObjectiveAI scoring pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a psyop by name
    Run {
        name: String,
        /// Detach when agent needs input, printing PID
        #[arg(long)]
        detach_stdin: bool,
    },
    /// Interact with a running agent intervention
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// Send a message to a detached agent and reattach to its output
    Reply {
        pid: u32,
        message: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Agent intervention timeout in seconds
    AgentTimeout {
        #[command(subcommand)]
        command: GetSet,
    },
    /// Agent intervention max retry attempts
    AgentMaxAttempts {
        #[command(subcommand)]
        command: GetSet,
    },
    /// Manage notification targets
    Notifications {
        #[command(subcommand)]
        command: NotificationsCommands,
    },
}

#[derive(Subcommand)]
enum GetSet {
    Get,
    Set { value: String },
}

#[derive(Subcommand)]
enum NotificationsCommands {
    /// Get all notifications or one by index
    Get {
        index: Option<usize>,
    },
    /// Add a notification target (JSON string)
    Add {
        json: String,
    },
    /// Remove a notification target by index
    Del {
        index: usize,
    },
}

pub async fn run() -> Result<String, error::Error> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { name, detach_stdin: _ } => {
            run_psyop(&name).await?;
            Ok(String::new())
        }
        Commands::Agent { command } => {
            match command {
                AgentCommands::Reply { pid, message } => {
                    agent::reply::send_reply(pid, &message).await?;
                    Ok(String::new())
                }
            }
        }
        Commands::Config { command } => handle_config(command),
    }
}

async fn run_psyop(name: &str) -> Result<(), error::Error> {
    let cfg = config::load();
    let psyop_dir = config::psyops_dir().join(name);
    let config_path = psyop_dir.join("psyop.json");

    if !config_path.exists() {
        return Err(error::Error::PsyopNotFound(config_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&config_path)?;
    let psyop: psyop::PsyOp = serde_json::from_str(&data)?;

    // Get commit SHA
    let repo = git2::Repository::open(&psyop_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    let commit_sha = head.id().to_string();

    // Scrape
    let playwright_dir = std::env::var("PSYOPS_PLAYWRIGHT_DIR")
        .unwrap_or_else(|_| "psychological-operations-playwright".to_string());
    let mut pw = playwright::Playwright::spawn(&playwright_dir)?;

    let target_count = psyop.stages.first()
        .and_then(|s| s.count)
        .unwrap_or(100) as usize;
    let now = chrono::Utc::now();

    let db = db::Db::open()?;

    // Open tabs
    let states = pw.open_tabs(&psyop.queries)?;
    for (query, state) in &states {
        if state == "unexpected" {
            // TODO: agent intervention
            return Err(error::Error::Playwright(format!("unexpected page state for query \"{query}\"")));
        }
    }

    // Scrape tweets
    let mut collected = 0;
    while collected < target_count {
        let Some((tweet, query)) = pw.next_tweet()? else { break };

        let validation = psyop::valid_for_psyop(&psyop, &tweet.created, tweet.likes, &now);
        if !validation.valid {
            if validation.reason == Some("max_age") {
                pw.close_query(&query)?;
            }
            continue;
        }

        let post = db::QueuedPost {
            id: tweet.id,
            scrape_id: name.to_string(),
            query: query.clone(),
            handle: tweet.handle,
            text: tweet.text,
            images: tweet.images,
            videos: tweet.videos,
            created: tweet.created,
            community: tweet.community,
            psyop: name.to_string(),
            psyop_commit_sha: commit_sha.clone(),
        };

        let inserted = db.insert_post(&post)?;
        if !inserted {
            if db.has_existing_post(&post.id, &query, name, &commit_sha)? {
                pw.close_query(&query)?;
            }
            continue;
        }

        collected += 1;
    }

    pw.close()?;

    // Score
    let posts = db.get_posts(name)?;
    if !posts.is_empty() {
        let scored = score::score(&psyop, posts)?;
        let ids: Vec<String> = scored.iter().map(|s| s.post.id.clone()).collect();
        let scores: Vec<f64> = scored.iter().map(|s| s.score).collect();
        db.finish_posts(&ids, &scores)?;
    }

    // Notify
    notifications::notify(&cfg.notifications, &format!("PsyOp \"{name}\": scraped {collected} posts.")).await;

    Ok(())
}

fn handle_config(command: ConfigCommands) -> Result<String, error::Error> {
    match command {
        ConfigCommands::AgentTimeout { command } => {
            let mut cfg = config::load();
            match command {
                GetSet::Get => Ok(serde_json::to_string(&cfg.agent_timeout)?),
                GetSet::Set { value } => {
                    cfg.agent_timeout = value.parse().map_err(|_| error::Error::Other("invalid number".into()))?;
                    config::save(&cfg)?;
                    Ok("ok".into())
                }
            }
        }
        ConfigCommands::AgentMaxAttempts { command } => {
            let mut cfg = config::load();
            match command {
                GetSet::Get => Ok(serde_json::to_string(&cfg.agent_max_attempts)?),
                GetSet::Set { value } => {
                    cfg.agent_max_attempts = value.parse().map_err(|_| error::Error::Other("invalid number".into()))?;
                    config::save(&cfg)?;
                    Ok("ok".into())
                }
            }
        }
        ConfigCommands::Notifications { command } => {
            let mut cfg = config::load();
            match command {
                NotificationsCommands::Get { index } => {
                    match index {
                        Some(i) => {
                            let entry = cfg.notifications.get(i)
                                .ok_or_else(|| error::Error::Other(format!("no notification at index {i}")))?;
                            Ok(serde_json::to_string(entry)?)
                        }
                        None => Ok(serde_json::to_string(&cfg.notifications)?),
                    }
                }
                NotificationsCommands::Add { json } => {
                    let parsed: notifications::NotificationConfig = serde_json::from_str(&json)?;
                    cfg.notifications.push(parsed);
                    config::save(&cfg)?;
                    Ok("ok".into())
                }
                NotificationsCommands::Del { index } => {
                    if index >= cfg.notifications.len() {
                        return Err(error::Error::Other(format!("no notification at index {index}")));
                    }
                    cfg.notifications.remove(index);
                    config::save(&cfg)?;
                    Ok("ok".into())
                }
            }
        }
    }
}
