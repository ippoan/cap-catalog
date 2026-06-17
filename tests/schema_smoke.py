"""Schema smoke test.

`schema/catalog.sql` を in-memory SQLite に読ませて、

  1. 全 object (symbols / features / FTS5 / triggers / indexes) が生成される
  2. symbols → features 1:n 引きが効く
  3. INSERT trigger で symbols_name_fts (trigram) / symbols_doc_fts (porter)
     に同期される

までを 1 ファイルで検証する。CI で `python3 tests/schema_smoke.py` から呼ぶ。
"""

from __future__ import annotations

import pathlib
import sqlite3
import sys

SCHEMA = pathlib.Path(__file__).resolve().parent.parent / "schema" / "catalog.sql"


def run() -> None:
    conn = sqlite3.connect(":memory:")
    conn.executescript(SCHEMA.read_text())

    [(version,)] = conn.execute("SELECT version FROM schema_version").fetchall()
    assert version == 1, f"expected schema_version=1, got {version}"

    conn.execute(
        "INSERT INTO symbols(repo, language, kind, name, fq_path, signature, doc, "
        "file, line, commit_sha) VALUES (?,?,?,?,?,?,?,?,?,?)",
        (
            "ippoan/auth-worker",
            "ts",
            "fn",
            "createAuthFetch",
            "auth-client::createAuthFetch",
            "(opts: Opts) => Fetch",
            "Wraps fetch with auth-worker JWT refresh.",
            "packages/auth-client/src/fetch.ts",
            42,
            "abc123",
        ),
    )
    symbol_id = conn.execute("SELECT id FROM symbols WHERE name='createAuthFetch'").fetchone()[0]
    conn.execute("INSERT INTO features(symbol_id, feature) VALUES (?, ?)", (symbol_id, "auth-fetch"))
    conn.commit()

    # FTS5 trigram (substring) — identifier 部分一致
    rows = conn.execute(
        "SELECT s.name FROM symbols_name_fts f JOIN symbols s ON s.id=f.rowid "
        "WHERE symbols_name_fts MATCH ?",
        ("auth",),
    ).fetchall()
    assert rows == [("createAuthFetch",)], f"trigram 'auth' match failed: {rows}"

    # FTS5 porter (NL) — 自然語ステミング
    rows = conn.execute(
        "SELECT s.name FROM symbols_doc_fts f JOIN symbols s ON s.id=f.rowid "
        "WHERE symbols_doc_fts MATCH ?",
        ("wrap",),  # porter は 'wraps' を 'wrap' にステミング
    ).fetchall()
    assert rows == [("createAuthFetch",)], f"porter 'wrap' match failed: {rows}"

    # feature 引き
    rows = conn.execute(
        "SELECT s.name FROM features f JOIN symbols s ON s.id=f.symbol_id "
        "WHERE f.feature=?",
        ("auth-fetch",),
    ).fetchall()
    assert rows == [("createAuthFetch",)], f"feature lookup failed: {rows}"

    # UPDATE で FTS が再同期されること
    conn.execute("UPDATE symbols SET name='createAuthClient' WHERE id=?", (symbol_id,))
    conn.commit()
    rows = conn.execute(
        "SELECT s.name FROM symbols_name_fts f JOIN symbols s ON s.id=f.rowid "
        "WHERE symbols_name_fts MATCH ?",
        ("Client",),
    ).fetchall()
    assert rows == [("createAuthClient",)], f"trigram after UPDATE failed: {rows}"

    # DELETE で FTS から消えること
    conn.execute("DELETE FROM symbols WHERE id=?", (symbol_id,))
    conn.commit()
    rows = conn.execute(
        "SELECT s.name FROM symbols_name_fts f JOIN symbols s ON s.id=f.rowid "
        "WHERE symbols_name_fts MATCH ?",
        ("auth",),
    ).fetchall()
    assert rows == [], f"trigram after DELETE not cleared: {rows}"

    print("OK: schema smoke test passed (schema_version=1)")


if __name__ == "__main__":
    try:
        run()
    except AssertionError as e:
        print(f"FAIL: {e}", file=sys.stderr)
        sys.exit(1)
