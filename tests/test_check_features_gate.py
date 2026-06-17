"""Unit tests for scripts/check-features-gate.py.

ファイル内容を tmp 経由で食わせ、検出ロジックを gate する (CI で nightly Rust
不要、テストのみで完結)。
"""

from __future__ import annotations

import importlib.util
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "check-features-gate.py"

spec = importlib.util.spec_from_file_location("check_features_gate", SCRIPT)
assert spec is not None and spec.loader is not None
mod = importlib.util.module_from_spec(spec)
sys.modules["check_features_gate"] = mod
spec.loader.exec_module(mod)


def write(tmpdir: pathlib.Path, rel: str, content: str) -> pathlib.Path:
    p = tmpdir / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content, encoding="utf-8")
    return p


def test_lib_root_with_feature_passes():
    src = (
        "//! Crate root.\n"
        "//!\n"
        "//! @feature: foo-cap\n"
        "\n"
        "#![forbid(unsafe_code)]\n"
        "\n"
        "/// Public bar.\n"
        "///\n"
        "/// @feature: foo-cap\n"
        "pub fn bar() {}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        assert mod.scan_file(p) == []


def test_lib_root_missing_inner_feature_is_caught():
    src = (
        "//! Crate root with no feature tag.\n"
        "\n"
        "/// Public bar.\n"
        "///\n"
        "/// @feature: foo-cap\n"
        "pub fn bar() {}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        misses = mod.scan_file(p)
        assert any("crate root" in m[1] for m in misses)


def test_pub_item_missing_feature_is_caught():
    src = (
        "//! @feature: root-cap\n"
        "\n"
        "/// Public fn without @feature.\n"
        "pub fn bar() {}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        misses = mod.scan_file(p)
        # 1 件: pub fn bar
        assert len(misses) == 1
        assert "pub fn bar" in misses[0][1]


def test_pub_use_is_skipped():
    src = "//! @feature: root\n\npub use crate::foo::Bar;\n"
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        assert mod.scan_file(p) == []


def test_pub_mod_declaration_is_skipped():
    """`pub mod foo;` は宣言だけ。`foo.rs` 側で gate されるのでここでは skip。"""
    src = "//! @feature: root\n\npub mod foo;\npub mod bar;\n"
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        assert mod.scan_file(p) == []


def test_pub_fn_main_in_main_rs_is_skipped():
    """`pub fn main(...)` は binary entrypoint。crate root の @feature: で表現する。"""
    src = "//! @feature: cli-app\n\npub fn main() {}\n"
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "main.rs", src)
        assert mod.scan_file(p) == []


def test_attribute_between_doc_and_pub_is_ok():
    """doc-comment と pub の間に `#[derive(...)]` 等が挟まっても block 内とみなす。"""
    src = (
        "//! @feature: root\n"
        "\n"
        "/// Public S.\n"
        "/// @feature: root\n"
        "#[derive(Debug, Clone)]\n"
        "pub struct S;\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        assert mod.scan_file(p) == []


def test_case_insensitive_feature_tag():
    src = (
        "//! @FEATURE: root-cap\n"
        "\n"
        "/// Public bar.\n"
        "/// @Feature: bar-cap\n"
        "pub fn bar() {}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        assert mod.scan_file(p) == []


def test_blank_line_between_doc_and_pub_breaks_block():
    """rustdoc は空行で doc-block を切る。gate もそれに従う。"""
    src = (
        "//! @feature: root\n"
        "\n"
        "/// stale doc with @feature: orphan\n"
        "\n"
        "pub fn bar() {}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        misses = mod.scan_file(p)
        assert len(misses) == 1
        assert "pub fn bar" in misses[0][1]


def test_pub_const_struct_enum_trait_all_gated():
    """全 supported kind が gate 対象であること。"""
    src = (
        "//! @feature: root\n"
        "\n"
        "pub const C: u32 = 1;\n"           # no doc
        "pub struct S;\n"                    # no doc
        "pub enum E { A }\n"                 # no doc
        "pub trait T {}\n"                   # no doc
        "pub type Alias = u32;\n"            # no doc
    )
    with tempfile.TemporaryDirectory() as td:
        p = write(pathlib.Path(td), "lib.rs", src)
        misses = mod.scan_file(p)
        assert len(misses) == 5
        kinds = [m[1] for m in misses]
        assert any("pub const" in k for k in kinds)
        assert any("pub struct" in k for k in kinds)
        assert any("pub enum" in k for k in kinds)
        assert any("pub trait" in k for k in kinds)
        assert any("pub type" in k for k in kinds)


if __name__ == "__main__":
    fails = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"PASS {name}")
            except AssertionError as e:
                fails += 1
                print(f"FAIL {name}: {e}", file=sys.stderr)
            except Exception as e:
                fails += 1
                print(f"ERROR {name}: {e}", file=sys.stderr)
    if fails:
        print(f"{fails} test(s) failed", file=sys.stderr)
        sys.exit(1)
    print("all check-features-gate tests passed")
