use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use git2::Repository;
use log::{info, trace};
#[cfg(test)]
use mockall::automock;
use url::Url;

pub(crate) struct Repo;

#[cfg_attr(test, automock)]
impl GitOp for Repo {
    fn open(path: PathBuf) -> Result<Repository, git2::Error> {
        Repository::open(path)
    }
    fn clone(url: &str, into: PathBuf) -> Result<Repository, git2::Error> {
        Repository::clone(url, into)
    }
}

pub(crate) trait GitOp {
    /// Clones or fetches the repo at `entry.repo_url` inside `working_dir`.
    fn clone_or_fetch_repo(
        url: &str,
        working_dir: &Path,
    ) -> anyhow::Result<(PathBuf, String, String)> {
        let repo_path = if let Ok(parsed_url) = Url::parse(url) {
            trace!("Repo url parsed: {}", parsed_url);
            // skip initial `/` in path
            parsed_url.path()[1..].to_owned()
        } else {
            trace!("Local repo path?: {}", url);
            // local repo?
            url.to_owned()
        };
        let mut dest = working_dir.join(repo_path);

        // :TRICKY: can't use \ as path separator here because of improper native path handling in some parts of libgit2
        // see https://github.com/libgit2/libgit2/issues/3012
        if cfg!(windows) {
            dest = PathBuf::from(dest.to_str().unwrap().replace('\\', "/"));
        }

        let repo = if let Ok(repo) = Self::open(dest.clone()) {
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
            info!("Cloning {:?} to {:?}", url, &dest);
            Self::clone(url, dest.clone())?
        };

        let commit = repo.head()?.peel_to_commit()?;
        let commit_sha = commit.id().to_string();
        let last_modified = Utc.timestamp(commit.time().seconds(), 0).to_rfc3339();

        Ok((dest, commit_sha, last_modified))
    }

    fn open(path: PathBuf) -> Result<Repository, git2::Error>;
    fn clone(url: &str, into: PathBuf) -> Result<Repository, git2::Error>;
}

#[cfg(test)]
mod tests {
    use git2::Repository;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    use crate::{git::GitOp, tests::repo_init_opts};

    #[test]
    fn test_open_repo() {
        let url = "https://github.com/rams3s/mdbook-dummy.git";
        let parsed_url = url::Url::parse(url).unwrap().path()[1..].to_owned();
        let dest = TempDir::new().unwrap();
        let expect_repo_dir = dest.path().join(parsed_url);
        struct RepoTest;
        impl GitOp for RepoTest {
            fn open(_path: PathBuf) -> Result<Repository, git2::Error> {
                let mut opts = git2::RepositoryInitOptions::new();
                opts.origin_url("https://github.com/rams3s/mdbook-dummy.git");
                repo_init_opts(&_path, opts)
            }
            fn clone(_url: &str, _into: PathBuf) -> Result<Repository, git2::Error> {
                unreachable!()
            }
        }

        let (got_dest, _, _) = RepoTest::clone_or_fetch_repo(url, dest.path()).unwrap();
        assert_eq!(got_dest, expect_repo_dir);
    }

    #[test]
    fn test_clone_remote_repo() {
        let url = "http://git.repo.com/owner/NOSRC";
        let parsed_url = url::Url::parse(url).unwrap().path()[1..].to_owned();
        let dest = TempDir::new().unwrap();
        let expect_repo_dir = dest.path().join(parsed_url);

        assert_cloned_repo_dir(url, dest.path(), &expect_repo_dir);
    }

    #[test]
    fn test_clone_local_repo() {
        let src = "tests/mdbook-dummy";
        let dest = TempDir::new().unwrap();
        let expect_repo_dir = dest.path().join(src);

        assert_cloned_repo_dir(src, dest.path(), &expect_repo_dir);
    }

    fn assert_cloned_repo_dir(src: &str, dest: &Path, expect_repo_dir: &Path) {
        struct RepoTest;
        impl GitOp for RepoTest {
            fn open(_path: PathBuf) -> Result<Repository, git2::Error> {
                Err(git2::Error::from_str("YOU SHALL NOT OPEN"))
            }
            fn clone(_url: &str, _into: PathBuf) -> Result<Repository, git2::Error> {
                crate::tests::repo_init(&_into)
            }
        }
        let (got_dest, _sha, _date) = RepoTest::clone_or_fetch_repo(src, dest).unwrap();
        assert_eq!(got_dest, expect_repo_dir);
    }
}
