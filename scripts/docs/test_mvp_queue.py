#!/usr/bin/env python3
"""Unit tests for the In progress and Up next logic in `mvp_queue.py`.

Run with `python3 -m unittest discover -s scripts/docs -p 'test_*.py'` from the
repository root; `scripts/verify_docs.sh` runs them as part of the docs gate.
"""
import json
import pathlib
import subprocess
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import mvp_queue as q  # noqa: E402
import update_mvp_progress as u  # noqa: E402

SECTION = """
1. [x] [Task 0.3](./implementation-plan.md#task-0.3) Read
    - [x] [IO-0.1](../a/plan.md#x) One
2. [ ] [Task 0.4](./implementation-plan.md#task-0.4) Write
    - [x] [IO-0.6](../a/plan.md#x) Six
    - [ ] [IO-0.9](../a/plan.md#x) Nine
    - [ ] [IO-0.10](../a/plan.md#x) Ten
3. [ ] [Task 0.5](./implementation-plan.md#task-0.5) Desktop
    - [ ] [DE-0.1](../d/plan.md#x) Caps
    - [ ] [DE-0.2](../d/plan.md#x) Catalog
4. [ ] [Task FS-3.9](./implementation-plan.md#task-fs-3.9) Gate
""".splitlines()


def rows():
    """Return the parsed sample section."""
    return q.parse_rows(SECTION)


def plan(done_ids, deps):
    """Build plan rows: IDs in `done_ids` ticked, `deps` maps ID to dependency text."""
    return {i: q.PlanRow(i in done_ids, deps[i]) for i in deps}


class ParseRows(unittest.TestCase):
    """Checklist row parsing."""

    def test_globals_and_modules(self):
        """Globals keep their module rows, ticks, links and titles."""
        parsed = rows()
        self.assertEqual([g.task_id for g in parsed], ["0.3", "0.4", "0.5", "FS-3.9"])
        self.assertTrue(parsed[0].done)
        self.assertEqual([m.task_id for m in parsed[1].modules], ["IO-0.6", "IO-0.9", "IO-0.10"])
        self.assertEqual(parsed[1].modules[1].link, "[IO-0.9](../a/plan.md#x)")
        self.assertEqual(parsed[1].modules[1].title, "Nine")


class ParsePlanRows(unittest.TestCase):
    """Module plan parsing."""

    def test_rows_and_dependencies(self):
        """Ticks and the Dependencies line are read per task."""
        text = (
            "- [x] **Task A-1.1: One**\n  - **Dependencies:** None\n\n"
            "- [ ] **Task A-1.2: Two**\n  - **Description:** x\n  - **Dependencies:** Task A-1.1\n\n"
            "- [ ] **Task A-1.3: Three**\n  - **Description:** y\n"
        )
        parsed = q.parse_plan_rows(text)
        self.assertEqual(list(parsed), ["A-1.1", "A-1.2", "A-1.3"])
        self.assertTrue(parsed["A-1.1"].done)
        self.assertEqual(parsed["A-1.2"].deps, "Task A-1.1")
        self.assertIsNone(parsed["A-1.3"].deps)


