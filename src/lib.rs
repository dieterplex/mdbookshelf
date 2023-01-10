#[allow(dead_code)]
mod book;
pub mod config;
#[allow(dead_code)]
mod git;

use anyhow::Result;
#[double]
use book::Book;
use chrono::Utc;
use config::Config;
#[double]
use git::Repo;
use log::info;
use mockall_double::double;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tera::Context;
use walkdir::WalkDir;

/// A manifest entry for the generated EPUB
#[derive(Default, Debug, PartialEq, Serialize)]
pub struct ManifestEntry {
    /// The commit sha
    pub commit_sha: String,
    /// The size of the EPUB in bytes
    pub epub_size: u64,
    /// The last modified date of the book (i.e. the datetime of the last commit)
    pub last_modified: String,
    /// The path to the generated EPUB
    pub path: PathBuf,
    /// The book repository URL
    pub repo_url: String,
    /// The book title
    pub title: String,
    /// The book online version URL
    pub url: String,
}

/// A Manifest contains the information about all EPUBs built
/// during one invocation of `mdbookshelf.run()`.
#[derive(Default, Debug, Serialize)]
pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
    pub timestamp: String,
    pub title: String,
}

impl Manifest {
    pub fn new() -> Manifest {
        Manifest {
            entries: Vec::new(),
            timestamp: Utc::now().to_rfc3339(),
            title: String::default(),
        }
    }
}

/// Generates all EPUBs defined in `config` and returns a `Manifest` containing
/// information about all generated books.
pub fn run(config: &Config) -> Result<Manifest> {
    let mut manifest = Manifest::new();
    manifest.entries.reserve(config.book_repo_configs.len());
    manifest.title = config.title.clone();

    let dest = config.destination_dir.as_ref().unwrap();
    let working_dir = config.working_dir.as_ref().unwrap();

    for repo_config in &config.book_repo_configs {
        let repo_url = repo_config.repo_url.to_owned();

        let (mut repo_path, commit_sha, last_modified) =
            Repo::clone_or_fetch_repo(repo_url.as_str(), working_dir)?;

        if let Some(repo_folder) = &repo_config.folder {
            repo_path = repo_path.join(repo_folder);
        }

        let (book_title, path, epub_size) = Book::generate_epub(repo_path.as_path(), dest)?;
        let title = repo_config
            .title
            .to_owned()
            .or(book_title)
            .unwrap_or_default();

        let entry = ManifestEntry {
            commit_sha,
            epub_size,
            last_modified,
            path,
            repo_url,
            title,
            url: repo_config.url.to_owned(),
        };

        manifest.entries.push(entry);
    }

    if let Some(templates_dir) = config.templates_dir.as_ref() {
        let templates_pattern = templates_dir.join("**/*");
        let tera = tera::Tera::new(templates_pattern.to_str().unwrap())?;

        for entry in WalkDir::new(templates_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|v| v.ok())
            .filter(|e| !e.file_type().is_dir())
        {
            let template_path = entry.path().strip_prefix(templates_dir).unwrap();
            let template_path = template_path.to_str().unwrap();
            let output_path = dest.join(template_path);

            info!(
                "Rendering template {} to {}",
                template_path,
                output_path.display()
            );

            let ctx = Context::from_serialize(&manifest)?;
            let page = tera.render(template_path, &ctx).expect("Template error");
            let mut f = File::create(&output_path).expect("Could not create file");

            f.write_all(page.as_bytes())
                .expect("Error while writing file");
        }
    } else {
        let manifest_path = dest.join("manifest.json");
        info!("Writing manifest to {}", manifest_path.display());

        let f = File::create(&manifest_path).expect("Could not create manifest file");
        serde_json::to_writer_pretty(f, &manifest).expect("Error while writing manifest to file");
    }

    Ok(manifest)
}

#[test]
fn test_run() {
    use std::str::FromStr;

    let expect_sha = "52476abfd5f0f1e8df272623eb6c9216db18f0b3".to_string();
    let expect_size = 9527u64;
    let expect_date = "2019-04-19T11:02:18+00:00".to_string();
    let expect_path = PathBuf::from("Hello Rust.epub");
    let expect_title = "Hello Rust".to_string();

    let ctx_repo = git::MockRepo::clone_or_fetch_repo_context();
    let repo_result = (
        PathBuf::from("tests/repos/rams3s/mdbook-dummy.git"),
        expect_sha.to_owned(),
        expect_date.to_owned(),
    );
    ctx_repo
        .expect()
        .once()
        .return_once(move |_url, _working_dir| Ok(repo_result));

    let ctx_book = book::MockBook::generate_epub_context();
    let book_result = (
        Some(expect_title.to_owned()),
        expect_path.to_owned(),
        expect_size.to_owned(),
    );
    ctx_book
        .expect()
        .once()
        .return_once(move |_path, _dest| Ok(book_result));

    const CONFIG: &str = r#"
    title = "My eBookshelf"
    destination-dir = "tests/out"
    working-dir = "tests/repos"
    templates-dir = "tests/templates"

    [[book]]
    repo-url = "https://github.com/rams3s/mdbook-dummy.git"
    url = "https://rams3s.github.io/mdbook-dummy/index.html"
    "#;

    let config = Config::from_str(CONFIG).unwrap();
    let got = run(&config).unwrap();
    let entry = ManifestEntry {
        commit_sha: expect_sha,
        epub_size: expect_size,
        last_modified: expect_date,
        path: expect_path,
        repo_url: config.book_repo_configs[0].repo_url.to_owned(),
        title: expect_title,
        url: config.book_repo_configs[0].url.to_owned(),
    };
    assert_eq!(got.entries[0], entry);
    assert_eq!(got.title, config.title);
}
