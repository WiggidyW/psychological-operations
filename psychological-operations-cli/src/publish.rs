use git2::{Repository, Signature};
use std::path::Path;

use crate::config;
use crate::psyop::PsyOp;

pub fn publish(name: &str, psyop: &PsyOp, message: &str) -> Result<String, crate::error::Error> {
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