class ParseDependencies(unittest.TestCase):
    """Dependency string forms seen in the module plans."""

    ORDER = [f"CO-0.{n}" for n in range(1, 11)]

    def ids(self, text):
        """Return the parsed IDs, or None when unparseable."""
        deps = q.parse_dependencies(text, self.ORDER)
        return None if deps is None else [d.task_id for d in deps]

    def test_none_forms(self):
        """`None` with or without a note means no dependencies."""
        self.assertEqual(self.ids("None"), [])
        self.assertEqual(self.ids("None (scaffold exists)"), [])

    def test_plain_lists(self):
        """Commas, `Task` prefixes and semicolons separate IDs."""
        self.assertEqual(self.ids("Task IO-0.7, Task IO-0.9"), ["IO-0.7", "IO-0.9"])
        self.assertEqual(self.ids("API-F1.2, API-F1.7, Task IO-1.2"), ["API-F1.2", "API-F1.7", "IO-1.2"])
        self.assertEqual(self.ids("Task CL-0.2; Task IO-1.4 for the repair hook"), ["CL-0.2", "IO-1.4"])

    def test_trailing_prose_and_parentheses(self):
        """A sentence after the list and parenthetical notes are dropped."""
        text = "UI-F2.1a, API-F1.10. The indicator MAY land as soon as API-F1.10 is done."
        self.assertEqual(self.ids(text), ["UI-F2.1a", "API-F1.10"])
        self.assertEqual(self.ids("API-F2.2, SV-F1.4 (the server-only operators; later)"), ["API-F2.2", "SV-F1.4"])

    def test_global_flags(self):
        """`global Task` marks a global dependency, including after `and`."""
        deps = q.parse_dependencies("API-F1.2, global Task 0.5", self.ORDER)
        self.assertEqual([(d.task_id, d.is_global) for d in deps], [("API-F1.2", False), ("0.5", True)])
        deps = q.parse_dependencies("SV-F1.3 and global Task 0.4 for the fencing", self.ORDER)
        self.assertEqual([d.task_id for d in deps], ["SV-F1.3", "0.4"])
        self.assertEqual(self.ids("metadata Task MD-0.8 (file record)"), ["MD-0.8"])

    def test_adr_review_notes_are_ignored(self):
        """An ADR review clause after the IDs is not a task reference."""
        text = "DE-F0.1, global Task FS-2.2; ADR 011 licence and patent review of the DMG's linked inventory before distribution"
        self.assertEqual(self.ids(text), ["DE-F0.1", "FS-2.2"])

    def test_range_expands(self):
        """`A through B` expands over the known module order."""
        self.assertEqual(self.ids("Task CO-0.1 through CO-0.3"), ["CO-0.1", "CO-0.2", "CO-0.3"])
        self.assertIsNone(self.ids("Task CO-0.1 through ZZ-9.9"))

    def test_unparseable(self):
        """Prose that is not an ID list yields None."""
        self.assertIsNone(self.ids("F-047 gates"))
        self.assertIsNone(self.ids("patent review of the DMG"))
        self.assertIsNone(self.ids("global Tasks 7.1–7.5"))


class MatchPrs(unittest.TestCase):
    """Pull request to task ID matching."""

    IDS = ["IO-0.1", "IO-0.10", "IO-0.9", "UI-F2.1a", "UI-F2.1b", "0.4", "FS-1.2"]

    def match(self, ref, title="x", number=1):
        """Match one PR with the given head ref and title."""
        return q.match_pr_ids([{"number": number, "head": {"ref": ref}, "title": title}], self.IDS)

    def test_branch_forms(self):
        """Dots and dashes both match; case is ignored."""
        self.assertEqual(self.match("feat/io-0.9-enumerate-logical-state"), {"IO-0.9": 1})
        self.assertEqual(self.match("feat/io-0-9-enumerate"), {"IO-0.9": 1})
        self.assertEqual(self.match("x", "IO-0.9: enumerate"), {"IO-0.9": 1})

    def test_prefix_ids_do_not_collide(self):
        """IO-0.1 never matches inside IO-0.10, nor UI-F2.1a inside UI-F2.1b."""
        self.assertEqual(self.match("feat/io-0.10-delta"), {"IO-0.10": 1})
        self.assertEqual(self.match("feat/ui-f2.1b-tree"), {"UI-F2.1b": 1})

    def test_bare_global_needs_task_prefix(self):
        """A bare global number matches only after `Task`."""
        self.assertEqual(self.match("bump", "Update to 0.4"), {})
        self.assertEqual(self.match("feat/task-0.4-write"), {"0.4": 1})
        self.assertEqual(self.match("x", "Task FS-1.2 file ops"), {"FS-1.2": 1})

    def test_bare_global_needs_task_token_or_type_prefix(self):
        """`feat/0.4-write` names Task 0.4; release branches, bare starts and mid-branch numbers do not."""
        self.assertEqual(self.match("feat/0.4-write"), {"0.4": 1})
        self.assertEqual(self.match("build/0.4-write"), {"0.4": 1})
        self.assertEqual(self.match("release/0.4-rc"), {})
        self.assertEqual(self.match("0.4-write"), {})
        self.assertEqual(self.match("feat/bump-0.4"), {})
        self.assertEqual(self.match("release/0.4.1"), {})

    def test_lowest_pr_number_wins(self):
        """Two PRs for one task keep the lowest number."""
        prs = [
            {"number": 9, "head": {"ref": "io-0.9-b"}, "title": ""},
            {"number": 4, "head": {"ref": "io-0.9-a"}, "title": ""},
        ]
        self.assertEqual(q.match_pr_ids(prs, self.IDS), {"IO-0.9": 4})


