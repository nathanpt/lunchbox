use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub fn hash_tree(root: &Path) -> Result<String> {
    let mut relatives: Vec<String> = Vec::new();
    collect_relative_paths(root, root, &mut relatives)?;
    relatives.sort();
    let mut tree_hasher = Sha256::new();
    for relative in relatives {
        let bytes = fs::read(root.join(&relative))?;
        let mut file_hasher = Sha256::new();
        file_hasher.update(&bytes);
        let hexdigest = hex::encode(file_hasher.finalize());
        tree_hasher.update(relative.as_bytes());
        tree_hasher.update([0]);
        tree_hasher.update(hexdigest.as_bytes());
        tree_hasher.update(b"\n");
    }
    Ok(format!("sha256:{}", hex::encode(tree_hasher.finalize())))
}

fn collect_relative_paths(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_skipped(&name) {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_relative_paths(root, &path, out)?;
        } else {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            out.push(relative);
        }
    }
    Ok(())
}

fn is_skipped(name: &str) -> bool {
    name == ".git"
        || name == "__pycache__"
        || name == ".DS_Store"
        || name == ".skill_metadata.json"
        || name.ends_with(".pyc")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn golden_tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("SKILL.md"), "hello golden\n").unwrap();
        fs::create_dir(dir.path().join("refs")).unwrap();
        fs::write(dir.path().join("refs").join("a.txt"), "abc").unwrap();
        dir
    }

    #[test]
    fn golden_fixture_hash_is_stable() {
        let dir = golden_tree();
        assert_eq!(
            hash_tree(dir.path()).unwrap(),
            "sha256:6cffec6f66e39626ec7618746bc538791aa2f3e38e0df079b0f825d1f3c15b0c"
        );
    }

    #[test]
    fn same_content_different_layout_differs() {
        let a = golden_tree();
        let b = TempDir::new().unwrap();
        fs::write(b.path().join("SKILL.md"), "hello golden\n").unwrap();
        fs::write(b.path().join("a.txt"), "abc").unwrap();
        assert_ne!(hash_tree(a.path()).unwrap(), hash_tree(b.path()).unwrap());
    }

    #[test]
    fn content_change_changes_hash() {
        let dir = golden_tree();
        let before = hash_tree(dir.path()).unwrap();
        fs::write(dir.path().join("refs").join("a.txt"), "abd").unwrap();
        assert_ne!(before, hash_tree(dir.path()).unwrap());
    }

    #[test]
    fn skip_rules_exclude_bookkeeping() {
        let plain = golden_tree();
        let noisy = golden_tree();
        fs::create_dir(noisy.path().join(".git")).unwrap();
        fs::write(noisy.path().join(".git").join("index"), "git-bytes").unwrap();
        fs::create_dir(noisy.path().join("__pycache__")).unwrap();
        fs::write(
            noisy.path().join("__pycache__").join("m.pyc"),
            "compiled",
        )
        .unwrap();
        fs::write(noisy.path().join(".DS_Store"), "junk").unwrap();
        fs::write(
            noisy.path().join(".skill_metadata.json"),
            "{\"manager\":\"bookkeeping\"}",
        )
        .unwrap();
        assert_eq!(
            hash_tree(plain.path()).unwrap(),
            hash_tree(noisy.path()).unwrap()
        );
    }

    #[test]
    fn empty_tree_hashes_to_stable_value() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            hash_tree(dir.path()).unwrap(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()
        );
    }
}
