from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.quality.analyzer import AnalysisError, analyze
from scripts.quality.complexity import LIMIT, load_allowlist, run
from test_analyzer import decisions


LEGACY_PATH = "legacy.ts"
LEGACY_NAME = "legacy"
RENAMED_NAME = "renamed"
VIOLATING_SCORE = LIMIT + 1
FAILURE_EXIT = 1
BASELINE_REF = "refs/remotes/origin/develop"
CONSTANT_DECLARATION = "const marker = 1;\n"
ARROW_TEMPLATE = "const {name} =\n() => {{\n{body}\n}};\n"
RUST_CONSTANT_DECLARATION = "const MARKER: usize = 1;\n"
MOVEMENT_FIXTURES = (
    ("legacy.ts", CONSTANT_DECLARATION, False),
    ("legacy.tsx", CONSTANT_DECLARATION, False),
    ("legacy.rs", RUST_CONSTANT_DECLARATION, True),
)
SUCCESS_EXIT = 0
DIAGNOSTIC_TEMPLATE = "{name}: observed {score}, limit {limit}"


def function(name="legacy", score=11, rust=False):
    return f"fn {name}() {{\n{decisions(score - 1, True)}\n}}\n" if rust else f"function {name}() {{\n{decisions(score - 1)}\n}}\n"


class GitGateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-b", "develop")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Gate fixture")
        self.write("legacy.ts", function() + "\nfunction small() {\n return 0;\n}\n")
        self.write("scripts/complexity-allowlist.json", "[]")
        self.git("add", ".")
        self.git("commit", "-m", "baseline")
        self.git("update-ref", "refs/remotes/origin/develop", "HEAD")
        self.git("checkout", "-b", "feature")

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.root, capture_output=True)
        if result.returncode:
            self.fail(result.stderr.decode(errors="replace"))
        return result.stdout.decode().strip()

    def write(self, path, content):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")

    def verify(self, expected, base="origin/develop"):
        output = io.StringIO()
        with redirect_stdout(output), redirect_stderr(output):
            result = run(self.root, base, self.root / "scripts/complexity-allowlist.json")
        self.assertEqual(result, expected, output.getvalue())
        return output.getvalue()

    def test_unchanged_legacy_checked_when_other_function_in_file_changes(self):
        source = (self.root / "legacy.ts").read_text().replace("return 0;", "return 1;")
        self.write("legacy.ts", source)
        output = self.verify(FAILURE_EXIT)
        self.assertIn(DIAGNOSTIC_TEMPLATE.format(
            name=LEGACY_NAME, score=VIOLATING_SCORE, limit=LIMIT), output)

    def test_unchanged_file_ignored_when_other_file_changes(self):
        self.write("other.ts", function(RENAMED_NAME, LIMIT))
        self.verify(SUCCESS_EXIT)

    def test_editing_legacy_even_only_comment_fails(self):
        self.write("legacy.ts", function().replace("if (ready)", "if (ready) /* touched */", 1))
        output = self.verify(1)
        self.assertIn("legacy.ts:1 legacy: observed 11, limit 10", output)

    def test_committed_staged_unstaged_and_untracked_functions_are_checked(self):
        for filename, action in [("committed.ts", "commit"), ("staged.tsx", "stage"),
                                 ("unstaged.rs", "unstaged"), ("untracked.ts", "untracked")]:
            with self.subTest(action=action):
                self.write(filename, function("work", rust=filename.endswith(".rs")))
                if action in {"commit", "stage", "unstaged"}:
                    if action == "unstaged":
                        self.write(filename, function("work", 10, True))
                    self.git("add", filename)
                if action == "commit":
                    self.git("commit", "-m", "branch change")
                if action == "unstaged":
                    self.write(filename, function("work", 11, True))
                self.assertIn(filename, self.verify(1))

    def test_index_cannot_be_hidden_by_unstaged_reversion(self):
        self.write("new.ts", function("work"))
        self.git("add", "new.ts")
        self.write("new.ts", function("work", 10))
        self.assertIn("[index]", self.verify(1))

    def test_ten_passes_eleven_fails_in_ts_tsx_and_rust(self):
        for suffix in ("ts", "tsx", "rs"):
            filename = "boundary." + suffix
            self.write(filename, function("work", 10, suffix == "rs"))
            self.verify(0)
            self.write(filename, function("work", 11, suffix == "rs"))
            self.verify(1)
            (self.root / filename).unlink()

    def test_rename_rechecks_all_functions(self):
        self.git("mv", "legacy.ts", "renamed.ts")
        self.assertIn("renamed.ts", self.verify(1))

    def test_move_into_new_file_rechecks_function(self):
        self.write("legacy.ts", "function small() { return 0; }\n")
        self.write("moved.ts", function())
        self.assertIn("moved.ts", self.verify(1))

    def test_move_within_file_rechecks_even_largest_aligned_body(self):
        self.write("legacy.ts", "function small() {\n return 0;\n}\n\n" + function())
        self.verify(1)

    def test_move_across_constant_rechecks_unchanged_function(self):
        self.write(LEGACY_PATH, function(LEGACY_NAME, LIMIT))
        for path, constant, rust in MOVEMENT_FIXTURES:
            legacy = function(LEGACY_NAME, VIOLATING_SCORE, rust)
            for before, after in ((constant + legacy, legacy + constant),
                                  (legacy + constant, constant + legacy)):
                with self.subTest(path=path, before=before):
                    self.write(path, before)
                    self.git("add", ".")
                    self.git("commit", "-m", "constant movement baseline")
                    self.git("update-ref", BASELINE_REF, "HEAD")
                    self.write(path, after)
                    output = self.verify(FAILURE_EXIT)
                    self.assertIn(DIAGNOSTIC_TEMPLATE.format(
                        name=LEGACY_NAME, score=VIOLATING_SCORE, limit=LIMIT), output)

    def test_inserting_and_deleting_constant_rechecks_whole_file(self):
        legacy = function(LEGACY_NAME, VIOLATING_SCORE)
        self.write(LEGACY_PATH, legacy)
        self.git("add", LEGACY_PATH)
        self.git("commit", "-m", "function without constant")
        self.git("update-ref", BASELINE_REF, "HEAD")
        self.write(LEGACY_PATH, CONSTANT_DECLARATION + legacy)
        self.verify(FAILURE_EXIT)
        self.git("add", LEGACY_PATH)
        self.git("commit", "-m", "constant insertion baseline")
        self.git("update-ref", BASELINE_REF, "HEAD")
        self.write(LEGACY_PATH, legacy)
        self.verify(FAILURE_EXIT)

    def test_renaming_multiline_arrow_binding_rechecks_unchanged_body(self):
        legacy = ARROW_TEMPLATE.format(name=LEGACY_NAME, body=decisions(VIOLATING_SCORE - 1))
        self.write(LEGACY_PATH, legacy)
        self.git("add", LEGACY_PATH)
        self.git("commit", "-m", "multiline arrow baseline")
        self.git("update-ref", BASELINE_REF, "HEAD")
        self.write(LEGACY_PATH, legacy.replace(LEGACY_NAME, RENAMED_NAME))
        output = self.verify(FAILURE_EXIT)
        self.assertIn(f"{RENAMED_NAME}: observed {VIOLATING_SCORE}, limit {LIMIT}", output)

    def test_inserting_comment_before_legacy_rechecks_whole_file(self):
        original = (self.root / "legacy.ts").read_text()
        self.write("legacy.ts", "// a module comment\n" + original)
        self.verify(FAILURE_EXIT)

    def test_deleted_function_and_deleted_file_do_not_fail(self):
        self.write("legacy.ts", "function small() { return 0; }\n")
        self.verify(0)
        (self.root / "legacy.ts").unlink()
        self.verify(0)

    def test_deleting_branch_within_function_rechecks_remaining_score(self):
        self.write("legacy.ts", function(score=12))
        self.git("add", "legacy.ts")
        self.git("commit", "-m", "update baseline")
        self.git("update-ref", "refs/remotes/origin/develop", "HEAD")
        self.write("legacy.ts", function(score=11))
        self.verify(1)

    def test_missing_base_and_unsupported_syntax_fail(self):
        with self.assertRaises(AnalysisError):
            self.verify(0, "missing-base")
        self.write("broken.ts", "function work( {")
        with self.assertRaisesRegex(AnalysisError, "mapping cannot be determined"):
            self.verify(0)

    def test_explicit_base_override(self):
        self.verify(0, "develop")

    def exception(self, **overrides):
        entry = {"path": "legacy.ts", "function": "legacy", "max_score": 11,
                 "reason": "Fixture approval for one specific function",
                 "approval": "https://github.com/Dzahc/MacroLoom/issues/25#issuecomment-123"}
        entry.update(overrides)
        self.write("scripts/complexity-allowlist.json", json.dumps([entry]))

    def test_approved_exception_permits_only_intended_function_and_score(self):
        self.exception()
        self.write("legacy.ts", function())
        self.verify(0)
        self.write("other.ts", function("other"))
        self.assertIn("other.ts", self.verify(1))
        (self.root / "other.ts").unlink()
        self.write("legacy.ts", function(score=12))
        self.assertIn("approved limit 11", self.verify(1))

    def test_stale_function_and_resolved_exception_fail(self):
        self.exception()
        self.write("legacy.ts", function(score=10))
        self.assertIn("stale exception", self.verify(1))
        (self.root / "legacy.ts").unlink()
        self.assertIn("stale exception", self.verify(1))

    def test_broad_unreviewed_and_invalid_exceptions_fail(self):
        for invalid in [{"path": "*.ts"}, {"function": "*"}, {"reason": ""},
                        {"approval": "approved"}, {"max_score": 10}, {"max_score": True}]:
            with self.subTest(invalid=invalid):
                self.exception(**invalid)
                with self.assertRaises(AnalysisError):
                    load_allowlist(self.root / "scripts/complexity-allowlist.json")

    def test_dependencies_and_generated_output_excluded(self):
        self.write("node_modules/vendor.ts", function())
        self.write("app/src-tauri/target/generated.rs", function(rust=True))
        self.write("app/src-tauri/gen/schemas/generated.ts", function())
        self.verify(0)


if __name__ == "__main__":
    unittest.main()
