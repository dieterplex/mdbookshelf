use std::path::{Path, PathBuf};

use anyhow::Error;
use chrono::{TimeZone, Utc};
use git2::Repository;
use log::info;
use url::Url;

/// Clones or fetches the repo at `entry.repo_url` inside `working_dir`.
pub(crate) fn clone_or_fetch_repo(
    url: &str,
    working_dir: &Path,
) -> Result<(PathBuf, String, String), Error> {
    let parsed_url = Url::parse(url)?;
    // skip initial `/` in path
    let mut dest = working_dir.join(&parsed_url.path()[1..]);

    // :TRICKY: can't use \ as path separator here because of improper native path handling in some parts of libgit2
    // see https://github.com/libgit2/libgit2/issues/3012
    if cfg!(windows) {
        dest = PathBuf::from(dest.to_str().unwrap().replace('\\', "/"));
    }

    let repo = if let Ok(repo) = Repository::open(&dest) {
        repo.find_remote("origin").and_then(|mut remote| {
            assert_eq!(
                remote.url().unwrap(),
                url,
                "Remote url for origin and requested url do not match"
            );
            info!("Found {:?}. Fetching {}", dest, url);
            remote.fetch(&["master"], None, None)
        })?;
        repo
    } else {
        // :TODO: shallow clone when supported by libgit2 (https://github.com/libgit2/libgit2/issues/3058)
        info!("Cloning {} to {:?}", url, dest);
        Repository::clone(url, &dest)?
    };

    let commit = repo.head()?.peel_to_commit()?;
    let commit_sha = commit.id().to_string();
    let last_modified = Utc.timestamp(commit.time().seconds(), 0).to_rfc3339();

    Ok((dest, commit_sha, last_modified))
}
