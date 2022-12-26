use anyhow::{anyhow, Result};
use mdbook::MDBook;
use std::path::{Path, PathBuf};

#[cfg(test)]
use mockall::automock;

pub(crate) struct BookOp;

#[cfg_attr(test, automock)]
impl BookOp {
    pub(crate) fn load(book_root: &Path) -> Result<mdbook::MDBook> {
        mdbook::MDBook::load(book_root)
    }
    pub(crate) fn epub_generate(md: &MDBook, dest: &Path) -> Result<(), mdbook_epub::Error> {
        mdbook_epub::generate_with_preprocessor(md, dest)
    }
}

pub(crate) struct Book;

#[cfg_attr(test, automock)]
impl Book {
    /// Generate an EPUB from `path` to `dest`. Also modify manifest `entry` accordingly.
    pub(crate) fn generate_epub(
        path: &Path,
        env_var: Vec<(String, Option<String>)>,
        dest: &Path,
    ) -> Result<(Option<String>, PathBuf, u64)> {
        // TODO: multi thread gereration
        // Env vars are global states, keep them only when loading mdbook config.
        let md = temp_env::with_vars(env_var, || BookOp::load(path))
            .map_err(|e| anyhow!("Could not load mdbook: {}", e))?;

        if let Err(e) = BookOp::epub_generate(&md, dest) {
            log::warn!("epub_generate fail: {:?}", e);
        }

        let output_file = mdbook_epub::output_filename(dest, &md.config);
        log::info!("Generated epub into {}", output_file.display());

        let metadata = std::fs::metadata(&output_file)?;
        let epub_size = metadata.len();
        let output_path = mdbook_epub::output_filename(Path::new(""), &md.config);
        let title = md.config.book.title;

        Ok((title, output_path, epub_size))
    }
}

#[test]
fn test_generate_epub() {
    use std::path::Path;

    let path = Path::new("tests").join("dummy");
    let dest = Path::new("tests").join("book");

    let (title, path, size) =
        Book::generate_epub(path.as_path(), Vec::new(), dest.as_path()).unwrap();

    assert!(size > 0, "Epub size should be bigger than 0");
    assert_eq!(title.unwrap(), "Hello Rust", "Title doesn't match");
    assert_eq!(
        path,
        Path::new("Hello Rust.epub"),
        "Manifest entry path should be filled"
    );
}
