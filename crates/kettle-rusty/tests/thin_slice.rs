use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use kettle_rusty::{
    apply_packaged_template_inventory, apply_project, apply_readme_style,
    plan_packaged_template_inventory, plan_project, plan_readme_style,
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
        "README.md".to_string(),
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
    let readme =
        fs::read_to_string(project_root.join("README.md")).expect("README template should exist");
    for snippet in [
        "# widget",
        "## Synopsis",
        "## Installation",
        "cargo add widget",
        "## Configuration",
        "## Basic Usage",
    ] {
        assert!(
            readme.contains(snippet),
            "expected README template to include {snippet:?}, got:\n{readme}"
        );
    }

    let second = apply_packaged_template_inventory(&project_root).expect("reapply should succeed");
    assert!(second.changed_files.is_empty());
    assert_eq!(
        fs::read_to_string(project_root.join(".github/workflows/ci.yml"))
            .expect("CI template should still exist"),
        ci
    );
    assert_eq!(
        fs::read_to_string(project_root.join("README.md")).expect("README template should exist"),
        readme
    );

    fs::remove_dir_all(project_root).expect("temporary project should be removable");
}

#[test]
fn conforms_to_readme_style_profile() {
    let style_fixture: Value = read_json(
        &repo_root()
            .join("fixtures/diagnostics/slice-740-kettle-readme-style-profile/kettle-readme-style-profile.json"),
    );
    assert_eq!(
        style_fixture["profile"]["name"],
        Value::String("kettle-readme-style-profile".to_string())
    );

    let project_root = manifest_dir().join("tmp/readme-style-profile");
    let _ = fs::remove_dir_all(&project_root);
    write_tree(
        &project_root,
        &BTreeMap::from([
            (
                "Cargo.toml".to_string(),
                "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.85\"\nrepository = \"https://github.com/acme/widget\"\nlicense = \"MIT\"\n".to_string(),
            ),
            (
                "kettle.yml".to_string(),
                [
                    "readme:",
                    "  style: thin",
                    "  project_emoji: \"🦀\"",
                    "  logo_row:",
                    "    enabled: true",
                    "    max_count: 3",
                    "    logos:",
                    "      - type: language",
                    "        slug: rust-lang",
                    "        alt: Rust language logo",
                    "      - type: org",
                    "        slug: acme",
                    "        alt: Acme org logo",
                    "      - type: affiliated_project",
                    "        slug: tree-sitter/tree-sitter",
                    "        alt: Tree-sitter project logo",
                    "      - type: project",
                    "        slug: acme/ignored",
                    "        alt: Ignored fourth logo",
                    "  preserve_sections:",
                    "    - Synopsis",
                    "    - Configuration",
                    "    - Basic Usage",
                    "  section_aliases:",
                    "    Usage: Basic Usage",
                    "  conditional_sections:",
                    "    floss_funding: default_for_mit_opt_in_otherwise",
                    "  badges:",
                    "    disabled:",
                    "      - coveralls",
                    "  license:",
                    "    spdx:",
                    "      - MIT",
                    "",
                ]
                .join("\n"),
            ),
            ("SECURITY.md".to_string(), "# Security\n".to_string()),
            (
                "README.md".to_string(),
                [
                    "# Old Widget",
                    "",
                    "## Summary",
                    "",
                    "Destination synopsis.",
                    "",
                    "## Configuration",
                    "",
                    "Destination configuration.",
                    "",
                    "## Usage",
                    "",
                    "Destination usage.",
                    "",
                ]
                .join("\n"),
            ),
        ]),
    );

    let plan = plan_readme_style(&project_root).expect("README style plan should succeed");
    assert!(plan.changed);
    assert_eq!(plan.style, "thin");
    for section in ["Synopsis", "Configuration", "Basic Usage"] {
        assert!(plan.preserved_sections.contains(&section.to_string()));
    }
    for section in [
        "Logos",
        "Project Name",
        "Badges",
        "Synopsis",
        "Installation",
        "Configuration",
        "Basic Usage",
        "FLOSS Funding",
        "Security",
        "Contributing",
        "Versioning",
        "License",
        "A request for help",
    ] {
        assert!(plan.rendered_sections.contains(&section.to_string()), "{section}");
    }
    assert!(plan.omitted_sections.contains(&"Hostile RubyGems Takeover".to_string()));
    assert!(plan.omitted_sections.contains(&"Secure Installation".to_string()));
    assert!(plan.missing_integrations.contains(&"codecov".to_string()));
    assert!(plan.missing_integrations.contains(&"qlty".to_string()));
    assert!(!plan.missing_integrations.contains(&"coveralls".to_string()));
    assert!(plan.disabled_integrations.contains(&"coveralls".to_string()));
    assert!(!plan.final_content.contains("Ignored fourth logo"));
    for snippet in [
        "# 🦀 widget",
        "## 🌻 Synopsis\n\nDestination synopsis.",
        "## ⚙️ Configuration\n\nDestination configuration.",
        "## 🔧 Basic Usage\n\nDestination usage.",
        "## 🔐 Security\n\nSee [SECURITY.md](SECURITY.md).",
        "## 🦷 FLOSS Funding",
        "cargo add widget",
        "https://logos.galtzo.com/assets/images/tree-sitter/tree-sitter/avatar-192px.svg",
        "StructuredMerge packages provide fixture-backed merge behavior",
        "| tree-sitter-language-pack | Go, Ruby, Rust, TypeScript | markdown, toml, yaml, source |",
        "| bash-merge, dotenv-merge, rbs-merge | Excluded from generated support tables until explicit scope decisions exist |",
    ] {
        assert!(plan.final_content.contains(snippet), "{snippet}\n{}", plan.final_content);
    }

    let apply = apply_readme_style(&project_root).expect("README style apply should succeed");
    assert!(apply.changed);
    let second = apply_readme_style(&project_root).expect("README style reapply should succeed");
    assert!(!second.changed);

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
