#!/usr/bin/env bash
# Disposable acceptance run for the claim PID-view design (tasks-860db2).
# Usage: pidview-acceptance.sh <tasks-binary> <out.jsonl>
# Everything lives under a fresh scratch dir: HOME, task registry/config, claim state,
# relay state (relay stays off). Only unprivileged user namespaces are used; no codex
# sandbox, no model call, no desktop. One JSON object per case on <out.jsonl>.
set -u
TASKS=$(readlink -f "$1")
OUT=$(readlink -f "$2")
: >"$OUT"
S=$(mktemp -d -p "${TMPDIR:-/tmp}" pidview-accept.XXXX)
PIDS=()
cleanup() {
  # unshare --fork ignores SIGTERM while it waits; SIGKILL it, and --kill-child takes
  # the namespace's pid 1 with it.
  for p in "${PIDS[@]}"; do kill -KILL "$p" 2>/dev/null; done
  wait 2>/dev/null
  rm -rf "$S"
}
trap cleanup EXIT TERM INT

for v in $(env | sed -n 's/^\(CLAUDE[A-Z_]*\|CODEX[A-Z_]*\|TASKS_[A-Z_]*\)=.*/\1/p'); do unset "$v"; done
export HOME=$S/home XDG_CONFIG_HOME=$S/config XDG_STATE_HOME=$S/state RELAY_STATE_DIR=$S/relay
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$RELAY_STATE_DIR" "$S/proj"
export S TASKS
STORE=$XDG_STATE_HOME/tasks/claims/pv.toml

cd "$S/proj" || exit 1
git init -q
TASKS_SESSION=setup "$TASKS" init --prefix pv >/dev/null || { echo "init failed" >&2; exit 1; }
for t in T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11; do
  id=$(TASKS_SESSION=setup "$TASKS" add "accept $t" -p 2 | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["id"])')
  export "$t=$id"
done

# claim <id> [field]: the stored claim (JSON), or one field of it; empty when absent.
claim() {
  python3 -I - "$STORE" "$1" "${2:-}" <<'PY'
import json, sys, tomllib
path, tid, field = sys.argv[1:]
try:
    data = tomllib.load(open(path, "rb"))
except FileNotFoundError:
    data = {}
c = data.get("claims", {}).get(tid)
if c is None:
    print("")
elif field:
    v = c.get(field)
    print("" if v is None else v)
else:
    print(json.dumps(c, sort_keys=True))
PY
}

emit() { # case expected observed verdict
  python3 -I -c 'import json,sys; print(json.dumps(dict(zip(["case","expected","observed","verdict"], sys.argv[1:]))))' "$@" >>"$OUT"
}

refused_claimed() { # session id: 0 when start is refused as claimed
  local out
  out=$(TASKS_SESSION=$1 "$TASKS" start "$2" 2>&1)
  [ $? -ne 0 ] && printf '%s' "$out" | grep -q '"claimed"'
}

waitfor() { # file [tenths]
  local n=0 max=${2:-200}
  while [ ! -e "$1" ]; do sleep 0.1; n=$((n+1)); [ $n -ge $max ] && return 1; done
  return 0
}

CHILD=(unshare --user --map-current-user --pid --fork --mount-proc)
ROOTCHILD=(unshare --user --map-root-user --pid --fork --mount-proc)
# Background namespaces die with their unshare, so cleanup reaches them.
BGCHILD=(unshare --user --map-current-user --pid --fork --kill-child --mount-proc)
BGROOTCHILD=(unshare --user --map-root-user --pid --fork --kill-child --mount-proc)
HOSTVIEW=$(stat -L -c '%d:%i' /proc/self/ns/pid)

