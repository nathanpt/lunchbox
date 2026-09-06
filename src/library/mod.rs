use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FoundSkill {
    pub name: String,
    pub description: String,
    pub source: PathBuf,
}

pub fn read_skill_meta(package_dir: &Path) -> Result<Option<SkillMeta>> {
    let skill_md = package_dir.join("SKILL.md");
    let text = match fs::read_to_string(&skill_md) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(e).context(format!(
                "failed to read {}",
                skill_md.display()
            ))
        }
    };
    let dir_name = package_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(parse_frontmatter(&text, &dir_name)))
}

pub fn parse_frontmatter(text: &str, dir_name: &str) -> SkillMeta {
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut lines = text.lines();
    if lines.next() == Some("---") {
        for line in lines {
            if line == "---" {
                break;
            }
            if let Some(value) = scalar_at_column_zero(line, "name:") {
                name = value;
            } else if let Some(value) = scalar_at_column_zero(line, "description:") {
                description = value;
            }
        }
    }
    SkillMeta {
        name: name.filter(|n| !n.is_empty()).unwrap_or_else(|| dir_name.to_string()),
        description: description.unwrap_or_default(),
    }
}

fn scalar_at_column_zero(line: &str, key: &str) -> Option<Option<String>> {
    let rest = line.strip_prefix(key)?;
    if rest.starts_with(' ') || rest.starts_with('\t') {
        Some(Some(unquote(rest.trim())))
    } else {
        Some(None)
    }
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

pub fn scan_root(root: &Path) -> Result<Vec<FoundSkill>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let found = list_valid_packages(root)?;
    let mut seen = HashSet::new();
    for skill in &found {
        if !seen.insert(skill.name.as_str()) {
            bail!(
                "two packages with the name '{}' in library root {}; pin by hash to disambiguate",
                skill.name,
                root.display()
            );
        }
    }
    Ok(found)
}

fn list_valid_packages(root: &Path) -> Result<Vec<FoundSkill>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to list library root {}", root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if let Some(meta) = read_skill_meta(&path)? {
            found.push(FoundSkill {
                name: meta.name,
                description: meta.description,
                source: path,
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

pub fn find_in_root(root: &Path, name: &str) -> Result<Vec<FoundSkill>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let matches: Vec<FoundSkill> = list_valid_packages(root)?
        .into_iter()
        .filter(|skill| skill.name == name)
        .collect();
    if matches.is_empty() {
        let by_dir_name = root.join(name);
        if by_dir_name.is_dir() && read_skill_meta(&by_dir_name)?.is_none() {
            bail!(
                "skill '{}' at {} is missing SKILL.md",
                name,
                by_dir_name.display()
            );
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_package(root: &Path, dir: &str, skill_md: &str) -> PathBuf {
        let package = root.join(dir);
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("SKILL.md"), skill_md).unwrap();
        package
    }

    #[test]
    fn frontmatter_name_and_description() {
        let meta = parse_frontmatter(
            "---\nname: my-skill\ndescription: Does things.\n---\n\nbody\n",
            "dirname",
        );
        assert_eq!(meta.name, "my-skill");
        assert_eq!(meta.description, "Does things.");
    }

    #[test]
    fn quoted_scalars_are_stripped() {
        let meta = parse_frontmatter(
            "---\nname: \"quoted\"\ndescription: 'single'\n---\n",
            "dirname",
        );
        assert_eq!(meta.name, "quoted");
        assert_eq!(meta.description, "single");
    }

    #[test]
    fn indented_keys_are_ignored() {
        let meta = parse_frontmatter(
            "---\nname: real\nnested:\n  name: ignored\n---\n",
            "dirname",
        );
        assert_eq!(meta.name, "real");
    }

    #[test]
    fn missing_name_falls_back_to_dir_name() {
        let meta = parse_frontmatter("---\ndescription: only description\n---\n", "dir-name");
        assert_eq!(meta.name, "dir-name");
        assert_eq!(meta.description, "only description");
    }

    #[test]
    fn no_frontmatter_uses_dir_name_and_empty_description() {
        let meta = parse_frontmatter("just a body\n", "dir-name");
        assert_eq!(meta.name, "dir-name");
        assert_eq!(meta.description, "");
    }

    #[test]
    fn missing_skill_md_is_invalid() {
        let dir = TempDir::new().unwrap();
        assert!(read_skill_meta(dir.path()).unwrap().is_none());
    }

    #[test]
    fn scan_root_lists_valid_packages_sorted() {
        let root = TempDir::new().unwrap();
        make_package(root.path(), "zeta", "---\nname: zeta\n---\n");
        make_package(root.path(), "alpha", "---\nname: alpha\n---\n");
        fs::create_dir(root.path().join("not-a-skill")).unwrap();
        let found = scan_root(root.path()).unwrap();
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn scan_root_rejects_duplicate_names_in_one_root() {
        let root = TempDir::new().unwrap();
        make_package(root.path(), "one", "---\nname: dup\n---\n");
        make_package(root.path(), "two", "---\nname: dup\n---\n");
        assert!(scan_root(root.path()).is_err());
    }

    #[test]
    fn find_in_root_reports_missing_skill_md() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("ghost")).unwrap();
        let err = find_in_root(root.path(), "ghost").unwrap_err().to_string();
        assert!(err.contains("missing SKILL.md"), "{err}");
    }
}
