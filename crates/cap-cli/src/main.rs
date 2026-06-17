//! `cap` CLI entrypoint (#8).
//!
//! @feature: cli
//!
//! 役割: 手元 download 済 `catalog.sqlite` (= `cap-catalog-build` の出力) を
//! read-only に query する。クエリは FTS5 (trigram name + porter doc) で一次
//! 絞り込み、最終的に symbols 表の列を JSON / table で吐く。
//!
//! - `cap search <query>` — name / doc 両方の FTS5 を OR で叩く
//! - `cap show <fq_path>` — 1 行 detail
//! - `cap features` — 全 feature タグ + 件数
//! - `cap repos` — 全 repo + 件数
//! - `cap schema` — embedded schema metadata
//!
//! schema_version が catalog.sqlite と code の `SCHEMA_VERSION` で一致しない
//! 時は **warn のみ** (= 古い CLI でも read は通す)。

#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use rusqlite::Connection;
use serde::Serialize;

const DEFAULT_DB: &str = "catalog.sqlite";

#[derive(Parser, Debug)]
#[command(
    name = "cap",
    version,
    about = "Query the cap-catalog (catalog.sqlite) from the shell",
    long_about = "cap-catalog CLI. Read-only queries over a local catalog.sqlite \
                  (downloaded from R2). schema_version 不一致時は warn のみ。"
)]
struct Cli {
    /// Path to catalog.sqlite. Default: `./catalog.sqlite`. Override with $CAP_CATALOG_DB.
    #[arg(long, env = "CAP_CATALOG_DB", default_value = DEFAULT_DB, global = true)]
    db: PathBuf,

    /// Output format. Defaults to `table` for terminals (which is just a 1-line-per-row text).
    #[arg(long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    format: OutputFormat,

    /// Limit on number of results (search). 0 = no limit.
    #[arg(long, default_value_t = 20, global = true)]
    limit: u32,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Full-text search over symbol names (trigram) and docs (porter, NL).
    Search {
        /// Query string. e.g. `auth`, `wrap fetch`, `JWT`.
        query: String,
    },
    /// Show detail for a single symbol by its fq_path.
    Show {
        /// Fully qualified symbol path (e.g. `auth_client::createAuthFetch`).
        fq_path: String,
    },
    /// List all feature tags in the catalog with hit count.
    Features,
    /// List all repos the catalog covers with symbol count.
    Repos,
    /// Print embedded schema metadata (does not need a DB).
    Schema,
}

#[derive(Serialize, Debug)]
struct SymbolRow {
    id: i64,
    repo: String,
    language: String,
    kind: String,
    name: String,
    fq_path: String,
    signature: Option<String>,
    doc: Option<String>,
    file: Option<String>,
    line: Option<i64>,
    commit_sha: Option<String>,
    features: Vec<String>,
}

fn open_db(path: &std::path::Path) -> Result<Connection, String> {
    if !path.exists() {
        return Err(format!(
            "catalog db not found: {} (set --db or $CAP_CATALOG_DB)",
            path.display()
        ));
    }
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| format!("failed to open {}: {}", path.display(), e))?;

    // schema_version mismatch は warn のみ — 古い CLI でも read は通す
    let db_version: Result<u32, _> =
        conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0));
    match db_version {
        Ok(v) if v != cap_catalog_schema::SCHEMA_VERSION => {
            eprintln!(
                "::warning::schema_version mismatch (db={}, cli={}); some queries may be wrong",
                v,
                cap_catalog_schema::SCHEMA_VERSION
            );
        }
        Err(e) => {
            eprintln!("::warning::could not read schema_version: {e}");
        }
        Ok(_) => {}
    }
    Ok(conn)
}