# 1. prune-from-sandbox
TASKS_SESSION=A TASKS_SESSION_PID=$$ "$TASKS" start "$T1" >/dev/null
v1=$(claim "$T1" pid_view)
"${CHILD[@]}" env TASKS_SESSION=B "$TASKS" start "$T2" >"$S/c1.out" 2>&1
rc=$?
after=$(claim "$T1")
if [ "$v1" = "$HOSTVIEW" ] && [ $rc -eq 0 ] && [ -n "$after" ] && refused_claimed C "$T1"; then v=pass; else v=fail; fi
emit prune-from-sandbox "host claim records the host view, survives a sandboxed write, refuses takeover" \
  "view=$v1 host=$HOSTVIEW child_rc=$rc present=$([ -n "$after" ] && echo yes || echo no)" "$v"

# 2. ns-explicit-pid1
"${BGCHILD[@]}" sh -c 'TASKS_SESSION=N TASKS_SESSION_PID=1 "$TASKS" start "$T3" >"$S/c2.out" 2>&1; echo $? >"$S/c2.rc"; sleep 30' &
PIDS+=($!)
waitfor "$S/c2.rc"
view3=$(claim "$T3" pid_view)
row=$("$TASKS" claims | python3 -I -c 'import json,sys,os; r=[c for c in json.load(sys.stdin)["claims"] if c["id"]==os.environ["T3"]]; print(json.dumps(r[0]) if r else "")')
live=$(printf '%s' "$row" | python3 -I -c 'import json,sys; t=sys.stdin.read(); print(json.loads(t).get("live") if t else "")')
unv=$(printf '%s' "$row" | python3 -I -c 'import json,sys; t=sys.stdin.read(); print(json.loads(t).get("unverified","") if t else "")')
if [ "$(cat "$S/c2.rc")" = 0 ] && [ -z "$view3" ] && [ "$live" = True ] && [ -n "$unv" ] && refused_claimed C "$T3"; then v=pass; else v=fail; fi
emit ns-explicit-pid1 "namespace-local pid 1 records no view; host holds it (live true, unverified) and refuses takeover" \
  "rc=$(cat "$S/c2.rc") view='$view3' live=$live unverified='$unv'" "$v"
kill -KILL "${PIDS[-1]}" 2>/dev/null

# 3. procfs unmount and reuse (root in its own user namespace; setsid controller)
cat >"$S/a.sh" <<'EOF'
stat -L -c '%d:%i' /proc/self/ns/pid >"$S/a.view"
setsid sh -c 'echo $$ >"$S/a.ctl"; "$TASKS" start "$T4" >"$S/a.start.out" 2>&1; echo $? >"$S/a.started"; exec sleep 120' &
while [ ! -e "$S/a.started" ]; do sleep 0.05; done
ctl=$(cat "$S/a.ctl")
awk '{print $22}' /proc/$ctl/stat >"$S/a.start1"
stat -c %d /proc >"$S/a.dev"
if umount /proc 2>"$S/a.umount.err"; then echo ok >"$S/a.unmounted"; else echo fail >"$S/a.unmounted"; fi
while [ ! -e "$S/b.done" ]; do sleep 0.05; done
mount -t proc proc /proc 2>"$S/a.mount.err"
awk '{print $22}' /proc/$ctl/stat >"$S/a.start2" 2>/dev/null
if kill -0 "$ctl" 2>/dev/null; then echo alive >"$S/a.alive"; else echo gone >"$S/a.alive"; fi
kill "$ctl" 2>/dev/null
echo done >"$S/a.final"
EOF
"${BGROOTCHILD[@]}" sh "$S/a.sh" &
PIDS+=($!)
v=fail; obs=""
if waitfor "$S/a.unmounted" 300; then
  aview=$(cat "$S/a.view"); ctl=$(cat "$S/a.ctl"); start1=$(cat "$S/a.start1")
  cview=$(claim "$T4" pid_view); cpid=$(claim "$T4" pid); cstart=$(claim "$T4" pid_start)
  obs="rc=$(cat "$S/a.started") aview=$aview claim_view=$cview ctl=$ctl claim_pid=$cpid start=$start1 claim_start=$cstart umount=$(cat "$S/a.unmounted") adev=$(cat "$S/a.dev")"
  if [ "$cview" = "$aview" ] && [ "$cpid" = "$ctl" ] && [ "$cstart" = "$start1" ]; then
    if [ "$(cat "$S/a.unmounted")" = ok ]; then
      "${ROOTCHILD[@]}" sh -c 'stat -c %d /proc >"$S/b.dev"; TASKS_SESSION=B "$TASKS" start "$T5" >"$S/b.out" 2>&1; echo $? >"$S/b.rc"'
      bdev=$(cat "$S/b.dev"); bpid=$(claim "$T4" pid); bstart=$(claim "$T4" pid_start)
      touch "$S/b.done"
      waitfor "$S/a.final" 100
      start2=$(cat "$S/a.start2" 2>/dev/null)
      obs="$obs bdev=$bdev reused=$([ "$bdev" = "$(cat "$S/a.dev")" ] && echo yes || echo no) b_rc=$(cat "$S/b.rc") after_pid=$bpid after_start=$bstart ctl_start_after=$start2 ctl=$(cat "$S/a.alive")"
      if [ "$(cat "$S/b.rc")" = 0 ] && [ "$bpid" = "$ctl" ] && [ "$bstart" = "$start1" ] && [ "$start2" = "$start1" ] && [ "$(cat "$S/a.alive")" = alive ]; then v=pass; fi
    else
      v=unavailable; obs="$obs umount_err=$(tr '\n' ' ' <"$S/a.umount.err")"
      touch "$S/b.done"
    fi
  else
    touch "$S/b.done"
  fi
