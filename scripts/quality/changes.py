"""Git snapshots and file rename detection."""

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
