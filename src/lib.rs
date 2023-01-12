mod book;
pub mod config;
mod git;

use anyhow::{Error, Result};
use book::Book;
use chrono::Utc;
use config::Config;
use log::info;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tera::Context;
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
            git::clone_or_fetch_repo(repo_url.as_str(), working_dir)?;

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
