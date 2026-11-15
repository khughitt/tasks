"""Which project a path belongs to, asked of the tasks CLI: the one ops client of its
resolver (docs/runbooks/link-a-repository.md#prerequisites). It also reads a
task's full note history, which a record with archived notes no longer holds in Markdown.

Three outcomes, never conflated. A project, or `None` for no project, come back as
values; the CLI's error envelope raises WhereError; a CLI that is missing, cannot run,
or prints no JSON raises Unavailable. Each caller decides what an exception means for
it — none may treat one as "no project".
"""
import json
import pathlib
import shutil
import subprocess


class Unavailable(Exception):
    """tasks is not on PATH, could not run, or printed no JSON."""


class WhereError(Exception):
    """The CLI answered with its error envelope."""

    def __init__(self, kind, detail):
        super().__init__(f"tasks: {kind}: {detail}")
        self.kind, self.detail = kind, detail


def _run(args, env):
    binary = shutil.which("tasks", path=env.get("PATH") or "")
    if binary is None:
        raise Unavailable("tasks is not on PATH")
    try:
        result = subprocess.run([binary, *args], capture_output=True, text=True, env=dict(env), timeout=10)
    except (OSError, subprocess.SubprocessError) as error:
        raise Unavailable(f"tasks could not run: {error}") from error
    # The CLI's stream contract: success JSON on stdout with exit 0, the error envelope
    # on stderr with a non-zero exit.
    stream = result.stdout if result.returncode == 0 else result.stderr
    try:
        data = json.loads(stream)
    except json.JSONDecodeError as error:
        raise Unavailable(f"tasks {args[0]} printed no JSON (exit {result.returncode})") from error
    envelope = data.get("error") if isinstance(data, dict) else None
    if result.returncode != 0 or envelope is not None:
        envelope = envelope if isinstance(envelope, dict) else {}
        raise WhereError(str(envelope.get("kind", "unknown")), str(envelope.get("detail", f"exit {result.returncode}")))
    valid = isinstance(data, dict)
    if args[0] == "where":
        valid = (valid and "project" in data and type(data.get("holds_records")) is bool
                 and isinstance(data.get("warnings"), list))
        project = data.get("project") if isinstance(data, dict) else None
        if project is not None:
            valid = (valid and isinstance(project, dict)
                     and all(isinstance(project.get(k), str) and project[k] for k in ("prefix", "root"))
                     and type(project.get("linked")) is bool and "checkout" in project
                     and (isinstance(project["checkout"], str) and bool(project["checkout"])
                          or "kind" not in project and not project["linked"] and project["checkout"] is None)
                     and ("kind" not in project or project["kind"] in ("in-tree", "detached", "linked")
                          and project["linked"] == (project["kind"] == "linked"))
                     and ("records" not in project and "kind" not in project
                          or isinstance(project.get("records"), str) and bool(project["records"])))
    elif args[0] == "show":
        task = data.get("task") if isinstance(data, dict) else None
        valid = (valid and isinstance(task, dict) and isinstance(task.get("notes", []), list)
                 and all(isinstance(note, dict) and isinstance(note.get("text"), str) for note in task.get("notes", [])))
    else:
        valid = valid and isinstance(data.get("projects"), list)
        if valid:
            valid = all(isinstance(row, dict)
                        and all(isinstance(row.get(k), str) and row[k] for k in ("prefix", "root"))
                        and ("kind" not in row and "checkout" not in row
                             or isinstance(row.get("checkout"), str) and bool(row["checkout"]))
                        and ("kind" not in row or row["kind"] in ("in-tree", "detached", "linked"))
                        and ("records" not in row and "kind" not in row
                             or isinstance(row.get("records"), str) and bool(row["records"]))
                        for row in data["projects"])
    if not valid:
        raise Unavailable(f"tasks {args[0]} returned an invalid success shape")
    return data


def where(path, env):
    """The `tasks where` answer for path: a dict whose `project` is None for no project."""
    return _run(["where", str(path), "--json"], env)


def records_dirs(env):
    """prefix -> records for every registered project: where its tasks/ lives, which is
    the code checkout only in-tree (tasks docs/specs/2026-10-22-records-home-design.md
    §3.1). A CLI whose rows predate records cannot say, and raises Unavailable."""
    data = _run(["projects", "--json"], env)
    found = {}
    for row in data["projects"]:
        if "records" not in row:
            raise Unavailable(f"tasks projects gives no records for {row['prefix']}: update the tasks binary")
        found[row["prefix"]] = pathlib.Path(row["records"])
    return found


def linked_checkouts(env):
    """prefix -> (root, checkout) for every registered linked project."""
    data = _run(["projects", "--json"], env)
    return {row["prefix"]: (pathlib.Path(row["root"]), pathlib.Path(row["checkout"]))
            for row in data["projects"] if (row["kind"] == "linked" if "kind" in row else row.get("checkout"))}


def notes(records, task_id, env):
    """Every note of task_id, read from the project at records through the CLI: archived
    segments verified and in descriptor order, then the inline suffix. A record carrying
    note_archives holds only that suffix in its Markdown."""
    return _run(["show", task_id, "-C", str(records), "--no-fetch", "--json"], env)["task"].get("notes", [])
