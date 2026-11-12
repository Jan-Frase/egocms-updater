# EgoCMS Updater Tool 🦀🔥🚀

![Check](https://github.com/Jan-Frase/egocms-updater/actions/workflows/check.yml/badge.svg)

A Rust-based tool for synchronizing Markdown files with EgoCMS pages.

This tool lets you keep the pages of an EgoCMS site as a directory of Markdown files and push them to the CMS via its
REST API. This approach avoids the rather cumbersome EgoCMS editor; thus editing pages should be much more pleasant.

Please note that it is very narrow in scope, it was made to accomplish the tasks required for the Parallel Computing and
I/O (ParCIO) group's website and nothing more.

Feel free to fork or ask questions :)

---

## Setup

1. **Configure** `config/config.toml`. Only the address, site and language are required:
    ```toml
    url = "https://www.example.org"
    site = "parcio"
    language = "de"
    ```
   All optional settings (pages directory, home page, page types, ...) are listed with their defaults in the file.
2. **Set the credentials** of the EgoCMS user the tool acts as. They are read from environment variables, so they
   never end up in the repository. The user id can be found in the admin area under
   Verwaltung > Rollen > [user name] > bottom right corner, the API token can be created on the same page.
    ```bash
    export EGOCMS_USER_ID=12345
    export EGOCMS_TOKEN=abcde
    ```

## Usage

```bash
cargo run --release -- upload             # push ./pages to the CMS
cargo run --release -- upload --dry-run   # only show what would change
cargo run --release -- download <dir>     # save the CMS pages as markdown into an empty <dir>
```

Use `--config-path <file>` for another config and `--help` for an overview. Example output of an upload:

```
Found 6 pages and 3 files in ./pages.
= adopted Research/CoSEMoS/CoSEMoS.md (id 693)
+ created Teaching/PSS/PSS.md (id 627)
+ attached slides.pdf to Teaching/PSS/PSS.md
~ updated Home.md (id 1)
- deleted Research/JULEA/JULEA.md (id 583)

Pages: 1 created, 1 adopted, 1 updated, 2 unchanged, 1 deleted. Files: 1 attached, 0 deleted.
```

## Writing pages

The directory structure is the page tree, and **every page has its own directory**:

```
pages/
├── Home.md                                   the home page (`home_page` in the config)
├── logo.png                                  attached to the home page
└── Research/
    ├── Research.md                           below the home page
    ├── SMASH/
    │   ├── SMASH.md                          below Research
    │   └── slides.pdf                        attached to SMASH
    └── CoSEMoS/
        ├── CoSEMoS.md
        └── Problem Statement/
            └── Problem Statement.md          below CoSEMoS
```

- A page's file is named after its directory. The **file name is the page name**, which is part of its URL.
  The only exception is the home page, which owns the top level.
- The **first line is the title** (`# My Title`), the rest is the content.
- **Every other file belongs to the page of its directory** and is attached to it (EgoCMS calls this the page's "pool").
  A directory without a page file must not contain files.
- Page names may contain `/` (e.g. `Winter 2026/2027`), file names can't. Write it as `∕` (U+2215, division slash)
  in the file name: `Teaching/Winter 2026∕2027/Winter 2026∕2027.md`.
- **Links** to other pages and files are written relative to the current file and are converted to the right URLs:
  `[SMASH](SMASH/SMASH.md)`, `[Home](../../Home.md#teaching)`, `![Logo](../logo.png)`, `[Slides](slides.pdf)`.
  A space ends a link in markdown, so paths with spaces are written as `<Problem Statement/Problem Statement.md>`
  or with `%20`. Links to missing files stop the upload before anything is changed.
- Only lowercase `.md` files are pages. Hidden files (starting with `.`) are ignored.
- **Deleting** a page's directory deletes the page (it is moved to the EgoCMS trash, so it can be restored).
  **Renaming or moving** it deletes the old page and creates a new one.

## How it works

Each upload reads the current state of the pages below the home page, compares it to the local files and only writes
what changed. An upload without changes sends no writes at all. It runs in four steps, so a page never links to
something that doesn't exist yet: make sure all pages exist, attach new and changed files, update the changed contents,
then delete removed files and pages.

To recognize its pages, the tool stores a marker in each page's `extra` data, e.g.
`"_sync": { "source": "Research/SMASH/SMASH.md", "files": { "slides.pdf": "sha256:…" } }`.
The hashes let the upload detect changed files without downloading them.

- **Pages without a marker are never changed or deleted.** Pages that can't be represented as markdown (people pages,
  publications, ...) can therefore live next to the synced ones.