else
  obs="namespace A did not reach its unmount step"
fi
emit procfs-unmount-reuse "A's claim records A's view and its live controller; survives B's write after A's procfs is unmounted" "$obs" "$v"

# 4. exported host pid
"${CHILD[@]}" env TASKS_SESSION=E TASKS_SESSION_PID=$$ "$TASKS" start "$T6" >"$S/c4.out" 2>&1
rc6=$?
view6=$(claim "$T6" pid_view); pid6=$(claim "$T6" pid)
"${CHILD[@]}" env TASKS_SESSION=F "$TASKS" start "$T7" >"$S/c4b.out" 2>&1
still=$(claim "$T6")
if [ $rc6 -eq 0 ] && [ "$pid6" = "$$" ] && [ -z "$view6" ] && [ -n "$still" ]; then v=pass; else v=fail; fi
emit exported-host-pid "an exported host pid absent from the child records no view and survives a second child write" \
  "rc=$rc6 pid=$pid6 view='$view6' present=$([ -n "$still" ] && echo yes || echo no)" "$v"

# 5. colliding ancestor: owner is pid 2 in outer O; inner I has its own pid 2.
cat >"$S/inner-body.sh" <<'EOF'
grep NSpid /proc/self/status | wc -w >"$S/i.nspid_words"
echo $$ >"$S/i.pid2"
"$TASKS" start "$T8" >"$S/i.t8.out" 2>&1; echo $? >"$S/i.t8.rc"
EOF
cat >"$S/inner.sh" <<'EOF'
sh "$S/inner-body.sh"; echo exited >"$S/i.pid2.exited"
TASKS_SESSION=X "$TASKS" start "$T9" >"$S/i.t9.out" 2>&1; echo $? >"$S/i.t9.rc"
out=$(TASKS_SESSION=X "$TASKS" start "$T8" 2>&1); echo $? >"$S/i.t8x.rc"; printf '%s' "$out" >"$S/i.t8x.out"
TASKS_SESSION=O2 "$TASKS" start "$T10" >"$S/i.t10.out" 2>&1; echo $? >"$S/i.t10.rc"
echo done >"$S/i.done"
EOF
cat >"$S/outer.sh" <<'EOF'
sleep 300 &
owner=$!
echo $owner >"$S/o.owner"
TASKS_SESSION=O2 TASKS_SESSION_PID=$owner "$TASKS" start "$T10" >"$S/o.t10.out" 2>&1; echo $? >"$S/o.t10.rc"
cp "$XDG_STATE_HOME/tasks/claims/pv.toml" "$S/o.store"
TASKS_SESSION=O2 TASKS_SESSION_PID=$owner unshare --user --map-current-user --pid --fork --mount-proc sh "$S/inner.sh"
kill -0 $owner 2>/dev/null && echo alive >"$S/o.owner.alive"
kill $owner
EOF
"${CHILD[@]}" sh "$S/outer.sh" >"$S/outer.out" 2>&1
o_owner=$(cat "$S/o.owner" 2>/dev/null)
i_pid2=$(cat "$S/i.pid2" 2>/dev/null)
t8view=$(claim "$T8" pid_view); t8pid=$(claim "$T8" pid); t8present=$(claim "$T8")
t10_before=$(python3 -I -c 'import tomllib,sys,os; c=tomllib.load(open(sys.argv[1],"rb"))["claims"][os.environ["T10"]]; print(c.get("pid"), c.get("pid_start"), c.get("pid_view"))' "$S/o.store" 2>/dev/null)
t10_after="$(claim "$T10" pid) $(claim "$T10" pid_start) $(v=$(claim "$T10" pid_view); echo "${v:-None}")"
nswords=$(cat "$S/i.nspid_words" 2>/dev/null)
obs="owner=$o_owner inner_pid2=$i_pid2 t8_rc=$(cat "$S/i.t8.rc" 2>/dev/null) t8_pid=$t8pid t8_view='$t8view' t9_rc=$(cat "$S/i.t9.rc" 2>/dev/null) t8_takeover_rc=$(cat "$S/i.t8x.rc" 2>/dev/null) t10_before='$t10_before' t10_after='$t10_after' t10_heartbeat_rc=$(cat "$S/i.t10.rc" 2>/dev/null) nspid_words=$nswords owner_alive=$(cat "$S/o.owner.alive" 2>/dev/null)"
if [ "$o_owner" = 2 ] && [ "$i_pid2" = 2 ] && [ "$(cat "$S/i.t8.rc")" = 0 ] && [ "$t8pid" = 2 ] && [ -z "$t8view" ] \
   && [ -n "$t8present" ] && [ "$(cat "$S/i.t9.rc")" = 0 ] && [ "$(cat "$S/i.t8x.rc")" != 0 ] && grep -q '"claimed"' "$S/i.t8x.out" \
   && [ "$(cat "$S/i.t10.rc")" = 0 ] && [ "$t10_before" = "$t10_after" ] && [ "$nswords" = 2 ] && [ "$(cat "$S/o.owner.alive" 2>/dev/null)" = alive ]; then
  v=pass
else
  v=fail
fi
emit colliding-ancestor "inner pid 2 collision records no view; claim survives the ancestor's exit, refuses takeover; outer evidence kept by an inner heartbeat; one NSpid entry" "$obs" "$v"

# 6. heartbeat from a sandbox
TASKS_SESSION=H TASKS_SESSION_PID=$$ "$TASKS" start "$T11" >/dev/null
before="$(claim "$T11" pid) $(claim "$T11" pid_start) $(claim "$T11" pid_view)"
"${CHILD[@]}" env TASKS_SESSION=H TASKS_SESSION_PID=$$ "$TASKS" start "$T11" >"$S/c6.out" 2>&1
rc=$?
after="$(claim "$T11" pid) $(claim "$T11" pid_start) $(claim "$T11" pid_view)"
if [ $rc -eq 0 ] && [ "$before" = "$after" ] && [ "$(claim "$T11" pid_view)" = "$HOSTVIEW" ]; then v=pass; else v=fail; fi
emit heartbeat-from-sandbox "an unestablished sandbox heartbeat of the same pid keeps the host evidence" \
  "rc=$rc before='$before' after='$after'" "$v"
