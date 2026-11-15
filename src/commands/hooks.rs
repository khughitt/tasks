use crate::cli::Cli;
use clap::{CommandFactory, builder::ValueParser};
use std::ffi::OsString;

/// Derive only the protocol exception from the canonical parser, preserving clap's arity.
pub(crate) fn try_run(args: &[OsString]) -> Option<i32> {
    let probe = Cli::command().args_override_self(true).mut_args(|arg| {
        if !arg.is_global_set() {
            return arg;
        }
        let value = matches!(arg.get_id().as_str(), "dir" | "color");
        let arg = arg.conflicts_with(clap::builder::Resettable::<clap::Id>::Reset);
        if value {
            arg.value_parser(ValueParser::os_string())
        } else {
            arg
        }
    });
    let matches = probe.try_get_matches_from(args).ok()?;
    let hook = matches.subcommand_matches("hook")?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut diagnostics = std::io::stderr().lock();
    match hook.subcommand_name()? {
        "claim-guard" => Some(crate::hooks::claim_guard(
            &mut input,
            &mut output,
            &mut diagnostics,
        )),
        "halt-notice" => Some(crate::hooks::halt_notice(
            &mut input,
            &mut output,
            &mut diagnostics,
        )),
        _ => None,
    }
}
