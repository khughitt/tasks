# Color output for pretty views — design

**Status:** implemented (2026-09-03); see docs/plans/2026-09-03-color-output.md.

## 1. Problem

`--pretty` renders tables and full text for humans, but every row is undifferentiated
text. Scanning `prime` or `ready` for what is blocked, what someone already claimed, and
what is merely closed context means reading each row's status column word by word. The
information is present; it just carries no weight.

Color is the obvious fix and the obvious hazard. This tool's primary consumer is an agent
reading JSON, and escape codes that reach an agent's context are noise it pays for on
every turn. So the question is not whether to colorize but what may switch it on.

## 2. Decision

Color is opt-in, off by default, and reachable only from the `--pretty` branch.

The original design (2026-08-29 §5) states "TTY detection is not used": the output format
is never inferred from the environment. Color does not overturn that. Detection is
available, but nothing infers anything until the operator asks for it by name:

```
--color <auto|always|never>      global, like --pretty
TASKS_COLOR=<auto|always|never>
```

- **Unset** — no color, ever. The invariant holds literally out of the box, and an agent
  that has never heard of this feature cannot be surprised by it.
- **`auto`** — color if and only if the stream being written to is a terminal
  (`std::io::IsTerminal`), decided per stream: see §4.
- **`always`** — color unconditionally, for `| less -R` and for tests.
- **`never`** — off.

Consequences the design accepts:

- **Detection is opt-in, not default.** An operator who wants color by default puts
  `TASKS_COLOR=auto` in a shell rc. Agent subprocesses inherit that variable and still get
  nothing, because they have no terminal on either stream. A plain boolean `TASKS_COLOR=1` would
  have no such backstop, which is why the variable is tri-state rather than a flag.
- **`TERM` is never consulted.** `TERM` is inherited by subprocesses that have no terminal
  at all; a session was observed carrying `TERM=xterm-kitty` with none of stdin, stdout, or
  stderr a tty. `IsTerminal` on the actual stream is the only probe.
- **The palette is the terminal's.** Colors are the basic ANSI codes (30-37) plus the dim
  and bold attributes, so they follow whatever theme the terminal already uses. No 256-color
  or truecolor values, and no attempt to detect what the terminal supports.

### 2.1 Precedence

1. `--color <mode>` on the command line, if given.
2. Otherwise `NO_COLOR`, if set and non-empty: `never`.
3. Otherwise `TASKS_COLOR`, if set.
4. Otherwise `never`.

An explicit flag beats `NO_COLOR` because a flag typed now is more specific than ambient
environment; the NO_COLOR convention permits a command-line override. Given that the
default is already off, `NO_COLOR` is close to a no-op here: its one real job is letting a
user who keeps `TASKS_COLOR=auto` in a shell rc suppress color for a single invocation.

`TASKS_COLOR` is parsed and validated whenever it is set, even when `NO_COLOR` or a flag
outranks it, so a typo is reported rather than silently ignored. An unparseable value is a
`config` error before any work is done.

That is deliberately stricter than `TASKS_FORMAT`, which is validated only when it is
actually consulted: `--pretty` with `TASKS_FORMAT=xml` succeeds today and ignores the
variable, because `main` matches the `--pretty` arm first. Fail-early is the rule this
repository states, so the new variable follows the rule rather than the precedent. Making
`TASKS_FORMAT` consistent is tasks-a14f0d and is not done here: it turns a
currently-succeeding command into an error, which deserves its own change.

`--color` takes a plain string parsed by the same `ColorMode::parse`, matching how
`--status` and `graph --format` are handled rather than clap's `ValueEnum`; an invalid
value is therefore a typed `validation` error at exit 1, not a clap usage error at exit 2.
The two kinds differ by source, consistently with the rest of the tool: a bad environment
variable is `config`, a bad argument is `validation`.

Color is unreachable from the JSON branch by construction: the painter is consulted only
inside `pretty`, so `--color always` without `--pretty` emits plain JSON with no runtime
check to get wrong.

## 3. What carries color

One visual axis per meaning, so the axes compose instead of competing:

- **Hue** — status and severity.
- **Dim** — chrome: the parts of a row you skip past on the way to the title.
- **Bold** — the thing you came for: an urgent priority, a section header.

Statuses:

| Status  | Treatment | Why |
|---------|-----------|-----|
| idea    | blue      | open but not actionable; `ready` never lists one |
| todo    | none      | the neutral bulk; styling it would style everything |
| doing   | yellow    | active and claimed |
| blocked | red       | needs attention |
| done    | dim green | closed; recedes, but stays distinct from dropped |
| dropped | dim red   | closed; recedes |

Both closed states are dim because they are usually context rather than subject: `tree`
and `prime`'s roadmap deliberately keep a closed ancestor visible above open work, and it
should not outweigh the work beneath it.

