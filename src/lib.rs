#[allow(dead_code)]
mod book;
pub mod config;
mod git;

#[cfg(test)]
mod tests;

use anyhow::{anyhow, Ok, Result};
#[double]
use book::Book;
use chrono::Utc;
use config::Config;
use git::GitOp;
#[double]
use git::Repo;
use log::{debug, info, trace, warn};
use mockall_double::double;
use serde::Serialize;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

/// Generates all EPUBs defined in `config` and returns a `Manifest` containing
/// information about all generated books.
pub fn run(config: &Config) -> Result<Manifest> {
    let dest = config.destination_dir.as_ref().unwrap();
    let working_dir = config.working_dir.as_ref().unwrap();

    check_or_create_dir(dest.as_path())?;
    let entries = generate_books(&config.book_repo_configs, working_dir, dest)
        .ok_or_else(|| anyhow!("Something bad happened."))?;
    let manifest = Manifest {
        entries,
        timestamp: Utc::now().to_rfc3339(),
        title: config.title.to_owned(),
    };

    if let Some(ref templates) = config.templates_dir {
        render_template(templates, dest, &manifest)?;
    } else {
        render_json(dest, &manifest)?;
    }
    Ok(manifest)
}

fn check_or_create_dir(dest: &Path) -> io::Result<()> {
    if !dest.exists() {
        debug!("Creating destination directory ({:?})", dest);
        std::fs::create_dir_all(dest)
    } else {
        core::result::Result::Ok(())
    }
}

fn generate_books(
    book_repo_configs: &Vec<config::BookRepoConfig>,
    working_dir: &Path,
    dest: &Path,
) -> Option<Vec<ManifestEntry>> {
    if book_repo_configs.is_empty() {
        warn!("No book to generate");
        return None;
    }
    let mut shelf = Vec::with_capacity(book_repo_configs.len());
    for repo_config in book_repo_configs {
        trace!("{:#?}", repo_config);
        let repo_url = repo_config.repo_url.to_owned();

        let (mut repo_path, commit_sha, last_modified) =
            Repo::clone_or_fetch_repo(repo_url.as_str(), working_dir).ok()?;

        if let Some(repo_folder) = &repo_config.folder {
            repo_path = repo_path.join(repo_folder);
        }

        let mut vars: Vec<(String, Option<String>)> = if let Some(mapping) = &repo_config.env_var {
            let to_owned_kv = |(k, v): (&String, &toml::Value)| (k.to_owned(), Some(v.to_string()));
            mapping.iter().map(to_owned_kv).collect()
        } else {
            Vec::new()
        };
        if let Some(new_filename) = &repo_config.title {
            vars.push((
                String::from("MDBOOK_BOOK__TITLE"),
                Some(new_filename.to_owned()),
            ));
        }
        let (book_title, path, epub_size) =
            Book::generate_epub(repo_path.as_path(), vars, dest).ok()?;
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

        shelf.push(entry);
    }
    Some(shelf)
}

fn render_template(templates_dir: &Path, dest: &Path, manifest: &Manifest) -> Result<()> {
    // let templates_dir = config.templates_dir.as_ref().unwrap();
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

        let ctx = Context::from_serialize(manifest)?;
        let page = tera.render(template_path, &ctx).expect("Template error");
        let mut f = File::create(&output_path).expect("Could not create file");

        f.write_all(page.as_bytes())
            .expect("Error while writing file");
    }
    Ok(())
}

fn render_json(dest: &Path, manifest: &Manifest) -> Result<PathBuf> {
    let manifest_path = dest.join("manifest.json");
    info!("Writing manifest to {}", manifest_path.display());

    let f = File::create(&manifest_path).expect("Could not create manifest file");
    serde_json::to_writer_pretty(f, &manifest).expect("Error while writing manifest to file");
    Ok(manifest_path)
}
