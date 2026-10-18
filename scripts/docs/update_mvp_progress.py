#!/usr/bin/env python3
"""Draw the MVP progress bar at the top of the implementation plan checklist.

Purpose:
    `resources/implementation-plan-checklist.md` lists the MVP global tasks
    under `## MVP`. This script weights each of them by its **Estimated Time**
    in `resources/implementation-plan.md` and counts how much of that time is
    done:

    - a ticked global task counts its full hours;
    - an open global task with module rows counts its hours times the share of
      its module rows that are ticked;
    - an open global task without module rows counts nothing.

    The result is drawn as `resources/assets/mvp-progress.svg`, one segment per
    global task with a divider line between tasks, and summarised
    between the `mvp-progress` markers at the top of the checklist. Under the
    summary the block lists the tasks In progress and the next five module
    tasks Up next (see `mvp_queue.py`). Write mode asks GitHub for the open
    pull requests with `gh api`; `--check` never does: it reads the PR-derived
    IDs and the timestamp back from the block, so it does not go stale when a
    pull request opens or closes or when time passes. The SVG also carries a
    hidden tick record (`tick_record`) that the git merge driver
    `merge_mvp_progress.py` uses to merge two bars without the checklist.

Usage:
    python3 scripts/docs/update_mvp_progress.py          # rewrite both files
    python3 scripts/docs/update_mvp_progress.py --check  # fail if either is stale

Returns:
    Exit code 0 when the files were written or are current, 1 when `--check`
    finds them stale or a task has no estimate. Problems are printed one per line;
    warnings (a failed GitHub call, an unparseable Dependencies line) go to stderr.
"""
import json
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from xml.sax.saxutils import escape, unescape

import mvp_queue

ROOT = pathlib.Path(__file__).resolve().parents[2]
PLAN = ROOT / "resources" / "implementation-plan.md"
CHECKLIST = ROOT / "resources" / "implementation-plan-checklist.md"
SVG = ROOT / "resources" / "assets" / "mvp-progress.svg"
PLAN_FILES = sorted((ROOT / "apps" / "filesquid" / "crates").glob("*/resources/plan.md")) + [
    ROOT / "apps" / "filesquid" / "ui" / "resources" / "plan.md"
]
PULLS_ENDPOINT = "repos/arkrim-tech/filesquid/pulls?state=open&per_page=100"
GH_TIMEOUT_SECONDS = 30
PULLS_PER_PAGE = 100
MAX_PULL_PAGES = 10
START = "<!-- mvp-progress:start -->"
END = "<!-- mvp-progress:end -->"
GLOBAL_ROW = re.compile(r"^\d+\. \[([ x])\] \[Task ([^\]]+)\]")
MODULE_ROW = re.compile(r"^\s+- \[([ x])\] (?:\[([^\]]+)\])?")
TICKS_START = '<metadata id="mvp-ticks">'
TICKS_END = "</metadata>"
TICK_LINE = re.compile(r"^Task (\S+) \[([ x])\] (\d+)h:(.*)$")
TICK_ITEM = re.compile(r"(\S+) \[([ x])\]")
ESTIMATE = re.compile(r'<a name="task-([^"]+)"></a>(?:\n  - (?!\*\*Estimated Time:).*)*\n  - \*\*Estimated Time:\*\* (\d+) hours')


@dataclass
class Task:
    """One MVP global task as listed in the checklist.

    Attributes:
        task_id: The global task ID, for example `FS-1.1`.
        done: Whether the global box is ticked.
        modules_done: Number of ticked module rows under the task.
        modules_total: Number of module rows under the task.
        hours: Estimated hours from the implementation plan.
        module_ticks: `(module ID, ticked)` pairs in checklist order. The SVG
            records them so the merge driver can merge two SVGs without the
            checklist (see `merge_mvp_progress.py`).
    """

    task_id: str
    done: bool
    modules_done: int = 0
    modules_total: int = 0
    hours: int = 0
    module_ticks: list = field(default_factory=list)

    def credit(self) -> float:
        """Return the hours counted as done for this task."""
        if self.done:
            return float(self.hours)
        if self.modules_total == 0:
            return 0.0
        return self.hours * self.modules_done / self.modules_total


