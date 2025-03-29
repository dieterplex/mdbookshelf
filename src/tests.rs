use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use chrono::{TimeZone, Utc};
use git2::Repository;
use mockall::predicate;

use super::{book, config::Config, git, ManifestEntry};

#[test]
fn test_run() {
    let config = Config::from_str(&format!(
        r#"
    title = "My eBookshelf"
    destination-dir = "tests/out"
    working-dir = "tests/repos"
    templates-dir = "tests/templates"

    [[book]]
    title = "Hello Rust"
    repo-url = "{REPO_URL}"
    url = "https://rams3s.github.io/mdbook-dummy/index.html"
    folder = "book"
    [book.env-var]
    MDBOOK_PREPROCESSOR__X = ""
    "#
    ))
    .unwrap();
    const REPO_URL: &str = "https://github.com/rams3s/mdbook-dummy.git";
    let repo_url_ = url::Url::parse(REPO_URL).unwrap();
    let repo_path = &repo_url_.path()[1..];
    let clone_path = config.working_dir.clone().unwrap().join(repo_path);

    let expect_size = 9527u64;
    let expect_title = String::from("Hello Rust");
    let expect_filename = PathBuf::from(format!("{expect_title}.epub"));

    let dest = tempfile::TempDir::new().unwrap();
    let sec_cell = Arc::new(Mutex::new(0));
    let sha_cell = Arc::new(Mutex::new(String::new()));

    // move to closure
    let dest_ = dest.path().to_path_buf();
    let sec_ref = Arc::clone(&sec_cell);
    let sha_ref = Arc::clone(&sha_cell);
    let book_result = (
        Some(expect_title.to_owned()),
        expect_filename.to_owned(),
        expect_size.to_owned(),
    );

    // mocks
    let ctx_clone = git::MockRepo::clone_context();
    ctx_clone
        .expect()
        .with(predicate::eq(REPO_URL), predicate::eq(clone_path))
        .once()
        .return_once(move |_, _| {
            let repo = repo_init(&dest_).unwrap();
            {
                let commit = repo.head()?.peel_to_commit()?;
                *sha_ref.lock().unwrap() = commit.id().to_string();
                *sec_ref.lock().unwrap() = commit.time().seconds();
            }
            Ok(repo)
        });
    let ctx_open = git::MockRepo::open_context();
    ctx_open
        .expect()
        .once()
        .return_once(|_| Err(git2::Error::from_str("YOU SHALL NOT OPEN")));

    let ctx_book = book::MockBook::generate_epub_context();
    ctx_book
        .expect()
        .once()
        .return_once(move |_path, vars, _dest| {
            assert_eq!(vars.len(), 2);
            assert_eq!(vars[0].0, "MDBOOK_PREPROCESSOR__X");
            assert_eq!(vars[0].1, Some(String::from("\"\"")));
            assert_eq!(vars[1].0, "MDBOOK_BOOK__TITLE");
            assert_eq!(&vars[1].1, &book_result.0);
            Ok(book_result)
        });

    let got = super::run(&config).unwrap();

    let entry = ManifestEntry {
        title: expect_title,
        path: expect_filename,
        epub_size: expect_size,
        url: config.book_repo_configs[0].url.to_owned(),
        repo_url: config.book_repo_configs[0].repo_url.to_owned(),
        commit_sha: sha_cell.lock().unwrap().to_string(),
        last_modified: Utc
            .timestamp_opt(*sec_cell.lock().unwrap(), 0)
            .unwrap()
            .to_rfc3339(),
    };
    assert_eq!(got.entries[0], entry);
    assert_eq!(got.title, config.title);
}

/// Dummy repo init. Copied from git2::test.
pub(crate) fn repo_init(dest: &Path) -> Result<Repository, git2::Error> {
    repo_init_opts(dest, git2::RepositoryInitOptions::new())
}

pub(crate) fn repo_init_opts(
    dest: &Path,
    opts: git2::RepositoryInitOptions,
) -> Result<Repository, git2::Error> {
    let mut opts = opts;
    _ = &opts.initial_head("main");
    let repo = Repository::init_opts(dest, &opts)?;
    {
        let mut config = repo.config()?;
        config.set_str("user.name", "name")?;
        config.set_str("user.email", "email")?;
        let mut index = repo.index()?;
        let id = index.write_tree()?;

        let tree = repo.find_tree(id)?;
        let sig = repo.signature()?;
        repo.commit(Some("HEAD"), &sig, &sig, "initial\n\nbody", &tree, &[])?;
    }
    Ok(repo)
}
