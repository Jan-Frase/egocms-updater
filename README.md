# EgoCMS Updater Tool 🦀🔥🚀

![Clippy-Formatting-Building](https://github.com/Jan-Frase/egocms-updater/actions/workflows/check.yml/badge.svg)
![Update](https://github.com/Jan-Frase/egocms-updater/actions/workflows/update.yml/badge.svg)

A Rust-based tool for synchronizing Markdown files with EgoCMS pages.

This tool allows you to have a collection of Markdown files, each mapped to an EgoCMS page.
On push, a GitHub-Action automatically sends any changes in a file to the mapped page via the EgoCMS API.
This approach avoids the rather cumbersome EgoCMS editor; thus editing pages should be much more pleasant.

Please note that it is very narrow in scope, it was made to accomplish the tasks required for the Parallel Computing and
I/O (ParCIO) group's website and nothing more.

Feel free to fork or ask questions :)

---

## How to use

- Configure the `config/config.toml`. It contains essential settings like the URLs or various relevant paths.
    ```csv
    # REST API base URL.
    rest_url = "https://localhost/rest/"

    # A single EgoCMS instance can have multiple sites. This defines which sites we are interested in.
    site_url = "materialkit/de/"
    ```
- Create or edit the markdown files in the defined directory.
- Edit mapping table CSV to map EgoCMS page IDs to Markdown files. If you are adding a new page, you will have
  to first create it via the EgoCMS editor.
    ```csv
    page_id,markdown_name
    56,landing_page.md
    58,research/cosemos.md
    ```
- Acquire your user-id and user-token for the EgoCMS API. They can be found/set in the admin section of EgoCMS. Then run
  the tool.
    ```
    Usage: cargo run --release -- [path-to-config.toml] [user-id] [user-token]

    Example: cargo run --release -- ./config/config.toml 12345 67890
    ```
- Done! Example output:
    ```
    ==========================
    1. Initializing Resources:
    ==========================
    1.1. Parsing arguments: => Success.
    1.2. Loading config: => Success.
    1.3. Loading Mapping table: => Success.
    1.4. Connecting to EgoCMS: => Success.

    ==========================
    2. Checking Mapping Table For Correctness:
    ==========================
    Are all IDs and names unique? => Success.
    Does the markdown dir only contain files ending on `.md`?=> Success.
    Are all files in the pages dir listed in the mapping table? => Success.
    Do all .mds listed in the table exist? => Success.
    Do all IDs in the table exist? => Success.

    ==========================
    3. Updating Pages:
    ==========================
    => The page: 56 <-> landing_page.md -> was up-to-date.
    => The page: 58 <-> research/cosemos.md -> was up-to-date.
    => The page: 60 <-> research/cosemos_problem_statement.md -> was up-to-date.
    => The page: 62 <-> research/julea.md -> was up-to-date.
    => The page: 517 <-> research/research.md -> was up-to-date.
    => The page: 572 <-> research/smash.md -> was up-to-date.
    Success. Bye :)

    ```
