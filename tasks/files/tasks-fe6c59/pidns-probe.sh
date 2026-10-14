#!/usr/bin/env bash
# Disposable PID-namespace probe for relay process proof (tasks-fe6c59).
# Usage: probe.sh <tasks-binary> <out.jsonl>
# Optional: PROBE_ROOT (scratch parent), PROBE_CODEX_HOME (a private CODEX_HOME enables the
# genuine `codex sandbox` cases; no model call), KEEP=1 (keep the scratch dir).
# Everything lives under a fresh scratch dir: task registry/config, claim state, relay
# registry. Harness actors are bash copies named `codex`, so the kernel comm is `codex`.
# Registry rows are SYNTHETIC fixtures written by this script, never producer delivery.
set -u
TASKS=$(readlink -f "$1")
OUT=$(readlink -f "$2")
: >"$OUT"
S=$(mktemp -d -p "${PROBE_ROOT:-${TMPDIR:-/tmp}}" pidns-probe.XXXX)
PIDS=()
cleanup() {
  for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null; done
  wait 2>/dev/null
  sleep 7  # the detached pid-1 namespace in ns-explicit-pid1 sleeps 6s, then exits
  [ -n "${KEEP:-}" ] || rm -rf "$S"
}
trap cleanup EXIT TERM INT

# Isolated environment: no inherited harness identity, private config/state/relay.
for v in $(env | sed -n 's/^\(CLAUDE[A-Z_]*\|CODEX[A-Z_]*\|TASKS_[A-Z_]*\)=.*/\1/p'); do unset "$v"; done
export XDG_CONFIG_HOME=$S/config XDG_STATE_HOME=$S/state RELAY_STATE_DIR=$S/relay TMPDIR=$S/tmp
mkdir -p "$XDG_CONFIG_HOME/tasks" "$XDG_STATE_HOME" "$RELAY_STATE_DIR" "$TMPDIR" "$S/bin" "$S/q" "$S/proj"
chmod 700 "$RELAY_STATE_DIR"
printf '[identity]\nrelay = true\n' >"$XDG_CONFIG_HOME/tasks/config.toml"
cp /usr/bin/bash "$S/bin/codex"
BOOT=$(cat /proc/sys/kernel/random/boot_id)
HOST=$(cat /proc/sys/kernel/hostname)
export S TASKS BOOT HOST

(cd "$S/proj" && git init -q && TASKS_SESSION=setup "$TASKS" init --prefix pns >/dev/null)
for t in T1 T2 T3 T4 T5 T6 T7; do
  id=$(cd "$S/proj" && TASKS_SESSION=setup "$TASKS" add "probe $t" -p 2 | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["id"])')
  echo "$id" >"$S/$t.id"
done

# A long-lived harness actor: runs queued step scripts as its own children.
cat >"$S/harness.sh" <<'EOF'
name=$1
cd "$S/proj" || exit 1
echo "$$ $(awk '{print $22}' /proc/$$/stat)" > "$S/$name.handle"
n=0
while :; do
  f="$S/q/$name.$n.sh"
  if [ -e "$f.go" ]; then
    # No subshell: a fork of this bash would keep the comm `codex` and become the
    # nearest harness itself. `bash` execs a fresh image, so its comm is `bash`.
    bash "$f" >"$f.out" 2>&1; echo $? >"$f.rc"
    n=$((n+1))
  fi
  sleep 0.05
done
EOF

start_actor() { # name thread [session]
  local name=$1 thread=$2 session=${3:-$2}
  CODEX_THREAD_ID=$thread CODEX_SESSION_ID=$session "$S/bin/codex" "$S/harness.sh" "$name" &
  PIDS+=($!)
  while [ ! -s "$S/$name.handle" ]; do sleep 0.05; done
  echo 0 >"$S/$name.n"
}

