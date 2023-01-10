use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use git2::Repository;
use log::info;
#[cfg(test)]
use mockall::automock;
use mockall_double::double;
use url::Url;

struct GitOp;

#[cfg_attr(test, automock)]
impl GitOp {
    fn open(path: PathBuf) -> Result<Repository, git2::Error> {
        Repository::open(path)
    }
    fn clone(url: &str, into: PathBuf) -> Result<Repository, git2::Error> {
        Repository::clone(url, into)
    }
}

pub(crate) struct Repo;

#[cfg_attr(test, automock)]
impl Repo {
    /// Clones or fetches the repo at `entry.repo_url` inside `working_dir`.
    pub(crate) fn clone_or_fetch_repo(
        url: &str,
        working_dir: &Path,
    ) -> anyhow::Result<(PathBuf, String, String)> {
        #[double]
        use GitOp as Git;

        let repo_path = if let Ok(parsed_url) = Url::parse(url) {
            log::trace!("Repo url parsed: {}", parsed_url);
            // skip initial `/` in path
            parsed_url.path()[1..].to_owned()
        } else {
            log::trace!("Local repo path?: {}", url);
            // local repo?
            url.to_owned()
        };
        let mut dest = working_dir.join(repo_path);

        // :TRICKY: can't use \ as path separator here because of improper native path handling in some parts of libgit2
        // see https://github.com/libgit2/libgit2/issues/3012
        if cfg!(windows) {
            dest = PathBuf::from(dest.to_str().unwrap().replace('\\', "/"));
        }

        let repo = if let Ok(repo) = Git::open(dest.clone()) {
            repo.find_remote("origin").and_then(|mut remote| {
                assert_eq!(
                    remote.url().unwrap(),
                    url,
                    "Remote url for origin and requested url do not match"
                );
                info!("Found {:?}. Fetching {}", &dest, url);
                remote.fetch(&["master"], None, None)
            })?;
            repo
        } else {
            // :TODO: shallow clone when supported by libgit2 (https://github.com/libgit2/libgit2/issues/3058)
            info!("Cloning {} to {:?}", url, &dest);
            Git::clone(url, dest.clone())?
        };

        let commit = repo.head()?.peel_to_commit()?;
        let commit_sha = commit.id().to_string();
        let last_modified = Utc.timestamp(commit.time().seconds(), 0).to_rfc3339();

        Ok((dest, commit_sha, last_modified))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use git2::Repository;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_clone_repo() {
        let src = "http://git.repo/NOSRC";
        let parsed_src = url::Url::parse(src).unwrap().path()[1..].to_owned();
        let dest = TempDir::new().unwrap();
        let expect_dest = dest.path().join(parsed_src);

        // Coundn't open dest dir as a repo and ..
        let ctx_open = super::MockGitOp::open_context();
        ctx_open
            .expect()
            .once()
            .returning(|_| Err(git2::Error::from_str("YOU SHALL NOT OPEN")));
        // Init a repo on dest dirs
        let ctx_clone = super::MockGitOp::clone_context();
        ctx_clone
            .expect()
            .once()
            .returning(|_, dest| Ok(repo_init(&dest)));

        let (got_dest, _sha, _date) = super::Repo::clone_or_fetch_repo(src, dest.path()).unwrap();
        assert_eq!(got_dest, expect_dest);
    }

    /// Dummy repo init. Copied from git2::test.
    pub fn repo_init(dest: &Path) -> Repository {
        let mut opts = git2::RepositoryInitOptions::new();
        opts.initial_head("main");
        let repo = Repository::init_opts(dest, &opts).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "name").unwrap();
            config.set_str("user.email", "email").unwrap();
            let mut index = repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = repo.find_tree(id).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial\n\nbody", &tree, &[])
                .unwrap();
        }
        repo
    }
}
