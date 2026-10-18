#!/usr/bin/env python3
"""Integration tests for the MVP progress merge driver, `merge_mvp_progress.py`.

Purpose:
    Each test builds a throwaway git repository under the system temporary
    directory (`TMPDIR` is honoured), with a small synthetic implementation
    plan, module plans, checklist and the three scripts copied in. Two branches
    tick boxes and regenerate the progress block offline, then they are merged
    with real `git merge` runs. The repository is deleted on every exit path
    (`addCleanup`), and git runs with the global and system configuration
    ignored, so a developer's own settings never change the result.

Run with `python3 -m unittest discover -s scripts/docs -p 'test_*.py'` from the
repository root; `scripts/verify_docs.sh` runs them as part of the docs gate.
"""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import merge_mvp_progress as driver  # noqa: E402

GIT_TIMEOUT_SECONDS = 60
SCRIPTS = ("update_mvp_progress.py", "mvp_queue.py", "merge_mvp_progress.py")
CHECKLIST = "resources/implementation-plan-checklist.md"
SVG = "resources/assets/mvp-progress.svg"
DRIVER_COMMAND = "python3 scripts/docs/merge_mvp_progress.py %O %A %B %L %P"

PLAN = """# Implementation Plan

- [ ] **Task 0.1: Alpha** <a name="task-0.1"></a>
  - **Estimated Time:** 10 hours
- [ ] **Task 0.2: Beta** <a name="task-0.2"></a>
  - **Estimated Time:** 30 hours
"""

MODULE_PLAN = "# Core plan\n\n" + "".join(
    f"- [ ] **Task CO-0.{n}: Row {n}**\n  - **Dependencies:** {'None' if n == 1 else f'CO-0.{n - 1}'}\n" for n in range(1, 8)
)

UI_PLAN = "# UI plan\n"

ROWS_ALPHA = "".join(f"    - [ ] [CO-0.{n}](../apps/filesquid/crates/core/resources/plan.md) Row {n}\n" for n in range(1, 6))
ROWS_BETA = "".join(f"    - [ ] [CO-0.{n}](../apps/filesquid/crates/core/resources/plan.md) Row {n}\n" for n in range(6, 8))

CHECKLIST_TEXT = f"""# Checklist

<!-- mvp-progress:start -->
<!-- generated-at: 2026-01-01T00:00:00Z -->
<!-- in-progress-prs: none -->
<!-- mvp-progress:end -->

Tick boxes below.

## MVP

1. [ ] [Task 0.1](./implementation-plan.md#task-0.1) Alpha
{ROWS_ALPHA}2. [ ] [Task 0.2](./implementation-plan.md#task-0.2) Beta
{ROWS_BETA}
## Later

- A note that two branches may edit.
"""

REGENERATE = (
    "import pathlib, sys; sys.path.insert(0, 'scripts/docs'); import update_mvp_progress as u; "
    "out, problems = u.expected_outputs(u.CHECKLIST.read_text(encoding='utf-8'), True); "
    "assert not problems, problems; "
    "[p.write_text(t, encoding='utf-8') for p, t in zip((u.CHECKLIST, u.SVG), out)]"
)


def isolated_env() -> dict:
    """Return an environment where git ignores the global and system config."""
    return dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0")


