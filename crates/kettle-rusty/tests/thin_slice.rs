use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use kettle_rusty::{
    apply_packaged_template_inventory, apply_project, plan_packaged_template_inventory,
    plan_project,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct ThinSliceFixture {
    case_id: String,
    ecosystem: String,
    inputs: FixtureInputs,
    expected: FixtureExpected,
}

#[derive(Debug, Deserialize)]
struct FixtureInputs {
    files: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct FixtureExpected {
    facts: Value,
    changed_files: Vec<String>,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ThinSliceContract {
    canonical_recipes: Vec<CanonicalRecipe>,
    required_fact_groups: Vec<String>,
    ecosystem_fact_groups: BTreeMap<String, String>,
    report_contract: ReportContract,
    validated_ecosystems: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CanonicalRecipe {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReportContract {
    request_envelope_kind: String,
    report_envelope_kind: String,
}

#[test]
fn plans_and_applies_cargo_package_templating_requests() {
    let fixture =
        read_json::<ThinSliceFixture>(&manifest_dir().join("tests/fixtures/thin-slice.json"));
    let contract = read_json::<ThinSliceContract>(
        &repo_root().join("fixtures/packaging/thin-slice-contract.json"),
    );
    let project_root = manifest_dir().join("tmp/thin-slice");
    let _ = fs::remove_dir_all(&project_root);
    write_tree(&project_root, &fixture.inputs.files);

    let expected_recipe_names =
        contract.canonical_recipes.iter().map(|recipe| recipe.name.clone()).collect::<Vec<_>>();
    assert!(fixture.case_id.starts_with("kettle-rusty-vnext-"));
    assert!(contract.validated_ecosystems.contains(&fixture.ecosystem));
    let facts = fixture.expected.facts.as_object().expect("expected facts should be an object");
    for group in &contract.required_fact_groups {
        assert!(facts.contains_key(group));
    }
    assert!(facts.contains_key(&contract.ecosystem_fact_groups[&fixture.ecosystem]));

    let plan = plan_project(&project_root).expect("plan should succeed");
    assert_eq!(
        serde_json::to_value(&plan.facts).expect("facts should serialize"),
        fixture.expected.facts
    );
    assert_eq!(
        plan.recipe_pack
            .recipes
            .iter()
            .map(|recipe| recipe.name.as_str().to_string())
            .collect::<Vec<_>>(),
        expected_recipe_names
    );
    assert_eq!(plan.changed_files, fixture.expected.changed_files);
    assert_eq!(
        unique_request_kinds(&plan.recipe_reports),
        vec![contract.report_contract.request_envelope_kind.clone()]
    );
    assert_eq!(
        unique_report_kinds(&plan.recipe_reports),
        vec![contract.report_contract.report_envelope_kind.clone()]
    );

    let apply = apply_project(&project_root).expect("apply should succeed");
    assert_eq!(apply.changed_files, fixture.expected.changed_files);
    assert_eq!(
        read_project_files(&project_root, fixture.expected.files.keys()),
        fixture.expected.files
    );

    fs::remove_dir_all(project_root).expect("temporary project should be removable");
}

#[test]
fn plans_applies_and_reapplies_packaged_template_inventory() {
    let project_root = manifest_dir().join("tmp/packaged-template-inventory");
    let _ = fs::remove_dir_all(&project_root);
    write_tree(
        &project_root,
        &BTreeMap::from([(
            "Cargo.toml".to_string(),
            "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2021\"\nrust-version = \"1.75\"\n".to_string(),
        )]),
    );

    let expected_changed = vec![
        ".cargo/config.toml".to_string(),
        ".editorconfig".to_string(),
        ".github/workflows/ci.yml".to_string(),
        ".gitignore".to_string(),
        "rustfmt.toml".to_string(),
    ];
    let plan = plan_packaged_template_inventory(&project_root).expect("plan should succeed");
    assert_eq!(plan.recipe_pack.name, "kettle-rusty-packaged-template-inventory");
    assert_eq!(plan.changed_files, expected_changed);

    let apply = apply_packaged_template_inventory(&project_root).expect("apply should succeed");
    assert_eq!(apply.changed_files, expected_changed);
    let ci = fs::read_to_string(project_root.join(".github/workflows/ci.yml"))
        .expect("CI template should exist");
    assert!(ci.contains("toolchain: \"1.75\""));

    let second = apply_packaged_template_inventory(&project_root).expect("reapply should succeed");
    assert!(second.changed_files.is_empty());
    assert_eq!(
        fs::read_to_string(project_root.join(".github/workflows/ci.yml"))
            .expect("CI template should still exist"),
        ci
    );

    fs::remove_dir_all(project_root).expect("temporary project should be removable");
}

fn unique_request_kinds(reports: &[kettle_rusty::RecipeRunReport]) -> Vec<String> {
    reports.iter().fold(Vec::new(), |mut kinds, report| {
        if !kinds.contains(&report.request_envelope.kind) {
            kinds.push(report.request_envelope.kind.clone());
        }
        kinds
    })
}

fn unique_report_kinds(reports: &[kettle_rusty::RecipeRunReport]) -> Vec<String> {
    reports.iter().fold(Vec::new(), |mut kinds, report| {
        if !kinds.contains(&report.report_envelope.kind) {
            kinds.push(report.report_envelope.kind.clone());
        }
        kinds
    })
}

fn write_tree(root: &Path, files: &BTreeMap<String, String>) {
    for (relative_path, content) in files {
        let target_path = root.join(relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be creatable");
        }
        fs::write(target_path, content).expect("fixture file should be writable");
    }
}

fn read_project_files<'a>(
    root: &Path,
    paths: impl Iterator<Item = &'a String>,
) -> BTreeMap<String, String> {
    paths
        .map(|relative_path| {
            (
                relative_path.clone(),
                fs::read_to_string(root.join(relative_path))
                    .expect("project file should be readable"),
            )
        })
        .collect()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> T {
    let source = fs::read_to_string(path).expect("json fixture should be readable");
    serde_json::from_str(&source).expect("json fixture should deserialize")
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    manifest_dir().join("../../..")
}
