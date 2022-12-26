//! Mdbookshelf's configuration.
//!
//! Heavily inspired by mdbook's Config.

#![deny(missing_docs)]

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Deserializer, Serialize};
use toml::{value::Table, Value};

/// The overall configuration object for MDBookshelf, essentially an in-memory
/// representation of `bookshelf.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// An array of BookRepoConfig
    pub book_repo_configs: Vec<BookRepoConfig>,
    /// Destination directory.
    pub destination_dir: Option<PathBuf>,
    /// Templates directory (if not set, will generate manifest.json).
    pub templates_dir: Option<PathBuf>,
    /// Title of the book collection.
    pub title: String,
    /// Working directory.
    pub working_dir: Option<PathBuf>,
}

impl Config {
    /// Load the configuration file from disk.
    pub fn from_disk<P: AsRef<Path>>(config_file: P) -> Result<Config, Error> {
        let mut buffer = String::new();
        File::open(config_file)?.read_to_string(&mut buffer)?;

        Config::from_str(&buffer)
    }
}

impl FromStr for Config {
    type Err = Error;

    /// Load a `Config` from some string.
    fn from_str(src: &str) -> Result<Self, Self::Err> {
        toml::from_str(src).map_err(|e| anyhow!("{}", e))
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(de)?;

        let mut table = match raw {
            Value::Table(t) => t,
            _ => {
                use serde::de::Error;
                return Err(D::Error::custom(
                    "A config file should always be a toml table",
                ));
            }
        };

        let book_repo_configs: Vec<BookRepoConfig> = table
            .remove("book")
            .and_then(|value| value.try_into().ok())
            .unwrap_or_default();
        let destination_dir: Option<PathBuf> = table
            .remove("destination-dir")
            .and_then(|value| value.try_into().ok())
            .unwrap_or_default();
        let templates_dir: Option<PathBuf> = table
            .remove("templates-dir")
            .and_then(|value| value.try_into().ok())
            .unwrap_or_default();
        let title: String = table
            .remove("title")
            .and_then(|value| value.try_into().ok())
            .unwrap_or_default();
        let working_dir: Option<PathBuf> = table
            .remove("working-dir")
            .and_then(|value| value.try_into().ok())
            .unwrap_or_default();

        Ok(Config {
            book_repo_configs,
            destination_dir,
            templates_dir,
            title,
            working_dir,
        })
    }
}

/// The configuration for a single book
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct BookRepoConfig {
    /// The book's title.
    /// If set, overwrites the value read from the book itself when generating the manifest.
    pub title: Option<String>,
    /// The book root directory.
    pub folder: Option<PathBuf>,
    /// The git repository url.
    pub repo_url: String,
    /// The online rendered book url.
    pub url: String,
    /// Dynamic mdBook config.
    /// Use special environment variables to change config while loading mdbook
    pub env_var: Option<Table>,
}

impl Eq for BookRepoConfig {}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use toml::value::Table;

    use super::*;

    const COMPLEX_CONFIG: &str = r#"
        title = "My bookshelf"
        templates-dir = "templates/"

        [[book]]
        title = "Some Book"
        repo-url = "git_source"
        url = "source"
        folder = "./foo"

        [[book]]
        repo-url = "git_source2"
        url = "source2"

        [book.env-var]
        MDBOOK_PREPROCESSOR__NOCOMMENT = """\
        multiline = ""\
        content = 42\
        """
        MDBOOK_PREPROCESSOR__NOP = ""
        "#;

    #[test]
    fn load_config_file() {
        let src = COMPLEX_CONFIG;

        let book_repo_configs = vec![
            BookRepoConfig {
                title: Some(String::from("Some Book")),
                folder: Some(PathBuf::from("./foo")),
                repo_url: String::from("git_source"),
                url: String::from("source"),
                ..Default::default()
            },
            BookRepoConfig {
                repo_url: String::from("git_source2"),
                url: String::from("source2"),
                env_var: Some(Table::from_iter([
                    (
                        String::from("MDBOOK_PREPROCESSOR__NOCOMMENT"),
                        Value::from("multiline = \"\"content = 42"),
                    ),
                    (String::from("MDBOOK_PREPROCESSOR__NOP"), Value::from("")),
                ])),
                ..Default::default()
            },
        ];

        let got = Config::from_str(src).unwrap();

        assert_eq!(got.title, "My bookshelf");
        assert_eq!(got.templates_dir.unwrap().to_str().unwrap(), "templates/");
        assert_eq!(got.book_repo_configs, book_repo_configs);
    }
}
