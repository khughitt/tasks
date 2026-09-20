mod claims;
mod cli;
mod commands;
mod complete;
mod complexity;
mod defer;
mod error;
mod format;
mod frontmatter;
mod hierarchy;
mod model;
mod output;
mod periodic;
mod provenance;
mod query;
mod registry;
mod rename;
mod repo;
mod resolve;
mod scope;
mod similarity;
mod style;
#[cfg(test)]
mod surface;
mod time;

use clap::{CommandFactory, Parser};
use output::Format;
use std::io::{IsTerminal, Write};

/// Write to a stream and flush, reporting whether it landed.
///
/// The flush is explicit: a piped stdout is block-buffered, so for output smaller than the
/// pipe buffer the write lands in the buffer and the failure surfaces only at the flush --
/// which, left implicit at process exit, is performed and its error discarded.
fn write_to(mut stream: impl Write, text: &str) -> std::io::Result<()> {
    stream.write_all(text.as_bytes())?;
    stream.flush()
}

/// stderr, with a reader that has gone away treated as an ordinary end of output. A
/// diagnostic we cannot deliver must not become a panic, and must not change the exit code
/// the command earned -- so unlike stdout, nothing here inspects the error.
fn to_stderr(text: &str) {
    let _ = write_to(std::io::stderr().lock(), text);
}

/// A usage error in the shape the shared CLI vocabulary asks for: the problem on one
/// line, then the usage line. clap's own rendering spreads the same content over blank
/// lines, indented continuations, tips, and a `--help` pointer; the continuations (the
/// missing arguments, the possible values) are folded into the problem line and the rest
/// dropped, so a caller reading stderr sees at most two lines.
fn usage_error(error: &clap::Error) -> String {
    let rendered = error.render().to_string();
    let mut lines = rendered.lines();
    let mut problem = lines.next().unwrap_or("error: usage").to_string();
    let mut usage = None;
    for line in lines {
        if line.starts_with("Usage:") {
            usage = Some(line);
        } else if usage.is_none() && line.starts_with(' ') && !line.trim_start().starts_with("tip:")
        {
            problem.push(' ');
            problem.push_str(line.trim());
        }
    }
    match usage {
        Some(usage) => format!("{problem}\n{usage}\n"),
        None => format!("{problem}\n"),
    }
}

fn main() {
    // Must run before anything writes to stdout. Returns immediately unless
    // TASKS_COMPLETE is set, so an ordinary run pays one getenv.
    clap_complete::CompleteEnv::with_factory(cli::Cli::command)
        .var("TASKS_COMPLETE")
        .complete();

    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        // Help and version on stdout at exit 0, and the full help a bare `tasks` earns on
        // stderr at exit 2: exactly as clap prints them.
        Err(error)
            if !error.use_stderr()
                || error.kind()
                    == clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand =>
        {
            error.exit()
        }
        Err(error) => {
            to_stderr(&usage_error(&error));
            std::process::exit(2);
        }
    };
    // clap checks a conflict within one command's arguments; a global given before the
    // command and its rival after it land in different commands, so the pair is checked
    // here, whatever the placement.
    if cli.json && cli.pretty {
        let error = cli::Cli::command().error(
            clap::error::ErrorKind::ArgumentConflict,
            "the argument '--json' cannot be used with '--pretty'",
        );
        to_stderr(&usage_error(&error));
        std::process::exit(2);
    }
    // The flags win over the variable, which is consulted only when neither is given.
    let format = match (
        cli.json,
        cli.pretty,
        std::env::var("TASKS_FORMAT").ok().as_deref(),
    ) {
        (true, _, _) => Format::Json,
        (false, true, _) | (false, false, Some("pretty")) => Format::Pretty,
        (false, false, None) | (false, false, Some("json")) => Format::Json,
        (false, false, Some(other)) => {
            to_stderr(&format!(
                "{}\n",
                output::render_error(&error::Error::Config(format!(
                    "TASKS_FORMAT must be json or pretty, got {other:?}"
                )))
            ));
            std::process::exit(1);
        }
    };
    let tasks_color = match std::env::var("TASKS_COLOR") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(value)) => {
            to_stderr(&format!(
                "{}\n",
                output::render_error(&error::Error::Config(format!(
                    "TASKS_COLOR must be valid UTF-8, got {value:?}"
                )))
            ));
            std::process::exit(1);
        }
    };
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
    let color_mode =
        match style::ColorMode::resolve(cli.color.as_deref(), tasks_color.as_deref(), no_color) {
            Ok(mode) => mode,
            Err(error) => {
                to_stderr(&format!("{}\n", output::render_error(&error)));
                std::process::exit(1);
            }
        };
    let stdout_painter = style::Painter::new(color_mode, format, std::io::stdout().is_terminal());
    let stderr_painter = style::Painter::new(color_mode, format, std::io::stderr().is_terminal());
    match commands::run(cli) {
        Ok(out) => {
            // A clean `check` says nothing in JSON mode: it ends every project's pre-commit
            // hook, the exit status carries the verdict, and nothing parses an empty
            // report. Pretty mode still prints `ok` for a person at a terminal.
            let silent = format == Format::Json
                && matches!(&out, output::Output::Check(check)
                    if check.errors.is_empty() && check.warnings.is_empty());
            if format == Format::Pretty {
                to_stderr(&output::pretty_warnings(
                    &output::warnings_of(&out),
                    &stderr_painter,
                ));
            }
            let rendered = output::render(&out, format, &stdout_painter);
            let written = if silent {
                Ok(())
            } else {
                write_to(std::io::stdout().lock(), &format!("{rendered}\n"))
            };
            match written {
                Ok(()) => {}
                // The reader closed the pipe -- `tasks show <id> | head`. That is an
                // ordinary end of output, not a failure, and falling through rather than
                // exiting here leaves the exit code the command earned intact below.
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
                Err(error) => {
                    to_stderr(&format!(
                        "{}\n",
                        output::render_error(&error::Error::Io(format!(
                            "failed writing to stdout: {error}"
                        )))
                    ));
                    std::process::exit(1);
                }
            }
            if let output::Output::Check(check) = &out
                && !check.errors.is_empty()
            {
                std::process::exit(1);
            }
        }
        Err(error) => {
            to_stderr(&format!("{}\n", output::render_error(&error)));
            std::process::exit(1);
        }
    }
}
