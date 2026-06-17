//! `cap` CLI entrypoint (v0.1 scaffold).
//!
//! 実装は #8 で進める。本コミット時点では `--help` / `--version` / subcommand
//! stub のみで、`cargo build --release` + smoke test (`./cap --help`) を
//! 通すための最小骨格。

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "cap",
    version,
    about = "Query the cap-catalog (catalog.sqlite) from the shell",
    long_about = "cap-catalog CLI. Read-only queries over a local catalog.sqlite \
                  (downloaded from R2). schema_version 不一致時は warn のみ。"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Full-text search over symbol names (trigram) and docs (porter).
    Search {
        /// Query string.
        query: String,
    },
    /// Show detail for a single symbol by its fq_path.
    Show {
        /// Fully qualified symbol path (e.g. `auth-client::createAuthFetch`).
        fq_path: String,
    },
    /// List all feature tags in the catalog.
    Features,
    /// List all repos the catalog covers.
    Repos,
    /// Print embedded schema metadata.
    Schema,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            eprintln!(
                "cap {} (schema v{})",
                env!("CARGO_PKG_VERSION"),
                cap_catalog_schema::SCHEMA_VERSION
            );
            eprintln!("use `cap --help` for usage.");
        }
        Some(Command::Schema) => {
            println!("schema_version = {}", cap_catalog_schema::SCHEMA_VERSION);
            println!("ddl_bytes      = {}", cap_catalog_schema::CATALOG_SQL.len());
        }
        Some(Command::Search { query: _ })
        | Some(Command::Show { fq_path: _ })
        | Some(Command::Features)
        | Some(Command::Repos) => {
            eprintln!("not implemented yet (Refs ippoan/cap-catalog#8)");
            std::process::exit(2);
        }
    }
}
