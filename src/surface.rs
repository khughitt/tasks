//! The live parser surface as row tuples, compared with tools/cli.toml in a unit test.
//! Row shapes are the ones every project's conformance test builds (ops plan
//! 2026-09-20-cli-conventions, Global Constraints).
use std::collections::BTreeSet;

use clap::{Arg, ArgAction, Command, CommandFactory};
use clap_complete::engine::ArgValueCandidates;

use crate::complete::ValueSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Row {
    Command {
        path: Vec<String>,
        summary: String,
    },
    Arg {
        path: Vec<String>,
        index: usize,
        name: String,
        value: String,
        values: Vec<String>,
        required: bool,
        variadic: bool,
    },
    Option {
        path: Vec<String>,
        names: Vec<String>,
        value: String,
        values: Vec<String>,
        default: Option<String>,
        arity: Option<String>,
        repeatable: bool,
        required: bool,
    },
}

const IMPLIED_EVERYWHERE: [&str; 5] = ["--json", "--pretty", "--color", "-h", "--help"];
const IMPLIED_AT_ROOT: [&str; 2] = ["-V", "--version"];

fn kind(arg: &Arg) -> (String, Vec<String>) {
    let possible: Vec<String> = arg
        .get_possible_values()
        .iter()
        .map(|v| v.get_name().to_string())
        .collect();
    if !possible.is_empty() {
        return ("enum".into(), possible);
    }
    if arg.get::<ValueSet>().is_some()
        && let Some(candidates) = arg.get::<ArgValueCandidates>()
    {
        let values = candidates
            .candidates()
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();
        return ("enum".into(), values);
    }
    let name = arg
        .get_value_names()
        .and_then(|n| n.first())
        .map(|n| n.to_string())
        .unwrap_or_default();
    let kind = match name.as_str() {
        "N" => "int",
        "ID" | "REF" => "ref",
        "DIR" | "PATH" => "path",
        "WHEN" => "when",
        "AGE" => "age",
        _ => "string",
    };
    (kind.into(), Vec::new())
}

fn walk(cmd: &Command, path: Vec<String>, rows: &mut BTreeSet<Row>) {
    if !path.is_empty() {
        let summary = cmd
            .get_about()
            .map(|s| s.to_string().trim_end_matches('.').to_string())
            .unwrap_or_default();
        rows.insert(Row::Command {
            path: path.clone(),
            summary,
        });
    }
    let mut index = 0;
    for arg in cmd.get_arguments() {
        if arg.is_global_set() && !path.is_empty() {
            continue; // a global is a root row only
        }
        let mut names: Vec<String> = Vec::new();
        if let Some(long) = arg.get_long() {
            names.push(format!("--{long}"));
        }
        if let Some(short) = arg.get_short() {
            names.push(format!("-{short}"));
        }
        let implied = names
            .iter()
            .any(|n| IMPLIED_EVERYWHERE.contains(&n.as_str()))
            || (path.is_empty() && names.iter().any(|n| IMPLIED_AT_ROOT.contains(&n.as_str())));
        if implied {
            continue;
        }
        let flag = matches!(
            arg.get_action(),
            ArgAction::SetTrue | ArgAction::SetFalse | ArgAction::Count
        );
        if arg.is_positional() {
            let (value, values) = kind(arg);
            let variadic = arg
                .get_num_args()
                .map(|n| n.max_values() > 1)
                .unwrap_or(false);
            rows.insert(Row::Arg {
                path: path.clone(),
                index,
                name: arg.get_id().to_string().replace('_', "-"),
                value,
                values,
                required: arg.is_required_set(),
                variadic,
            });
            index += 1;
            continue;
        }
        let (value, values) = if flag {
            ("none".to_string(), Vec::new())
        } else {
            kind(arg)
        };
        let default = if flag {
            matches!(arg.get_action(), ArgAction::SetFalse).then(|| "true".to_string())
        } else {
            arg.get_default_values()
                .first()
                .map(|v| v.to_string_lossy().into_owned())
        };
        let arity = if flag {
            None
        } else if arg
            .get_num_args()
            .map(|n| n.max_values() > 1)
            .unwrap_or(false)
        {
            Some("1..".to_string())
        } else {
            Some("1".to_string())
        };
        let repeatable = matches!(arg.get_action(), ArgAction::Append);
        rows.insert(Row::Option {
            path: path.clone(),
            names,
            value,
            values,
            default,
            arity,
            repeatable,
            required: arg.is_required_set(),
        });
    }
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let mut next = path.clone();
        next.push(sub.get_name().to_string());
        walk(sub, next, rows);
    }
}

pub fn live() -> BTreeSet<Row> {
    let mut rows = BTreeSet::new();
    walk(&crate::cli::Cli::command(), Vec::new(), &mut rows);
    rows
}

pub fn table(text: &str, cli: &str) -> BTreeSet<Row> {
    let doc: toml::Value = toml::from_str(text).expect("tools/cli.toml parses");
    let vocab = &doc["vocabulary"]["options"];
    let strings = |v: Option<&toml::Value>| -> Vec<String> {
        v.and_then(|v| v.as_array())
            .map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    };
    let mut rows = BTreeSet::new();
    for cmd in doc["cli"][cli]["commands"].as_array().unwrap() {
        let path = strings(cmd.get("path"));
        if !path.is_empty() {
            rows.insert(Row::Command {
                path: path.clone(),
                summary: cmd["summary"].as_str().unwrap().to_string(),
            });
        }
        for (index, arg) in cmd
            .get("args")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            rows.insert(Row::Arg {
                path: path.clone(),
                index,
                name: arg["name"].as_str().unwrap().into(),
                value: arg["value"].as_str().unwrap().into(),
                values: strings(arg.get("values")),
                required: arg["required"].as_bool().unwrap(),
                variadic: arg
                    .get("variadic")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            });
        }
        for opt in cmd
            .get("options")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
        {
            let names = match opt.get("shared") {
                Some(key) => strings(vocab[key.as_str().unwrap()].get("names")),
                None => strings(opt.get("names")),
            };
            let value = opt["value"].as_str().unwrap().to_string();
            let flag = value == "none";
            rows.insert(Row::Option {
                path: path.clone(),
                names,
                value,
                values: strings(opt.get("values")),
                default: opt
                    .get("default")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                arity: if flag {
                    None
                } else {
                    Some(
                        opt.get("arity")
                            .and_then(|a| a.as_str())
                            .unwrap_or("1")
                            .to_string(),
                    )
                },
                repeatable: opt
                    .get("repeatable")
                    .and_then(|r| r.as_bool())
                    .unwrap_or(false),
                required: opt
                    .get("required")
                    .and_then(|r| r.as_bool())
                    .unwrap_or(false),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_surface_equals_table() {
        let live = live();
        let table = table(include_str!("../tools/cli.toml"), "tasks");
        let parser_only: Vec<_> = live.difference(&table).collect();
        let table_only: Vec<_> = table.difference(&live).collect();
        assert!(
            parser_only.is_empty() && table_only.is_empty(),
            "parser only: {parser_only:#?}\ntable only: {table_only:#?}"
        );
    }
}