fn load_features(conn: &Connection, symbol_id: i64) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT feature FROM features WHERE symbol_id = ? ORDER BY feature")?;
    let rows: Vec<String> = stmt
        .query_map([symbol_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn row_to_struct(conn: &Connection, r: &rusqlite::Row) -> Result<SymbolRow, rusqlite::Error> {
    let id: i64 = r.get("id")?;
    Ok(SymbolRow {
        id,
        repo: r.get("repo")?,
        language: r.get("language")?,
        kind: r.get("kind")?,
        name: r.get("name")?,
        fq_path: r.get("fq_path")?,
        signature: r.get("signature")?,
        doc: r.get("doc")?,
        file: r.get("file")?,
        line: r.get("line")?,
        commit_sha: r.get("commit_sha")?,
        features: load_features(conn, id)?,
    })
}

fn print_rows(rows: &[SymbolRow], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let out = serde_json::to_string_pretty(rows).expect("serde");
            println!("{out}");
        }
        OutputFormat::Table => {
            for r in rows {
                let feats = if r.features.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", r.features.join(","))
                };
                let line = r.line.map(|l| format!(":{l}")).unwrap_or_default();
                let file = r.file.as_deref().unwrap_or("?");
                println!(
                    "{} {} {} {}{}\t{}\t{}{}",
                    r.repo,
                    r.language,
                    r.kind,
                    r.fq_path,
                    feats,
                    file,
                    line,
                    summary(&r.doc)
                );
            }
        }
    }
}

fn summary(doc: &Option<String>) -> String {
    match doc {
        None => String::new(),
        Some(s) => {
            let first = s.lines().next().unwrap_or("").trim();
            if first.is_empty() {
                String::new()
            } else {
                format!("\n    {first}")
            }
        }
    }
}

fn cmd_search(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<SymbolRow>, rusqlite::Error> {
    // FTS5 trigram (name) は短文字列でも hit する。
    // FTS5 porter (doc) は自然語 stem。両方 UNION して symbols から JOIN。
    let limit_clause = if limit == 0 {
        "".to_string()
    } else {
        format!(" LIMIT {limit}")
    };
    let sql = format!(
        "SELECT s.* FROM symbols s WHERE s.id IN ( \
           SELECT rowid FROM symbols_name_fts WHERE symbols_name_fts MATCH ?1 \
           UNION \
           SELECT rowid FROM symbols_doc_fts  WHERE symbols_doc_fts  MATCH ?1 \
         ) ORDER BY s.repo, s.language, s.fq_path{limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<SymbolRow> = stmt
        .query_map([query], |r| row_to_struct(conn, r))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn cmd_show(conn: &Connection, fq_path: &str) -> Result<Option<SymbolRow>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT * FROM symbols WHERE fq_path = ?")?;
    let mut hits = stmt
        .query_map([fq_path], |r| row_to_struct(conn, r))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(hits.pop())
}

#[derive(Serialize, Debug)]
struct CountRow {
    name: String,
    count: i64,
}

fn cmd_features(conn: &Connection) -> Result<Vec<CountRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT feature, COUNT(*) AS c FROM features GROUP BY feature ORDER BY c DESC, feature",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CountRow {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn cmd_repos(conn: &Connection) -> Result<Vec<CountRow>, rusqlite::Error> {
    let mut stmt = conn
        .prepare("SELECT repo, COUNT(*) AS c FROM symbols GROUP BY repo ORDER BY c DESC, repo")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CountRow {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn print_counts(rows: &[CountRow], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(rows).expect("serde"));
        }
        OutputFormat::Table => {
            for r in rows {
                println!("{}\t{}", r.count, r.name);
            }
        }
    }
}

