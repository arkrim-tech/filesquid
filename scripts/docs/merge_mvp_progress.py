#!/usr/bin/env python3
"""Git merge driver for the generated MVP progress block and its SVG bar.

Purpose:
    Every task pull request ticks checklist boxes and commits a regenerated
    progress block (`resources/implementation-plan-checklist.md`) and bar
    (`resources/assets/mvp-progress.svg`). Two open pull requests therefore
    conflict on those generated lines as soon as one merges. This driver,
    registered for both files in `.gitattributes` (`merge=mvp-progress`),
    merges them without that conflict:

    - Checklist: the generated block in base, ours and theirs is replaced by a
      fixed placeholder, the rest is merged with `git merge-file`, and on a
      clean merge the block is regenerated offline (no GitHub call) from the
      merged ticks with `update_mvp_progress.expected_outputs`. The recorded
      pull requests are the union of both sides minus tasks that are now
      ticked (`mvp_queue.only_unticked`); the timestamp is the newer one. A
      real conflict outside the block keeps git's markers there, puts ours'
      block back and exits 1 so git reports the conflict.
    - SVG: every SVG carries a tick record (`<metadata id="mvp-ticks">`, see
      `update_mvp_progress.tick_record`). The driver merges the three records
      row by row and redraws the bar with `update_mvp_progress.render_svg`.
      It never reads the checklist, so the result does not depend on which of
      the two files git merges first. When the checklist merge is clean, the
      row-by-row result equals the merged checklist's ticks, so the bar
      matches the regenerated block.

    Limits: the block's Up next list reads the module plans and the hours
    read `resources/implementation-plan.md` from the working tree, which still
    holds ours while git merges. When theirs changed a plan's Dependencies,
    ticks or an estimate, the block can be stale after the merge. The
    pre-commit hook's `update_mvp_progress.py --check` reports that; rerun
    `python3 scripts/docs/update_mvp_progress.py`. Two branches that tick
    adjacent checklist lines are a real conflict for `git merge-file`.

Usage (git runs it from the repository's top-level directory):
    git config merge.mvp-progress.name "MVP progress block"
    git config merge.mvp-progress.driver "python3 scripts/docs/merge_mvp_progress.py %O %A %B %L %P"

Arguments:
    %O base file, %A ours file (the result is written here), %B theirs file,
    %L conflict marker size, %P path of the merged file in the repository.

Returns:
    Exit code 0 when the result in %A is clean, 1 when git must report a
    conflict (markers, an unmergeable SVG record or an error, printed to
    stderr), 2 on a usage error.
"""
import pathlib
import subprocess
import sys
import tempfile

import mvp_queue
import update_mvp_progress as progress

MERGE_FILE_TIMEOUT_SECONDS = 60
DEFAULT_MARKER_SIZE = 7
CLEAN = 0
CONFLICTED = 1
USAGE = 2
PLACEHOLDER = f"{progress.START}\n<!-- mvp-progress: placeholder while merging -->\n{progress.END}"
CONFLICT = object()


def read_text(path: str) -> str:
    """Read a file as UTF-8 without newline translation."""
    return pathlib.Path(path).read_bytes().decode("utf-8")


def write_text(path: str, text: str):
    """Write `text` to a file as UTF-8 without newline translation."""
    pathlib.Path(path).write_bytes(text.encode("utf-8"))


def split_block(text: str):
    """Replace the generated block with the placeholder.

    Args:
        text: One version of the checklist.

    Returns:
        A tuple of (text with the placeholder, the block or None when the text
        has no complete block; the text is then returned unchanged).
    """
    if progress.START not in text or progress.END not in text.split(progress.START, 1)[1]:
        return text, None
    head, tail = text.split(progress.START, 1)
    body, after = tail.split(progress.END, 1)
    return head + PLACEHOLDER + after, progress.START + body + progress.END


