use git2::{Repository, Signature};
use std::path::Path;

/// Write `content` to `dir/filename` (creating `dir` and a git repo if
/// needed) and commit it. Returns the new commit's SHA.
pub fn publish_file(
    dir: &Path,
    filename: &str,
    content: &str,
    message: &str,
) -> Result<String, crate::error::Error> {
    let repo = if dir.join(".git").exists() {
        Repository::open(dir)?
    } else {
        std::fs::create_dir_all(dir)?;
        Repository::init(dir)?
    };

    let target = dir.join(filename);
    std::fs::write(&target, content)?;

    let mut index = repo.index()?;
    index.add_path(Path::new(filename))?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let sig = Signature::now("psychological-operations", "psyops@localhost")?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;

    Ok(oid.to_string())
}