fn run(cli: Cli) -> Result<i32, String> {
    if let Some(Command::Schema) = cli.command {
        println!("schema_version = {}", cap_catalog_schema::SCHEMA_VERSION);
        println!("ddl_bytes      = {}", cap_catalog_schema::CATALOG_SQL.len());
        return Ok(0);
    }
    match cli.command {
        None => {
            eprintln!(
                "cap {} (schema v{})",
                env!("CARGO_PKG_VERSION"),
                cap_catalog_schema::SCHEMA_VERSION
            );
            eprintln!("use `cap --help` for usage.");
            Ok(0)
        }
        Some(Command::Search { query }) => {
            let conn = open_db(&cli.db)?;
            let rows =
                cmd_search(&conn, &query, cli.limit).map_err(|e| format!("search failed: {e}"))?;
            print_rows(&rows, cli.format);
            Ok(if rows.is_empty() { 1 } else { 0 })
        }
        Some(Command::Show { fq_path }) => {
            let conn = open_db(&cli.db)?;
            let row = cmd_show(&conn, &fq_path).map_err(|e| format!("show failed: {e}"))?;
            match row {
                Some(r) => {
                    print_rows(std::slice::from_ref(&r), cli.format);
                    Ok(0)
                }
                None => {
                    eprintln!("not found: {fq_path}");
                    Ok(1)
                }
            }
        }
        Some(Command::Features) => {
            let conn = open_db(&cli.db)?;
            let rows = cmd_features(&conn).map_err(|e| format!("features failed: {e}"))?;
            print_counts(&rows, cli.format);
            Ok(0)
        }
        Some(Command::Repos) => {
            let conn = open_db(&cli.db)?;
            let rows = cmd_repos(&conn).map_err(|e| format!("repos failed: {e}"))?;
            print_counts(&rows, cli.format);
            Ok(0)
        }
        Some(Command::Schema) => unreachable!("handled above"),
    }
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn fixture_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(cap_catalog_schema::CATALOG_SQL).unwrap();
        // 2 symbols, 1 with features
        conn.execute(
            "INSERT INTO symbols (repo, language, kind, name, fq_path, doc, file, line, commit_sha) \
             VALUES ('ippoan/auth-worker', 'ts', 'fn', 'createAuthFetch', \
                     'auth_client::createAuthFetch', 'Wraps fetch with JWT refresh.', \
                     'packages/auth-client/src/fetch.ts', 42, 'abc')",
            [],
        )
        .unwrap();
        let id1 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO features (symbol_id, feature) VALUES (?, 'auth-fetch')",
            params![id1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO features (symbol_id, feature) VALUES (?, 'jwt')",
            params![id1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols (repo, language, kind, name, fq_path, doc) \
             VALUES ('ippoan/bar', 'rust', 'struct', 'Bar', 'bar::Bar', 'a struct')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn search_finds_by_trigram_name_and_porter_doc() {
        let conn = fixture_db();

        // trigram substring (`auth` in createAuthFetch)
        let rows = cmd_search(&conn, "auth", 20).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "createAuthFetch");
        assert_eq!(rows[0].features, vec!["auth-fetch", "jwt"]);

        // porter stem (`wrap` matches `Wraps`)
        let rows = cmd_search(&conn, "wrap", 20).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "createAuthFetch");

        // no hit
        let rows = cmd_search(&conn, "zzznonexistent", 20).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn search_limit_applies() {
        let conn = fixture_db();
        // doc 'a' would match too many porter stems; use trigram which is exact
        let rows = cmd_search(&conn, "Bar", 1).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn show_returns_one_or_none() {
        let conn = fixture_db();
        let row = cmd_show(&conn, "auth_client::createAuthFetch").unwrap();
        assert!(row.is_some());
        assert_eq!(row.unwrap().name, "createAuthFetch");
        assert!(cmd_show(&conn, "nope::nope").unwrap().is_none());
    }

    #[test]
    fn features_counts() {
        let conn = fixture_db();
        let rows = cmd_features(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.count == 1));
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"auth-fetch"));
        assert!(names.contains(&"jwt"));
    }

    #[test]
    fn repos_counts() {
        let conn = fixture_db();
        let rows = cmd_repos(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"ippoan/auth-worker"));
        assert!(names.contains(&"ippoan/bar"));
    }
}
