use anyhow::{Result, anyhow};
use git2::{DiffOptions, IndexAddOption, Oid, Repository, StatusOptions};
use std::path::Path;

pub struct GitOps {
    repo: Repository,
}

pub struct GitStatus {
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
    pub untracked: Vec<String>,
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
}

pub struct FileStatus {
    pub path: String,
    pub status: FileChangeType,
}

#[derive(Debug, Clone, Copy)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub time: u64,
    pub author: String,
}


impl GitOps {
    pub fn init(vault_path: &Path) -> Result<Self> {
        let repo = if Repository::discover(vault_path).is_ok() {
            Repository::open(vault_path)?
        } else {
            Repository::init(vault_path)?
        };

        // Normalize .gitignore on every init
        let gitignore_path = vault_path.join(".gitignore");
        let mut lines: Vec<String> = if gitignore_path.exists() {
            std::fs::read_to_string(&gitignore_path)?
                .lines()
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            Vec::new()
        };

        let mut changed = false;
        // Remove stale clin.key if present
        if let Some(pos) = lines.iter().position(|l| l == "clin.key") {
            lines.remove(pos);
            changed = true;
        }
        // Ensure key.bin is present
        if !lines.iter().any(|l| l == "key.bin") {
            lines.push("key.bin".to_string());
            changed = true;
        }

        if changed || !gitignore_path.exists() {
            let mut content = lines.join("\n");
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            crate::fsutil::atomic_write(&gitignore_path, content.as_bytes())?;
        }

        Ok(Self { repo })
    }

    pub fn is_initialized(vault_path: &Path) -> bool {
        Repository::discover(vault_path).is_ok()
    }