class Repo:
    """A throwaway git repository with the fixture files committed on `main`.

    Attributes:
        path: The repository's top-level directory.
    """

    def __init__(self, path: pathlib.Path):
        """Remember the repository directory; `create` fills it."""
        self.path = path

    def run(self, *args, check=True):
        """Run a command in the repository with a time limit and return the result.

        Args:
            args: The command and its arguments.
            check: Raise `AssertionError` with the output when it exits non-zero.

        Returns:
            The `subprocess.CompletedProcess`.
        """
        done = subprocess.run(
            list(args), cwd=self.path, env=isolated_env(), capture_output=True, text=True, timeout=GIT_TIMEOUT_SECONDS, check=False
        )
        if check and done.returncode != 0:
            raise AssertionError(f"{args} exited {done.returncode}:\n{done.stdout}\n{done.stderr}")
        return done

    def git(self, *args, check=True):
        """Run `git` with `args` in the repository."""
        return self.run("git", *args, check=check)

    def write(self, relative: str, text: str):
        """Write `text` to a repository file, creating its folders."""
        target = self.path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(text.encode("utf-8"))

    def read(self, relative: str) -> str:
        """Return the text of a repository file."""
        return (self.path / relative).read_bytes().decode("utf-8")

    def create(self, with_driver: bool):
        """Write the fixture, regenerate the block offline and commit it on `main`.

        Args:
            with_driver: Register the merge driver in the repository's config,
                exactly as the README documents.
        """
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.invalid")
        for name in SCRIPTS:
            self.write(f"scripts/docs/{name}", (HERE / name).read_text(encoding="utf-8"))
        self.write("resources/implementation-plan.md", PLAN)
        self.write("apps/filesquid/crates/core/resources/plan.md", MODULE_PLAN)
        self.write("apps/filesquid/ui/resources/plan.md", UI_PLAN)
        self.write(CHECKLIST, CHECKLIST_TEXT)
        self.write(SVG, "")
        self.write(".gitattributes", (HERE.parents[1] / ".gitattributes").read_text(encoding="utf-8"))
        self.regenerate()
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "base")
        if with_driver:
            self.git("config", "merge.mvp-progress.name", "MVP progress block")
            self.git("config", "merge.mvp-progress.driver", DRIVER_COMMAND)

    def regenerate(self):
        """Redraw the block and the SVG offline, as `update_mvp_progress.py --check` expects."""
        self.run(sys.executable, "-c", REGENERATE)

    def tick(self, task_id: str):
        """Tick the module row `[task_id]` or the global row `[Task task_id]`; fail when neither is open."""
        text = self.read(CHECKLIST)
        markers = [(f"- [ ] [{task_id}]", f"- [x] [{task_id}]"), (f". [ ] [Task {task_id}]", f". [x] [Task {task_id}]")]
        found = [(old, new) for old, new in markers if old in text]
        if not found:
            raise AssertionError(f"no open checklist row for {task_id}")
        self.write(CHECKLIST, text.replace(found[0][0], found[0][1], 1))

    def record(self, prs: str, stamp: str):
        """Set the recorded PR IDs and timestamp in the block, as write mode would."""
        text = self.read(CHECKLIST)
        head, rest = text.split("<!-- generated-at: ", 1)
        rest = rest.split("<!-- mvp-progress:end -->", 1)[1]
        block = f"<!-- generated-at: {stamp} -->\n<!-- in-progress-prs: {prs} -->\n<!-- mvp-progress:end -->"
        self.write(CHECKLIST, head + block + rest)

    def branch(self, name: str, ticks: list, prs: str = "none", stamp: str = "2026-01-01T00:00:00Z", note: str = ""):
        """Create branch `name` from `main` that ticks `ticks`, records `prs` and commits.

        Args:
            name: Branch name.
            ticks: Module or global IDs to tick.
            prs: Value for the `in-progress-prs` comment.
            stamp: Value for the `generated-at` comment.
            note: Replacement text for the note under `## Later`, if any.
        """
        self.git("checkout", "-q", "-b", name, "main")
        for task_id in ticks:
            self.tick(task_id)
        if note:
            self.write(CHECKLIST, self.read(CHECKLIST).replace("- A note that two branches may edit.", note))
        self.record(prs, stamp)
        self.regenerate()
        self.git("commit", "-q", "-a", "-m", f"tick {name}")
        self.git("checkout", "-q", "main")

    def merge(self, ours: str, theirs: str, *strategy):
        """Check out `ours` and merge `theirs` into it; return the merge result."""
        self.git("checkout", "-q", ours)
        return self.git("merge", "--no-edit", *strategy, theirs, check=False)

    def check(self):
        """Return the result of the offline `update_mvp_progress.py --check`."""
        return self.run(sys.executable, "scripts/docs/update_mvp_progress.py", "--check", check=False)


class MergeDriverTest(unittest.TestCase):
    """Shared fixture: a fresh repository per test, removed on every exit path."""

    with_driver = True

    def setUp(self):
        """Create the repository under the temporary directory."""
        folder = tempfile.mkdtemp(prefix="mvp-merge-test-")
        self.addCleanup(shutil.rmtree, folder, ignore_errors=True)
        self.repo = Repo(pathlib.Path(folder))
        self.repo.create(self.with_driver)

    def assert_clean_and_current(self, result):
        """Assert the merge succeeded, left no markers and both files are current."""
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("<<<<<<<", self.repo.read(CHECKLIST))
        self.assertNotIn("<<<<<<<", self.repo.read(SVG))
        check = self.repo.check()
        self.assertEqual(check.returncode, 0, check.stdout + check.stderr)


