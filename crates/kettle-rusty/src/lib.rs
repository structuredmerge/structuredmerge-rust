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
use serde::{Deserialize, Serialize};
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadmeStyleReport {
    pub readme_path: String,
    pub changed: bool,
    pub style: String,
    pub preserved_sections: Vec<String>,
    pub rendered_sections: Vec<String>,
    pub omitted_sections: Vec<String>,
    pub missing_integrations: Vec<String>,
    pub disabled_integrations: Vec<String>,
    pub unresolved_logo_slugs: Vec<String>,
    pub license_files_changed: bool,
    pub copyright_authors: Vec<String>,
    pub final_content: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct KettleConfig {
    #[serde(default)]
    pub readme: ReadmeConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReadmeConfig {
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub project_emoji: String,
    #[serde(default)]
    pub logo_row: ReadmeLogoRowConfig,
    #[serde(default)]
    pub badges: ReadmeBadgesConfig,
    #[serde(default)]
    pub preserve_sections: Vec<String>,
    #[serde(default)]
    pub section_aliases: HashMap<String, String>,
    #[serde(default)]
    pub conditional_sections: ReadmeConditionalConfig,
    #[serde(default)]
    pub license: ReadmeLicenseConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReadmeLogoRowConfig {
    pub enabled: Option<bool>,
    #[serde(default)]
    pub max_count: usize,
    #[serde(default)]
    pub logos: Vec<ReadmeLogo>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReadmeLogo {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub alt: String,
    #[serde(default)]
    pub href: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReadmeBadgesConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReadmeConditionalConfig {
    #[serde(default)]
    pub floss_funding: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReadmeLicenseConfig {
    #[serde(default)]
    pub spdx: Vec<String>,
}

#[derive(Debug)]
pub enum KettleRustyError {
    Io { path: PathBuf, source: io::Error },
    Toml { path: PathBuf, source: toml::de::Error },
    Yaml { path: PathBuf, source: serde_yaml::Error },
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
            template_recipe("README.md"),
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

pub fn plan_readme_style(project_root: &Path) -> Result<ReadmeStyleReport, KettleRustyError> {
    let facts = discover_facts(project_root)?;
    let config = read_kettle_config(project_root)?;
    let readme_path = project_root.join("README.md");
    let original = if readme_path.exists() {
        fs::read_to_string(&readme_path)
            .map_err(|source| KettleRustyError::Io { path: readme_path.clone(), source })?
    } else {
        String::new()
    };
    let has_security = project_root.join("SECURITY.md").exists();
    Ok(render_readme_style(&original, &facts, config.readme, has_security))
}

pub fn apply_readme_style(project_root: &Path) -> Result<ReadmeStyleReport, KettleRustyError> {
    let report = plan_readme_style(project_root)?;
    if report.changed {
        let target_path = project_root.join(&report.readme_path);
        fs::write(&target_path, &report.final_content)
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
        "README.md" => {
            "# {{PACKAGE_NAME}}\n\n## Synopsis\n\n## Installation\n\n```sh\ncargo add {{PACKAGE_NAME}}\n```\n\n## Configuration\n\n## Basic Usage\n"
        }
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

fn read_kettle_config(project_root: &Path) -> Result<KettleConfig, KettleRustyError> {
    let path = project_root.join("kettle.yml");
    if !path.exists() {
        return Ok(KettleConfig::default());
    }
    let source = fs::read_to_string(&path)
        .map_err(|source| KettleRustyError::Io { path: path.clone(), source })?;
    serde_yaml::from_str(&source).map_err(|source| KettleRustyError::Yaml { path, source })
}

fn render_readme_style(
    destination: &str,
    facts: &PackageFacts,
    mut config: ReadmeConfig,
    has_security: bool,
) -> ReadmeStyleReport {
    default_readme_config(&mut config);
    let preserved = preserved_readme_sections(destination, &config);
    let license = readme_license(&config, facts);
    let (logo_row, unresolved_logo_slugs) = readme_logo_row(&config);
    let (badge_cloud, missing_integrations, disabled_integrations) =
        readme_badge_cloud(&config, facts, &license, has_security);
    let include_funding = should_include_funding(&config, &license);
    let mut rendered_sections = vec![
        "Project Name",
        "Badges",
        "Synopsis",
        "Info you can shake a stick at",
        "Installation",
        "Configuration",
        "Basic Usage",
        "Versioning",
        "License",
        "A request for help",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    if !logo_row.is_empty() {
        rendered_sections.insert(0, "Logos".to_string());
    }
    if include_funding {
        rendered_sections.push("FLOSS Funding".to_string());
    }
    if has_security {
        rendered_sections.push("Security".to_string());
    }
    rendered_sections.push("Contributing".to_string());

    let mut omitted_sections =
        vec!["Hostile RubyGems Takeover".to_string(), "Secure Installation".to_string()];
    if !include_funding {
        omitted_sections.push("FLOSS Funding".to_string());
    }
    if !has_security {
        omitted_sections.push("Security".to_string());
    }

    let mut sections = Vec::new();
    if !logo_row.is_empty() {
        sections.push(logo_row);
    }
    sections.push(format!("# {} {}", config.project_emoji, facts.package.name));
    if !badge_cloud.is_empty() {
        sections.push(badge_cloud);
    }
    sections.extend([
        format!("## 🌻 Synopsis\n\n{}", preserved.get("synopsis").cloned().unwrap_or_default()),
        format!(
            "## 💡 Info you can shake a stick at\n\nCompatible with Rust {}.\n\n{}",
            facts.cargo.rust_version.as_deref().unwrap_or("unknown"),
            readme_family_intro_and_backend_matrix()
        ),
        format!("## ✨ Installation\n\n```console\ncargo add {}\n```", facts.package.name),
        format!(
            "## ⚙️ Configuration\n\n{}",
            preserved.get("configuration").cloned().unwrap_or_default()
        ),
        format!(
            "## 🔧 Basic Usage\n\n{}",
            preserved.get("basic usage").cloned().unwrap_or_default()
        ),
    ]);
    if include_funding {
        sections.push("## 🦷 FLOSS Funding\n\nThis free software project accepts funding support when configured by the package maintainer.".to_string());
    }
    if has_security {
        sections.push("## 🔐 Security\n\nSee [SECURITY.md](SECURITY.md).".to_string());
    }
    sections.extend([
        "## 🤝 Contributing\n\nContributions are welcome. Missing optional service integrations are reported by the generator instead of rendered as broken badges.".to_string(),
        "## 📌 Versioning\n\nThis project follows semantic versioning for its public API where practical.".to_string(),
        format!("## 📄 License\n\n{}", license_paragraph(&license)),
        "## 🤑 A request for help\n\nPlease support the project by using it, reporting issues, and contributing improvements.".to_string(),
    ]);
    let final_content = ensure_trailing_newline(&sections.join("\n\n"));
    ReadmeStyleReport {
        readme_path: "README.md".to_string(),
        changed: final_content != destination,
        style: config.style,
        preserved_sections: vec![
            "Synopsis".to_string(),
            "Configuration".to_string(),
            "Basic Usage".to_string(),
        ],
        rendered_sections,
        omitted_sections,
        missing_integrations,
        disabled_integrations,
        unresolved_logo_slugs,
        license_files_changed: false,
        copyright_authors: vec![],
        final_content,
    }
}

fn readme_family_intro_and_backend_matrix() -> String {
    [
        "<details markdown=\"1\">",
        "<summary>StructuredMerge package family and backend compatibility</summary>",
        "",
        "StructuredMerge packages provide fixture-backed merge behavior for document, configuration, source, archive, and binary formats. Shared contracts live in fixtures, while Go, Ruby, Rust, and TypeScript packages expose language-native APIs over the same behavior.",
        "",
        "| Package | Layer | Families | Status | README role |",
        "|---|---|---|---|---|",
        "| ast-template | workflow | template, readme | active | applies shared templates, package README sections, and package-directory sync workflows |",
        "| ast-merge | core | template, review, structured-edit | active | documents provider-neutral contracts, token resolution, review state, and execution reports |",
        "| tree-haver | backend substrate | parser, backend | active | documents backend selection, language-pack integration, position data, and capability reporting |",
        "| markdown-merge | family | markdown | active | documents Markdown heading, fenced-code, nested-family, and provider behavior |",
        "| json-merge | family | json, jsonc | active | documents JSON and JSONC merge behavior; old jsonc-merge is superseded |",
        "| toml-merge | family | toml | active | documents TOML table, value, parser, and backend behavior |",
        "| yaml-merge | family | yaml | active | documents YAML mapping, sequence, scalar, and backend behavior |",
        "| ruby-merge | family | ruby-source | active | documents Ruby source merge behavior; old prism-merge is backend/provider prior art |",
        "| zip-merge | family | zip, archive | active | documents ZIP member planning and raw-preservation behavior |",
        "| binary-merge | family | binary | active | documents binary preservation and diagnostics behavior |",
        "",
        "JSONC migration note: JSONC is handled by `json-merge` as the `jsonc` dialect. The old `jsonc-merge` package name is superseded in the cross-language toolset; only Ruby may grow a legacy `require \"jsonc/merge\"` wrapper if packaging compatibility requires it. Current fixture-backed JSONC claims are parse support and comment-neutral owner structure; comment-preserving merge output, freeze blocks, and JSONC emitter behavior need dedicated fixtures before they appear in package examples.",
        "",
        "YAML provider note: `yaml-merge` is the canonical YAML family package. Ruby's `psych-merge` package is the Psych provider for that family, not a separate YAML family; old `Psych::Merge::*` examples remain provider-specific until portable fixtures cover the behavior.",
        "",
        "Markdown provider note: `markdown-merge` is the canonical Markdown family package. Provider packages own parser-specific docs and backend defaults: Go `goldmarkmerge`, Ruby `commonmarker-merge`, `markly-merge`, and `kramdown-merge`, Rust `pulldown-cmark-merge`, and TypeScript `@structuredmerge/markdown-it-merge`.",
        "",
        "| Backend | Languages | Families | Note |",
        "|---|---|---|---|",
        "| tree-sitter-language-pack | Go, Ruby, Rust, TypeScript | markdown, toml, yaml, source | Preferred cross-language parser substrate where a family has language-pack support. |",
        "| native ecosystem parser | Ruby | ruby, yaml, markdown, toml | Backend-specific Ruby packages are provider prior art or adapters, not the source schema. |",
        "| plain structured text | Go, Ruby, Rust, TypeScript | plain, binary, zip | Families without parser requirements document preservation, byte ranges, archive members, and diagnostics. |",
        "",
        "| Compatibility claim | Current disposition | Fixture source |",
        "|---|---|---|",
        "| Old Ruby runtime backend tables | Prior art only; not a cross-language support promise | slice-741 backend/platform reconciliation |",
        "| tree-sitter-language-pack | Current portable parser substrate for Go, Ruby, Rust, and TypeScript | slices 122, 135, 171, 195, 215 |",
        "| Native parser/adaptor backends | Implementation-specific providers documented through family fixtures | slices 122 and 183 |",
        "| bash-merge, dotenv-merge, rbs-merge | Excluded from generated support tables until explicit scope decisions exist | slice-741 unresolved package list |",
        "",
        "| Reusable example | README role | Source fixture |",
        "|---|---|---|",
        "| Freeze tokens | Show how destination-owned regions are preserved without filling project-specific usage sections | slice-743 reusable README configuration examples |",
        "| Match preference | Summarize template-wins and destination-wins conflict choices through current policy vocabulary | slice-743 reusable README configuration examples |",
        "| Template-only behavior | Explain accept/skip handling for unmatched template entries | slice-743 reusable README configuration examples |",
        "| Debug report inspection | Point users to structured reports and diagnostics instead of ad hoc debug prose | slice-743 reusable README configuration examples |",
        "| Backend selection | Describe portable backend selection without old Ruby runtime support tables | slice-743 reusable README configuration examples |",
        "| Package-directory README command | Document plan/apply/convergence workflow for shared README updates | slice-743 reusable README configuration examples |",
        "",
        "</details>",
    ]
    .join("\n")
}

fn default_readme_config(config: &mut ReadmeConfig) {
    if config.style.is_empty() {
        config.style = "thin".to_string();
    }
    if config.project_emoji.is_empty() {
        config.project_emoji = "💎".to_string();
    }
    if config.preserve_sections.is_empty() {
        config.preserve_sections =
            vec!["Synopsis".to_string(), "Configuration".to_string(), "Basic Usage".to_string()];
    }
    for (from, to) in [
        ("summary", "synopsis"),
        ("usage", "basic usage"),
        ("configuration options", "configuration"),
        ("setup", "basic usage"),
    ] {
        config.section_aliases.entry(from.to_string()).or_insert_with(|| to.to_string());
    }
    if config.logo_row.max_count == 0 {
        config.logo_row.max_count = 3;
    }
}

fn preserved_readme_sections(content: &str, config: &ReadmeConfig) -> HashMap<String, String> {
    let sections = markdown_section_bodies(content);
    let aliases = config
        .section_aliases
        .iter()
        .map(|(from, to)| (normalize_readme_heading(from), normalize_readme_heading(to)))
        .collect::<HashMap<_, _>>();
    let mut result = HashMap::new();
    for section in &config.preserve_sections {
        let key = normalize_readme_heading(section);
        let value = sections.get(&key).cloned().or_else(|| {
            aliases
                .iter()
                .find_map(|(from, to)| (to == &key).then(|| sections.get(from).cloned()).flatten())
        });
        result.insert(key, value.unwrap_or_default());
    }
    result
}

fn markdown_section_bodies(content: &str) -> HashMap<String, String> {
    let lines = content.lines().collect::<Vec<_>>();
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let level = line.chars().take_while(|ch| *ch == '#').count();
            if level == 0 || level > 6 || line.chars().nth(level).is_none_or(|ch| ch != ' ') {
                return None;
            }
            Some((index, level, normalize_readme_heading(&line[level + 1..])))
        })
        .collect::<Vec<_>>();
    let mut result = HashMap::new();
    for (index, (start, level, key)) in headings.iter().enumerate() {
        let end = headings[index + 1..]
            .iter()
            .find_map(|(candidate_start, candidate_level, _)| {
                (candidate_level <= level).then_some(*candidate_start)
            })
            .unwrap_or(lines.len());
        result.insert(key.clone(), lines[start + 1..end].join("\n").trim().to_string());
    }
    result
}

fn normalize_readme_heading(value: &str) -> String {
    let trimmed = value.trim();
    let fields = trimmed.split_whitespace().collect::<Vec<_>>();
    if fields.len() > 1 && !fields[0].chars().next().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
        fields[1..].join(" ").to_ascii_lowercase()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn readme_license(config: &ReadmeConfig, facts: &PackageFacts) -> String {
    if !config.license.spdx.is_empty() {
        return config.license.spdx.join(" OR ");
    }
    facts.package.license_expression.clone().unwrap_or_else(|| "MIT".to_string())
}

fn readme_logo_row(config: &ReadmeConfig) -> (String, Vec<String>) {
    if config.logo_row.enabled == Some(false) {
        return (String::new(), vec![]);
    }
    let max_count = config.logo_row.max_count.clamp(1, 3);
    let mut parts = Vec::new();
    let mut unresolved = Vec::new();
    for logo in config.logo_row.logos.iter().take(max_count) {
        let logo_type = logo.r#type.trim().to_ascii_lowercase().replace('-', "_");
        if !["language", "org", "project", "affiliated_project"].contains(&logo_type.as_str())
            || logo.slug.trim().is_empty()
        {
            unresolved.push(logo.slug.clone());
            continue;
        }
        let slug = logo.slug.trim();
        let reference = slug.replace('/', "-");
        let alt = if logo.alt.trim().is_empty() { slug } else { logo.alt.trim() };
        let href = if logo.href.trim().is_empty() {
            format!("https://logos.galtzo.com/assets/images/{slug}/")
        } else {
            logo.href.clone()
        };
        parts.push(format!(
            "[![{alt}][🖼️{reference}-i]][🖼️{reference}]\n[🖼️{reference}-i]: https://logos.galtzo.com/assets/images/{slug}/avatar-192px.svg\n[🖼️{reference}]: {href}"
        ));
    }
    (parts.join("\n"), unresolved)
}

fn readme_badge_cloud(
    config: &ReadmeConfig,
    facts: &PackageFacts,
    license: &str,
    has_security: bool,
) -> (String, Vec<String>, Vec<String>) {
    let disabled = config.badges.disabled.clone();
    let missing = ["codecov", "coveralls", "qlty", "codeql"]
        .into_iter()
        .filter(|integration| !disabled.contains(&(*integration).to_string()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut badges = Vec::new();
    if let Some(source_url) = &facts.package.source_url {
        badges.push(format!(
            "[![Source](https://img.shields.io/badge/source-github-238636.svg)]({source_url})"
        ));
    }
    if !license.is_empty() {
        badges.push(format!(
            "![License](https://img.shields.io/badge/license-{}-259D6C.svg)",
            license.replace(' ', "%20")
        ));
    }
    if has_security {
        badges.push(
            "[![Security](https://img.shields.io/badge/security-policy-259D6C.svg)](SECURITY.md)"
                .to_string(),
        );
    }
    (badges.join(" "), missing, disabled)
}

fn should_include_funding(config: &ReadmeConfig, license: &str) -> bool {
    match config.conditional_sections.floss_funding.trim().to_ascii_lowercase().as_str() {
        "disabled" | "false" | "never" => false,
        "enabled" | "true" | "always" => true,
        _ => license == "MIT",
    }
}

fn license_paragraph(license: &str) -> String {
    if license == "MIT" {
        "This project is made available under the terms of the MIT License.".to_string()
    } else {
        format!("This project is made available under the following license expression: {license}.")
    }
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
