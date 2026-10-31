use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn emits_json_plan_report_from_the_executable() {
    let project_root = temporary_project("json-plan");
    write_file(
        &project_root.join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let output = run(&["plan", "--json", project_root.to_str().unwrap()]);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["mode"], "plan");
    assert_eq!(report["ready"], true);
    assert!(report["recipe_reports"].as_array().is_some());

    fs::remove_dir_all(project_root).expect("temporary project should be removable");
}

#[test]
fn returns_usage_status_for_unknown_actions() {
    let output = run(&["unknown-action"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown action"));
    assert!(stderr.contains("usage: kettle-rusty"));
}

#[test]
fn returns_project_error_status_for_missing_cargo_manifest() {
    let project_root = temporary_project("missing-manifest");
    let output = run(&["plan", "--json", project_root.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cargo.toml"));

    fs::remove_dir_all(project_root).expect("temporary project should be removable");
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kettle-rusty"))
        .args(arguments)
        .output()
        .expect("kettle-rusty executable should launch")
}

fn temporary_project(name: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("tmp")
        .join(format!("cli-{name}-{}-{timestamp}-{id}", std::process::id()));
    fs::create_dir_all(&path).expect("temporary project should be created");
    path
}

fn write_file(path: &std::path::Path, content: &str) {
    fs::create_dir_all(path.parent().expect("file should have a parent"))
        .expect("parent directories should be created");
    fs::write(path, content).expect("fixture file should be written");
}
