"""Exercise the commit guard against a real temporary Git index."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECKER = Path(__file__).with_name("check_unstaged_code.py").resolve()


class CommitCompletenessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        self.git("init", "-q")
        (self.root / "src").mkdir()
        (self.root / "src/main.rs").write_text("fn main() {}\n")
        (self.root / ".gitignore").write_text("target/\n*.log\n")
        self.git("add", ".")

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, env=self.env,
                              check=True, capture_output=True)

    def check(self):
        return subprocess.run([sys.executable, str(CHECKER)], cwd=self.root,
                              env=self.env, capture_output=True, text=True)

    def test_complete_index_passes(self):
        self.assertEqual(self.check().returncode, 0)

    def test_untracked_source_blocks_and_staging_fixes_it(self):
        (self.root / "src/new file.rs").write_text("// new\n")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("src/new file.rs", result.stderr)
        self.git("add", "src/new file.rs")
        self.assertEqual(self.check().returncode, 0)

    def test_partial_edit_and_unstaged_deletion_block(self):
        source = self.root / "src/main.rs"
        source.write_text("fn main() { println!(\"hello\"); }\n")
        self.assertEqual(self.check().returncode, 1)
        self.git("add", "src/main.rs")
        source.unlink()
        self.assertEqual(self.check().returncode, 1)
        self.git("add", "-u")
        self.assertEqual(self.check().returncode, 0)

    def test_dependency_translation_and_hook_are_code(self):
        for name in ("Cargo.toml", "i18n/it.json", ".githooks/pre-commit"):
            p = self.root / name
            p.parent.mkdir(exist_ok=True)
            p.write_text("test\n")
            self.assertEqual(self.check().returncode, 1, name)
            self.git("add", name)
        self.assertEqual(self.check().returncode, 0)

    def test_docs_and_ignored_build_output_do_not_block(self):
        (self.root / "notes.md").write_text("personal notes\n")
        (self.root / "target").mkdir()
        (self.root / "target/generated.rs").write_text("// generated\n")
        (self.root / "build.log").write_text("build log\n")
        self.assertEqual(self.check().returncode, 0)


if __name__ == "__main__":
    unittest.main()
