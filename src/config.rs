use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(version, about, long_about = None)]
pub(crate) struct Settings {
  /// List of files or directories to test
  #[clap(default_value = ".")]
  pub paths: Vec<std::path::PathBuf>,

  /// List of files or directories to exclude from testing
  #[clap(long, value_name = "FILE_PATTERN")]
  pub exclude: Vec<std::path::PathBuf>,

  #[clap(flatten)]
  pub coverage: CoverageSettings,

  /// Don't stop executing tests after one has failed
  #[clap(long, default_value_t = false)]
  pub no_fail_fast: bool,

  /// How test results should be reported
  #[clap(long, value_enum, default_value_t = OutputFormat::Standard)]
  pub output: OutputFormat,
}

#[derive(clap::Args, Debug, Default)]
pub(crate) struct CoverageSettings {
  /// Enable line coverage gathering and reporting
  #[clap(long = "coverage", default_value_t = false)]
  pub enabled: bool,

  /// List of paths, used to determine files to report coverage for
  #[clap(
    name = "coverage-include",
    long = "coverage-include",
    value_name = "FILE_PATTERN",
    help_heading = "Coverage"
  )]
  pub include: Vec<std::path::PathBuf>,

  /// List of paths, used to omit files and/or directories from coverage reporting
  #[clap(
    name = "coverage-exclude",
    long = "coverage-exclude",
    value_name = "FILE_PATTERN",
    help_heading = "Coverage"
  )]
  pub exclude: Vec<std::path::PathBuf>,
}

#[derive(Copy, Clone, Default, Debug, clap::ValueEnum)]
pub(crate) enum OutputFormat {
  /// The standard output format to the terminal
  #[default]
  Standard,
  /// Output each test as a JSON object on a new line
  Json,
}

/// Reads settings from command line arguments and `pyproject.toml`
pub fn read_settings() -> Settings {
  let mut settings = Settings::parse();

  if let Some(pyproject_toml) = pyproject_toml::find() {
    if let Some(xc_config) = pyproject_toml::load(&pyproject_toml) {
      pyproject_toml::update_settings(&mut settings, xc_config);
    }
  }

  settings
}

mod pyproject_toml {
  use serde::Deserialize;
  use std::{
    env, fs, mem,
    path::{Path, PathBuf},
  };

  #[derive(Deserialize, Default)]
  struct PyprojectToml {
    tool: Option<PyprojectTomlTool>,
  }

  #[derive(Deserialize, Default)]
  struct PyprojectTomlTool {
    xc: Option<XCSettings>,
  }

  #[derive(Deserialize, Default)]
  pub struct XCSettings {
    include: Option<Vec<PathBuf>>,
    exclude: Option<Vec<PathBuf>>,
    no_fail_fast: Option<bool>,
    coverage: Option<bool>,
    coverage_include: Option<Vec<PathBuf>>,
    coverage_exclude: Option<Vec<PathBuf>>,
  }

  /// Get the path to a `pyproject.toml` file, if one exists in the current tree
  pub fn find() -> Option<PathBuf> {
    let mut path = env::current_dir().unwrap();

    while path.parent().is_some() {
      path.push("./pyproject.toml");
      if path.exists() {
        return Some(path);
      }

      // Remove the `pyproject.toml` file
      path.pop();

      // Go up to the next folder
      path.pop();
    }

    None
  }

  pub fn load(path: &Path) -> Option<XCSettings> {
    let pyproject_file = fs::read_to_string(path).ok()?;
    let pyproject = toml::from_str::<PyprojectToml>(&pyproject_file).ok()?;

    pyproject.tool?.xc
  }

  pub fn update_settings(settings: &mut super::Settings, mut toml_config: XCSettings) {
    if let Some(include) = &mut toml_config.include {
      if settings.paths.is_empty() {
        settings.paths = mem::take(include);
      }
    }

    if let Some(exclude) = &mut toml_config.exclude {
      if settings.exclude.is_empty() {
        settings.exclude = mem::take(exclude);
      }
    }

    if let Some(no_fail_fast) = toml_config.no_fail_fast {
      if !settings.no_fail_fast {
        settings.no_fail_fast = no_fail_fast;
      }
    }

    if let Some(coverage) = toml_config.coverage {
      if !settings.coverage.enabled {
        settings.coverage.enabled = coverage;
      }
    }

    if let Some(coverage_include) = &mut toml_config.coverage_include {
      if settings.coverage.include.is_empty() {
        settings.coverage.include = mem::take(coverage_include);
      }
    }

    if let Some(coverage_exclude) = &mut toml_config.coverage_exclude {
      if settings.coverage.exclude.is_empty() {
        settings.coverage.exclude = mem::take(coverage_exclude);
      }
    }
  }
}
