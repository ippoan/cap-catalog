-- catalog.sqlite schema v1
-- Refs ippoan/cap-catalog#2
--
-- 1 row per extracted symbol (function / struct / class / interface / type / lib).
-- Features (= source @feature: tags) are many-to-many.
-- FTS5: name=trigram (substring search), doc=porter unicode61 (NL search).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_version (
  version    INTEGER PRIMARY KEY,
  applied_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);

CREATE TABLE IF NOT EXISTS symbols (
  id         INTEGER PRIMARY KEY,
  repo       TEXT NOT NULL,
  language   TEXT NOT NULL CHECK (language IN ('rust', 'ts', 'js', 'go')),
  kind       TEXT NOT NULL,
  name       TEXT NOT NULL,
  fq_path    TEXT NOT NULL,
  signature  TEXT,
  doc        TEXT,
  file       TEXT,
  line       INTEGER,
  commit_sha TEXT,
  UNIQUE (repo, language, fq_path)
);

CREATE INDEX IF NOT EXISTS idx_symbols_repo ON symbols(repo);
CREATE INDEX IF NOT EXISTS idx_symbols_lang ON symbols(language);
CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);

CREATE TABLE IF NOT EXISTS features (
  symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
  feature   TEXT NOT NULL,
  PRIMARY KEY (symbol_id, feature)
);

CREATE INDEX IF NOT EXISTS idx_features_feature ON features(feature);

CREATE VIRTUAL TABLE IF NOT EXISTS symbols_name_fts USING fts5(
  name,
  content='symbols',
  content_rowid='id',
  tokenize='trigram'
);

CREATE VIRTUAL TABLE IF NOT EXISTS symbols_doc_fts USING fts5(
  doc,
  content='symbols',
  content_rowid='id',
  tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS symbols_ai AFTER INSERT ON symbols BEGIN
  INSERT INTO symbols_name_fts(rowid, name) VALUES (new.id, new.name);
  INSERT INTO symbols_doc_fts(rowid, doc) VALUES (new.id, coalesce(new.doc, ''));
END;

CREATE TRIGGER IF NOT EXISTS symbols_ad AFTER DELETE ON symbols BEGIN
  INSERT INTO symbols_name_fts(symbols_name_fts, rowid, name) VALUES ('delete', old.id, old.name);
  INSERT INTO symbols_doc_fts(symbols_doc_fts, rowid, doc) VALUES ('delete', old.id, coalesce(old.doc, ''));
END;

CREATE TRIGGER IF NOT EXISTS symbols_au AFTER UPDATE ON symbols BEGIN
  INSERT INTO symbols_name_fts(symbols_name_fts, rowid, name) VALUES ('delete', old.id, old.name);
  INSERT INTO symbols_doc_fts(symbols_doc_fts, rowid, doc) VALUES ('delete', old.id, coalesce(old.doc, ''));
  INSERT INTO symbols_name_fts(rowid, name) VALUES (new.id, new.name);
  INSERT INTO symbols_doc_fts(rowid, doc) VALUES (new.id, coalesce(new.doc, ''));
END;
