"""Reject incomplete code commits without staging or changing any files."""
import os
from pathlib import PurePosixPath
import subprocess
import sys

CODE_ROOTS = {
    "src", "i18n", "bridge", "tests", "scripts", "sapi4_bridge",
    "novapad", ".github", ".cargo", ".githooks", "packager", "assets",
}
CODE_SUFFIXES = {
    ".rs", ".py", ".ps1", ".sh", ".bat", ".cmd", ".toml", ".lock",
    ".json", ".yaml", ".yml", ".c", ".cpp", ".h", ".hpp", ".rc",
    ".manifest", ".wxs", ".wxi", ".xml", ".js", ".ts", ".html",
    ".css", ".swift", ".dart", ".kt", ".java", ".php", ".arb",
}
GENERATED_SUFFIXES = {
    ".pyc", ".pyo", ".exe", ".dll", ".pdb", ".log", ".zip", ".bac",
}


def is_code(path):
    p = PurePosixPath(path)
    if p.suffix.lower() in GENERATED_SUFFIXES:
        return False
    if "__pycache__" in p.parts or "target" in p.parts:
        return False
    return (p.parts[0] in CODE_ROOTS or p.suffix.lower() in CODE_SUFFIXES
            or p.name in {".gitignore", ".gitattributes", "Makefile", "Dockerfile"})


def git_paths(*args):
    result = subprocess.run(["git", *args], check=True, stdout=subprocess.PIPE)
    return {os.fsdecode(p) for p in result.stdout.split(b"\0") if p}


def main():
    # Compare against the index, including partially staged edits and deletions.
    missing = git_paths("diff", "--name-only", "--no-renames", "-z", "--")
    missing |= git_paths("ls-files", "--others", "--exclude-standard", "-z")
    missing = sorted(path for path in missing if is_code(path))
    if missing:
        print("ERROR: code changes are missing from this commit:", file=sys.stderr)
        for path in missing:
            print("  " + path, file=sys.stderr)
        print("Review and stage every intended code change before committing. "
              "Do not add generated files or secrets.", file=sys.stderr)
        return 1
    print("OK: no code changes left outside the index.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        print("Cannot verify commit completeness: " + str(error), file=sys.stderr)
        sys.exit(2)
