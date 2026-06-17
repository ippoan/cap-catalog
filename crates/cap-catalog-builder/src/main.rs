//! `cap-catalog-build`: catalog-extract.jsonl の集合から catalog.sqlite を build する。
//!
//! Refs ippoan/cap-catalog#7.
//!
//! 入力: 1 つ以上の `*.jsonl` ファイル (ci-workflows `catalog-extract.yml` artifact
//! から download した形)。各行は cap-catalog-schema が定める JSONL contract:
//!
//! ```json
//! {
//!   "repo": "ippoan/auth-worker",
//!   "language": "rust",
//!   "kind": "fn",
//!   "name": "createAuthFetch",
//!   "fq_path": "auth_client::createAuthFetch",
//!   "signature": null,
//!   "doc": "...",
//!   "file": "src/lib.rs",
//!   "line": 42,
//!   "commit_sha": "abc123",
//!   "features": ["auth-fetch"]
//! }
//! ```
//!
//! 出力: `--out <path>` で指定した sqlite ファイル。schema は
//! `cap_catalog_schema::CATALOG_SQL` を `execute_batch` で適用 (idempotent)。
//! 同一 `(repo, language, fq_path)` は UPSERT (= UNIQUE で 1 行に統合)、features は
//! `INSERT OR IGNORE` で m:n に積む。
//!
//! Build summary を stderr に書く (= CI log に残る)。

#![forbid(unsafe_code)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use clap::Parser;
use rusqlite::{params, Connection};
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(
    name = "cap-catalog-build",
    version,
    about = "Build catalog.sqlite from catalog-extract.jsonl streams"
)]
struct Cli {
    /// Output catalog.sqlite path. Created (overwritten) if exists.
    #[arg(long)]
    out: PathBuf,

    /// Input catalog-extract.jsonl files. Order does not matter; UPSERT dedupes
    /// across files. At least one is required.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
}

#[derive(Deserialize, Debug)]
struct Row {
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
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Default, Debug)]
struct Stats {
    files_read: usize,
    lines_parsed: usize,
    rows_upserted: usize,
    features_inserted: usize,
    parse_errors: usize,
}

fn build_db(out: &Path, inputs: &[PathBuf]) -> rusqlite::Result<Stats> {
    if out.exists() {
        std::fs::remove_file(out).expect("failed to remove existing output file");
    }
    let mut conn = Connection::open(out)?;
    conn.execute_batch(cap_catalog_schema::CATALOG_SQL)?;

    let mut stats = Stats::default();

    let tx = conn.transaction()?;
    {
        let mut stmt_sym = tx.prepare(
            "INSERT INTO symbols (repo, language, kind, name, fq_path, signature, doc, file, line, commit_sha) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(repo, language, fq_path) DO UPDATE SET \
               kind = excluded.kind, \
               name = excluded.name, \
               signature = excluded.signature, \
               doc = excluded.doc, \
               file = excluded.file, \
               line = excluded.line, \
               commit_sha = excluded.commit_sha \
             RETURNING id",
        )?;
        let mut stmt_feat =
            tx.prepare("INSERT OR IGNORE INTO features (symbol_id, feature) VALUES (?1, ?2)")?;

        for input in inputs {
            stats.files_read += 1;
            let file = File::open(input).expect("failed to open input file");
            for (lineno, line) in BufReader::new(file).lines().enumerate() {
                let line = line.expect("failed to read line");
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                stats.lines_parsed += 1;

                let row: Row = match serde_json::from_str(line) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "::warning::{}:{}: invalid JSON: {}",
                            input.display(),
                            lineno + 1,
                            e
                        );
                        stats.parse_errors += 1;
                        continue;
                    }
                };

                let id: i64 = stmt_sym.query_row(
                    params![
                        row.repo,
                        row.language,
                        row.kind,
                        row.name,
                        row.fq_path,
                        row.signature,
                        row.doc,
                        row.file,
                        row.line,
                        row.commit_sha,
                    ],
                    |r| r.get(0),
                )?;
                stats.rows_upserted += 1;

                for feat in &row.features {
                    let changed = stmt_feat.execute(params![id, feat])?;
                    if changed > 0 {
                        stats.features_inserted += 1;
                    }
                }
            }
        }
    }
    tx.commit()?;

    Ok(stats)
}

