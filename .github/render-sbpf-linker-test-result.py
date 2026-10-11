#!/usr/bin/env python3
"""Render the comment that reports a test of this sbpf against sbpf-linker to stdout."""

import argparse
import textwrap
from string import Template

RESULTS = ("success", "failure")
LINKER = "sbpf-linker"
LINKER_REPO = f"blueshift-gg/{LINKER}"


def template(text: str) -> Template:
    return Template(textwrap.dedent(text).strip())


TEMPLATES = {
    "linker-pr-success": lambda **v: template("""
        $subject builds and passes its tests against this PR.

        **Maintainers:** This PR is ready to be merged. Please merge $subject after this PR lands.
        """).substitute(**v),

    "linker-pr-failure": lambda **v: template("""
        $subject does not build or pass its tests against this PR.

        Push the fix to $subject and ask a maintainer to retest with `/test-sbpf-linker $pr`.
        """).substitute(**v),

    # Unused at the moment. Left to make the result renderer robust against all ref-result combo.
    "linker-next-success": lambda **v: template("""
        `$linker_ref` builds and passes its tests against this PR.
        """).substitute(**v),
    "linker-next-failure": lambda **v: template("""
        $subject does not build or pass its tests against this PR.

        If the failure is the linker needing to adapt to a change here:

        1. Branch off [`$linker_ref`](https://github.com/$linker_repo/tree/$linker_ref) and apply the linker-side fix there.
        2. Open a PR in sbpf-linker against that branch, and comment its number here.

        **Maintainers:** retest this PR against that linker PR with `/test-sbpf-linker <pr>`; it does
        not turn this check green, but it shows the adaptation works.
        """).substitute(**v),
}


parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
parser.add_argument("--result", required=True, choices=RESULTS)
parser.add_argument("--linker-ref", required=True,
                    help="the blueshift-gg/sbpf-linker ref this was tested against")
parser.add_argument("--run-url", default="",
                    help="the run to link to, if the caller has one")
parser.add_argument("--comment-marker", default="",
                    help="identifies the comment, for a caller that posts it only once")
args = parser.parse_args()
result, linker_ref = args.result, args.linker_ref
url = args.run_url

linker_pr = linker_ref.startswith("refs/pull/")
pr = linker_ref.removeprefix("refs/pull/").removesuffix("/head")

body = TEMPLATES[f"{'linker-pr' if linker_pr else 'linker-next'}-{result}"](
    linker_ref=linker_ref, linker_repo=LINKER_REPO,
    pr=pr, subject=f"{LINKER_REPO}#{pr}" if linker_pr else LINKER,
)

parts = [args.comment_marker] if args.comment_marker else []
parts.append(body)
if url:
    parts.append(f"[run log]({url})")

print("\n\n".join(parts))