    pub fn add_all(&self) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;
        Ok(())
    }

    pub fn add_paths(&self, paths: &[String]) -> Result<()> {
        let mut index = self.repo.index()?;
        for p in paths {
            index.add_path(std::path::Path::new(p))?;
        }
        index.write()?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<Oid> {
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let sig = self.repo.signature()?;

        let parent_commit = match self.repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None,
        };

        let parents = match &parent_commit {
            Some(c) => vec![c],
            None => vec![],
        };

        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        Ok(oid)
    }

    pub fn status(&self) -> Result<GitStatus> {
        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        let statuses = self.repo.statuses(Some(&mut status_options))?;

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("unknown").to_string();
            let status = entry.status();

            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_renamed()
            {
                staged.push(FileStatus {
                    path: path.clone(),
                    status: self.map_status(status, true),
                });
            }

            if status.is_wt_modified() || status.is_wt_deleted() || status.is_wt_renamed() {
                unstaged.push(FileStatus {
                    path: path.clone(),
                    status: self.map_status(status, false),
                });
            }

            if status.is_wt_new() {
                untracked.push(path);
            }
        }

        let branch = self
            .repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().ok().map(|s| s.to_string()))
            .unwrap_or_else(|| "HEAD".to_string());

        // Ahead/behind
        let (ahead, behind) = self.get_ahead_behind()?;

        Ok(GitStatus {
            staged,
            unstaged,
            untracked,
            branch,
            ahead,
            behind,
        })
    }

    fn map_status(&self, status: git2::Status, is_staged: bool) -> FileChangeType {
        if is_staged {
            if status.is_index_new() {
                FileChangeType::Added
            } else if status.is_index_modified() {
                FileChangeType::Modified
            } else if status.is_index_deleted() {
                FileChangeType::Deleted
            } else if status.is_index_renamed() {
                FileChangeType::Renamed
            } else {
                FileChangeType::Modified
            }
        } else if status.is_wt_new() {
            FileChangeType::Added
        } else if status.is_wt_modified() {
            FileChangeType::Modified
        } else if status.is_wt_deleted() {
            FileChangeType::Deleted
        } else if status.is_wt_renamed() {
            FileChangeType::Renamed
        } else {
            FileChangeType::Modified
        }
    }

    fn get_ahead_behind(&self) -> Result<(usize, usize)> {
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(_) => return Ok((0, 0)),
        };
        let head_oid = head.target().ok_or_else(|| anyhow!("No head target"))?;

        let upstream = match self.repo.branch_upstream_name(head.name()?) {
            Ok(u) => u,
            Err(_) => return Ok((0, 0)),
        };
        let upstream_obj = self.repo.revparse_single(upstream.as_str()?)?;
        let upstream_oid = upstream_obj.id();

        let (ahead, behind) = self.repo.graph_ahead_behind(head_oid, upstream_oid)?;
        Ok((ahead, behind))
    }

    pub fn log(&self, max_count: usize) -> Result<Vec<CommitInfo>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head().ok(); // Ignore error if no HEAD (empty repo)

        let mut commits = Vec::new();
        for id in revwalk.take(max_count) {
            let id = id?;
            let commit = self.repo.find_commit(id)?;
            commits.push(CommitInfo {
                id: id.to_string(),
                message: commit.message().unwrap_or("").trim().to_string(),
                time: commit.time().seconds() as u64,
                author: commit.author().name().unwrap_or("").to_string(),
            });
        }
        Ok(commits)
    }
    pub fn unstage_paths(&self, paths: &[String]) -> Result<()> {
        let head_tree = match self.repo.head() {
            Ok(h) => Some(h.peel_to_tree()?),
            Err(_) => None,
        };
        self.repo.reset_default(head_tree.as_ref().map(|t| t.as_object()), paths.iter().map(std::path::Path::new))?;
        Ok(())
    }

    pub fn get_commit_diff(&self, commit_id: &str) -> Result<Vec<String>> {
        let oid: Oid = commit_id.parse()?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let diff = self.repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        let mut lines = Vec::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let origin = line.origin();
            let content = std::str::from_utf8(line.content()).unwrap_or("");
            match origin {
                '+' | '-' | ' ' => {
                    lines.push(format!("{}{}", origin, content.trim_end()));
                }
                _ => {}
            }
            true
        })?;
        Ok(lines)
    }

    pub fn pull(&self, remote_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)?;
        let branch = self.repo.head()?.shorthand().unwrap_or("main").to_string();
        remote.fetch(&[&branch], None, None)?;
        let tracking_branch = format!("refs/remotes/{}/{}", remote_name, branch);
        let fetch_head = self.repo.find_reference(&tracking_branch)?;
        let fetch_commit = self.repo.reference_to_annotated_commit(&fetch_head)?;
        let (analysis, _) = self.repo.merge_analysis(&[&fetch_commit])?;
        if analysis.is_fast_forward() {
            let mut reference = self.repo.find_reference(&format!("refs/heads/{}", branch))?;
            reference.set_target(fetch_commit.id(), "fast-forward pull")?;
            self.repo.set_head(&format!("refs/heads/{}", branch))?;
            self.repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        } else if analysis.is_normal() {
            return Err(anyhow!("Merge required - pull aborted. Resolve manually."));
        }
        Ok(())
    }

    pub fn get_file_diff(&self, path: &str) -> Result<Vec<String>> {
        let mut diff_options = DiffOptions::new();
        diff_options.pathspec(path);

        let head = match self.repo.head() {
            Ok(h) => Some(h.peel_to_tree()?),
            Err(_) => None,
        };

        let diff = self
            .repo
            .diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut diff_options))?;
        let mut lines = Vec::new();

        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let origin = line.origin();
            let content = std::str::from_utf8(line.content()).unwrap_or("");
            match origin {
                '+' | '-' | ' ' => {
                    lines.push(format!("{}{}", origin, content.trim_end()));
                }
                _ => {}
            }
            true
        })?;

        Ok(lines)
    }

    pub fn set_remote(&self, name: &str, url: &str) -> Result<()> {
        if self.repo.find_remote(name).is_ok() {
            self.repo.remote_set_url(name, url)?;
        } else {
            self.repo.remote(name, url)?;
        }
        Ok(())
    }

    pub fn has_changes(&self) -> Result<bool> {
        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        let statuses = self.repo.statuses(Some(&mut status_options))?;
        Ok(!statuses.is_empty())
    }

    pub fn push(&self, remote_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)?;
        let head = self.repo.head()?;
        let refname = head.name()?;

        remote.push(&[format!("{refname}:{refname}")], None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_gitignore_normalization() -> Result<()> {
        let tmp = tempdir()?;
        let vault_path = tmp.path();

        // 1. Fresh init
        let _ = GitOps::init(vault_path)?;
        let gitignore_path = vault_path.join(".gitignore");
        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path)?;
        assert!(content.contains("key.bin"));
        assert!(!content.contains("clin.key"));

        // 2. Init with stale clin.key
        fs::write(&gitignore_path, "some-other-file\nclin.key\nanother-file\n")?;
        let _ = GitOps::init(vault_path)?;
        let content = fs::read_to_string(&gitignore_path)?;
        assert!(content.contains("some-other-file"));
        assert!(content.contains("another-file"));
        assert!(content.contains("key.bin"));
        assert!(!content.contains("clin.key"));

        Ok(())
    }
}
