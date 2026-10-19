#!/usr/bin/env python3
"""Check that every requirement is accounted for by an implementation plan task.

Purpose:
    The vision -> PRD step is checked by `check_prd_allocation.py` and the
    PRD -> SRS step by `scripts/verify_srs_traceability.sh`. This script checks
    the last step, SRS -> plan, for both products:

    1. FileSquid: every PRD requirement (`## F-###:`, `NFR-`, `TC-`, `RC-` in
       `resources/prd.md`) is cited, directly or as `SRS-<ID>`, in the Related
       Requirements of at least one active task in `resources/implementation-plan.md`
       or a module `apps/filesquid/**/resources/plan.md`.
    2. FileSquid modules: every module SRS row (`| <MOD>-S-### ...`) names a PRD
       or product-SRS requirement ID (`F-`, `NFR-`, `TC-`, optionally `SRS-`
       prefixed) in its last cell and is cited by a plan task. A bare link to
       prd.md or srs.md without a requirement ID does not count.
    3. ImageSquid: every requirement in `apps/imagesquid/resources/prd.md`, every
       SRS-UI surface and every IS-S/UI-S row in its SRS is cited by an
       ImageSquid plan task.

    A task whose Status line starts with "**Superseded" does not count. A
    citation range such as `MD-S-001–MD-S-004` counts for every ID in it.

Usage:
    python3 scripts/docs/check_plan_traceability.py

Returns:
    Exit code 0 when every requirement is accounted for, 1 otherwise. Each
    problem is printed on its own line.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
ID = r"(?:SRS-(?:F|NFR|TC|UI)|F|NFR|TC|RC|IS-S|UI-S|CORE-S|OCFL-S|MD-S|C2PA-S|PROV-S|API-S|DE-S|SV-S|BE-S)-\d+"
LINKED = rf"\[?(?:PRD )?({ID})\]?(?:\([^)]*\))?"
RANGE = re.compile(rf"{LINKED}\s*[–-]\s*{LINKED}")
TASK = re.compile(r"^- \[[ x]\] \*\*Task ([A-Za-z0-9.\-]+):")
PARENT = re.compile(r"(?<![\w-])(?:SRS-)?(?:F|NFR|TC)-\d+\b")


def ids_in(text: str) -> set:
    """Return every requirement ID in `text`, expanding same-prefix ranges.

    Args:
        text: Any Markdown text.

    Returns:
        The set of IDs mentioned, with `A–B` ranges filled in.
    """
    out = set(re.findall(rf"(?<![\w-])({ID})\b", text))
    for first, last in RANGE.findall(text):
        prefix, start = first.rsplit("-", 1)
        prefix_b, end = last.rsplit("-", 1)
        if prefix == prefix_b and int(start) <= int(end):
            out |= {f"{prefix}-{n:0{len(start)}d}" for n in range(int(start), int(end) + 1)}
    return out


def active_task_citations(paths: list) -> set:
    """Return every ID cited in the Related Requirements of active tasks.

    Args:
        paths: Plan files to read.

    Returns:
        IDs cited by at least one task that is not superseded.
    """
    cited = set()
    for path in paths:
        body, superseded = [], False
        for line in path.read_text().splitlines() + ["- [ ] **Task END:"]:
            if TASK.match(line):
                cited |= set() if superseded else ids_in("\n".join(body))
                body, superseded = [], False
                continue
            superseded = superseded or line.strip().startswith("- **Status:** **Superseded")
            if line.strip().startswith("- **Related Requirements:**"):
                body.append(line)
    return cited


def prd_ids(path: pathlib.Path) -> list:
    """Return the requirement IDs that head a section of a PRD.

    Args:
        path: A PRD Markdown file.

    Returns:
        IDs from headings such as `## F-001: Title`.
    """
    return re.findall(r"^## ((?:F|NFR|TC|RC)-\d+):", path.read_text(), re.M)


def check_filesquid() -> list:
    """Return problems in the FileSquid PRD -> plan and module SRS -> plan steps."""
    plans = [ROOT / "resources" / "implementation-plan.md",
             *sorted((ROOT / "apps" / "filesquid").glob("**/resources/plan.md"))]
    cited = active_task_citations(plans)
    problems = [f"FileSquid PRD {rid}: no active plan task cites {rid} or SRS-{rid}"
                for rid in prd_ids(ROOT / "resources" / "prd.md")
                if rid not in cited and f"SRS-{rid}" not in cited]
    for srs in sorted((ROOT / "apps" / "filesquid").glob("**/resources/srs.md")):
        for line in srs.read_text().splitlines():
            row = re.match(r"^\|\s*((?:CORE|OCFL|MD|C2PA|PROV|API|DE|SV|BE|UI)-S-\d+)\b", line)
            if not row:
                continue
            rel = srs.relative_to(ROOT)
            if not PARENT.search(line.rstrip(" |").rsplit("|", 1)[-1]):
                problems.append(f"{rel} {row.group(1)}: parent names no PRD or product-SRS requirement")
            if row.group(1) not in cited:
                problems.append(f"{rel} {row.group(1)}: no active plan task cites it")
    return problems


def check_imagesquid() -> list:
    """Return problems in the ImageSquid PRD/SRS -> plan step."""
    base = ROOT / "apps" / "imagesquid" / "resources"
    cited = active_task_citations([base / "plan.md"])
    srs = (base / "srs.md").read_text()
    wanted = prd_ids(base / "prd.md")
    wanted += re.findall(r"^### (SRS-UI-\d+):", srs, re.M)
    wanted += re.findall(r"^\| ((?:IS|UI)-S-\d+) ", srs, re.M)
    return [f"ImageSquid {rid}: no active ImageSquid plan task cites it" for rid in wanted if rid not in cited]


def main() -> int:
    """Run both product checks and print a summary.

    Returns:
        Process exit code: 0 when clean, 1 when any problem was found.
    """
    problems = check_filesquid() + check_imagesquid()
    for problem in problems:
        print(problem)
    print("OK: every requirement is accounted for in a plan task" if not problems
          else f"{len(problems)} requirement(s) not accounted for in a plan task")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
