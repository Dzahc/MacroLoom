"""Git snapshots and line-based changed-function selection."""

from difflib import SequenceMatcher
from pathlib import PurePosixPath
import subprocess

from .analyzer import AnalysisError


def git(root, *args):
    result = subprocess.run(["git", *args], cwd=root, capture_output=True)
    if result.returncode:
        error = result.stderr.decode("utf-8", errors="replace").strip()
        raise AnalysisError(f"git {' '.join(args)} failed: {error}")
    return result.stdout


def maintained(path):
    parts = PurePosixPath(path).parts
    excluded = {"node_modules", "target", "dist", ".git", ".venv", ".quality-output"}
    return (PurePosixPath(path).suffix in {".ts", ".tsx", ".rs"}
            and not excluded.intersection(parts)
            and "app/src-tauri/gen/schemas/" not in path)


def paths(output):
    return {value.decode("utf-8") for value in output.split(b"\0") if value}


def renames(root, base, cached):
    args = ["diff", "--name-status", "-z", "--find-renames", base]
    if cached:
        args.append("--cached")
    fields = git(root, *args).decode("utf-8").split("\0")
    result = {}
    index = 0
    while index < len(fields) and fields[index]:
        status, old = fields[index:index + 2]
        index += 2
        if status.startswith(("R", "C")):
            result[fields[index]] = old
            index += 1
    return result


def snapshots(root, base):
    old_paths = paths(git(root, "ls-tree", "-r", "--name-only", "-z", base))
    indexed = paths(git(root, "ls-files", "--cached", "-z"))
    untracked = paths(git(root, "ls-files", "--others", "--exclude-standard", "-z"))
    stage = git(root, "ls-files", "--unmerged", "-z")
    if stage:
        raise AnalysisError("unmerged index: resolve conflicts before verification")
    original = {path: git(root, "show", f"{base}:{path}").decode("utf-8")
                for path in old_paths if maintained(path)}
    index = {path: git(root, "show", f":{path}").decode("utf-8")
             for path in indexed if maintained(path)}
    work = {}
    for path in indexed | untracked:
        if maintained(path) and (root / path).is_file():
            work[path] = (root / path).read_text(encoding="utf-8")
    return original, index, work


def overlaps(function, start, end):
    return start <= function.end and end >= function.start


def touched(old_source, new_source, old_functions, new_functions, renamed=False):
    if renamed or old_source is None:
        return {function.name for function in new_functions}
    selected = set()
    current_names = {function.name for function in new_functions}
    shared = current_names.intersection(function.name for function in old_functions)
    old_order = [function.name for function in old_functions if function.name in shared]
    new_order = [function.name for function in new_functions if function.name in shared]
    # A line diff can align a large moved body and report its smaller neighbour
    # as the move. Relative-order changes must still recheck the large function.
    selected.update(name for position, name in enumerate(old_order)
                    if new_order[position] != name)
    matcher = SequenceMatcher(None, old_source.splitlines(), new_source.splitlines(), autojunk=False)
    unchanged_lines = {}
    old_lines = old_source.splitlines()
    for block in matcher.get_matching_blocks():
        for offset in range(block.size):
            if old_lines[block.a + offset].strip():
                unchanged_lines[block.a + offset + 1] = block.b + offset + 1
    for tag, old_start, old_end, new_start, new_end in matcher.get_opcodes():
        if tag == "equal":
            continue
        if new_end > new_start:
            selected.update(function.name for function in new_functions
                            if overlaps(function, new_start + 1, new_end))
        if old_end > old_start:
            selected.update(function.name for function in old_functions
                            if function.name in current_names and overlaps(function, old_start + 1, old_end))
            for previous in old_functions:
                if previous.name in current_names or not overlaps(previous, old_start + 1, old_end):
                    continue
                # Anonymous identities include source hashes. A deletion can
                # change the hash without adding any current lines. Surviving
                # lines anchor the same function; fully deleted bodies vanish.
                anchors = [new_line for old_line, new_line in unchanged_lines.items()
                           if previous.start <= old_line <= previous.end]
                if not anchors:
                    continue
                candidates = [function for function in new_functions
                              if function.kind == previous.kind
                              and function.start <= min(anchors) and function.end >= max(anchors)]
                if candidates:
                    candidate = min(candidates, key=lambda function: function.end - function.start)
                    selected.add(candidate.name)
                else:
                    raise AnalysisError(f"cannot map edited function {previous.name} after deleted lines")
    return selected
