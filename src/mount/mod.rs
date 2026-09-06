use crate::config::MountMode;
use crate::resolve::Locked;
use anyhow::{Context, Result, bail};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

pub fn mount(locked: &[Locked], workdir: &Path, mode: MountMode) -> Result<MountMode> {
    fs::create_dir_all(workdir)
        .with_context(|| format!("failed to create workdir {}", workdir.display()))?;
    match mode {
        MountMode::Copy => {
            for skill in locked {
                copy_tree(&skill.source, &workdir.join(&skill.name))?;
            }
            Ok(MountMode::Copy)
        }
        MountMode::Symlink => {
            let mut linked = true;
            for skill in locked {
                let link = workdir.join(&skill.name);
                if symlink(&skill.source, &link).is_err() {
                    linked = false;
                    break;
                }
            }
            if linked {
                return Ok(MountMode::Symlink);
            }
            clean_dir(workdir)?;
            for skill in locked {
                copy_tree(&skill.source, &workdir.join(&skill.name))?;
            }
            Ok(MountMode::Copy)
        }
    }
}

fn clean_dir(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("failed to create {}", dst.display()))?;
    let canonical_src = src
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", src.display()))?;
    for entry in fs::read_dir(src)
        .with_context(|| format!("failed to list {}", src.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&path, &target)?;
        } else if file_type.is_symlink() {
            copy_symlink(&path, &target, &canonical_src)?;
        } else {
            fs::copy(&path, &target)
                .with_context(|| format!("failed to copy {}", path.display()))?;
        }
    }
    Ok(())
}

fn copy_symlink(link: &Path, target: &Path, canonical_src: &Path) -> Result<()> {
    let resolved = fs::canonicalize(link)
        .with_context(|| format!("failed to resolve symlink {}", link.display()))?;
    if !resolved.starts_with(canonical_src) {
        bail!(
            "symlink escape: {} resolves to {}, outside the package root {}",
            link.display(),
            resolved.display(),
            canonical_src.display()
        );
    }
    let original = fs::read_link(link)?;
    symlink(&original, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn locked_one(name: &str, source: &Path) -> Vec<Locked> {
        vec![Locked {
            name: name.to_string(),
            source: source.to_path_buf(),
            hash: "sha256:0".to_string(),
            description_tokens: 1,
        }]
    }

    fn package(dir: &TempDir) -> PathBuf {
        let package = dir.path().join("pkg");
        fs::create_dir_all(package.join("refs")).unwrap();
        fs::write(package.join("SKILL.md"), "---\nname: pkg\n---\n").unwrap();
        fs::write(package.join("refs").join("a.txt"), "abc").unwrap();
        package
    }

    #[test]
    fn symlink_mode_links_packages() {
        let dir = TempDir::new().unwrap();
        let package = package(&dir);
        let workdir = dir.path().join("workdir");
        let mode = mount(&locked_one("pkg", &package), &workdir, MountMode::Symlink).unwrap();
        assert_eq!(mode, MountMode::Symlink);
        assert!(workdir.join("pkg").is_symlink());
        assert_eq!(
            fs::read_link(workdir.join("pkg")).unwrap(),
            package
        );
    }

    #[test]
    fn copy_mode_copies_files() {
        let dir = TempDir::new().unwrap();
        let package = package(&dir);
        let workdir = dir.path().join("workdir");
        let mode = mount(&locked_one("pkg", &package), &workdir, MountMode::Copy).unwrap();
        assert_eq!(mode, MountMode::Copy);
        assert!(workdir.join("pkg").join("SKILL.md").is_file());
        assert_eq!(
            fs::read_to_string(workdir.join("pkg").join("refs").join("a.txt")).unwrap(),
            "abc"
        );
        assert!(!workdir.join("pkg").is_symlink());
    }

    #[test]
    fn internal_symlink_inside_package_is_copied_as_link() {
        let dir = TempDir::new().unwrap();
        let package = package(&dir);
        symlink("a.txt", package.join("refs").join("alias.txt")).unwrap();
        let workdir = dir.path().join("workdir");
        mount(&locked_one("pkg", &package), &workdir, MountMode::Copy).unwrap();
        assert_eq!(
            fs::read_link(workdir.join("pkg").join("refs").join("alias.txt")).unwrap(),
            PathBuf::from("a.txt")
        );
    }

    #[test]
    fn symlink_escape_errors() {
        let dir = TempDir::new().unwrap();
        let package = package(&dir);
        let outside = dir.path().join("outside.txt");
        fs::write(&outside, "secret").unwrap();
        symlink("../outside.txt", package.join("escape")).unwrap();
        let workdir = dir.path().join("workdir");
        let err = mount(&locked_one("pkg", &package), &workdir, MountMode::Copy)
            .unwrap_err()
            .to_string();
        assert!(err.contains("symlink escape"), "{err}");
    }

    #[test]
    fn symlink_failure_falls_back_to_copy_uniformly() {
        let dir = TempDir::new().unwrap();
        let package = package(&dir);
        let workdir = dir.path().join("workdir");
        fs::create_dir_all(&workdir).unwrap();
        symlink("/nonexistent-blocker", workdir.join("pkg")).unwrap();
        let mode = mount(&locked_one("pkg", &package), &workdir, MountMode::Symlink).unwrap();
        assert_eq!(mode, MountMode::Copy);
        assert!(!workdir.join("pkg").is_symlink());
        assert!(workdir.join("pkg").join("SKILL.md").is_file());
    }
}
