#!/usr/bin/env python3
"""Pure logic behind the checklist's "In progress" and "Up next" sections.

Purpose:
    `update_mvp_progress.py` draws the MVP progress bar. This module decides
    which open MVP tasks are being worked on and which new module tasks start
    next, and renders both lists as Markdown. Nothing here reads files, calls
    GitHub or reads the clock: the caller passes everything in, so every
    function can be unit tested (see `test_mvp_queue.py`).

    - In progress: a global task with some but not all module rows ticked
      (its unticked rows are listed), plus every unticked task whose ID appears
      in the head branch or title of an open pull request.
    - Up next: the first five unticked, not-in-progress module rows, in
      checklist order, whose Dependencies are all ticked or in progress.

    Pull request IDs are the only input that is not derived from the repository.
    Write mode fetches them; `--check` reads them back from the block's
    `in-progress-prs` comment and ignores any whose task has since been ticked
    (`only_unticked`), so merging a task PR never makes the offline check fail
    by itself.
"""
import re
from dataclasses import dataclass, field

PR_URL = "https://github.com/arkrim-tech/filesquid/pull/"
UP_NEXT_LIMIT = 5
BRANCH_TYPES = "feat|fix|build|docs|refactor|test|chore|perf|ci|style"

GLOBAL_ROW = re.compile(r"^\d+\. \[([ x])\] (\[Task ([^\]]+)\]\([^)]*\))\s*(.*)$")
MODULE_ROW = re.compile(r"^\s+- \[([ x])\] (\[([^\]]+)\]\([^)]*\))\s*(.*)$")
PLAN_ROW = re.compile(r"^- \[([ x])\] \*\*Task ([^:*]+):")
PLAN_DEPS = re.compile(r"^\s+- \*\*Dependencies:\*\* (.*)$")
ID = r"[A-Za-z]{1,4}-[A-Za-z0-9]+(?:\.\d+[a-z]?)?|\d+(?:\.\d+)?"
DEP_PART = re.compile(rf"^(?:(global|metadata)\s+)?(?:Tasks?\s+)?({ID})(?:\s+through\s+({ID}))?$")
RECORDED_PRS = re.compile(r"<!-- in-progress-prs: ([^>]*?) -->")
RECORDED_TIME = re.compile(r"<!-- generated-at: (\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) -->")


@dataclass
class ModuleRow:
    """One module row under a global task in the checklist.

    Attributes:
        task_id: Module task ID, for example `IO-0.9`.
        done: Whether its box is ticked.
        link: The row's Markdown link, `[IO-0.9](relative/path#anchor)`.
        title: The text after the link.
    """

    task_id: str
    done: bool
    link: str
    title: str


@dataclass
class GlobalRow:
    """One global task in the checklist with its module rows.

    Attributes:
        task_id: Global task ID without the word Task, for example `0.4`.
        done: Whether its box is ticked.
        link: The row's Markdown link, `[Task 0.4](relative/path#anchor)`.
        title: The text after the link.
        modules: Its module rows in checklist order.
    """

    task_id: str
    done: bool
    link: str
    title: str
    modules: list = field(default_factory=list)


@dataclass
class PlanRow:
    """One task row from a plan file.

    Attributes:
        done: Whether its box is ticked in the plan.
        deps: The text after `**Dependencies:**`, or None when absent.
    """

    done: bool
    deps: object = None


@dataclass
class Dep:
    """One parsed dependency reference.

    Attributes:
        task_id: The referenced ID, for example `IO-0.9` or `0.5`.
        is_global: True for `global Task` references and bare global numbers.
    """

    task_id: str
    is_global: bool


@dataclass
class Entry:
    """One In progress item.

    Attributes:
        global_row: The global task the item belongs to.
        modules: Module rows to list (all unticked rows when `nested`).
        nested: True when the global task itself is listed with rows under it.
    """

    global_row: GlobalRow
    modules: list
    nested: bool


@dataclass
class InProgress:
    """The In progress selection.

    Attributes:
        entries: Items in checklist order.
        active: IDs (modules and globals) counted as in progress.
    """

    entries: list
    active: set


def parse_rows(section: list) -> list:
    """Return global tasks, each with its module rows, from checklist lines.

    Args:
        section: Lines of the checklist's MVP section.

    Returns:
        A list of `GlobalRow` in checklist order.
    """
    rows = []
    for line in section:
        head = GLOBAL_ROW.match(line)
        module = MODULE_ROW.match(line)
        if head:
            rows.append(GlobalRow(head.group(3), head.group(1) == "x", head.group(2), head.group(4)))
        elif module and rows:
            rows[-1].modules.append(ModuleRow(module.group(3), module.group(1) == "x", module.group(2), module.group(4)))
    return rows