class CleanMerge(MergeDriverTest):
    """(a) and (b): different ticks merge cleanly, whatever the order."""

    def test_different_ticks_merge_into_the_offline_block(self):
        """The merged block and SVG equal what the offline generator draws from the merged ticks."""
        self.repo.branch("a", ["CO-0.1"])
        self.repo.branch("b", ["CO-0.4"])
        self.assert_clean_and_current(self.repo.merge("a", "b"))
        text = self.repo.read(CHECKLIST)
        self.assertIn("- [x] [CO-0.1]", text)
        self.assertIn("- [x] [CO-0.4]", text)
        self.assertIn("**MVP progress: 10%**", text)
        self.assertIn("CO-0.1 [x] CO-0.2 [ ] CO-0.3 [ ] CO-0.4 [x]", self.repo.read(SVG))

    def test_merge_from_a_subdirectory(self):
        """Git runs the driver from the top-level directory, so its relative path works anywhere."""
        self.repo.branch("a", ["CO-0.1"])
        self.repo.branch("b", ["CO-0.4"])
        self.repo.git("checkout", "-q", "a")
        result = Repo(self.repo.path / "resources").git("merge", "--no-edit", "b", check=False)
        self.assert_clean_and_current(result)

    def test_merge_direction_does_not_matter(self):
        """Merging a into b gives the same checklist and SVG as merging b into a."""
        self.repo.branch("a", ["CO-0.1", "CO-0.6"])
        self.repo.branch("b", ["CO-0.4"])
        self.repo.git("branch", "a2", "a")
        self.assert_clean_and_current(self.repo.merge("a", "b"))
        first = (self.repo.read(CHECKLIST), self.repo.read(SVG))
        self.assert_clean_and_current(self.repo.merge("b", "a2"))
        self.assertEqual((self.repo.read(CHECKLIST), self.repo.read(SVG)), first)

    def test_both_strategies_give_the_same_result(self):
        """The default strategy and `-s recursive` (which may visit files in another order) agree."""
        self.repo.branch("a", ["CO-0.1"])
        self.repo.branch("b", ["CO-0.4", "0.2"])
        self.repo.git("branch", "a2", "a")
        self.assert_clean_and_current(self.repo.merge("a", "b"))
        first = (self.repo.read(CHECKLIST), self.repo.read(SVG))
        self.assert_clean_and_current(self.repo.merge("a2", "b", "-s", "recursive"))
        self.assertEqual((self.repo.read(CHECKLIST), self.repo.read(SVG)), first)

    def test_driver_call_order_does_not_matter(self):
        """Running the SVG driver before or after the checklist driver gives the same files."""
        self.repo.branch("a", ["CO-0.1"])
        self.repo.branch("b", ["CO-0.4"])
        results = [self.run_drivers(order) for order in ((CHECKLIST, SVG), (SVG, CHECKLIST))]
        self.assertEqual(results[0], results[1])

    def run_drivers(self, order: tuple) -> dict:
        """Run the driver on base/a/b blobs of each file in `order`; return the outputs."""
        base = self.repo.git("merge-base", "a", "b").stdout.strip()
        outputs = {}
        for path in order:
            files = []
            for index, rev in enumerate((base, "a", "b")):
                blob = self.repo.git("show", f"{rev}:{path}").stdout
                files.append(self.repo.path / f".driver-{index}")
                files[-1].write_bytes(blob.encode("utf-8"))
            code = self.repo.run(sys.executable, "scripts/docs/merge_mvp_progress.py", *map(str, files), "7", path, check=False)
            self.assertEqual(code.returncode, 0, code.stderr)
            outputs[path] = files[1].read_bytes()
        return outputs


class RealConflict(MergeDriverTest):
    """(c): a conflict outside the block still fails the merge."""

    def test_same_line_edited_differently_conflicts(self):
        """The merge fails, markers stay around the edited line and the block is valid."""
        self.repo.branch("a", ["CO-0.1"], note="- Note from branch a.")
        self.repo.branch("b", ["CO-0.4"], note="- Note from branch b.")
        result = self.repo.merge("a", "b")
        self.assertNotEqual(result.returncode, 0)
        text = self.repo.read(CHECKLIST)
        self.assertIn("<<<<<<<", text)
        self.assertIn("- Note from branch a.", text)
        self.assertIn("- Note from branch b.", text)
        self.assertEqual(text.count("<!-- mvp-progress:start -->"), 1)
        self.assertEqual(text.count("<!-- mvp-progress:end -->"), 1)
        self.assertIn(CHECKLIST, self.repo.git("diff", "--name-only", "--diff-filter=U").stdout)