class InProgress(unittest.TestCase):
    """In progress selection."""

    def test_partial_global_lists_unticked_rows(self):
        """A partly ticked global lists its unticked module rows."""
        result = q.compute_in_progress(rows(), {})
        self.assertEqual([e.global_row.task_id for e in result.entries], ["0.4"])
        self.assertEqual([m.task_id for m in result.entries[0].modules], ["IO-0.9", "IO-0.10"])
        self.assertEqual(result.active, {"IO-0.9", "IO-0.10", "0.4"})

    def test_pr_module_outside_partial_global(self):
        """A PR on a module of an untouched global lists just that module."""
        result = q.compute_in_progress(rows(), {"DE-0.2": 47})
        flat = [e for e in result.entries if not e.nested]
        self.assertEqual([(e.global_row.task_id, e.modules[0].task_id) for e in flat], [("0.5", "DE-0.2")])
        self.assertIn("DE-0.2", result.active)
        self.assertNotIn("DE-0.1", result.active)

    def test_pr_module_under_ticked_global(self):
        """An unticked module with a PR under a ticked global is not listed: the global is done."""
        parsed = rows()
        parsed[0].modules.append(q.ModuleRow("IO-0.2", False, "[IO-0.2](../a/plan.md#x)", "Two"))
        result = q.compute_in_progress(parsed, {"IO-0.2": 52})
        self.assertEqual([e for e in result.entries if e.global_row.done], [])
        self.assertNotIn("IO-0.2", result.active)

    def test_recorded_pr_of_a_since_ticked_task_is_dropped(self):
        """A recorded PR whose task is now ticked is neither listed nor kept."""
        parsed = rows()
        parsed[1].modules[1].done = True  # IO-0.9 merged and ticked on main
        kept = q.only_unticked({"IO-0.9": 48, "DE-0.1": 47, "0.3": 5, "GONE-1.1": 9}, parsed)
        self.assertEqual(kept, {"DE-0.1": 47})
        result = q.compute_in_progress(parsed, kept)
        listed = [m.task_id for e in result.entries for m in e.modules]
        self.assertNotIn("IO-0.9", listed)
        self.assertNotIn("IO-0.9", q.render_queue(result, [], kept, "2026-10-18T17:40:00Z"))

    def test_under_way_counts_flat_pr_only_entries(self):
        """A PR on a module of an untouched global counts its global once as under way."""
        result = q.compute_in_progress(rows(), {"DE-0.1": 46, "DE-0.2": 47})
        self.assertEqual([(e.global_row.task_id, e.nested) for e in result.entries], [("0.4", True), ("0.5", False), ("0.5", False)])
        self.assertEqual(q.count_under_way(result), 2)

    def test_under_way_skips_ticked_globals(self):
        """A flat entry under a ticked global is not counted; that global is already done."""
        parsed = rows()
        parsed[0].modules.append(q.ModuleRow("IO-0.2", False, "[IO-0.2](../a/plan.md#x)", "Two"))
        result = q.compute_in_progress(parsed, {"IO-0.2": 52})
        self.assertEqual(q.count_under_way(result), 1)

    def test_pr_on_global_without_modules(self):
        """A PR on a global task lists it, even with no module rows."""
        result = q.compute_in_progress(rows(), {"FS-3.9": 50})
        self.assertIn("FS-3.9", result.active)
        self.assertEqual(result.entries[-1].global_row.task_id, "FS-3.9")


