import json
import re
import unicodedata
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parents[1]
pairs = [
    ("i18n/en.json", "i18n/id.json"),
    ("i18n/features/treccani_it.json", "i18n/features/treccani_id.json"),
    ("i18n/features/la7_play/it.json", "i18n/features/la7_play/id.json"),
    ("i18n/features/tv/it.json", "i18n/features/tv/id.json"),
]

def no_duplicates(items):
    result = {}
    for key, value in items:
        assert key not in result, f"Duplicate key: {key}"
        result[key] = value
    return result

def load(path):
    return json.loads((root / path).read_text(encoding="utf-8"), object_pairs_hook=no_duplicates)

def mnemonic_count(text):
    return len(re.findall(r"(?<!&)&(?!&)", text))

def guarded_symbols(text):
    """Allow unusual punctuation only when present in the corresponding source."""
    return Counter(
        char for char in text
        if char in ";#$%*+<=>@[]^_`|~\\"
        or (
            ord(char) > 127
            and unicodedata.category(char)[0] in "PSZC"
        )
    )

def added_bullets(source, target):
    pattern = r"(?m)^\s*([-*+])\s+"
    return Counter(re.findall(pattern, target)) - Counter(re.findall(pattern, source))

failures = []
unchanged = []
total = 0
for source_path, target_path in pairs:
    source, target = load(source_path), load(target_path)
    assert list(source) == list(target), f"Key/order mismatch: {target_path}"
    for key, original in source.items():
        translated = target[key]
        total += 1
        tests = {
            "nonempty": isinstance(translated, str) and bool(translated.strip()),
            "placeholders": Counter(re.findall(r"\{[^{}]+\}", original)) == Counter(re.findall(r"\{[^{}]+\}", translated)),
            "shortcut": original.split("\t")[1:] == translated.split("\t")[1:],
            "mnemonics": mnemonic_count(original) == mnemonic_count(translated),
            "literal ampersands": original.count("&&") == translated.count("&&"),
            "line breaks": original.count("\n") == translated.count("\n") and original.count("\r") == translated.count("\r"),
            "literal escapes": all(original.count(token) == translated.count(token) for token in (r"\0", r"\n", r"\r")),
            "clean unicode": "\ufffd" not in translated,
            "source punctuation": not (guarded_symbols(translated) - guarded_symbols(original)),
            "source bullets": not added_bullets(original, translated),
            "outer whitespace": (re.match(r"^\s*", original)[0], re.search(r"\s*$", original)[0]) == (re.match(r"^\s*", translated)[0], re.search(r"\s*$", translated)[0]),
        }
        if r"\0" in original:
            tests["file filter patterns"] = original.split(r"\0")[1::2] == translated.split(r"\0")[1::2]
        for check, passed in tests.items():
            if not passed:
                failures.append((target_path, key, check))
        if original == translated:
            unchanged.append((key, translated))
    print(f"{target_path}: {len(target)} keys")

for source_path, target_path in [
    ("guida_en.txt", "guida_id.txt"),
    ("donations_en.txt", "donations_id.txt"),
]:
    # Paragraphs correspond in order; translated documents add blank separators.
    source_lines = [
        line for line in (root / source_path).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    target_lines = [
        (number, line)
        for number, line in enumerate(
            (root / target_path).read_text(encoding="utf-8").splitlines(), 1
        )
        if line.strip()
    ]
    assert len(source_lines) == len(target_lines), f"Paragraph count mismatch: {target_path}"
    for original, (number, translated) in zip(source_lines, target_lines):
        if guarded_symbols(translated) - guarded_symbols(original):
            failures.append((target_path, number, "source punctuation"))
        if added_bullets(original, translated):
            failures.append((target_path, number, "source bullets"))
        if "\ufffd" in translated:
            failures.append((target_path, number, "clean unicode"))
    print(f"{target_path}: {len(target_lines)} nonblank lines")

print(f"TOTAL: {total}")
print("FAILURES:", json.dumps(failures, ensure_ascii=False))
print(f"Unchanged technical terms and names: {len(unchanged)}")
assert not failures