def parse_plan_rows(text: str) -> dict:
    """Return each task row of a plan file with its tick and Dependencies.

    Args:
        text: The plan Markdown.

    Returns:
        A dict from task ID to `PlanRow`, in file order.
    """
    rows = {}
    current = None
    for line in text.splitlines():
        row = PLAN_ROW.match(line)
        deps = PLAN_DEPS.match(line)
        if row:
            current = row.group(2).strip()
            rows[current] = PlanRow(row.group(1) == "x")
        elif deps and current and rows[current].deps is None:
            rows[current].deps = deps.group(1).strip()
    return rows


def expand_range(first: str, last: str, ordered_ids: list):
    """Return the IDs from `first` to `last` in plan order, or None if unknown.

    Args:
        first: First ID of the range.
        last: Last ID of the range.
        ordered_ids: Known module IDs in plan order.

    Returns:
        The inclusive slice of `ordered_ids`, or None when either end is
        unknown or the range runs backwards.
    """
    if first not in ordered_ids or last not in ordered_ids:
        return None
    start, stop = ordered_ids.index(first), ordered_ids.index(last)
    return ordered_ids[start:stop + 1] if start <= stop else None


def dependency_parts(text: str) -> list:
    """Split a Dependencies string into candidate references.

    Drops parenthetical notes, anything after the first `. ` or `:`, a
    trailing `for ...` explanation on each item, and items that are ADR
    references (`ADR 011 licence ...`), which are review notes, not tasks.

    Args:
        text: The Dependencies text without the bold label.

    Returns:
        The stripped, non-empty parts, one per referenced task.
    """
    cleaned = re.sub(r"\([^)]*\)", "", text)
    cleaned = cleaned.split(". ")[0].split(":")[0].strip().rstrip(".")
    parts = []
    for item in re.split(r"[;,]", cleaned):
        if re.match(r"ADR\s+\d+", item.strip()):
            continue
        head = re.split(r"\s+for\s+", item.strip())[0]
        parts.extend(p.strip() for p in re.split(r"\s+and\s+", head) if p.strip())
    return parts


def parse_dependencies(text: str, ordered_ids: list):
    """Parse a Dependencies string into references.

    Args:
        text: The Dependencies text.
        ordered_ids: Known module IDs in plan order, for `A through B` ranges.

    Returns:
        A list of `Dep` (empty for `None ...`), or None when any part cannot
        be parsed as a task reference.
    """
    if re.match(r"none\b", text.strip(), re.IGNORECASE):
        return []
    deps = []
    for part in dependency_parts(text):
        match = DEP_PART.match(part)
        if not match:
            return None
        flag, first, last = match.groups()
        ids = [first] if last is None else expand_range(first, last, ordered_ids)
        if ids is None:
            return None
        deps.extend(Dep(i, flag == "global" or i[0].isdigit()) for i in ids)
    return deps


def id_regex(task_id: str):
    """Return the compiled pattern that finds `task_id` in lower-cased text.

    A `.` in the ID also matches `-`. The ID must not be glued to other
    letters or digits, nor continue as `.N`/`-N` (so IO-0.1 never matches
    inside IO-0.10). A bare global number such as `0.4` needs a `task` token before it or an exact
    `<type>/<id>` branch prefix (`feat/0.4-write`); `release/0.4-rc`, a number
    in the middle of a branch or title, and a version never match.

    Args:
        task_id: A task ID from the checklist.

    Returns:
        A compiled regular expression.
    """
    body = re.escape(task_id.lower()).replace(r"\.", r"[.\-]")
    prefix = rf"(?:task[\s_\-/]*|^(?:{BRANCH_TYPES})/)" if task_id[0].isdigit() else ""
    return re.compile(rf"(?<![a-z0-9.]){prefix}{body}(?![a-z0-9])(?![.\-]\d)")


def match_pr_ids(prs: list, known_ids: list) -> dict:
    """Map task IDs to the lowest-numbered open PR that names them.

    Args:
        prs: Pull request dicts from the GitHub API (`number`, `title`, `head.ref`).
        known_ids: The real task IDs a PR may match.

    Returns:
        A dict from task ID to pull request number.
    """
    found = {}
    patterns = {task_id: id_regex(task_id) for task_id in known_ids}
    for pr in sorted(prs, key=lambda item: item["number"]):
        text = f"{pr['head']['ref']} {pr.get('title') or ''}".lower()
        for task_id, pattern in patterns.items():
            if task_id not in found and pattern.search(text):
                found[task_id] = pr["number"]
    return found