def run_merge_file(ours: str, base: str, theirs: str, marker_size: int):
    """Three-way merge texts with one bounded `git merge-file -p` call.

    The three versions are written to a temporary directory that is removed
    on every exit path.

    Args:
        ours: Ours text.
        base: Base text.
        theirs: Theirs text.
        marker_size: Conflict marker length (%L).

    Returns:
        A tuple of (merged text with any conflict markers, number of conflicts)
        on success, or (None, cause string) on failure. Never raises.
    """
    with tempfile.TemporaryDirectory(prefix="mvp-merge-") as folder:
        paths = [str(pathlib.Path(folder) / name) for name in ("ours", "base", "theirs")]
        for path, text in zip(paths, (ours, base, theirs)):
            write_text(path, text)
        command = ["git", "merge-file", "-p", f"--marker-size={marker_size}", "-L", "ours", "-L", "base", "-L", "theirs", *paths]
        try:
            done = subprocess.run(command, capture_output=True, timeout=MERGE_FILE_TIMEOUT_SECONDS, check=False)
        except subprocess.TimeoutExpired:
            return None, f"git merge-file timed out after {MERGE_FILE_TIMEOUT_SECONDS} seconds"
        except OSError as error:
            return None, f"could not run git merge-file: {error}"
    if not 0 <= done.returncode <= 127:
        return None, f"git merge-file exited {done.returncode}: {done.stderr.decode('utf-8', 'replace').strip()[:200]}"
    return done.stdout.decode("utf-8"), done.returncode


def merged_recorded(ours_block, theirs_block):
    """Return the union of both sides' recorded pull requests and the newer timestamp.

    Args:
        ours_block: Ours' generated block, or None.
        theirs_block: Theirs' generated block, or None.

    Returns:
        A tuple of (task ID to PR number, timestamp or None). A task recorded
        with two different PRs keeps the lower number, as `match_pr_ids` does.
    """
    ours, ours_stamp = mvp_queue.parse_recorded(ours_block or "")
    theirs, theirs_stamp = mvp_queue.parse_recorded(theirs_block or "")
    union = dict(theirs)
    for task_id, number in ours.items():
        union[task_id] = min(number, union.get(task_id, number))
    stamps = [stamp for stamp in (ours_stamp, theirs_stamp) if stamp]
    return union, max(stamps) if stamps else None


def stub_block(recorded: dict, stamp) -> str:
    """Return a block holding only the recorded PRs and timestamp for `--check` mode to read."""
    stamp_line = f"<!-- generated-at: {stamp} -->\n" if stamp else ""
    return f"{progress.START}\n{stamp_line}<!-- in-progress-prs: {mvp_queue.format_recorded(recorded)} -->\n{progress.END}"


def regenerate(merged: str, recorded: dict, stamp):
    """Regenerate the block and the bar for a cleanly merged checklist.

    Args:
        merged: The merged checklist with the placeholder block.
        recorded: Union of the recorded pull requests.
        stamp: Newer of the two timestamps, or None.

    Returns:
        A tuple of ((checklist text, SVG text), []) or (None, problems).
    """
    if "## MVP" not in merged.splitlines():
        return None, ["No '## MVP' section in the merged checklist"]
    rows = mvp_queue.parse_rows(progress.mvp_section(merged))
    kept = mvp_queue.only_unticked(recorded, rows)
    return progress.expected_outputs(merged.replace(PLACEHOLDER, stub_block(kept, stamp), 1), True)


def merge_checklist(base_path: str, ours_path: str, theirs_path: str, marker_size: int) -> int:
    """Merge the checklist and write the result to the ours file.

    Args:
        base_path: %O.
        ours_path: %A, overwritten with the result.
        theirs_path: %B.
        marker_size: %L.

    Returns:
        `CLEAN` or `CONFLICTED`.
    """
    base, _ = split_block(read_text(base_path))
    ours, ours_block = split_block(read_text(ours_path))
    theirs, theirs_block = split_block(read_text(theirs_path))
    merged, conflicts = run_merge_file(ours, base, theirs, marker_size)
    if merged is None:
        print(f"mvp-progress merge driver: {conflicts}; keeping ours", file=sys.stderr)
        return CONFLICTED
    if conflicts or not (ours_block or theirs_block):
        write_text(ours_path, merged.replace(PLACEHOLDER, ours_block or theirs_block or PLACEHOLDER))
        return CONFLICTED if conflicts else CLEAN
    outputs, problems = regenerate(merged, *merged_recorded(ours_block, theirs_block))
    if problems:
        write_text(ours_path, merged.replace(PLACEHOLDER, ours_block or theirs_block))
        print("mvp-progress merge driver: cannot regenerate the block: " + "; ".join(problems), file=sys.stderr)
        return CONFLICTED
    write_text(ours_path, outputs[0])
    return CLEAN


