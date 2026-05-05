use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use ast_merge::{
    ContentRecipeExecutionReport, ContentRecipeExecutionReportEnvelope,
    ContentRecipeExecutionRequest, ContentRecipeExecutionRequestEnvelope, ContentRecipeStep,
    ContentRecipeStepReport, Diagnostic, STRUCTURED_EDIT_TRANSPORT_VERSION,
    StructuredEditApplication, StructuredEditOperationProfile, StructuredEditRequest,
    StructuredEditResult,
};
use serde::Serialize;
use serde_json::{Value, json};
use toml::Value as TomlValue;

pub const PACKAGE_NAME: &str = "kettle-rusty";

const MANAGED_BLOCK_OPEN: &str = "// <<kettle-rusty:generated>> do not edit below this line";
const MANAGED_BLOCK_CLOSE: &str = "// <</kettle-rusty:generated>>";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PackageFacts {
    pub package: PackageFactGroup,
    pub cargo: CargoFactGroup,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PackageFactGroup {
    pub ecosystem: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_expression: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CargoFactGroup {
    pub manifest_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecipePack {
    pub name: String,
    pub version: u32,
    pub ecosystem: String,
    pub recipes: Vec<PackagingRecipe>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagingRecipeName {
    ReadmeMetadata,
    ChangelogUnreleased,
    GeneratedBlockSync,
    TemplateSourceApplication,
}

impl PackagingRecipeName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadmeMetadata => "readme_metadata",
            Self::ChangelogUnreleased => "changelog_unreleased",
            Self::GeneratedBlockSync => "generated_block_sync",
            Self::TemplateSourceApplication => "template_source_application",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PackagingRecipe {
    pub name: PackagingRecipeName,
    pub target_path: String,
    pub provider_family: String,
    pub primitive: String,
    pub facts: Vec<String>,
    pub selectors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RecipeRunReport {
    pub recipe_name: PackagingRecipeName,
    pub relative_path: String,
    pub changed: bool,
    pub request_envelope: ContentRecipeExecutionRequestEnvelope,
    pub report_envelope: ContentRecipeExecutionReportEnvelope,
    pub final_content: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectReport {
    pub mode: String,
    pub ready: bool,
    pub facts: PackageFacts,
    pub recipe_pack: RecipePack,
    pub recipe_reports: Vec<RecipeRunReport>,
    pub changed_files: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub enum KettleRustyError {
    Io { path: PathBuf, source: io::Error },
    Toml { path: PathBuf, source: toml::de::Error },
    MissingPackageTable { path: PathBuf },
    MissingPackageName { path: PathBuf },
}

pub fn discover_facts(project_root: &Path) -> Result<PackageFacts, KettleRustyError> {
    let manifest_path = project_root.join("Cargo.toml");
    let manifest_source = fs::read_to_string(&manifest_path)
        .map_err(|source| KettleRustyError::Io { path: manifest_path.clone(), source })?;
    let manifest: TomlValue = toml::from_str(&manifest_source)
        .map_err(|source| KettleRustyError::Toml { path: manifest_path.clone(), source })?;
    let package = manifest
        .get("package")
        .and_then(TomlValue::as_table)
        .ok_or_else(|| KettleRustyError::MissingPackageTable { path: manifest_path.clone() })?;
    let name = string_field(package, "name")
        .ok_or_else(|| KettleRustyError::MissingPackageName { path: manifest_path.clone() })?;

    Ok(PackageFacts {
        package: PackageFactGroup {
            ecosystem: "crates".to_string(),
            name: name.clone(),
            slug: name,
            description: string_field(package, "description"),
            homepage_url: string_field(package, "homepage"),
            source_url: string_field(package, "repository"),
            license_expression: string_field(package, "license"),
        },
        cargo: CargoFactGroup {
            manifest_path: "Cargo.toml".to_string(),
            rust_version: string_field(package, "rust-version"),
            edition: string_field(package, "edition"),
        },
    })
}

pub fn recipe_pack() -> RecipePack {
    RecipePack {
        name: "kettle-rusty-core".to_string(),
        version: 1,
        ecosystem: "crates".to_string(),
        recipes: vec![
            recipe_entry(
                PackagingRecipeName::ReadmeMetadata,
                "README.md",
                "markdown",
                "supplied_readme_metadata_synchronization",
                &["package", "funding", "readme"],
            ),
            recipe_entry(
                PackagingRecipeName::ChangelogUnreleased,
                "CHANGELOG.md",
                "markdown",
                "changelog_unreleased_normalization",
                &["package", "changelog"],
            ),
            recipe_entry(
                PackagingRecipeName::GeneratedBlockSync,
                "src/generated_package_info.rs",
                "text",
                "supplied_managed_text_block_replacement",
                &["package", "generated_blocks"],
            ),
        ],
    }
}

pub fn packaged_template_inventory_pack() -> RecipePack {
    RecipePack {
        name: "kettle-rusty-packaged-template-inventory".to_string(),
        version: 1,
        ecosystem: "crates".to_string(),
        recipes: vec![
            template_recipe(".cargo/config.toml"),
            template_recipe(".editorconfig"),
            template_recipe(".github/workflows/ci.yml"),
            template_recipe(".gitignore"),
            template_recipe("rustfmt.toml"),
        ],
    }
}

pub fn plan_project(project_root: &Path) -> Result<ProjectReport, KettleRustyError> {
    let facts = discover_facts(project_root)?;
    let pack = recipe_pack();
    let files = read_project_files(project_root, &pack)?;
    let recipe_reports = pack
        .recipes
        .iter()
        .map(|recipe| execute_recipe(project_root, recipe, &facts, &files))
        .collect::<Vec<_>>();
    let changed_files = changed_files_for_reports(&recipe_reports);

    Ok(ProjectReport {
        mode: "plan".to_string(),
        ready: true,
        facts,
        recipe_pack: pack,
        recipe_reports,
        changed_files,
        diagnostics: vec![],
    })
}

pub fn plan_packaged_template_inventory(
    project_root: &Path,
) -> Result<ProjectReport, KettleRustyError> {
    let facts = discover_facts(project_root)?;
    let pack = packaged_template_inventory_pack();
    let files = read_project_files(project_root, &pack)?;
    let recipe_reports = pack
        .recipes
        .iter()
        .map(|recipe| execute_recipe(project_root, recipe, &facts, &files))
        .collect::<Vec<_>>();
    let changed_files = changed_files_for_reports(&recipe_reports);

    Ok(ProjectReport {
        mode: "plan".to_string(),
        ready: true,
        facts,
        recipe_pack: pack,
        recipe_reports,
        changed_files,
        diagnostics: vec![],
    })
}

pub fn apply_packaged_template_inventory(
    project_root: &Path,
) -> Result<ProjectReport, KettleRustyError> {
    let mut report = plan_packaged_template_inventory(project_root)?;
    report.mode = "apply".to_string();
    for recipe_report in &report.recipe_reports {
        if !recipe_report.changed {
            continue;
        }

        let target_path = project_root.join(&recipe_report.relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| KettleRustyError::Io { path: parent.to_path_buf(), source })?;
        }
        fs::write(&target_path, &recipe_report.final_content)
            .map_err(|source| KettleRustyError::Io { path: target_path, source })?;
    }

    Ok(report)
}

pub fn apply_project(project_root: &Path) -> Result<ProjectReport, KettleRustyError> {
    let mut report = plan_project(project_root)?;
    report.mode = "apply".to_string();
    for recipe_report in &report.recipe_reports {
        if !recipe_report.changed {
            continue;
        }

        let target_path = project_root.join(&recipe_report.relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| KettleRustyError::Io { path: parent.to_path_buf(), source })?;
        }
        fs::write(&target_path, &recipe_report.final_content)
            .map_err(|source| KettleRustyError::Io { path: target_path, source })?;
    }

    Ok(report)
}

pub fn content_recipe_execution_request(
    request: ContentRecipeExecutionRequest,
) -> ContentRecipeExecutionRequest {
    request
}

pub fn content_recipe_execution_request_envelope(
    request: ContentRecipeExecutionRequest,
) -> ContentRecipeExecutionRequestEnvelope {
    ContentRecipeExecutionRequestEnvelope {
        kind: "content_recipe_execution_request".to_string(),
        version: STRUCTURED_EDIT_TRANSPORT_VERSION,
        request,
    }
}

pub fn content_recipe_execution_report(
    report: ContentRecipeExecutionReport,
) -> ContentRecipeExecutionReport {
    report
}

pub fn content_recipe_execution_report_envelope(
    report: ContentRecipeExecutionReport,
) -> ContentRecipeExecutionReportEnvelope {
    ContentRecipeExecutionReportEnvelope {
        kind: "content_recipe_execution_report".to_string(),
        version: STRUCTURED_EDIT_TRANSPORT_VERSION,
        report,
    }
}

fn execute_recipe(
    project_root: &Path,
    recipe: &PackagingRecipe,
    facts: &PackageFacts,
    files: &HashMap<String, String>,
) -> RecipeRunReport {
    let original = files.get(&recipe.target_path).cloned().unwrap_or_default();
    let final_content = match recipe.name {
        PackagingRecipeName::ReadmeMetadata => synchronize_readme(&original, facts),
        PackagingRecipeName::ChangelogUnreleased => normalize_changelog(&original),
        PackagingRecipeName::GeneratedBlockSync => synchronize_managed_block(&original, facts),
        PackagingRecipeName::TemplateSourceApplication => {
            render_packaged_template(&recipe.target_path, facts)
        }
    };
    let request = content_recipe_execution_request(ContentRecipeExecutionRequest {
        recipe_name: recipe.primitive.clone(),
        recipe_version: "1".to_string(),
        relative_path: recipe.target_path.clone(),
        provider_family: recipe.provider_family.clone(),
        provider_backend: None,
        template_content: String::new(),
        destination_content: original.clone(),
        steps: vec![content_recipe_step(recipe)],
        runtime_context: runtime_context(facts),
        metadata: HashMap::from([
            ("packaging_recipe".to_string(), json!(recipe.name.as_str())),
            ("project_root".to_string(), json!(project_root.display().to_string())),
        ]),
    });
    let changed = final_content != original;
    let step_report =
        content_recipe_step_report(recipe, &request, &original, &final_content, changed);
    let report = content_recipe_execution_report(ContentRecipeExecutionReport {
        request: request.clone(),
        final_content: final_content.clone(),
        changed,
        step_reports: vec![step_report],
        diagnostics: vec![],
        metadata: HashMap::from([("packaging_recipe".to_string(), json!(recipe.name.as_str()))]),
    });

    RecipeRunReport {
        recipe_name: recipe.name,
        relative_path: recipe.target_path.clone(),
        changed,
        request_envelope: content_recipe_execution_request_envelope(request),
        report_envelope: content_recipe_execution_report_envelope(report),
        final_content,
        diagnostics: vec![],
    }
}

fn template_recipe(target_path: &str) -> PackagingRecipe {
    recipe_entry(
        PackagingRecipeName::TemplateSourceApplication,
        target_path,
        "text",
        "supplied_template_source_application",
        &["package", "templates"],
    )
}

fn changed_files_for_reports(reports: &[RecipeRunReport]) -> Vec<String> {
    let mut changed_files = reports
        .iter()
        .filter(|report| report.changed)
        .map(|report| report.relative_path.clone())
        .collect::<Vec<_>>();
    changed_files.sort();
    changed_files
}

fn render_packaged_template(target_path: &str, facts: &PackageFacts) -> String {
    packaged_template_content(target_path)
        .replace("{{PACKAGE_NAME}}", &facts.package.name)
        .replace("{{RUST_VERSION}}", facts.cargo.rust_version.as_deref().unwrap_or("stable"))
        .replace("{{RUST_EDITION}}", facts.cargo.edition.as_deref().unwrap_or("2021"))
}

fn packaged_template_content(target_path: &str) -> &'static str {
    match target_path {
        ".cargo/config.toml" => "[build]\nrustflags = []\n",
        ".editorconfig" => {
            "root = true\n\n[*]\ncharset = utf-8\nend_of_line = lf\ninsert_final_newline = true\ntrim_trailing_whitespace = true\n"
        }
        ".github/workflows/ci.yml" => {
            "name: CI\n\non:\n  push:\n  pull_request:\n\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: dtolnay/rust-toolchain@stable\n        with:\n          toolchain: \"{{RUST_VERSION}}\"\n      - run: cargo test --all-features\n"
        }
        ".gitignore" => "/target/\nCargo.lock\n",
        "rustfmt.toml" => "edition = \"{{RUST_EDITION}}\"\nnewline_style = \"Unix\"\n",
        _ => "",
    }
}

fn content_recipe_step(recipe: &PackagingRecipe) -> ContentRecipeStep {
    ContentRecipeStep {
        step_id: recipe.name.as_str().to_string(),
        step_kind: "structured_edit".to_string(),
        name: recipe.name.as_str().to_string(),
        provider_family: Some(recipe.provider_family.clone()),
        provider_backend: None,
        merge_profile: None,
        partial_target: None,
        structured_edit_request: None,
        policy: None,
        metadata: HashMap::from([
            ("primitive".to_string(), json!(recipe.primitive)),
            ("target_path".to_string(), json!(recipe.target_path)),
        ]),
    }
}

fn content_recipe_step_report(
    recipe: &PackagingRecipe,
    _request: &ContentRecipeExecutionRequest,
    original: &str,
    final_content: &str,
    changed: bool,
) -> ContentRecipeStepReport {
    let operation_profile = StructuredEditOperationProfile {
        operation_kind: recipe.primitive.clone(),
        operation_family: Some("kettle-rusty".to_string()),
        known_operation_kind: true,
        source_requirement: "destination_content".to_string(),
        destination_requirement: "relative_path".to_string(),
        replacement_source: "runtime_context".to_string(),
        captures_source_text: false,
        supports_if_missing: true,
        metadata: HashMap::new(),
    };
    let result = StructuredEditResult {
        operation_kind: recipe.primitive.clone(),
        updated_content: final_content.to_string(),
        changed,
        captured_text: None,
        match_count: None,
        operation_profile,
        destination_profile: None,
        metadata: HashMap::new(),
    };
    let application = StructuredEditApplication {
        request: StructuredEditRequest {
            operation_kind: recipe.primitive.clone(),
            content: original.to_string(),
            source_label: recipe.target_path.clone(),
            target_selector: None,
            target_selector_family: None,
            target_selection: None,
            target_match: None,
            destination_selector: None,
            destination_selector_family: None,
            payload_text: None,
            if_missing: None,
            callable_destination: None,
            metadata: HashMap::from([(
                "packaging_recipe".to_string(),
                json!(recipe.name.as_str()),
            )]),
        },
        result,
        metadata: HashMap::new(),
    };

    ContentRecipeStepReport {
        step_id: recipe.name.as_str().to_string(),
        step_kind: recipe.primitive.clone(),
        status: if changed { "applied" } else { "unchanged" }.to_string(),
        changed,
        input_content: original.to_string(),
        output_content: final_content.to_string(),
        application: Some(application),
        diagnostics: vec![],
        metadata: HashMap::from([("target_path".to_string(), json!(recipe.target_path))]),
    }
}

fn synchronize_readme(content: &str, facts: &PackageFacts) -> String {
    let mut lines = content.split('\n').map(str::to_string).collect::<Vec<_>>();
    let heading = format!("# {}", facts.package.name);
    if let Some(index) = lines.iter().position(|line| line.starts_with("# ")) {
        lines[index] = heading;
    } else {
        lines.insert(0, String::new());
        lines.insert(0, heading);
    }

    replace_markdown_managed_block(
        &lines.join("\n"),
        "kettle-rusty:metadata",
        &readme_metadata_block(facts),
    )
}

fn normalize_changelog(content: &str) -> String {
    let mut text = content.to_string();
    if !text.lines().next().is_some_and(|line| line.starts_with("# ")) {
        text = format!("# Changelog\n\n{text}");
    }
    if text.lines().any(|line| {
        line.to_ascii_lowercase().starts_with("## [unreleased]")
            || line.to_ascii_lowercase().starts_with("## unreleased")
    }) {
        return ensure_trailing_newline(&text);
    }

    let mut lines = text.split('\n').map(str::to_string).collect::<Vec<_>>();
    let insert_at = lines.iter().position(|line| line.starts_with("## ")).unwrap_or(lines.len());
    let section = ["", "## [Unreleased]", "", "### Added", "", "### Changed", "", "### Fixed", ""];
    for (offset, line) in section.iter().enumerate() {
        lines.insert(insert_at + offset, (*line).to_string());
    }

    ensure_trailing_newline(&collapse_blank_lines(&lines.join("\n")))
}

fn synchronize_managed_block(content: &str, facts: &PackageFacts) -> String {
    let replacement = format!(
        "{MANAGED_BLOCK_OPEN}\npub const PACKAGE_NAME: &str = {:?};\npub const PACKAGE_ECOSYSTEM: &str = {:?};\n{MANAGED_BLOCK_CLOSE}\n",
        facts.package.name, facts.package.ecosystem
    );
    replace_text_managed_block(content, &replacement)
}

fn read_project_files(
    project_root: &Path,
    pack: &RecipePack,
) -> Result<HashMap<String, String>, KettleRustyError> {
    pack.recipes
        .iter()
        .map(|recipe| {
            let target_path = project_root.join(&recipe.target_path);
            if target_path.exists() {
                fs::read_to_string(&target_path)
                    .map(|content| (recipe.target_path.clone(), content))
                    .map_err(|source| KettleRustyError::Io { path: target_path, source })
            } else {
                Ok((recipe.target_path.clone(), String::new()))
            }
        })
        .collect()
}

fn recipe_entry(
    name: PackagingRecipeName,
    target_path: &str,
    provider_family: &str,
    primitive: &str,
    facts: &[&str],
) -> PackagingRecipe {
    PackagingRecipe {
        name,
        target_path: target_path.to_string(),
        provider_family: provider_family.to_string(),
        primitive: primitive.to_string(),
        facts: facts.iter().map(|fact| (*fact).to_string()).collect(),
        selectors: vec![],
    }
}

fn readme_metadata_block(facts: &PackageFacts) -> String {
    let rows = [
        ("Package", Some(facts.package.name.as_str())),
        ("Description", facts.package.description.as_deref()),
        ("Homepage", facts.package.homepage_url.as_deref()),
        ("Source", facts.package.source_url.as_deref()),
        ("License", facts.package.license_expression.as_deref()),
    ]
    .into_iter()
    .filter_map(|(field, value)| value.map(|value| format!("| {field} | {value} |")))
    .collect::<Vec<_>>();

    [
        "<!-- kettle-rusty:metadata:start -->".to_string(),
        "| Field | Value |".to_string(),
        "|---|---|".to_string(),
        rows.join("\n"),
        "<!-- kettle-rusty:metadata:end -->".to_string(),
    ]
    .into_iter()
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn replace_markdown_managed_block(content: &str, marker: &str, replacement: &str) -> String {
    replace_between_markers(
        content,
        &format!("<!-- {marker}:start -->"),
        &format!("<!-- {marker}:end -->"),
        replacement,
        || format!("{}\n\n{replacement}\n", content.trim_end()),
    )
}

fn replace_text_managed_block(content: &str, replacement: &str) -> String {
    replace_between_markers(content, MANAGED_BLOCK_OPEN, MANAGED_BLOCK_CLOSE, replacement, || {
        [content.trim_end(), replacement]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn replace_between_markers(
    content: &str,
    open_marker: &str,
    close_marker: &str,
    replacement: &str,
    fallback: impl FnOnce() -> String,
) -> String {
    let Some(open_index) = content.find(open_marker) else {
        return fallback();
    };
    let Some(close_index) =
        content[open_index..].find(close_marker).map(|index| open_index + index)
    else {
        return fallback();
    };

    let mut close_end = close_index + close_marker.len();
    if content[close_end..].starts_with('\n') {
        close_end += 1;
    }

    format!("{}{replacement}\n{}", &content[..open_index], &content[close_end..])
}

fn runtime_context(facts: &PackageFacts) -> HashMap<String, Value> {
    HashMap::from([
        ("package".to_string(), json!(facts.package)),
        ("cargo".to_string(), json!(facts.cargo)),
    ])
}

fn string_field(package: &toml::map::Map<String, TomlValue>, key: &str) -> Option<String> {
    package
        .get(key)
        .and_then(TomlValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn ensure_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') { text.to_string() } else { format!("{text}\n") }
}

fn collapse_blank_lines(text: &str) -> String {
    let mut collapsed = String::new();
    let mut previous_blank = false;
    for line in text.lines() {
        let blank = line.is_empty();
        if blank && previous_blank {
            continue;
        }
        collapsed.push_str(line);
        collapsed.push('\n');
        previous_blank = blank;
    }
    collapsed.trim_end_matches('\n').to_string()
}