def mvp_section(checklist: str) -> list:
    """Return the checklist lines between `## MVP` and the next `## ` heading.

    Args:
        checklist: The full checklist Markdown.

    Returns:
        The lines of the MVP section, without its heading.
    """
    lines = checklist.splitlines()
    start = lines.index("## MVP") + 1
    end = next((i for i in range(start, len(lines)) if lines[i].startswith("## ")), len(lines))
    return lines[start:end]


def parse_tasks(section: list) -> list:
    """Return the MVP global tasks with their module-row counts.

    Args:
        section: Lines of the checklist's MVP section.

    Returns:
        A list of `Task` objects in checklist order, with hours still 0.
    """
    tasks = []
    for line in section:
        row = GLOBAL_ROW.match(line)
        module = MODULE_ROW.match(line)
        if row:
            tasks.append(Task(task_id=row.group(2), done=row.group(1) == "x"))
        elif module and tasks:
            tasks[-1].modules_total += 1
            tasks[-1].modules_done += module.group(1) == "x"
            module_id = module.group(2) or f"row-{tasks[-1].modules_total}"
            tasks[-1].module_ticks.append((module_id, module.group(1) == "x"))
    return tasks


def attach_hours(tasks: list, plan: str) -> list:
    """Set each task's hours from the implementation plan.

    Args:
        tasks: Tasks parsed from the checklist.
        plan: The full implementation plan Markdown.

    Returns:
        A list of problems: no MVP tasks at all, or a task without a positive
        numeric estimate. Either would leave the bar with zero width to divide.
    """
    if not tasks:
        return ["No MVP tasks found in the checklist's MVP section"]
    estimates = {anchor: int(hours) for anchor, hours in ESTIMATE.findall(plan)}
    problems = []
    for task in tasks:
        anchor = task.task_id.lower()
        if anchor not in estimates:
            problems.append(f"Task {task.task_id}: no 'Estimated Time: N hours' in the implementation plan")
            continue
        if estimates[anchor] <= 0:
            problems.append(f"Task {task.task_id}: estimated time must be more than 0 hours")
            continue
        task.hours = estimates[anchor]
    return problems


def segment_shapes(tasks: list) -> list:
    """Return the SVG fill rectangles and divider lines for the task segments.

    Each task gets a segment of the 600-unit bar proportional to its hours, in
    checklist order. The segment is filled by the share of its hours that is
    done, and a divider line marks the boundary between two tasks.

    Args:
        tasks: MVP tasks with hours attached.

    Returns:
        A list of SVG element strings.
    """
    total = sum(task.hours for task in tasks)
    shapes = []
    x = 20.0
    for index, task in enumerate(tasks):
        width = 600 * task.hours / total
        fill = width * task.credit() / task.hours
        label = f"Task {task.task_id}: {round(task.credit())} of {task.hours} hours"
        shapes.append(f'<rect x="{x:.2f}" y="28" width="{fill:.2f}" height="18" fill="#1a7f37"><title>{label}</title></rect>')
        if index > 0:
            shapes.append(f'<line class="divider" x1="{x:.2f}" y1="28" x2="{x:.2f}" y2="46"/>')
        x += width
    return shapes


def percent_done(tasks: list) -> int:
    """Return the weighted completion of `tasks` (hours attached), 0 to 100."""
    return round(100 * sum(task.credit() for task in tasks) / sum(task.hours for task in tasks))