fn main() -> rusqlite::Result<()> {
    let cli = Cli::parse();
    let stats = build_db(&cli.out, &cli.inputs)?;
    eprintln!(
        "cap-catalog-build: files={} lines={} upserted={} features={} parse_errors={} → {}",
        stats.files_read,
        stats.lines_parsed,
        stats.rows_upserted,
        stats.features_inserted,
        stats.parse_errors,
        cli.out.display(),
    );
    if stats.parse_errors > 0 {
        std::process::exit(2);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cap-catalog-builder-test-{}-{}-{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        p
    }

    fn write_jsonl(path: &Path, lines: &[&str]) {
        std::fs::write(path, lines.join("\n")).unwrap();
    }

    #[test]
    fn build_inserts_symbols_and_features() {
        let in_path = tmp_path("input.jsonl");
        let out_path = tmp_path("catalog.sqlite");

        write_jsonl(
            &in_path,
            &[
                r#"{"repo":"ippoan/foo","language":"rust","kind":"fn","name":"createAuthFetch","fq_path":"foo::createAuthFetch","doc":"Wraps fetch.","file":"src/lib.rs","line":42,"commit_sha":"abc","features":["auth-fetch","fetch"]}"#,
                r#"{"repo":"ippoan/bar","language":"ts","kind":"class","name":"Client","fq_path":"bar.Client","features":[]}"#,
            ],
        );

        let stats = build_db(&out_path, std::slice::from_ref(&in_path)).unwrap();
        assert_eq!(stats.files_read, 1);
        assert_eq!(stats.lines_parsed, 2);
        assert_eq!(stats.rows_upserted, 2);
        assert_eq!(stats.features_inserted, 2);
        assert_eq!(stats.parse_errors, 0);

        let conn = Connection::open(&out_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        let feats: Vec<String> = conn
            .prepare(
                "SELECT f.feature FROM features f \
                 JOIN symbols s ON s.id = f.symbol_id \
                 WHERE s.fq_path = 'foo::createAuthFetch' ORDER BY f.feature",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(feats, vec!["auth-fetch", "fetch"]);

        // FTS5 trigram lookup works
        let hit: String = conn
            .query_row(
                "SELECT s.name FROM symbols_name_fts f JOIN symbols s ON s.id=f.rowid \
                 WHERE symbols_name_fts MATCH ?",
                ["auth"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, "createAuthFetch");

        std::fs::remove_file(in_path).ok();
        std::fs::remove_file(out_path).ok();
    }

    #[test]
    fn upsert_dedupes_across_files() {
        let a = tmp_path("a.jsonl");
        let b = tmp_path("b.jsonl");
        let out_path = tmp_path("dup.sqlite");

        let same = r#"{"repo":"ippoan/foo","language":"rust","kind":"fn","name":"foo","fq_path":"foo::foo","doc":"old doc","features":["a"]}"#;
        let updated = r#"{"repo":"ippoan/foo","language":"rust","kind":"fn","name":"foo","fq_path":"foo::foo","doc":"new doc","features":["a","b"]}"#;
        write_jsonl(&a, &[same]);
        write_jsonl(&b, &[updated]);

        let stats = build_db(&out_path, &[a.clone(), b.clone()]).unwrap();
        assert_eq!(stats.files_read, 2);
        assert_eq!(stats.lines_parsed, 2);
        assert_eq!(stats.rows_upserted, 2); // both INSERTs counted; UPSERT path
        assert_eq!(stats.features_inserted, 2); // 'a' once, 'b' once (a is INSERT OR IGNORE)

        let conn = Connection::open(&out_path).unwrap();
        // Only 1 row in symbols (UNIQUE constraint)
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        // doc is updated to the newer value
        let doc: String = conn
            .query_row("SELECT doc FROM symbols", [], |r| r.get(0))
            .unwrap();
        assert_eq!(doc, "new doc");
        let feats: Vec<String> = conn
            .prepare("SELECT feature FROM features ORDER BY feature")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(feats, vec!["a", "b"]);

        std::fs::remove_file(a).ok();
        std::fs::remove_file(b).ok();
        std::fs::remove_file(out_path).ok();
    }

    #[test]
    fn parse_errors_are_counted_not_fatal() {
        let in_path = tmp_path("bad.jsonl");
        let out_path = tmp_path("bad.sqlite");
        write_jsonl(
            &in_path,
            &[
                r#"{"repo":"ippoan/x","language":"rust","kind":"fn","name":"ok","fq_path":"x::ok"}"#,
                "not json at all",
                "",
                r#"{"repo":"ippoan/x","language":"rust","kind":"fn","name":"ok2","fq_path":"x::ok2"}"#,
            ],
        );
        let stats = build_db(&out_path, std::slice::from_ref(&in_path)).unwrap();
        assert_eq!(stats.lines_parsed, 3); // empty line skipped
        assert_eq!(stats.parse_errors, 1);
        assert_eq!(stats.rows_upserted, 2);

        std::fs::remove_file(in_path).ok();
        std::fs::remove_file(out_path).ok();
    }
}
