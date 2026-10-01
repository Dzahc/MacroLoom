"""Fail closed on changed functions above the reviewed cyclomatic limit."""

import argparse
import json
from pathlib import Path, PurePosixPath
import re
import sys

from .analyzer import AnalysisError, analyze, check_versions
from .changes import git, maintained, renames, snapshots, touched


LIMIT = 10


def load_allowlist(path):
    entries = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(entries, list):
        raise AnalysisError("complexity allowlist must be an array")
    result = {}
    fields = {"path", "function", "max_score", "reason", "approval"}
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != fields:
            raise AnalysisError(f"allowlist entry must contain exactly {sorted(fields)}")
        source, name = entry["path"], entry["function"]
        if not isinstance(source, str) or not isinstance(name, str):
            raise AnalysisError("allowlist path and function must be strings")
        if any(char in source + name for char in "*?[]\\") or not name.strip():
            raise AnalysisError("allowlist requires an exact path and function; patterns are forbidden")
        if PurePosixPath(source).is_absolute() or ".." in PurePosixPath(source).parts or not maintained(source):
            raise AnalysisError(f"invalid allowlist source: {source}")
        maximum = entry["max_score"]
        if type(maximum) is not int or maximum <= LIMIT:
            raise AnalysisError("exception max_score must be an integer above 10")
        if not isinstance(entry["reason"], str) or len(entry["reason"].strip()) < 12:
            raise AnalysisError("exception requires a specific, reviewable reason")
        approval = entry["approval"]
        if not isinstance(approval, str) or not re.fullmatch(
            r"https://github\.com/Dzahc/MacroLoom/(?:issues|pull)/[1-9]\d*#(?:issuecomment-\d+|discussion_r\d+)", approval
        ):
            raise AnalysisError("exception requires a link to the owner's explicit approval comment")
        key = (source, name)
        if key in result:
            raise AnalysisError(f"duplicate exception: {source} {name}")
        result[key] = entry
    return result


def validate_allowlist(entries, sources):
    errors = []
    for (path, name), entry in entries.items():
        if path not in sources:
            errors.append(f"stale exception: {path} {name} no longer exists")
            continue
        functions = {function.name: function for function in analyze(path, sources[path])}
        function = functions.get(name)
        if function is None:
            errors.append(f"stale exception: {path} {name} cannot be identified")
        elif function.score <= LIMIT:
            errors.append(f"stale exception: {path}:{function.start} {name} now scores {function.score} <= {LIMIT}")
        elif function.score > entry["max_score"]:
            errors.append(f"{path}:{function.start} {name}: observed {function.score}, approved limit {entry['max_score']}")
    return errors


def check_snapshot(original, sources, rename_map, entries, label):
    errors, checked = [], 0
    for path, source in sorted(sources.items()):
        previous = rename_map.get(path, path)
        old_source = original.get(previous)
        if old_source == source and path not in rename_map:
            continue
        current = analyze(path, source)
        old = analyze(previous, old_source) if old_source is not None else []
        selected = touched(old_source, source, old, current, path in rename_map)
        for function in current:
            if function.name not in selected:
                continue
            checked += 1
            maximum = entries.get((path, function.name), {}).get("max_score", LIMIT)
            if function.score > maximum:
                errors.append(f"[{label}] {path}:{function.start} {function.name}: observed {function.score}, limit {maximum}; refactor or request owner approval")
    return errors, checked


def run(root, base_ref, allowlist_path):
    check_versions()
    git(root, "rev-parse", "--verify", f"{base_ref}^{{commit}}")
    base = git(root, "merge-base", base_ref, "HEAD").decode().strip()
    if not base:
        raise AnalysisError(f"cannot determine merge base with {base_ref}")
    print(f"Complexity base: {base_ref} -> {base}")
    original, index, work = snapshots(root, base)
    entries = load_allowlist(allowlist_path)
    errors = validate_allowlist(entries, work)
    count = 0
    for label, sources, cached in [("index", index, True), ("worktree", work, False)]:
        failures, checked = check_snapshot(original, sources, renames(root, base, cached), entries, label)
        errors.extend(failures)
        count += checked
    for error in errors:
        print(error, file=sys.stderr)
    print(f"Complexity: {count} changed function checks across index/worktree; limit {LIMIT}; {len(errors)} failures")
    return 1 if errors else 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-ref", default="origin/develop")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args(argv)
    try:
        return run(args.root, args.base_ref, args.root / "scripts/complexity-allowlist.json")
    except (AnalysisError, OSError, ValueError, UnicodeError) as error:
        print(f"Complexity failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
