# MDBookshelf

[![Build Status](https://github.com/dieterplex/mdbookshelf/workflows/Tests/badge.svg?branch=main)](https://github.com/dieterplex/mdbookshelf/actions?workflow=Tests)

A Rust library/application to render a collection of books to EPUB using [forked mdbook-epub](https://github.com/dieterplex/mdbook-epub).
It uses [Tera](https://github.com/Keats/tera) template engine to render an optional template.

Used to generate the [Rust eBookshelf](https://dieterplex.github.io/rust-ebookshelf) nightly.

## Configuration

The configuration is handled through a `bookshelf.toml` file.

```toml
title = "The Rust Language & Ecosystem"
destination-dir = "out"
templates-dir = "templates"
working-dir = "tmp"

[[book]]
repo-url = "https://github.com/rust-lang/book.git"
url = "https://doc.rust-lang.org/stable/book/index.html"
[book.env-var]
MDBOOK_PREPROCESSOR__NOCOMMENT = ""

[[book]]
repo-url = "https://github.com/rust-lang/rust-by-example.git"
url = "https://doc.rust-lang.org/stable/rust-by-example/"

[[book]]
repo-url = "https://github.com/rust-lang-nursery/rust-cookbook.git"
url = "https://github.com/rust-lang-nursery/rust-cookbook"
```

### Preprocessing

mdBook build-in preprocessors is enabled tranparently and is affected by book.yaml per Book if there is any.
If you want to filter with custom preprocessors, using book.env-var, like the conf above, to specify special [enviroment variables](https://rust-lang.github.io/mdBook/format/configuration/environment-variables.html) that could be accepted by mdBook.
And don't forget to install preprocessors before building your bookshelf, or it would just generate books without these preprocessors.

## Usage

```
USAGE:
    mdbookshelf [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -d, --destination_dir <DESTINATION_DIR>    Sets the destination directory
    -t, --templates_dir <TEMPLATES_DIR>        Sets the templates directory  (if not set, will generate manifest.json)
    -w, --working_dir <WORKING_DIR>            Sets a custom working directory where the book repositories will be
                                               cloned
```

The options can be used to override values specified in `bookshelf.toml`.

## Contributions

- Cleanup some code - this is my very first Rust code. I wrote this while still reading [the Book](https://doc.rust-lang.org/book/) (to be able to finish it on my Kindle). If you know of things that are not idiomatic or could be done better, please do not hesitate ;)
- Fix a bug or implement a new thing
- Make a Pull Request

# Recent Changes

- 0.3.0 Support custom book preprocessing
- 0.2.0 Forked with custom epub-builder & mdbook-epub
- 0.1.1 Updated README
- 0.1.0 First release

# License

Licensed under the MIT license http://opensource.org/licenses/MIT.
This file may not be copied, modified, or distributed except according to those terms.