class UnderWayCount(unittest.TestCase):
    """The "N more are under way" count, the In progress list and the prose agree."""

    def test_all_rows_ticked_but_global_open_is_under_way(self):
        """A global whose module rows are all ticked but whose box is open is listed and counted."""
        parsed = rows()
        for module in parsed[2].modules:  # Task 0.5
            module.done = True
        result = q.compute_in_progress(parsed, {})
        self.assertEqual([(e.global_row.task_id, e.nested) for e in result.entries], [("0.4", True), ("0.5", True)])
        self.assertEqual(result.entries[1].modules, [])
        self.assertEqual(q.count_under_way(result), 2)
        self.assertIn("2 of 2 module rows ticked", q.render_queue(result, [], {}, "2026-10-18T17:40:00Z"))

    def test_open_pull_request_on_open_rows_only_counts_once(self):
        """Two PRs on open rows of one untouched global count that global once."""
        result = q.compute_in_progress(rows(), {"DE-0.1": 46, "DE-0.2": 47})
        self.assertEqual(len([e for e in result.entries if e.global_row.task_id == "0.5"]), 2)
        self.assertEqual(q.count_under_way(result), 2)

    def test_no_ticked_rows_and_no_pull_request_is_not_under_way(self):
        """An open global with no ticked row and no PR is neither listed nor counted."""
        result = q.compute_in_progress(rows(), {})
        self.assertEqual([e.global_row.task_id for e in result.entries], ["0.4"])
        self.assertEqual(q.count_under_way(result), 1)

    def test_done_global_is_never_listed_or_counted(self):
        """A ticked global with an unticked row and a PR is done: not listed, not counted, not active."""
        parsed = rows()
        parsed[0].modules.append(q.ModuleRow("IO-0.2", False, "[IO-0.2](../a/plan.md#x)", "Two"))
        result = q.compute_in_progress(parsed, {"IO-0.2": 52})
        self.assertEqual([e.global_row.task_id for e in result.entries], ["0.4"])
        self.assertNotIn("IO-0.2", result.active)
        self.assertEqual(q.count_under_way(result), 1)

    def test_pr_on_partial_global_and_its_row_counts_once(self):
        """A PR on a partly ticked global and on one of its rows is one entry, one count."""
        result = q.compute_in_progress(rows(), {"0.4": 50, "IO-0.9": 51})
        self.assertEqual(len(result.entries), 1)
        self.assertEqual(q.count_under_way(result), 1)

    def test_count_matches_list_and_excludes_done(self):
        """The count equals the distinct open globals named by the list."""
        parsed = rows()
        result = q.compute_in_progress(parsed, {"DE-0.1": 46, "FS-3.9": 50})
        listed = {e.global_row.task_id for e in result.entries}
        open_ids = {g.task_id for g in parsed if not g.done}
        self.assertTrue(listed <= open_ids)
        self.assertEqual(q.count_under_way(result), len(listed))

    def test_summary_prose_defines_the_count(self):
        """The summary states the same definition the code uses."""
        block = u.render_summary([u.Task("0.4", False, 1, 2, 10)], "queue", 1)
        self.assertIn("with at least one module row ticked", block)
        self.assertNotIn("some but not all module rows ticked", block)