def tick_record(tasks: list) -> str:
    """Return the SVG `<metadata>` element that records every task's ticks and hours.

    One line per global task: `Task <ID> [x] <hours>h:` (or `[ ]` when open)
    followed by `<module ID> [x]` or `<module ID> [ ]` for each module row.
    It is not drawn. `merge_mvp_progress.py` reads it back to merge two SVGs.

    Args:
        tasks: MVP tasks with hours attached, in checklist order.

    Returns:
        The element source, indented for the SVG body.
    """
    lines = []
    for task in tasks:
        modules = "".join(f" {escape(module_id)} [{'x' if done else ' '}]" for module_id, done in task.module_ticks)
        lines.append(f"Task {escape(task.task_id)} [{'x' if task.done else ' '}] {task.hours}h:{modules}")
    return f"{TICKS_START}\n" + "\n".join(lines) + f"\n  {TICKS_END}"


def parse_tick_record(svg: str):
    """Read the tasks back from the tick record of an SVG drawn by `render_svg`.

    Args:
        svg: SVG source.

    Returns:
        A list of `Task` with ticks and hours, or None when the SVG has no
        tick record, a record line cannot be parsed or the record is empty.
    """
    if TICKS_START not in svg or TICKS_END not in svg.split(TICKS_START, 1)[1]:
        return None
    body = svg.split(TICKS_START, 1)[1].split(TICKS_END, 1)[0]
    tasks = []
    for line in (raw.strip() for raw in body.splitlines() if raw.strip()):
        match = TICK_LINE.match(line)
        if not match:
            return None
        ticks = [(unescape(module_id), mark == "x") for module_id, mark in TICK_ITEM.findall(match.group(4))]
        done = sum(ticked for _, ticked in ticks)
        tasks.append(Task(unescape(match.group(1)), match.group(2) == "x", done, len(ticks), int(match.group(3)), ticks))
    return tasks or None


