//! `catalog.sqlite` schema DDL + version constant.
//!
//! `schema/catalog.sql` の中身を `include_str!` で持ち、builder / CLI / UI が
//! 同一バイナリリテラルを共有する。`SCHEMA_VERSION` は schema 内の
//! `INSERT INTO schema_version VALUES (1)` と整合する。

#![forbid(unsafe_code)]

/// catalog.sqlite が require する schema version。
///
/// CLI 起動時に `SELECT version FROM schema_version` の値と比較し、低ければ
/// `warn` する。
pub const SCHEMA_VERSION: u32 = 1;

/// catalog.sqlite を build する DDL (1 ファイル、idempotent)。
///
/// `executescript` で読ませる前提。トリガーで FTS5 を symbols テーブルに
/// 同期する。
pub const CATALOG_SQL: &str = include_str!("../../../schema/catalog.sql");

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn ddl_loads_and_version_matches() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CATALOG_SQL).unwrap();
        let version: u32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn fts5_trigram_substring_match() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CATALOG_SQL).unwrap();
        conn.execute(
            "INSERT INTO symbols(repo, language, kind, name, fq_path) VALUES (?,?,?,?,?)",
            ["r", "ts", "fn", "createAuthFetch", "a::createAuthFetch"],
        )
        .unwrap();
        let name: String = conn
            .query_row(
                "SELECT s.name FROM symbols_name_fts f JOIN symbols s ON s.id=f.rowid \
                 WHERE symbols_name_fts MATCH ?",
                ["auth"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "createAuthFetch");
    }

    #[test]
    fn features_many_to_many() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CATALOG_SQL).unwrap();
        conn.execute(
            "INSERT INTO symbols(repo, language, kind, name, fq_path) VALUES (?,?,?,?,?)",
            ["r", "rust", "fn", "rollcall", "tenko::rollcall"],
        )
        .unwrap();
        let id: i64 = conn.last_insert_rowid();
        for feat in ["tenko-rollcall", "auth-fetch"] {
            conn.execute(
                "INSERT INTO features(symbol_id, feature) VALUES (?, ?)",
                rusqlite::params![id, feat],
            )
            .unwrap();
        }
        let mut stmt = conn
            .prepare("SELECT feature FROM features WHERE symbol_id = ? ORDER BY feature")
            .unwrap();
        let feats: Vec<String> = stmt
            .query_map([id], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(feats, vec!["auth-fetch", "tenko-rollcall"]);
    }
}
