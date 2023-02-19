use std::path::PathBuf;
use std::process;

use clap::{crate_version, Arg, ArgAction, Command};
use env_logger::Env;
use log::{error, info};
use mdbookshelf::config::Config;

/// `mdbookshelf` binary reads config from `bookshelf.toml` file and allows
/// overwriting some of the value using command line arguments.
///
/// Run `mdbookshelf --help` for documentation.
fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    color_backtrace::install();

    let matches = Command::new("mdbookshelf")
        .about("Executes mdbook-epub on a collection of repositories")
        .version(concat!("v", crate_version!()))
        .author("Ramses Ladlani <rladlani@gmail.com>")
        .arg(
            Arg::new("working_dir")
                .short('w')
                .long("working_dir")
                .value_name("WORKING_DIR")
                .help("Sets a custom working directory where the book repositories will be cloned")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("destination_dir")
                .short('d')
                .long("destination_dir")
                .value_name("DESTINATION_DIR")
                .help("Sets the destination directory")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("templates_dir")
                .short('t')
                .long("templates_dir")
                .value_name("TEMPLATES_DIR")
                .help("Sets the templates directory (if not set, will generate manifest.json)")
                .action(ArgAction::Set),
        )
        .get_matches();

    // :TODO: add argument to set config path (bookshelf.toml)

    let config_location = std::env::current_dir()
        .unwrap_or(PathBuf::from("."))
        .join("bookshelf.toml");
    let mut config = if config_location.exists() {
        info!("Loading config from {}", config_location.display());
        Config::from_disk(&config_location).unwrap_or_default()
    } else {
        Config::default()
    };

    if let Some(destination_dir) = matches.get_one::<String>("destination_dir") {
        config.destination_dir = Some(destination_dir.into());
    }

    assert!(
        config.destination_dir.is_some(),
        "Destination dir must be set in toml file or through command line"
    );

    info!(
        "Running mdbookshelf with destination {}",
        config.destination_dir.as_ref().unwrap().display()
    );

    if let Some(working_dir) = matches.get_one::<String>("working_dir") {
        config.working_dir = Some(working_dir.into());
    }

    config.working_dir = config.working_dir.or_else(|| Some(PathBuf::from("repos")));

    info!(
        "Will Clone repositories to {}",
        config.working_dir.as_ref().unwrap().display()
    );

    if let Some(templates_dir) = matches.get_one::<String>("templates_dir") {
        config.templates_dir = Some(templates_dir.into());
    }

    match config.templates_dir.as_ref() {
        Some(templates_dir) => info!("Using templates in {}", templates_dir.display()),
        None => info!("No templates dir provided"),
    }

    if let Err(e) = mdbookshelf::run(&config) {
        error!("Application error {:?}", e.backtrace());
        e.chain().for_each(|c| error!("  caused by: {}", c));
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::error::Error;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use std::process::Command;
    use std::str::FromStr;

    use assert_cmd::prelude::*;
    use predicates::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn fail_when_missing_config_without_dest_args() -> Result<(), Box<dyn Error>> {
        let mut cmd = Command::cargo_bin("mdbookshelf")?;
        let pred = predicate::str::contains("Destination dir must be set");

        cmd.assert().failure().stderr(pred);
        Ok(())
    }

    #[test]
    fn gen_manifest_when_missing_config_without_templates_arg(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dest = tempdir()?;
        let mut cmd = Command::cargo_bin("mdbookshelf")?;
        cmd.arg("-d").arg(dest.path().as_os_str());

        let pred = predicate::str::contains("No book to generate");
        cmd.assert().stderr(pred).success();
        // manifest.json created when there is no template dir.
        let manifest = dest.path().join("manifest.json");
        assert!(predicate::path::is_file().eval(manifest.as_path()));
        let buf = fs::read_to_string(manifest.as_path())?;
        let m = serde_json::from_str::<mdbookshelf::Manifest>(buf.as_str())?;
        assert!(m.entries.is_empty());
        assert!(m.title.is_empty());
        Ok(())
    }

    #[test]
    fn render_when_missing_config_with_templates_arg() -> Result<(), Box<dyn Error>> {
        let dest = tempdir()?;
        let templates = Path::new("tests").join("templates");
        let mut cmd = Command::cargo_bin("mdbookshelf")?;
        cmd.arg("-d")
            .arg(dest.path().as_os_str())
            .arg("-t")
            .arg(templates.as_os_str());

        let pred = predicate::str::contains("No book to generate");
        cmd.assert().stderr(pred).success();
        let summary = dest.path().join("SUMMARY.md");
        let books = dest.path().join("books.md");
        assert!(predicate::path::is_file().eval(summary.as_path()));
        assert!(predicate::path::is_file().eval(books.as_path()));
        Ok(())
    }

    #[test]
    #[should_panic(expected = "Something bad happened.")]
    fn test_config_nosuchrepo() {
        const NOSUCHREPOCONF: &str = r#"
        destination-dir = "."
        working-dir = "repos"
        [[book]]
        repo-url = "https://github.com/mdbookepub/nosuch.git"
        url = "https://mdbookepub.github.io/nosuch/""#;
        let config = mdbookshelf::config::Config::from_str(NOSUCHREPOCONF).unwrap();
        panic!("{}", super::run(config).unwrap_err().to_string())
    }

    #[test]
    fn test_config_without_args() -> Result<(), Box<dyn Error>> {
        let dest = tempfile::tempdir()?;
        let curr_dir = env::current_dir()?;
        let file_path = dest.path().join("bookshelf.toml");
        let mut file = File::create(&file_path)?;
        let cfg = format!(
            r#"{}
destination-dir = "."
working-dir = "repos"
templates-dir = "{}"
"#,
            CONFIG_TITLE,
            curr_dir.join("tests").join("templates").display()
        );
        writeln!(file, "{}", cfg)?;

        let mut cmd = Command::cargo_bin("mdbookshelf")?;
        cmd.current_dir(dest.path());
        let pred = predicate::str::contains("No book to generate");
        cmd.assert().stderr(pred).success();

        let summary = dest.path().join("SUMMARY.md");
        let summary_content = fs::read_to_string(summary.as_path())?;
        assert_eq!(summary_content, "# Summary\n\n- [shelf](./books.md)\n");

        let books = dest.path().join("books.md");
        let books_content = fs::read_to_string(books.as_path())?;
        assert_eq!(books_content.len(), 42);
        assert!(books_content.starts_with("# shelf\n\nLast updated: "));
        assert!(books_content.ends_with("\n\n\n"));
        Ok(())
    }

    #[test]
    fn test_config_only_title_and_book() -> Result<(), Box<dyn Error>> {
        let dest = tempfile::tempdir()?;
        let file_path = dest.path().join("bookshelf.toml");
        let mut file = File::create(&file_path)?;
        writeln!(file, "{}{}", CONFIG_TITLE, CONFIG_BOOK)?;
        let curr_dir = env::current_dir()?;

        let mut cmd = Command::cargo_bin("mdbookshelf")?;
        cmd.current_dir(dest.path())
            .arg("-d")
            .arg(dest.path().join("out").as_os_str())
            .arg("-w")
            .arg(dest.path().join("repos").as_os_str())
            .arg("-t")
            .arg(curr_dir.join("tests").join("templates").as_os_str());
        cmd.assert().success();

        let epub = dest.path().join("out").join("Hello Rust.epub");
        assert!(predicate::path::is_file().eval(epub.as_path()));
        let epub_meta = fs::File::open(epub.as_path())?.metadata()?;
        assert!(epub_meta.len() > 0);

        let books = dest.path().join("out").join("books.md");
        let books_content = fs::read_to_string(books.as_path())?;
        assert!(books_content.contains("Hello Rust"));
        Ok(())
    }

    #[test]
    fn test_config_fromdisk() -> Result<(), Box<dyn Error>> {
        let dest = tempfile::tempdir()?;
        let dest_str = dest.path().as_os_str().to_string_lossy();

        let curr_dir = env::current_dir()?;
        let file_path = curr_dir.join("bookshelf.toml");
        let mut file = File::create(&file_path)?;
        const NOSUCHREPOCONF: &str = r#"
        destination-dir = "."
        working-dir = "repos"
        [[book]]
        repo-url = "https://github.com/mdbookepub/nosuch.git"
        url = "https://mdbookepub.github.io/nosuch/""#;
        writeln!(file, "{NOSUCHREPOCONF}")?;

        let args = vec!["mdbookshelf", "-d", &dest_str, "-w", "src"];
        let arg_matches = super::cmd().get_matches_from(args);
        let config = super::cfg(arg_matches);
        assert_eq!(config.working_dir.unwrap(), std::path::PathBuf::from("src"));

        fs::remove_file(file_path)?;
        Ok(())
    }

    const CONFIG_TITLE: &str = "title = \"shelf\"\n";
    const CONFIG_BOOK: &str = r#"
[[book]]
repo-url = "https://github.com/rams3s/mdbook-dummy.git"
url = "https://rams3s.github.io/mdbook-dummy/"
"#;
}
