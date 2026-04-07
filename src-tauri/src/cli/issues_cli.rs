use anyhow::{bail, Result};

use crate::core::data_store::NewIssue;

use super::{bootstrap_cli_state, print_json};

pub(crate) fn run_issues_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;
    let ds = startup.data_store();

    match args {
        [command] if command == "list" => {
            let issues = ds.list_issues(None)?;
            print_json(&issues)
        }
        [command, key] if command == "get" => match ds.get_issue_by_key(key)? {
            Some(issue) => print_json(&issue),
            None => bail!("issue `{key}` not found"),
        },
        [command, key, flag, new_status] if command == "status" && flag == "--set" => {
            let updated = ds.update_issue_status(key, new_status)?;
            print_json(&updated)
        }
        [command, rest @ ..] if command == "create" => {
            let (title, description, priority, assignee) = parse_create_args(rest)?;
            let issue = ds.create_issue(NewIssue {
                title: &title,
                description: description.as_deref().unwrap_or(""),
                status: "todo",
                priority: &priority,
                labels: "",
                assignee: assignee.as_deref().unwrap_or(""),
            })?;
            print_json(&issue)
        }
        [command, key] if command == "comments" => {
            let comments = ds.list_issue_comments(key)?;
            print_json(&comments)
        }
        [command, key, rest @ ..] if command == "comment" => {
            let (author, body) = parse_comment_args(rest)?;
            let comment = ds.add_issue_comment(key, &author, &body)?;
            print_json(&comment)
        }
        [command, key] if command == "delete" => {
            ds.delete_issue(key)?;
            println!("Deleted issue {key}");
            Ok(())
        }
        _ => bail!("unsupported issues command; run `entrance issues --help` for usage"),
    }
}

fn parse_create_args(args: &[String]) -> Result<(String, Option<String>, String, Option<String>)> {
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut priority = "none".to_string();
    let mut assignee: Option<String> = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--title" => {
                index += 1;
                title = Some(args.get(index).cloned().unwrap_or_default());
            }
            "--description" | "--desc" => {
                index += 1;
                description = Some(args.get(index).cloned().unwrap_or_default());
            }
            "--priority" => {
                index += 1;
                priority = args.get(index).cloned().unwrap_or_else(|| "none".into());
            }
            "--assignee" => {
                index += 1;
                assignee = Some(args.get(index).cloned().unwrap_or_default());
            }
            other => {
                // If no --title flag, treat first positional arg as title
                if title.is_none() {
                    title = Some(other.to_string());
                } else {
                    bail!("unexpected argument: `{other}`");
                }
            }
        }
        index += 1;
    }

    let title = title.unwrap_or_default();
    if title.is_empty() {
        bail!("issue title is required; use `entrance issues create --title <text>`");
    }

    Ok((title, description, priority, assignee))
}

fn parse_comment_args(args: &[String]) -> Result<(String, String)> {
    let mut author: Option<String> = None;
    let mut body: Option<String> = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--author" => {
                index += 1;
                author = Some(args.get(index).cloned().unwrap_or_default());
            }
            "--body" => {
                index += 1;
                body = Some(args.get(index).cloned().unwrap_or_default());
            }
            _ => {}
        }
        index += 1;
    }

    let author = author.unwrap_or_else(|| "cli".to_string());
    let body =
        body.ok_or_else(|| anyhow::anyhow!("comment body is required; use `--body <text>`"))?;

    Ok((author, body))
}
