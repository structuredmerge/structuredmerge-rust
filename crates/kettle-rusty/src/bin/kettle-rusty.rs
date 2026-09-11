use std::{env, path::PathBuf, process::ExitCode};

use kettle_rusty::{
    ProjectReport, apply_packaged_template_inventory, apply_project,
    plan_packaged_template_inventory, plan_project,
};

#[derive(Debug, Eq, PartialEq)]
struct Command {
    action: Action,
    project_root: PathBuf,
    json: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum Action {
    Plan,
    Apply,
    TemplatePlan,
    TemplateApply,
}

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("kettle-rusty: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let command = parse_args(&args)?;
    let report = match command.action {
        Action::Plan => plan_project(&command.project_root),
        Action::Apply => apply_project(&command.project_root),
        Action::TemplatePlan => plan_packaged_template_inventory(&command.project_root),
        Action::TemplateApply => apply_packaged_template_inventory(&command.project_root),
    }
    .map_err(|error| error.to_string())?;

    if command.json {
        println!("{}", serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?);
    } else {
        print_human_report(&report);
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let usage =
        "usage: kettle-rusty <plan|apply|template-plan|template-apply> [--json] [PROJECT_ROOT]";
    let action = match args.first().map(String::as_str) {
        Some("plan") => Action::Plan,
        Some("apply") => Action::Apply,
        Some("template-plan") => Action::TemplatePlan,
        Some("template-apply") => Action::TemplateApply,
        Some("--help" | "-h") => return Err(usage.to_string()),
        Some(value) => return Err(format!("unknown action {value:?}; {usage}")),
        None => return Err(usage.to_string()),
    };

    let mut json = false;
    let mut project_root = None;
    for argument in &args[1..] {
        if argument == "--json" {
            json = true;
        } else if project_root.is_none() {
            project_root = Some(PathBuf::from(argument));
        } else {
            return Err(format!("unexpected argument {argument:?}; {usage}"));
        }
    }

    Ok(Command { action, project_root: project_root.unwrap_or_else(|| PathBuf::from(".")), json })
}

fn print_human_report(report: &ProjectReport) {
    println!("mode: {}", report.mode);
    println!("ready: {}", report.ready);
    println!("changed files: {}", report.changed_files.len());
    for path in &report.changed_files {
        println!("  {path}");
    }
    for diagnostic in &report.diagnostics {
        println!("diagnostic: {}", diagnostic.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plan_with_json_and_explicit_root() {
        assert_eq!(
            parse_args(&["plan".into(), "--json".into(), "project".into()])
                .expect("arguments should parse"),
            Command { action: Action::Plan, project_root: PathBuf::from("project"), json: true }
        );
    }

    #[test]
    fn rejects_duplicate_project_roots() {
        let error = parse_args(&["apply".into(), "one".into(), "two".into()])
            .expect_err("arguments should fail");
        assert!(error.contains("unexpected argument"));
    }

    #[test]
    fn parses_packaged_template_application() {
        assert_eq!(
            parse_args(&["template-apply".into()]).expect("arguments should parse"),
            Command {
                action: Action::TemplateApply,
                project_root: PathBuf::from("."),
                json: false
            }
        );
    }
}