run_as() { # name script-text -> prints rc and output
  local name=$1 n f
  n=$(cat "$S/$name.n"); f="$S/q/$name.$n.sh"
  printf '%s\n' "$2" >"$f"; touch "$f.go"; echo $((n+1)) >"$S/$name.n"
  for _ in $(seq 1 600); do [ -e "$f.rc" ] && break; sleep 0.05; done
  RC=$(cat "$f.rc" 2>/dev/null || echo timeout); OUTPUT=$(cat "$f.out" 2>/dev/null)
}

# Rows: name:session[:startdelta[:boot]] ... Writes a schema-2 registry from actor handles.
register() {
  python3 -I - "$@" <<'EOF'
import json, os, sys, uuid
S = os.environ["S"]; agents = {}
for spec in sys.argv[1:]:
    parts = spec.split(":")
    name, session = parts[0], parts[1]
    delta = int(parts[2]) if len(parts) > 2 and parts[2] else 0
    boot = parts[3] if len(parts) > 3 else os.environ["BOOT"]
    pid, start = open(f"{S}/{name}.handle").read().split()
    aid = f"codex:{session}"
    agents[aid] = {"id": aid, "harness": "codex", "sessionId": session, "scope": "session",
        "cwd": f"{S}/proj", "repoRoot": None, "remote": None, "projectKey": "probe",
        "project": "probe", "state": "working", "updatedAt": 1,
        "process": {"platform": "linux", "host": os.environ["HOST"], "bootId": boot,
                    "pid": int(pid), "start": str(int(start) + delta)}}
path = os.environ["RELAY_STATE_DIR"] + "/agents.json"
tmp = path + ".tmp"
with open(os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w") as fh:
    json.dump({"schema": 2, "generation": str(uuid.uuid4()), "revision": 1, "agents": agents}, fh)
os.replace(tmp, path)
EOF
}

record() { # case expect observed-note
  python3 -I - "$1" "$2" "$RC" "$OUTPUT" "$3" >>"$OUT" <<'EOF'
import json, sys
case, expect, rc, output, note = sys.argv[1:]
print(json.dumps({"case": case, "expect": expect, "rc": rc, "output": output, "note": note}))
EOF
}

# Shell snippets used inside steps.
VIEW='echo "view: ns=$(readlink /proc/self/ns/pid) NSpid=$(awk "/^NSpid/{\$1=\"\";print}" /proc/self/status) host=$(cat /proc/sys/kernel/hostname | sed "s/.*/<host>/")"; p=$PPID; while [ "$p" != 0 ] && [ -r /proc/$p/stat ]; do echo "  chain $p comm=$(cat /proc/$p/comm) ppid=$(awk "{print \$4}" /proc/$p/stat) start=$(awk "{print \$22}" /proc/$p/stat)"; [ "$p" = 1 ] && break; p=$(awk "{print \$4}" /proc/$p/stat); done'
NS='unshare --user --map-current-user --pid --fork --mount-proc'
NS_HOSTPROC='unshare --user --map-current-user --pid --fork'
id_of() { cat "$S/$1.id"; }

start_actor A thread-A root-A
start_actor B thread-B root-B
read -r APID ASTART <"$S/A.handle"; read -r BPID BSTART <"$S/B.handle"
echo "{\"case\":\"setup\",\"A\":[$APID,$ASTART],\"B\":[$BPID,$BSTART]}" >>"$OUT"

T1=$(id_of T1); T2=$(id_of T2); T3=$(id_of T3); T4=$(id_of T4); T5=$(id_of T5); T6=$(id_of T6); T7=$(id_of T7)

# --- same-namespace control -------------------------------------------------------
register A:thread-A B:thread-B
run_as A "$VIEW; $TASKS start $T1 && $TASKS claims"
record same-ns-acquire "relay match: claim codex:thread-A with A's host pid/start/boot" ""

# --- wrong start token / wrong boot ---------------------------------------------------
register A:thread-A:1 B:thread-B
run_as A "$TASKS start $T2"
record wrong-start "refuse: no relay agent matches" ""
register A:thread-A::00000000-0000-4000-8000-000000000000 B:thread-B
run_as A "$TASKS start $T2"
record wrong-boot "refuse: recorded on an earlier boot" ""
register A:thread-A B:thread-B

# --- sibling actor -------------------------------------------------------------------
run_as B "$TASKS done $T1 x"
record sibling-close "refuse: claimed (foreign live claim)" ""
run_as B "CODEX_THREAD_ID=thread-A $TASKS start $T2"
record sibling-forged-hint "refuse: hint does not match qualifying row" "B process, A's thread"
mv "$RELAY_STATE_DIR/agents.json" "$S/agents.json.away"
run_as B "CODEX_THREAD_ID=thread-A $TASKS park $T1 x"
record sibling-forged-proof-only "refuse: proof fails (nearest pid is B, not A); identity unresolved" "registry absent"
run_as A "$TASKS note $T1 heartbeat && $TASKS claims"
record owner-proof-only "accept: A proves ownership from claim with registry absent" "registry absent"
mv "$S/agents.json.away" "$RELAY_STATE_DIR/agents.json"

# --- nested nearest harness ----------------------------------------------------------
run_as A "$S/bin/codex -c '$VIEW; $TASKS park $T1 x; echo rc=\$?; true'"
record nested-inner-park "refuse: nearest harness is the unregistered inner codex" "inner codex inherits A's env"

# --- cross namespace, Codex shape: pid 1 is a codex-named helper, private /proc --------
cat >"$S/inner.sh" <<EOF
$VIEW
echo "--- start T3 (acquire)"; $TASKS start $T3; echo rc=\$?
echo "--- park T1 (A's own claim)"; $TASKS park $T1 x; echo rc=\$?
echo "--- claims liveness from inside"; $TASKS claims
EOF
run_as A "$NS $S/bin/codex $S/inner.sh"
record ns-helper-private-proc "refuse acquisition; proof fails; A's live claim looks stale inside" "same session A, sandboxed"

# --- cross namespace, pid 1 not a harness, private /proc -> walk is Outside ----------------
cat >"$S/inner-plain.sh" <<EOF
$VIEW
$TASKS start $T4; echo rc=\$?; $TASKS claims
EOF
run_as A "$NS bash $S/inner-plain.sh"
record ns-plain-private-proc "out of scope: native codex level, TTL claim without proof" ""

# --- cross namespace, host /proc retained (no mount-proc) ---------------------------
run_as A "$NS_HOSTPROC bash -c '$VIEW; $TASKS start $T5; echo rc=\$?; true'"
record ns-plain-host-proc "relay match: host /proc makes A visible as nearest" ""
run_as A "$NS_HOSTPROC $S/bin/codex -c '$VIEW; $TASKS start $T6; echo rc=\$?; true'"
record ns-helper-host-proc "refuse: nearest harness is the helper even in host view" ""

# --- liveness hazard: explicit identity inside the namespace ---------------------------
run_as A "$NS bash -c 'TASKS_SESSION=intruder $TASKS start $T1; echo rc=\$?; true'"
record ns-explicit-takeover "hazard probe: A's live claim judged in the wrong pid view" "no --force"
run_as A "($NS bash -c 'TASKS_SESSION=ns-owner TASKS_SESSION_PID=1 $TASKS start $T7; echo rc=\$?; $TASKS claims; touch $S/ns-up; sleep 6; true' > $S/ns-pid1.out 2>&1 &); while [ ! -e $S/ns-up ]; do sleep 0.05; done; cat $S/ns-pid1.out"
record ns-explicit-pid1 "claim records namespace-local pid 1; that namespace stays alive" ""
run_as B "$TASKS claims; TASKS_SESSION=host-other $TASKS start $T7; echo rc=\$?; true"
record host-view-of-ns-pid "hazard probe: ns-local pid judged in host view" "no --force"

# --- silent prune: an unrelated write from inside the namespace -------------------------
for t in T8 T9 T10 T11; do
  (cd "$S/proj" && TASKS_SESSION=setup "$TASKS" add "probe $t" -p 2 | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["id"])') >"$S/$t.id"
done
T8=$(id_of T8); T9=$(id_of T9); T10=$(id_of T10); T11=$(id_of T11)
run_as A "$TASKS start $T8 && $TASKS claims"
record prune-setup "A holds T8 live in the host view" ""
run_as A "$NS bash -c 'TASKS_SESSION=unrelated $TASKS start $T9; echo rc=\$?; true'"
record prune-unrelated-ns-write "an unrelated task's start inside the namespace" ""
run_as B "$TASKS claims; $TASKS show $T8 | python3 -I -c 'import json,sys; d=json.load(sys.stdin); print({\"status\": d[\"task\"].get(\"status\"), \"claim\": d.get(\"claim\")})'; TASKS_SESSION=host-other $TASKS start $T8; echo rc=\$?; true"
record prune-host-after "host: is A's live T8 claim still present; can another session start T8 without --force" "A still alive"

# --- genuine Codex sandbox (codex sandbox, no model call), workspace-write in scratch -----
if [ -n "${PROBE_CODEX_HOME:-}" ]; then
  run_as A "$TASKS start $T10 && $TASKS claims"
  record codex-setup "A holds T10 live in the host view" ""
  cat >"$S/codex-inner.sh" <<EOF2
$VIEW
cd $S/proj || exit 1
echo "--- claims liveness inside codex sandbox"; $TASKS claims
echo "--- unrelated explicit claim write inside codex sandbox"; TASKS_SESSION=codex-unrelated $TASKS start $T11; echo rc=\$?
EOF2
  RC=$(cd "$S/proj" && CODEX_HOME=$PROBE_CODEX_HOME timeout 60 codex sandbox -c sandbox_mode='"workspace-write"' -c "sandbox_workspace_write.writable_roots=[\"$S\"]" -- bash "$S/codex-inner.sh" >"$S/codex-inner.out" 2>&1; echo $?)
  OUTPUT=$(cat "$S/codex-inner.out")
  record codex-sandbox-view "genuine sandbox: pid 1 helper, A's claim judged from inside" "codex sandbox, workspace-write, cwd=scratch"
  run_as B "$TASKS claims"
  record codex-host-after "host: does A's T10 claim survive a write inside the genuine sandbox" ""
fi

# --- peer-credential oracle: kernel-translated pid of a sandboxed caller ------------------
python3 -I - "$S/oracle.sock" >"$S/oracle.log" 2>&1 <<'EOF' &
import json, os, socket, struct, sys
path = sys.argv[1]
srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); srv.bind(path); srv.listen(4)
for _ in range(2):
    conn, _ = srv.accept()
    pid, uid, gid = struct.unpack("3i", conn.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
    chain, p = [], pid
    while p > 1 and len(chain) < 16:
        f = open(f"/proc/{p}/stat").read(); rest = f[f.rfind(")") + 2:].split()
        argv0 = open(f"/proc/{p}/cmdline", "rb").read().split(b"\0")[0].decode(errors="replace")
        chain.append({"pid": p, "comm": f[f.find("(") + 1:f.rfind(")")], "argv0": os.path.basename(argv0), "start": int(rest[19])})
        p = int(rest[1])
    conn.sendall(json.dumps({"peer_pid_in_server_view": pid, "chain": chain}).encode()); conn.close()
EOF
PIDS+=($!)
while [ ! -S "$S/oracle.sock" ]; do sleep 0.05; done
CLIENT="python3 -I -c 'import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); print(s.recv(65536).decode())' $S/oracle.sock"
run_as A "$NS $S/bin/codex -c \"$CLIENT; true\""
record oracle-from-ns "server sees host pid of sandboxed caller and the host chain up to A" "unshare"
if [ -n "${PROBE_CODEX_HOME:-}" ]; then
  RC=$(cd "$S" && CODEX_HOME=$PROBE_CODEX_HOME timeout 60 codex sandbox -c sandbox_mode='"workspace-write"' -- bash -c "$CLIENT; true" >"$S/codex-oracle.out" 2>&1; echo $?)
  OUTPUT=$(cat "$S/codex-oracle.out")
  record oracle-from-codex-sandbox "connect from genuine sandbox; server sees host pid chain" "codex sandbox, workspace-write"
fi
echo "$S" >"$OUT.scratch"