def render_svg(tasks: list, percent: int) -> str:
    """Return the progress bar as an SVG document.

    Args:
        tasks: MVP tasks with hours attached, in checklist order.
        percent: Weighted completion, 0 to 100.

    Returns:
        SVG source for a 640x56 bar with one segment per task and a percentage label.
    """
    shapes = "\n    ".join(segment_shapes(tasks))
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="640" height="56" viewBox="0 0 640 56" role="img" aria-label="MVP progress: {percent}% complete across {len(tasks)} tasks">
  <style>
    text {{ font: 600 14px -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; fill: #1f2328; }}
    .track {{ fill: #d0d7de; }}
    .divider {{ stroke: #ffffff; stroke-width: 1.5; }}
    @media (prefers-color-scheme: dark) {{ text {{ fill: #e6edf3; }} .track {{ fill: #30363d; }} .divider {{ stroke: #0d1117; }} }}
  </style>
  <defs><clipPath id="bar"><rect x="20" y="28" width="600" height="18" rx="9"/></clipPath></defs>
  {tick_record(tasks)}
  <text x="20" y="18">MVP (early access): {len(tasks)} tasks</text>
  <text x="620" y="18" text-anchor="end">{percent}% complete</text>
  <g clip-path="url(#bar)">
    <rect class="track" x="20" y="28" width="600" height="18"/>
    {shapes}
  </g>
</svg>
"""


def render_summary(tasks: list, queue_md: str) -> str:
    """Return the Markdown block that goes between the progress markers.

    Args:
        tasks: MVP tasks with hours attached.
        queue_md: The In progress and Up next Markdown from `mvp_queue.render_queue`.

    Returns:
        The marker block, including both markers.
    """
    total = sum(task.hours for task in tasks)
    credit = sum(task.credit() for task in tasks)
    percent = percent_done(tasks)
    done = sum(task.done for task in tasks)
    started = sum(not task.done and task.modules_done > 0 for task in tasks)
    return (
        f"{START}\n"
        f"![MVP progress: {percent}% complete](./assets/mvp-progress.svg)\n\n"
        f"**MVP progress: {percent}%**, about {round(credit)} of {total} estimated hours. "
        f"{done} of {len(tasks)} tasks are done and {started} more are under way. "
        "Each segment of the bar is one task below, as wide as its estimated hours in the implementation "
        "plan; an open task fills by the share of its module rows that are ticked. Below the bar, In progress "
        "lists the open tasks being worked on (global tasks with some but not all module rows ticked, and "
        "tasks with an open pull request) and Up next lists the next five new module tasks whose listed "
        "dependencies are done or in progress. Regenerate with "
        "`python3 scripts/docs/update_mvp_progress.py` after ticking boxes or opening a pull request; "
        "the check run by `scripts/verify_docs.sh` does not call GitHub and uses the pull requests recorded "
        "in the block. When two pull requests both redraw it, the `mvp-progress` git merge driver "
        "(see the README) merges their ticks and redraws it instead of reporting a conflict.\n\n"
        f"{queue_md}\n"
        f"{END}"
    )


def replace_block(checklist: str, block: str) -> str:
    """Return the checklist with the marker block replaced, or inserted after the title.

    Args:
        checklist: The full checklist Markdown.
        block: The new marker block.

    Returns:
        The updated checklist Markdown.
    """
    if START in checklist:
        head, rest = checklist.split(START, 1)
        return head + block + rest.split(END, 1)[1]
    title, rest = checklist.split("\n\n", 1)
    return f"{title}\n\n{block}\n\n{rest}"


def fetch_pulls_page(page: int):
    """Fetch one page of open pull requests with a single bounded `gh api` call.

    Args:
        page: One-based page number.

    Returns:
        A tuple of (list of pull request dicts, None) on success, or
        (None, cause string) when `gh` is missing, times out, exits non-zero or
        returns something that is not a JSON list. Never raises.
    """
    command = ["gh", "api", f"{PULLS_ENDPOINT}&page={page}"]
    try:
        done = subprocess.run(command, capture_output=True, text=True, timeout=GH_TIMEOUT_SECONDS, check=False)
    except subprocess.TimeoutExpired:
        return None, f"gh api timed out after {GH_TIMEOUT_SECONDS} seconds"
    except OSError as error:
        return None, f"could not run gh: {error}"
    if done.returncode != 0:
        return None, f"gh api exited {done.returncode}: {done.stderr.strip()[:200]}"
    try:
        prs = json.loads(done.stdout)
    except json.JSONDecodeError as error:
        return None, f"gh api returned invalid JSON: {error}"
    if not isinstance(prs, list):
        return None, "gh api did not return a list of pull requests"
    return prs, None


def fetch_open_prs():
    """Ask GitHub for every open pull request, one page at a time.

    Stops at the first short page, or after `MAX_PULL_PAGES` pages, which is
    reported as a failure so the recorded list is kept rather than truncated.

    Returns:
        A tuple of (list of pull request dicts, None) on success, or
        (None, cause string) on any failure. Never raises.
    """
    found = []
    for page in range(1, MAX_PULL_PAGES + 1):
        prs, cause = fetch_pulls_page(page)
        if prs is None:
            return None, cause
        found.extend(prs)
        if len(prs) < PULLS_PER_PAGE:
            return found, None
    return None, f"more than {MAX_PULL_PAGES * PULLS_PER_PAGE} open pull requests; not all were fetched"


def now_utc() -> str:
    """Return the current time as an RFC 3339 UTC timestamp."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def current_pr_ids(rows: list, recorded: dict, old_stamp, checking: bool):
    """Return the PR-derived task IDs and the time they are as of.

    Args:
        rows: Parsed MVP global tasks.
        recorded: PR IDs recorded in the existing block.
        old_stamp: Timestamp recorded in the existing block, or None.
        checking: True in `--check` mode, which never calls GitHub.

    Returns:
        A tuple of (task ID to PR number, timestamp). On a failed GitHub call
        the recorded IDs and timestamp are kept and a warning names the cause.
    """
    if checking:
        return recorded, old_stamp or "missing"
    prs, cause = fetch_open_prs()
    if prs is None:
        print(f"warning: keeping the recorded pull requests: {cause}", file=sys.stderr)
        return recorded, old_stamp or now_utc()
    unticked = [g.task_id for g in rows if not g.done] + [m.task_id for g in rows for m in g.modules if not m.done]
    return mvp_queue.match_pr_ids(prs, unticked), now_utc()


def load_plans():
    """Read every module plan and the implementation plan.

    Returns:
        A tuple of (module plan rows by ID in plan order, global ID to ticked state).
    """
    plan_rows = {}
    for path in PLAN_FILES:
        plan_rows.update(mvp_queue.parse_plan_rows(path.read_text(encoding="utf-8")))
    global_rows = mvp_queue.parse_plan_rows(PLAN.read_text(encoding="utf-8"))
    return plan_rows, {task_id: row.done for task_id, row in global_rows.items()}


def build_queue(checklist: str, rows: list, checking: bool):
    """Compute the In progress and Up next selections.

    Args:
        checklist: The full checklist Markdown (its block holds the recorded PRs).
        rows: Parsed MVP global tasks.
        checking: True in `--check` mode.

    Returns:
        A tuple of (render function taking a timestamp, timestamp to use).
    """
    block = checklist[checklist.index(START):] if START in checklist else ""
    recorded, old_stamp = mvp_queue.parse_recorded(block)
    pr_ids, stamp = current_pr_ids(rows, recorded, old_stamp, checking)
    raw_ids = pr_ids
    pr_ids = mvp_queue.only_unticked(pr_ids, rows)
    inprog = mvp_queue.compute_in_progress(rows, pr_ids)
    plan_rows, global_done = load_plans()
    up_next, warnings = mvp_queue.select_up_next(rows, inprog.active, plan_rows, global_done)
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    kept = raw_ids if checking else None
    return (lambda when: mvp_queue.render_queue(inprog, up_next, pr_ids, when, kept)), stamp, old_stamp


def render_block(tasks: list, render, stamp: str) -> str:
    """Return the full marker block for one timestamp."""
    return render_summary(tasks, render(stamp))


def expected_outputs(checklist: str, checking: bool):
    """Return the checklist and SVG text that match the checklist's ticks.

    Args:
        checklist: The full checklist Markdown, including its marker block.
        checking: True for the offline `--check` computation: no GitHub call,
            and the PR IDs and timestamp are read back from the block. The
            merge driver (`merge_mvp_progress.py`) uses this mode too.

    Returns:
        A tuple of ((checklist text, SVG text), []), or (None, problems) when
        the checklist has no `## MVP` section or a task has no estimate.
    """
    if "## MVP" not in checklist.splitlines():
        return None, ["No '## MVP' section in the checklist"]
    tasks = parse_tasks(mvp_section(checklist))
    problems = attach_hours(tasks, PLAN.read_text(encoding="utf-8"))
    if problems:
        return None, problems
    render, stamp, old_stamp = build_queue(checklist, mvp_queue.parse_rows(mvp_section(checklist)), checking)
    block = render_block(tasks, render, stamp)
    if not checking and old_stamp and replace_block(checklist, render_block(tasks, render, old_stamp)) == checklist:
        block = render_block(tasks, render, old_stamp)
    return (replace_block(checklist, block), render_svg(tasks, percent_done(tasks))), []


def main(argv: list) -> int:
    """Compute MVP progress and write or check the checklist block and the SVG.

    Args:
        argv: Command-line arguments; `--check` compares instead of writing.

    Returns:
        The process exit code.
    """
    checking = "--check" in argv
    outputs, problems = expected_outputs(CHECKLIST.read_text(encoding="utf-8"), checking)
    if problems:
        print("\n".join(problems))
        return 1
    wanted = dict(zip((CHECKLIST, SVG), outputs))
    stale = [path for path, text in wanted.items() if not path.exists() or path.read_text(encoding="utf-8") != text]
    if checking:
        for path in stale:
            print(f"{path.relative_to(ROOT)} is stale: run python3 scripts/docs/update_mvp_progress.py")
        return 1 if stale else 0
    for path in stale:
        path.write_text(wanted[path], encoding="utf-8")
        print(f"Wrote {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