- **Existing pages are adopted:** if a file has no page yet, but its parent has an unmarked child with the same name,
  that page is taken over instead of creating a duplicate. Pages that markdown can't represent are never taken over.
- The CMS editor is not meant to be used anymore. Changes made there are overwritten with the next change of the file.
- The home page keeps its CMS name (renaming it would change every URL), only its title and content are synced.

A page counts as representable if its type is listed in `page_types` (default `["page"]`) and its content is at most a
single text block.

## Migrating an existing site

1. `cargo run --release -- download pages-from-cms` saves the home page and all representable pages below it, together
   with their attached files. Links between them are converted to relative paths. Skipped pages are listed by reason,
   e.g. `Skipped 12 pages of type ovgu/person (and the pages below them): ...`. Add page types to `page_types` and
   download again if something is missing.
2. Check the result, delete what shouldn't be managed by the tool and move the rest to `pages/`.
3. `cargo run --release -- upload --dry-run` shows what will happen: the pages should be adopted, not created.
4. `cargo run --release -- upload`. The first upload rewrites the HTML of every adopted page, as it is converted from
   markdown now, so check a few pages afterwards.

Files on the site's media site (not attached to a page) aren't downloaded; links to them stay absolute URLs.

## Limitations

- A page's content is a single text block. Layouts with several blocks (columns, image blocks, ...) aren't supported.
- The order of pages in the navigation can't be set from the files.
- Links to pages that weren't downloaded stay `index.php?…` URLs and are not checked.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

The GitHub workflow `check.yml` runs these on every push and pull request.

| Module           | Purpose                                                                       |
|------------------|-------------------------------------------------------------------------------|
| `settings/`      | Parses `config.toml` and reads the credentials.                               |
| `local/`         | Reads and validates the pages directory, see `local_page.rs` for the layout.  |
| `upload/`        | Resolves links, renders HTML and syncs pages and files (`sync.rs`).           |
| `download/`      | Converts CMS pages and their files into the pages directory.                  |
| `cms/`           | The `CmsApi` trait, the REST client and an in-memory fake for tests.          |

The logic only talks to EgoCMS through the `CmsApi` trait, so everything except the HTTP client is unit tested against
the in-memory fake. To test against a real instance, start the EgoCMS docker containers and use a config with
`url = "https://localhost"` and `accept_invalid_certs = true`.

### EgoCMS API quirks worth knowing

- Status codes say little: failed calls often return `2xx` with `null`, `false` or an `error` field, failed `GET`s
  return `204`, and every failed `DELETE` returns `400`. The client checks the bodies.
- The `field` returned by `getPage` contains an outdated serialized copy of `extra`. Sending it back overwrites
  `extra`, so the tool only sends `name` and `title`.
- Strings consisting only of digits are stored as numbers, and empty values are dropped from `extra`.
- The REST API can list, add and delete a page's files (`pool/list`, `pool/put`, `pool/delete`), but not return their
  content. The website serves them at `<page URL without .html>/_/<name>`.
- The editor stores links as `index.php?site=…&id=…` and `index.php?pool=…&id=…`, EgoCMS turns them into URLs when
  rendering. The upload writes the same format.
- Pages of type `blog/entry` can't be published in the editor without a short description and a keyword, which is
  why new pages use the type `page` by default.
