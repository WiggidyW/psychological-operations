use clap::Args;
use git2::{Repository, Signature};
use std::path::Path;

use crate::config;
use crate::psyop::PsyOp;

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct PsyopSource {
    /// Inline JSON psyop definition
    #[arg(long)]
    psyop_inline: Option<String>,
    /// Path to a JSON file containing the psyop definition
    #[arg(long)]
    psyop_file: Option<std::path::PathBuf>,
}

#[derive(Args)]
pub struct PublishArgs {
    /// Psyop name
    #[arg(long)]
    pub name: String,
    #[command(flatten)]
    pub source: PsyopSource,
    /// Commit message
    #[arg(long)]
    pub message: String,
}

impl PublishArgs {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        let psyop: PsyOp = if let Some(inline) = self.source.psyop_inline {
            serde_json::from_str(&inline)?
        } else if let Some(path) = self.source.psyop_file {
            let data = std::fs::read_to_string(&path)?;
            serde_json::from_str(&data)?
        } else {
            unreachable!("clap group ensures one is set")
        };

        psyop.validate()?;

        let sha = publish(&self.name, &psyop, &self.message)?;
        Ok(crate::Output::Api(sha))
    }
}

fn publish(name: &str, psyop: &PsyOp, message: &str) -> Result<String, crate::error::Error> {
    let dir = config::psyops_dir().join(name);
    let config_path = dir.join("psyop.json");

    // Initialize repo if it doesn't exist
    let repo = if dir.join(".git").exists() {
        Repository::open(&dir)?
    } else {
        std::fs::create_dir_all(&dir)?;
        Repository::init(&dir)?
    };

    // Write psyop.json
    let json = serde_json::to_string_pretty(psyop)? + "\n";
    std::fs::write(&config_path, &json)?;

    // Stage
    let mut index = repo.index()?;
    index.add_path(Path::new("psyop.json"))?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // Commit
    let sig = Signature::now("psychological-operations", "psyops@localhost")?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;

    Ok(oid.to_string())
}