class UpNext(unittest.TestCase):
    """Up next selection."""

    def pick(self, deps, done=("IO-0.1", "IO-0.6"), prs=None, limit=5):
        """Run selection over the sample rows; return (ids, warnings)."""
        parsed = rows()
        inprog = q.compute_in_progress(parsed, prs or {})
        picked, warnings = q.select_up_next(parsed, inprog.active, plan(set(done), deps), {}, limit)
        return [m.task_id for m, _ in picked], warnings

    def test_in_progress_rows_skipped_and_count_as_met(self):
        """In-progress rows are skipped, and depending on them is allowed."""
        deps = {"IO-0.9": "Task IO-0.6", "IO-0.10": "Task IO-0.9", "DE-0.1": "Task IO-0.10", "DE-0.2": "Task DE-0.1"}
        ids, warnings = self.pick(deps)
        self.assertEqual(ids, ["DE-0.1"])
        self.assertEqual(warnings, [])

    def test_unticked_dependency_blocks(self):
        """A dependency that is neither ticked nor in progress is not ready."""
        ids, _ = self.pick({"DE-0.1": "Task DE-0.2", "DE-0.2": "None"})
        self.assertEqual(ids, ["DE-0.2"])

    def test_global_dependency_means_all_rows(self):
        """`global Task 0.5` is met only when all its rows are ticked or in progress."""
        ids, _ = self.pick({"DE-0.1": "global Task 0.5", "DE-0.2": "None"})
        self.assertEqual(ids, ["DE-0.2"])
        ids, _ = self.pick({"DE-0.1": "global Task 0.4", "DE-0.2": "global Task 0.5"})
        self.assertEqual(ids, ["DE-0.1"])

    def test_outside_checklist_by_tick_and_unknown_blocks(self):
        """A plan row outside the checklist counts by its tick; unknown IDs block."""
        parsed = rows()
        active = q.compute_in_progress(parsed, {}).active
        plan_rows = plan(set(), {"DE-0.1": "Task SV-F1.4", "DE-0.2": "Task ZZ-9.9", "SV-F1.4": "None"})
        picked, _ = q.select_up_next(parsed, active, plan_rows, {}, 5)
        self.assertEqual(picked, [])
        plan_rows["SV-F1.4"] = q.PlanRow(True, "None")
        picked, _ = q.select_up_next(parsed, active, plan_rows, {}, 5)
        self.assertEqual([m.task_id for m, _ in picked], ["DE-0.1"])

    def test_unparseable_warns_and_blocks(self):
        """An unparseable dependency string is not ready and warns once."""
        ids, warnings = self.pick({"DE-0.1": "F-047 gates", "DE-0.2": "None"})
        self.assertEqual(ids, ["DE-0.2"])
        self.assertEqual(len(warnings), 1)
        self.assertIn("DE-0.1", warnings[0])

    def test_missing_dependencies_line_warns(self):
        """A row with no Dependencies line is not ready and warns."""
        ids, warnings = self.pick({"DE-0.1": None, "DE-0.2": "None"})
        self.assertEqual(ids, ["DE-0.2"])
        self.assertEqual(len(warnings), 1)

    def test_limit(self):
        """No more than the limit is returned."""
        ids, _ = self.pick({"DE-0.1": "None", "DE-0.2": "None"}, limit=1)
        self.assertEqual(ids, ["DE-0.1"])


