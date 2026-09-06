use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(test)]
pub(crate) fn with_home<T>(home: &Path, body: impl FnOnce() -> T) -> T {
    static HOME_MUTEX: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _guard = HOME_MUTEX.lock();
    let previous = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", home) };
    let result = body();
    match previous {
        Some(value) => unsafe { std::env::set_var("HOME", value) },
        None => unsafe { std::env::remove_var("HOME") },
    }
    result
}
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MountMode {
    Symlink,
    Copy,
}

impl Default for MountMode {
    fn default() -> Self {
        MountMode::Symlink
    }
}

impl MountMode {
    pub fn as_str(self) -> &'static str {
        match self {
            MountMode::Symlink => "symlink",
            MountMode::Copy => "copy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub library_paths: Vec<PathBuf>,
    pub default_adapter: String,
    pub mount_mode: MountMode,
    pub scan_command: String,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub max_menu_tokens: u64,
    pub fail_on_budget: bool,
    pub runs_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            library_paths: Vec::new(),
            default_adapter: "pi".to_string(),
            mount_mode: MountMode::Symlink,
            scan_command: String::new(),
            allow: Vec::new(),
            deny: Vec::new(),
            max_menu_tokens: 2000,
            fail_on_budget: false,
            runs_dir: PathBuf::from("~/.lunchbox/runs"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct Layer {
    library_paths: Option<Vec<PathBuf>>,
    default_adapter: Option<String>,
    mount_mode: Option<MountMode>,
    scan_command: Option<String>,
    allow: Option<Vec<String>>,
    deny: Option<Vec<String>>,
    max_menu_tokens: Option<u64>,
    fail_on_budget: Option<bool>,
    runs_dir: Option<PathBuf>,
}

impl Config {
    pub fn load() -> Result<Config> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set; cannot locate the global lunchbox config")?;
        let global = read_layer(&home.join(".lunchbox").join("config.toml"))?;
        let project = read_layer(Path::new("lunchbox.toml"))?;
        let mut config = merge(global, project);
        config.expand_paths(&home);
        Ok(config)
    }

    pub fn search_roots(&self, cli_libraries: &[PathBuf]) -> Vec<PathBuf> {
        let home = env::var_os("HOME").map(PathBuf::from);
        let mut roots: Vec<PathBuf> = cli_libraries
            .iter()
            .map(|p| absolutize(p, home.as_deref()))
            .collect();
        roots.extend(self.library_paths.iter().cloned());
        roots.push(absolutize(Path::new(".agents/skills"), home.as_deref()));
        if let Some(home) = home {
            roots.push(home.join(".agents").join("skills"));
        }
        roots
    }

    fn expand_paths(&mut self, home: &Path) {
        let expanded_home = home.to_path_buf();
        self.library_paths = self
            .library_paths
            .iter()
            .map(|p| absolutize(p, Some(&expanded_home)))
            .collect();
        self.runs_dir = expand_tilde(&self.runs_dir, home);
    }
}

fn read_layer(path: &Path) -> Result<Layer> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .with_context(|| format!("failed to parse config at {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Layer::default()),
        Err(e) => return Err(e).context(format!("failed to read config at {}", path.display())),
    }
}

fn merge(global: Layer, project: Layer) -> Config {
    let defaults = Config::default();
    let pick = |g: Option<String>, p: Option<String>| p.or(g).unwrap_or_default();
    Config {
        library_paths: project
            .library_paths
            .clone()
            .unwrap_or_default()
            .into_iter()
            .chain(global.library_paths.clone().unwrap_or_default())
            .collect(),
        default_adapter: project
            .default_adapter
            .or(global.default_adapter)
            .unwrap_or(defaults.default_adapter),
        mount_mode: project
            .mount_mode
            .or(global.mount_mode)
            .unwrap_or(defaults.mount_mode),
        scan_command: pick(global.scan_command, project.scan_command),
        allow: merge_allow(global.allow.unwrap_or_default(), project.allow.unwrap_or_default()),
        deny: union_dedup(global.deny.unwrap_or_default(), project.deny.unwrap_or_default()),
        max_menu_tokens: project
            .max_menu_tokens
            .or(global.max_menu_tokens)
            .unwrap_or(defaults.max_menu_tokens),
        fail_on_budget: project
            .fail_on_budget
            .or(global.fail_on_budget)
            .unwrap_or(defaults.fail_on_budget),
        runs_dir: project.runs_dir.or(global.runs_dir).unwrap_or(defaults.runs_dir),
    }
}

fn merge_allow(global: Vec<String>, project: Vec<String>) -> Vec<String> {
    if global.is_empty() {
        return project;
    }
    if project.is_empty() {
        return global;
    }
    let project_set: HashSet<&str> = project.iter().map(String::as_str).collect();
    global
        .into_iter()
        .filter(|name| project_set.contains(name.as_str()))
        .collect()
}

fn union_dedup(global: Vec<String>, project: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    global
        .into_iter()
        .chain(project)
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" {
        home.to_path_buf()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home.join(rest)
    } else {
        path.to_path_buf()
    }
}

fn absolutize(path: &Path, home: Option<&Path>) -> PathBuf {
    let expanded = match home {
        Some(home) => expand_tilde(path, home),
        None => path.to_path_buf(),
    };
    if expanded.is_absolute() {
        expanded
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(&expanded))
            .unwrap_or(expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer_from(text: &str) -> Layer {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn defaults_when_both_layers_absent() {
        let config = merge(Layer::default(), Layer::default());
        assert_eq!(config, Config::default());
        assert_eq!(config.default_adapter, "pi");
        assert_eq!(config.max_menu_tokens, 2000);
        assert_eq!(config.runs_dir, PathBuf::from("~/.lunchbox/runs"));
    }

    #[test]
    fn project_scalar_wins_over_global() {
        let global = layer_from(r#"default_adapter = "none""#);
        let project = layer_from(r#"default_adapter = "pi""#);
        assert_eq!(merge(global, project).default_adapter, "pi");
    }

    #[test]
    fn global_scalar_used_when_project_silent() {
        let global = layer_from(r#"max_menu_tokens = 500"#);
        assert_eq!(merge(global, Layer::default()).max_menu_tokens, 500);
    }

    #[test]
    fn deny_unions_across_layers_deduped() {
        let global = layer_from(r#"deny = ["a", "b"]"#);
        let project = layer_from(r#"deny = ["b", "c"]"#);
        assert_eq!(merge(global, project).deny, vec!["a", "b", "c"]);
    }

    #[test]
    fn allow_intersects_in_global_order() {
        let global = layer_from(r#"allow = ["a", "b", "c"]"#);
        let project = layer_from(r#"allow = ["c", "a", "zzz"]"#);
        assert_eq!(merge(global, project).allow, vec!["a", "c"]);
    }

    #[test]
    fn allow_single_side_passes_through() {
        let global = layer_from(r#"allow = ["a"]"#);
        assert_eq!(merge(global.clone(), Layer::default()).allow, vec!["a"]);
        assert_eq!(
            merge(Layer::default(), layer_from(r#"allow = ["b"]"#)).allow,
            vec!["b"]
        );
    }

    #[test]
    fn allow_both_empty_means_allow_any() {
        assert!(merge(Layer::default(), Layer::default()).allow.is_empty());
    }

    #[test]
    fn library_paths_project_prepended_to_global() {
        let global = layer_from(r#"library_paths = ["/g1", "/g2"]"#);
        let project = layer_from(r#"library_paths = ["/p1"]"#);
        assert_eq!(
            merge(global, project).library_paths,
            vec![PathBuf::from("/p1"), PathBuf::from("/g1"), PathBuf::from("/g2")]
        );
    }

    #[test]
    fn unknown_key_is_a_hard_error() {
        let result = toml::from_str::<Layer>(r#"nonsense_key = 1"#);
        assert!(result.is_err());
    }

    #[test]
    fn unparseable_file_is_a_hard_error() {
        let result = toml::from_str::<Layer>("not toml at all {{{");
        assert!(result.is_err());
    }

    #[test]
    fn mount_mode_parses_lowercase() {
        let layer = layer_from(r#"mount_mode = "copy""#);
        assert_eq!(merge(Layer::default(), layer).mount_mode, MountMode::Copy);
    }

    #[test]
    fn tilde_expansion() {
        let home = Path::new("/home/maya");
        assert_eq!(expand_tilde(Path::new("~"), home), PathBuf::from("/home/maya"));
        assert_eq!(
            expand_tilde(Path::new("~/.agents/skills"), home),
            PathBuf::from("/home/maya/.agents/skills")
        );
        assert_eq!(
            expand_tilde(Path::new("/abs/path"), home),
            PathBuf::from("/abs/path")
        );
    }
}