def only_unticked(pr_ids: dict, rows: list) -> dict:
    """Drop PR-derived IDs whose task is ticked (or not in the checklist) now.

    A recorded PR may have merged since the block was drawn; its task then
    counts as done, so it must neither be listed nor recorded again.

    Args:
        pr_ids: Task ID to PR number, fresh or read back from the block.
        rows: Parsed MVP global tasks.

    Returns:
        The entries of `pr_ids` whose task is an unticked global or module row.
    """
    unticked = {g.task_id for g in rows if not g.done} | {m.task_id for g in rows for m in g.modules if not m.done}
    return {task_id: number for task_id, number in pr_ids.items() if task_id in unticked}


def compute_in_progress(rows: list, pr_ids: dict) -> InProgress:
    """Select the tasks being worked on.

    Args:
        rows: Parsed MVP global tasks.
        pr_ids: Task ID to open PR number, for unticked tasks only.

    Returns:
        The In progress entries and the set of active IDs.
    """
    entries = []
    active = set()
    for task in rows:
        open_rows = [m for m in task.modules if not m.done]
        partial = 0 < len(open_rows) < len(task.modules)
        if not task.done and (partial or task.task_id in pr_ids):
            entries.append(Entry(task, open_rows, True))
            active.update([task.task_id, *(m.task_id for m in open_rows)])
            continue
        for module in (m for m in open_rows if m.task_id in pr_ids):
            entries.append(Entry(task, [module], False))
            active.add(module.task_id)
    return InProgress(entries, active)


def count_under_way(inprog: InProgress) -> int:
    """Count the distinct open global tasks the In progress list shows.

    A nested entry and a flat module entry (a PR on a module of an otherwise
    untouched global) both name their global task; two flat entries under one
    global count it once. A global whose box is already ticked is done, so a
    flat entry under it is not counted.

    Args:
        inprog: The In progress selection from `compute_in_progress`.

    Returns:
        The number of distinct unticked global tasks among the entries.
    """
    return len({entry.global_row.task_id for entry in inprog.entries if not entry.global_row.done})


@dataclass
class Context:
    """What dependency checks look up.

    Attributes:
        active: IDs counted as in progress.
        module_done: Module ID to ticked state, from plans and the checklist.
        globals_by_id: MVP global tasks by ID.
        global_done: Global ID to ticked state from the implementation plan.
    """

    active: set
    module_done: dict
    globals_by_id: dict
    global_done: dict


def global_met(task_id: str, ctx: Context):
    """Return True/False for a global task, or None when it is unknown."""
    row = ctx.globals_by_id.get(task_id)
    if row is None:
        return ctx.global_done.get(task_id)
    if row.done or task_id in ctx.active:
        return True
    return bool(row.modules) and all(m.done or m.task_id in ctx.active for m in row.modules)


def module_met(task_id: str, ctx: Context):
    """Return True/False for a module task, or None when it is unknown."""
    if task_id in ctx.active:
        return True
    return ctx.module_done.get(task_id)


def dependency_met(dep: Dep, ctx: Context) -> bool:
    """Return True when the dependency is ticked or in progress.

    Args:
        dep: The reference to check.
        ctx: The lookup tables.

    Returns:
        True when met; an ID found nowhere is not met.
    """
    checks = [global_met, module_met] if dep.is_global else [module_met, global_met]
    for check in checks:
        result = check(dep.task_id, ctx)
        if result is not None:
            return result
    return False


def row_ready(task_id: str, plan_rows: dict, ctx: Context, warnings: list) -> bool:
    """Return True when every listed dependency of a module row is met.

    Args:
        task_id: The module row's ID.
        plan_rows: All plan rows by ID, in plan order.
        ctx: The lookup tables.
        warnings: Appended to when the Dependencies text is missing or unparseable.

    Returns:
        True when ready; missing or unparseable Dependencies are not ready.
    """
    plan = plan_rows.get(task_id)
    if plan is None or plan.deps is None:
        warnings.append(f"Up next: {task_id} has no Dependencies line in its module plan; treated as not ready")
        return False
    deps = parse_dependencies(plan.deps, list(plan_rows))
    if deps is None:
        warnings.append(f"Up next: cannot parse the Dependencies of {task_id} ({plan.deps!r}); treated as not ready")
        return False
    return all(dependency_met(dep, ctx) for dep in deps)