class Render(unittest.TestCase):
    """Block rendering and the recorded PR list."""

    def test_recorded_roundtrip(self):
        """The PR comment and timestamp survive a render and parse."""
        prs = {"DE-0.2": 47, "IO-0.9": 46}
        inprog = q.compute_in_progress(rows(), prs)
        text = q.render_queue(inprog, [], prs, "2026-10-18T17:40:00Z")
        self.assertIn("<!-- in-progress-prs: DE-0.2=#47,IO-0.9=#46 -->", text)
        self.assertIn("https://github.com/arkrim-tech/filesquid/pull/46", text)
        self.assertIn("2026-10-18T17:40:00Z", text)
        self.assertEqual(q.parse_recorded(text), (prs, "2026-10-18T17:40:00Z"))

    def test_malformed_timestamp_reads_as_missing(self):
        """Only RFC 3339 UTC timestamps are read back."""
        self.assertIsNone(q.parse_recorded("<!-- generated-at: yesterday -->")[1])
        self.assertIsNone(q.parse_recorded("<!-- generated-at: 2026-10-18T17:40:00+02:00 -->")[1])

    def test_check_keeps_recorded_ids_verbatim(self):
        """With `recorded` given (check mode) the comment is the one read back."""
        inprog = q.compute_in_progress(rows(), {})
        text = q.render_queue(inprog, [], {}, "2026-10-18T17:40:00Z", recorded={"IO-0.9": 48})
        self.assertIn("<!-- in-progress-prs: IO-0.9=#48 -->", text)
        self.assertNotIn("PR #48", text)

    def test_parse_recorded_empty(self):
        """A block without the comments records nothing."""
        self.assertEqual(q.parse_recorded("nothing"), ({}, None))

    def test_up_next_listed(self):
        """Up next rows carry their link and global task."""
        parsed = rows()
        inprog = q.compute_in_progress(parsed, {})
        text = q.render_queue(inprog, [(parsed[2].modules[0], parsed[2])], {}, "2026-10-18T17:40:00Z")
        self.assertIn("[DE-0.1](../d/plan.md#x) Caps", text)
        self.assertIn("[Task 0.5](./implementation-plan.md#task-0.5)", text)


class Fetch(unittest.TestCase):
    """GitHub fetch bounds and failure handling."""

    def run_fetch(self, **kwargs):
        """Call fetch_open_prs with subprocess.run patched; return (result, mock)."""
        with mock.patch.object(u.subprocess, "run", **kwargs) as run:
            return u.fetch_open_prs(), run

    def test_success_is_one_bounded_gh_call(self):
        """The call is `gh api` on the open-pulls endpoint with the 30 second timeout."""
        done = subprocess.CompletedProcess([], 0, stdout='[{"number": 1}]', stderr="")
        result, run = self.run_fetch(return_value=done)
        self.assertEqual(result, ([{"number": 1}], None))
        run.assert_called_once()
        command = run.call_args.args[0]
        self.assertEqual(command[:2], ["gh", "api"])
        self.assertIn("repos/arkrim-tech/filesquid/pulls?state=open&per_page=100", command[2])
        self.assertEqual(run.call_args.kwargs["timeout"], 30)

    def test_pages_are_flattened(self):
        """A full page is followed by the next one until a short page."""
        full = subprocess.CompletedProcess([], 0, stdout=json.dumps([{"number": n} for n in range(100)]), stderr="")
        short = subprocess.CompletedProcess([], 0, stdout='[{"number": 100}]', stderr="")
        (prs, cause), run = self.run_fetch(side_effect=[full, short])
        self.assertIsNone(cause)
        self.assertEqual(len(prs), 101)
        self.assertEqual(run.call_count, 2)
        self.assertIn("page=2", run.call_args.args[0][2])

    def test_page_cap_is_a_failure(self):
        """Endless full pages stop at the cap and report it instead of truncating."""
        full = subprocess.CompletedProcess([], 0, stdout=json.dumps([{"number": n} for n in range(100)]), stderr="")
        (prs, cause), run = self.run_fetch(return_value=full)
        self.assertIsNone(prs)
        self.assertIn("not all were fetched", cause)
        self.assertEqual(run.call_count, u.MAX_PULL_PAGES)

    def test_failures_report_cause(self):
        """Timeouts, a missing gh, a bad exit and bad JSON never raise."""
        cases = [
            {"side_effect": subprocess.TimeoutExpired("gh", 1)},
            {"side_effect": FileNotFoundError("gh")},
            {"return_value": subprocess.CompletedProcess([], 1, stdout="", stderr="auth required")},
            {"return_value": subprocess.CompletedProcess([], 0, stdout="not json", stderr="")},
            {"return_value": subprocess.CompletedProcess([], 0, stdout='{"a": 1}', stderr="")},
        ]
        for case in cases:
            (prs, cause), _ = self.run_fetch(**case)
            self.assertIsNone(prs)
            self.assertTrue(cause)


if __name__ == "__main__":
    unittest.main()
