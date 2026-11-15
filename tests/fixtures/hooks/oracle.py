"""Run the frozen ops entry/function, without SessionStart's repair/index main."""
import json
import os
import pathlib
import runpy
import sys

SOURCE = pathlib.Path(__file__).parent / "sources" / "ops" / "hooks"
if sys.argv[1] == "hq-evidence":
    old = runpy.run_path(str(SOURCE.parent.parent / "hq/harnesses/agy/hook"), run_name="displaced_policy")
    ops = runpy.run_path(str(SOURCE / "claim-guard"), run_name="policy_inputs")
    reason = old["decide_stop"](sys.stdin.read(), ops["read_claims"], lambda: ops["read_facts"]("agy"))
    print(json.dumps({"decision": "deny" if reason else "allow", "reason": reason,
                      "stage": "displaced-inline-function", "owner_line": old["OWNER_LINE"]}))
elif sys.argv[1] == "guard":
    harness = sys.argv[2]
    sys.argv = [str(SOURCE / "claim-guard"), "--harness", harness]
    runpy.run_path(sys.argv[0], run_name="__main__")
else:
    hook = runpy.run_path(str(SOURCE / "claude-sessionstart"), run_name="fixture_oracle")
    env = {key: os.environ[key] for key in (
        "PATH", "HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "USER", "LC_ALL",
        "GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM", "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL",
        "GIT_CEILING_DIRECTORIES", "TASKS_PALETTE")}
    context = hook["halt_notice"](env, sys.argv[2])
    print(json.dumps({"context": context} if context else {}))