def select_up_next(rows: list, active: set, plan_rows: dict, global_done: dict, limit: int = UP_NEXT_LIMIT):
    """Pick the next module tasks to start.

    Args:
        rows: Parsed MVP global tasks.
        active: IDs in progress.
        plan_rows: Every module plan row by ID, in plan order.
        global_done: Global ID to ticked state from the implementation plan.
        limit: How many tasks to return.

    Returns:
        A tuple of (list of `(ModuleRow, GlobalRow)` pairs in checklist order,
        list of warning strings).
    """
    module_done = {i: p.done for i, p in plan_rows.items()}
    module_done.update({m.task_id: m.done for g in rows for m in g.modules})
    ctx = Context(active, module_done, {g.task_id: g for g in rows}, global_done)
    picked, warnings = [], []
    for task in rows:
        for module in task.modules:
            if len(picked) >= limit:
                return picked, warnings
            if module.done or module.task_id in active:
                continue
            if row_ready(module.task_id, plan_rows, ctx, warnings):
                picked.append((module, task))
    return picked, warnings


def pr_link(number: int) -> str:
    """Return the Markdown link to pull request `number`."""
    return f"[PR #{number}]({PR_URL}{number})"


def render_entry(entry: Entry, pr_ids: dict) -> list:
    """Return the Markdown lines of one In progress item.

    Args:
        entry: The item.
        pr_ids: Task ID to open PR number.

    Returns:
        One line for a flat module item, or the global line plus one line per row.
    """
    task = entry.global_row
    if not entry.nested:
        module = entry.modules[0]
        return [f"- {module.link} {module.title} (part of {task.link}; {pr_link(pr_ids[module.task_id])})"]
    suffix = f" ({pr_link(pr_ids[task.task_id])})" if task.task_id in pr_ids else ""
    done = len(task.modules) - len(entry.modules)
    lines = [f"- {task.link} {task.title}{suffix}: {done} of {len(task.modules)} module rows ticked"]
    for module in entry.modules:
        extra = f" ({pr_link(pr_ids[module.task_id])})" if module.task_id in pr_ids else ""
        lines.append(f"    - {module.link} {module.title}{extra}")
    return lines


def format_recorded(pr_ids: dict) -> str:
    """Return the value of the `in-progress-prs` comment."""
    if not pr_ids:
        return "none"
    return ",".join(f"{task_id}=#{pr_ids[task_id]}" for task_id in sorted(pr_ids))


def parse_recorded(text: str):
    """Read the recorded PR IDs and timestamp back from a rendered block.

    Args:
        text: Any text containing the generated comments.

    Returns:
        A tuple of (dict from task ID to PR number, timestamp or None). A
        timestamp that is not RFC 3339 UTC (`YYYY-MM-DDTHH:MM:SSZ`) reads as None.
    """
    found = RECORDED_PRS.search(text)
    stamp = RECORDED_TIME.search(text)
    prs = {}
    value = found.group(1) if found else "none"
    for pair in (p for p in value.split(",") if "=#" in p):
        task_id, number = pair.strip().split("=#", 1)
        if number.isdigit():
            prs[task_id] = int(number)
    return prs, (stamp.group(1) if stamp else None)


def render_queue(inprog: InProgress, up_next: list, pr_ids: dict, timestamp: str, recorded=None) -> str:
    """Return the In progress and Up next Markdown for the progress block.

    Args:
        inprog: The In progress selection.
        up_next: `(ModuleRow, GlobalRow)` pairs from `select_up_next`.
        pr_ids: Task ID to open PR number, as recorded.
        timestamp: RFC 3339 UTC time the PR list was fetched.
        recorded: PR IDs to write into the `in-progress-prs` comment; defaults to
            `pr_ids`. `--check` passes the IDs it read back unchanged, so a recorded
            PR whose task has since been ticked does not make the block stale.

    Returns:
        Markdown ending with two parseable comments.
    """
    lines = ["**In progress**", ""]
    for entry in inprog.entries:
        lines.extend(render_entry(entry, pr_ids))
    if not inprog.entries:
        lines.append("- Nothing is under way right now.")
    lines += ["", f"**Up next** (the next {UP_NEXT_LIMIT} new module tasks, once their listed dependencies are done or in progress)", ""]
    for module, task in up_next:
        lines.append(f"- {module.link} {module.title} (part of {task.link})")
    if not up_next:
        lines.append("- No new module task is ready.")
    lines += [
        "",
        f"Generated {timestamp} (RFC 3339, UTC) from the ticked boxes, the module plans and the open pull requests at that time.",
        f"<!-- generated-at: {timestamp} -->",
        f"<!-- in-progress-prs: {format_recorded(pr_ids if recorded is None else recorded)} -->",
    ]
    return "\n".join(lines)
