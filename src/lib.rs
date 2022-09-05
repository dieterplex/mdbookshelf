pub mod config;

use anyhow::{anyhow, Error, Result};
use chrono::{TimeZone, Utc};
use config::Config;
use git2::Repository;
use log::info;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tera::Context;
use url::Url;
use walkdir::WalkDir;

/// A manifest entry for the generated EPUB
#[derive(Default, Serialize)]
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
#[derive(Default, Serialize)]
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
pub fn run(config: &Config) -> Result<Manifest, Error> {
    let mut manifest = Manifest::new();
    manifest.entries.reserve(config.book_repo_configs.len());
    manifest.title = config.title.clone();

    let dest = config.destination_dir.as_ref().unwrap();
    let working_dir = config.working_dir.as_ref().unwrap();

    for repo_config in &config.book_repo_configs {
        let repo_url = repo_config.repo_url.to_owned();

        let (mut repo_path, commit_sha, last_modified) =
            clone_or_fetch_repo(repo_url.as_str(), working_dir)?;

        if let Some(repo_folder) = &repo_config.folder {
            repo_path = repo_path.join(repo_folder);
        }

        let (book_title, path, epub_size) = generate_epub(repo_path.as_path(), dest)?;
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

/// Generate an EPUB from `path` to `dest`. Also modify manifest `entry` accordingly.
fn generate_epub(path: &Path, dest: &Path) -> Result<(Option<String>, PathBuf, u64), Error> {
    let md = MDBook::load(path).map_err(|e| anyhow!("Could not load mdbook: {}", e))?;

    let ctx = RenderContext::new(md.root.clone(), md.book.clone(), md.config.clone(), dest);

    _ = mdbook_epub::generate(&ctx);

    let output_file = mdbook_epub::output_filename(dest, &ctx.config);
    info!("Generated epub into {}", output_file.display());

    let metadata = std::fs::metadata(&output_file)?;
    let epub_size = metadata.len();
    let output_path = mdbook_epub::output_filename(Path::new(""), &ctx.config);
    let title = md.config.book.title;

    Ok((title, output_path, epub_size))
}

/// Clones or fetches the repo at `entry.repo_url` inside `working_dir`.
fn clone_or_fetch_repo(url: &str, working_dir: &Path) -> Result<(PathBuf, String, String), Error> {
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

#[test]
fn test_generate_epub() {
    let path = Path::new("tests").join("dummy");
    let dest = Path::new("tests").join("book");

    let (title, path, size) = generate_epub(path.as_path(), dest.as_path()).unwrap();

    assert!(size > 0, "Epub size should be bigger than 0");
    assert_eq!(title.unwrap(), "Hello Rust", "Title doesn't match");
    assert_eq!(
        path,
        Path::new("Hello Rust.epub"),
        "Manifest entry path should be filled"
    );
}