Elsewhere:

- **Chrome (dim):** the id, the `[tag, tag]` group, the `@owner` suffix.
- **Emphasis (bold):** `P0` and `P1` in the priority column; `prime`'s section headers
  (`closeout:`, `roadmap:`, `ready:`, `doing:`).
- **Severity:** `check`'s `error:` lines red and its `ok` line green; the `warning: `
  prefix on stderr yellow; `show`'s `# step MISSING` red.
- **`show`'s footers:** the `# depends on`, `# parent`, and `# children` lists each print a
  `[status]` and an id. Those take the same status hue and the same dim id as a table row
  does, because a status means the same thing wherever it appears.

Not colored, deliberately:

- `graph` output. Mermaid and dot are machine-readable text that happens to print.
- The task text `show --pretty` prints ahead of those footers, which is `serialize_task`
  output: frontmatter, body, and notes. That is file text, and it stays copy-pasteable.

## 4. Shape

A `src/style.rs` module owning three things and nothing else:

```rust
pub enum ColorMode { Auto, Always, Never }

/// A role, never a color. Call sites name what a span means; this module decides how
/// that looks, so the same meaning renders identically in every view.
pub enum Style { Status(crate::model::Status), Chrome, Emphasis, Error, Ok, Warning }

pub struct Painter { enabled: bool }
impl Painter {
    /// `stream_is_terminal` answers the `Auto` probe for the stream this painter writes to.
    pub fn new(mode: ColorMode, format: Format, stream_is_terminal: bool) -> Painter;
    pub fn paint(&self, style: Style, text: &str) -> String;
}
```

`Painter` is threaded, not global: `render(&out, format, &painter)` and
`pretty_warnings(&warnings, &painter)` take it. This keeps `pretty` a pure function of its
inputs, so both modes are unit-testable and tests cannot leak modes into each other.

**One painter per stream.** Warnings go to stderr and everything else to stdout, and the
two are redirected independently: `tasks --pretty list > out.txt` run from a terminal
leaves stderr a tty and stdout a file. A single painter probing stdout would therefore
either write escapes into a redirected stderr or drop color from a terminal one. `main`
builds two painters from the same mode, one per stream — `render` takes the stdout one,
`pretty_warnings` the stderr one. Only `auto` can differ between them; `always` and
`never` are the same on both.

**Statuses stay typed.** `DepInfo.status` is `Option<String>` and `Related.status` is
`String` today, so coloring `show`'s footers would mean reparsing a status string during
rendering. They become `Option<Status>` and `Status`. `Status` already derives `Serialize`
with `rename_all = "lowercase"`, producing exactly the strings `as_str` produces at the two
construction sites in `show.rs`, so the emitted JSON is byte-identical and the output
contract in §5.1 of the original design is untouched.

**Pad first, paint last.** `table` aligns columns with `{:<2}` and `{:<7}` width
specifiers, and escape bytes count toward those widths. Painting before padding silently
destroys alignment in exactly the rows that have color. `paint` therefore takes text that
is already padded and wraps it without changing its visible width; every call site pads,
then paints. A disabled painter, and `Style::Status(Todo)`, return the input verbatim.

## 5. Testing

End to end (`tests/cli.rs`), which pipes stdout and so has no terminal:

- `--pretty --color always` emits escape sequences; the same command without `--color`
  emits none, and neither does `--color auto`, which is the agent-safety property;
- `--color always` without `--pretty` emits plain JSON;
- `NO_COLOR=1` with `TASKS_COLOR=always` emits no color, and `--color always` alongside
  `NO_COLOR=1` still emits color. The `always` pairing is the load-bearing one: with
  `TASKS_COLOR=auto` the piped stdout would have disabled color anyway, so an
  implementation that ignored `NO_COLOR` entirely would pass such a test;
- `TestEnv::cmd` removes `TASKS_COLOR` and `NO_COLOR` from the child environment, beside
  the `TASKS_FORMAT` and `TASKS_OWNER` it already removes, so a developer's shell cannot
  change the result of an unrelated test;
- `TASKS_COLOR=chartreuse` is a `config` error, and remains one when `NO_COLOR` is also
  set, since the variable is validated whenever present;
- a colored `--pretty ready` table has the same visible column layout as an uncolored one.

Unit (`src/style.rs`): mode precedence including the flag-beats-`NO_COLOR` case; that
`paint` preserves visible width for every `Style`; that a disabled painter is the identity
function.

## 6. Out of scope

Per-status or per-tag palette configuration; 256-color or truecolor output; terminal
capability detection beyond `IsTerminal`; colored `graph` output; coloring the task file
text in `show`; a `--color` value of `auto` meaning anything other than "the output stream
is a tty".