def pick(base, ours, theirs):
    """Return the three-way choice for one value, or `CONFLICT`.

    Args:
        base: The value in base (None when absent).
        ours: The value in ours.
        theirs: The value in theirs.

    Returns:
        The common value, the side that changed, or `CONFLICT` when both
        changed it differently.
    """
    if ours == theirs or theirs == base:
        return ours
    if ours == base:
        return theirs
    return CONFLICT


def shape(tasks: list) -> tuple:
    """Return the row structure of a tick record: task IDs with their module IDs."""
    return tuple((task.task_id, tuple(module_id for module_id, _ in task.module_ticks)) for task in tasks)


def values(tasks: list) -> dict:
    """Return every tick and estimate of a tick record keyed by row."""
    found = {}
    for task in tasks:
        found[("done", task.task_id)] = task.done
        found[("hours", task.task_id)] = task.hours
        found.update({("module", task.task_id, module_id): done for module_id, done in task.module_ticks})
    return found


def merge_ticks(base: list, ours: list, theirs: list):
    """Merge three tick records row by row.

    Args:
        base: Base tasks from `parse_tick_record`.
        ours: Ours tasks.
        theirs: Theirs tasks.

    Returns:
        The merged list of `progress.Task`, or None when the row structure or
        one value was changed differently on both sides. A row one side
        removed while the other changed it is such a clash, as it is for
        `git merge-file` on the checklist.
    """
    layout = pick(shape(base), shape(ours), shape(theirs))
    if layout is CONFLICT:
        return None
    sides = [values(tasks) for tasks in (base, ours, theirs)]
    merged = {key: pick(*(side.get(key) for side in sides)) for key in set().union(*sides)}
    if CONFLICT in merged.values():
        return None
    return [rebuild_task(task_id, modules, merged) for task_id, modules in layout]


def rebuild_task(task_id: str, modules: tuple, merged: dict):
    """Return the `progress.Task` for one merged row of the tick record."""
    ticks = [(module_id, merged[("module", task_id, module_id)]) for module_id in modules]
    done = sum(ticked for _, ticked in ticks)
    return progress.Task(task_id, merged[("done", task_id)], done, len(ticks), merged[("hours", task_id)], ticks)


def merge_svg(base_path: str, ours_path: str, theirs_path: str) -> int:
    """Merge the progress bar SVG through its tick records and write it to the ours file.

    Args:
        base_path: %O.
        ours_path: %A, overwritten with the result on success.
        theirs_path: %B.

    Returns:
        `CLEAN`, or `CONFLICTED` (ours kept) when a version has no tick record
        or both sides changed the same row differently.
    """
    records = [progress.parse_tick_record(read_text(path)) for path in (base_path, ours_path, theirs_path)]
    if None in records:
        print("mvp-progress merge driver: an SVG version has no tick record; keeping ours. "
              "Run python3 scripts/docs/update_mvp_progress.py after the merge.", file=sys.stderr)
        return CONFLICTED
    tasks = merge_ticks(*records)
    if tasks is None or sum(task.hours for task in tasks) <= 0:
        print("mvp-progress merge driver: both sides changed the same SVG row; keeping ours. "
              "Run python3 scripts/docs/update_mvp_progress.py after resolving the checklist.", file=sys.stderr)
        return CONFLICTED
    write_text(ours_path, progress.render_svg(tasks, progress.percent_done(tasks)))
    return CLEAN


def main(argv: list) -> int:
    """Dispatch on the merged path: `.svg` files merge as the bar, others as the checklist.

    Args:
        argv: `%O %A %B %L %P`.

    Returns:
        The process exit code. Read and decode errors keep ours and report a conflict.
    """
    if len(argv) != 5:
        print("usage: merge_mvp_progress.py %O %A %B %L %P", file=sys.stderr)
        return USAGE
    base_path, ours_path, theirs_path, marker, path = argv
    marker_size = int(marker) if marker.isdigit() else DEFAULT_MARKER_SIZE
    try:
        if path.endswith(".svg"):
            return merge_svg(base_path, ours_path, theirs_path)
        return merge_checklist(base_path, ours_path, theirs_path, marker_size)
    except (OSError, UnicodeDecodeError) as error:
        print(f"mvp-progress merge driver: {path}: {error}; keeping ours", file=sys.stderr)
        return CONFLICTED


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
