mod cli;
mod commands;
mod error;
mod format;
mod frontmatter;
mod model;
mod output;
mod registry;
mod repo;
mod resolve;
mod time;

use clap::Parser;
use output::Format;

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
    match commands::run(cli) {
        Ok(out) => {
            if format == Format::Pretty {
                eprint!("{}", output::pretty_warnings(&output::warnings_of(&out)));
            }
            println!("{}", output::render(&out, format));
        }
        Err(error) => {
            eprintln!("{}", output::render_error(&error));
            std::process::exit(1);
        }
    }
}
