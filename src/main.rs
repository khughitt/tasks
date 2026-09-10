mod claims;
mod cli;
mod commands;
mod complete;
mod error;
mod format;
mod frontmatter;
mod hierarchy;
mod model;
mod output;
mod periodic;
mod query;
mod registry;
mod rename;
mod repo;
mod resolve;
mod scope;
mod similarity;
mod style;
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

fn main() {
    // Must run before anything writes to stdout. Returns immediately unless
    // TASKS_COMPLETE is set, so an ordinary run pays one getenv.
    clap_complete::CompleteEnv::with_factory(cli::Cli::command)
        .var("TASKS_COMPLETE")
        .complete();

    let cli = cli::Cli::parse();
    let format = match (cli.pretty, std::env::var("TASKS_FORMAT").ok().as_deref()) {
        (true, _) | (false, Some("pretty")) => Format::Pretty,
        (false, None) | (false, Some("json")) => Format::Json,
        (false, Some(other)) => {
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
            if format == Format::Pretty {
                to_stderr(&output::pretty_warnings(
                    &output::warnings_of(&out),
                    &stderr_painter,
                ));
            }
            let rendered = output::render(&out, format, &stdout_painter);
            match write_to(std::io::stdout().lock(), &format!("{rendered}\n")) {
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
