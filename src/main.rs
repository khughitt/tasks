mod claims;
mod cli;
mod commands;
mod error;
mod format;
mod frontmatter;
mod hierarchy;
mod model;
mod output;
mod query;
mod registry;
mod repo;
mod resolve;
mod scope;
mod similarity;
mod style;
mod time;

use clap::Parser;
use output::Format;
use std::io::IsTerminal;

fn main() {
    let cli = cli::Cli::parse();
    let format = match (cli.pretty, std::env::var("TASKS_FORMAT").ok().as_deref()) {
        (true, _) | (false, Some("pretty")) => Format::Pretty,
        (false, None) | (false, Some("json")) => Format::Json,
        (false, Some(other)) => {
            eprintln!(
                "{}",
                output::render_error(&error::Error::Config(format!(
                    "TASKS_FORMAT must be json or pretty, got {other:?}"
                )))
            );
            std::process::exit(1);
        }
    };
    let tasks_color = match std::env::var("TASKS_COLOR") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(value)) => {
            eprintln!(
                "{}",
                output::render_error(&error::Error::Config(format!(
                    "TASKS_COLOR must be valid UTF-8, got {value:?}"
                )))
            );
            std::process::exit(1);
        }
    };
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
    let color_mode =
        match style::ColorMode::resolve(cli.color.as_deref(), tasks_color.as_deref(), no_color) {
            Ok(mode) => mode,
            Err(error) => {
                eprintln!("{}", output::render_error(&error));
                std::process::exit(1);
            }
        };
    let stdout_painter = style::Painter::new(color_mode, format, std::io::stdout().is_terminal());
    let stderr_painter = style::Painter::new(color_mode, format, std::io::stderr().is_terminal());
    match commands::run(cli) {
        Ok(out) => {
            if format == Format::Pretty {
                eprint!(
                    "{}",
                    output::pretty_warnings(&output::warnings_of(&out), &stderr_painter)
                );
            }
            println!("{}", output::render(&out, format, &stdout_painter));
            if let output::Output::Check(check) = &out
                && !check.errors.is_empty()
            {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{}", output::render_error(&error));
            std::process::exit(1);
        }
    }
}