class RecordedPrs(MergeDriverTest):
    """(d): recorded PR IDs merge as the union, minus ticked tasks, with the newer time."""

    def test_union_filtered_by_ticks(self):
        """A recorded PR for a task the other side ticked is dropped."""
        self.repo.branch("a", ["CO-0.1"], prs="CO-0.2=#11,CO-0.7=#14", stamp="2026-01-02T00:00:00Z")
        self.repo.branch("b", ["CO-0.4"], prs="CO-0.1=#10,CO-0.5=#12", stamp="2026-02-01T00:00:00Z")
        self.assert_clean_and_current(self.repo.merge("a", "b"))
        text = self.repo.read(CHECKLIST)
        self.assertIn("<!-- in-progress-prs: CO-0.2=#11,CO-0.5=#12,CO-0.7=#14 -->", text)
        self.assertIn("<!-- generated-at: 2026-02-01T00:00:00Z -->", text)


class WithoutConfig(MergeDriverTest):
    """(e): without the git config the attribute falls back to the normal text merge."""

    with_driver = False

    def test_undefined_driver_is_the_builtin_text_merge(self):
        """Both branches redraw the block, so the plain text merge conflicts there."""
        self.repo.branch("a", ["CO-0.1"])
        self.repo.branch("b", ["CO-0.4"])
        result = self.repo.merge("a", "b")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("<<<<<<<", self.repo.read(CHECKLIST))
        conflicted = self.repo.git("diff", "--name-only", "--diff-filter=U").stdout
        self.assertIn(CHECKLIST, conflicted)
        self.assertIn(SVG, conflicted)


class Unit(unittest.TestCase):
    """Pure helpers of the driver."""

    def test_pick_three_way(self):
        """A side that changed wins; two different changes conflict."""
        self.assertEqual(driver.pick(1, 1, 2), 2)
        self.assertEqual(driver.pick(1, 2, 1), 2)
        self.assertEqual(driver.pick(1, 3, 3), 3)
        self.assertIs(driver.pick(1, 2, 3), driver.CONFLICT)

    def test_union_keeps_lowest_pr_and_newer_time(self):
        """The same task recorded with two PRs keeps the lower number."""
        ours = "<!-- generated-at: 2026-01-01T00:00:00Z -->\n<!-- in-progress-prs: X-1=#9 -->"
        theirs = "<!-- generated-at: 2026-03-01T00:00:00Z -->\n<!-- in-progress-prs: X-1=#4,Y-2=#5 -->"
        self.assertEqual(driver.merged_recorded(ours, theirs), ({"X-1": 4, "Y-2": 5}, "2026-03-01T00:00:00Z"))
        self.assertEqual(driver.merged_recorded(None, None), ({}, None))

    def test_tick_record_round_trip(self):
        """The SVG tick record reads back to the same tasks; an SVG without one reads as None."""
        tasks = [driver.progress.Task("0.1", False, 1, 2, 10, [("CO-0.1", True), ("CO-0.2", False)]),
                 driver.progress.Task("FS-3.9", True, 0, 0, 12, [])]
        svg = driver.progress.render_svg(tasks, driver.progress.percent_done(tasks))
        self.assertEqual(driver.progress.parse_tick_record(svg), tasks)
        self.assertIsNone(driver.progress.parse_tick_record("<svg></svg>"))
        self.assertIsNone(driver.progress.parse_tick_record(svg.replace("CO-0.2 [ ]", "CO-0.2 [?]")))

    def test_rows_changed_differently_do_not_merge(self):
        """The same row ticked on one side and changed in hours on the other still merges; a clash does not."""
        task = driver.progress.Task
        base = [task("0.1", False, 0, 1, 10, [("CO-0.1", False)])]
        ours = [task("0.1", False, 1, 1, 10, [("CO-0.1", True)])]
        theirs = [task("0.1", False, 0, 1, 12, [("CO-0.1", False)])]
        self.assertEqual(driver.merge_ticks(base, ours, theirs), [task("0.1", False, 1, 1, 12, [("CO-0.1", True)])])
        self.assertIsNone(driver.merge_ticks(base, ours, [task("0.1", False, 0, 1, 10, [("CO-0.2", False)])]))


if __name__ == "__main__":
    unittest.main()
