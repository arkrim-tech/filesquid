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
    between the `mvp-progress` markers at the top of the checklist.

Usage:
    python3 scripts/docs/update_mvp_progress.py          # rewrite both files
    python3 scripts/docs/update_mvp_progress.py --check  # fail if either is stale

Returns:
    Exit code 0 when the files were written or are current, 1 when `--check`
    finds them stale or a task has no estimate. Problems are printed one per line.
"""
import pathlib
import re
import sys
from dataclasses import dataclass

ROOT = pathlib.Path(__file__).resolve().parents[2]
PLAN = ROOT / "resources" / "implementation-plan.md"
CHECKLIST = ROOT / "resources" / "implementation-plan-checklist.md"
SVG = ROOT / "resources" / "assets" / "mvp-progress.svg"
START = "<!-- mvp-progress:start -->"
END = "<!-- mvp-progress:end -->"
GLOBAL_ROW = re.compile(r"^\d+\. \[([ x])\] \[Task ([^\]]+)\]")
MODULE_ROW = re.compile(r"^\s+- \[([ x])\] ")
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
    """

    task_id: str
    done: bool
    modules_done: int = 0
    modules_total: int = 0
    hours: int = 0

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
  <text x="20" y="18">MVP (early access): {len(tasks)} tasks</text>
  <text x="620" y="18" text-anchor="end">{percent}% complete</text>
  <g clip-path="url(#bar)">
    <rect class="track" x="20" y="28" width="600" height="18"/>
    {shapes}
  </g>
</svg>
"""


def render_summary(tasks: list) -> str:
    """Return the Markdown block that goes between the progress markers.

    Args:
        tasks: MVP tasks with hours attached.

    Returns:
        The marker block, including both markers.
    """
    total = sum(task.hours for task in tasks)
    credit = sum(task.credit() for task in tasks)
    percent = round(100 * credit / total)
    done = sum(task.done for task in tasks)
    started = sum(not task.done and task.modules_done > 0 for task in tasks)
    return (
        f"{START}\n"
        f"![MVP progress: {percent}% complete](./assets/mvp-progress.svg)\n\n"
        f"**MVP progress: {percent}%**, about {round(credit)} of {total} estimated hours. "
        f"{done} of {len(tasks)} tasks are done and {started} more are under way. "
        "Each segment of the bar is one task below, as wide as its estimated hours in the implementation "
        "plan; an open task fills by the share of its module rows that are ticked. Regenerate with "
        "`python3 scripts/docs/update_mvp_progress.py` after ticking boxes.\n"
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


def main(argv: list) -> int:
    """Compute MVP progress and write or check the checklist block and the SVG.

    Args:
        argv: Command-line arguments; `--check` compares instead of writing.

    Returns:
        The process exit code.
    """
    checklist = CHECKLIST.read_text(encoding="utf-8")
    tasks = parse_tasks(mvp_section(checklist))
    problems = attach_hours(tasks, PLAN.read_text(encoding="utf-8"))
    if problems:
        print("\n".join(problems))
        return 1
    block = render_summary(tasks)
    percent = int(re.search(r"\*\*MVP progress: (\d+)%", block).group(1))
    wanted = {CHECKLIST: replace_block(checklist, block), SVG: render_svg(tasks, percent)}
    stale = [path for path, text in wanted.items() if not path.exists() or path.read_text(encoding="utf-8") != text]
    if "--check" in argv:
        for path in stale:
            print(f"{path.relative_to(ROOT)} is stale: run python3 scripts/docs/update_mvp_progress.py")
        return 1 if stale else 0
    for path in stale:
        path.write_text(wanted[path], encoding="utf-8")
        print(f"Wrote {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
