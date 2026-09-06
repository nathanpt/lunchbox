use crate::config::Config;
use crate::hash::hash_tree;
use crate::library::find_in_root;
use crate::tokens::estimate;
use anyhow::{Result, bail};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Locked {
    pub name: String,
    pub source: PathBuf,
    pub hash: String,
    pub description_tokens: u64,
}

#[derive(Debug)]
pub struct Pin {
    pub name: String,
    pub pinned_hash: Option<String>,
}

pub fn parse_pin(pin: &str) -> Result<Pin> {
    match pin.split_once('@') {
        None => Ok(Pin {
            name: pin.to_string(),
            pinned_hash: None,
        }),
        Some((name, tail)) => {
            let digest = tail.strip_prefix("sha256:").unwrap_or_default();
            let valid = digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_lowercase_digit());
            if valid {
                Ok(Pin {
                    name: name.to_string(),
                    pinned_hash: Some(format!("sha256:{digest}")),
                })
            } else {
                bail!("tag pins are not supported yet; pin by hash (got '{pin}')")
            }
        }
    }
}

trait AsciiLowerHex {
    fn is_ascii_lowercase_digit(self) -> bool;
}

impl AsciiLowerHex for u8 {
    fn is_ascii_lowercase_digit(self) -> bool {
        self.is_ascii_digit() || (b'a'..=b'f').contains(&self)
    }
}

pub fn resolve(pins: &[String], cfg: &Config, roots_extra: &[PathBuf]) -> Result<Vec<Locked>> {
    if !cfg.scan_command.is_empty() {
        bail!("scan_command is configured but not supported in this build");
    }
    let roots = cfg.search_roots(roots_extra);
    let mut locked: Vec<Locked> = Vec::new();
    for pin in pins {
        let pin = parse_pin(pin)?;
        let found = expand_pin(&pin, &roots)?;
        if cfg.deny.iter().any(|d| d == &found.name) {
            bail!("skill '{}' is denied by policy", found.name);
        }
        if !cfg.allow.is_empty() && !cfg.allow.iter().any(|a| a == &found.name) {
            bail!(
                "skill '{}' is not on the allow list (allow = [{}])",
                found.name,
                cfg.allow.join(", ")
            );
        }
        let tokens = estimate(&found.name, &found.description);
        if let Some(existing) = locked.iter().find(|l| l.name == found.name) {
            if existing.hash != found.hash {
                bail!(
                    "skill '{}' pinned twice with conflicting hashes ({} vs {})",
                    found.name,
                    existing.hash,
                    found.hash
                );
            }
            continue;
        }
        locked.push(Locked {
            name: found.name,
            source: found.source,
            hash: found.hash,
            description_tokens: tokens,
        });
    }
    Ok(locked)
}

fn expand_pin(pin: &Pin, roots: &[PathBuf]) -> Result<Expanded> {
    for root in roots {
        let matches = find_in_root(root, &pin.name)?;
        match matches.len() {
            0 => continue,
            1 => {
                let found = matches.into_iter().next().unwrap();
                let hash = hash_tree(&found.source)?;
                if let Some(pinned) = &pin.pinned_hash {
                    if pinned != &hash {
                        bail!(
                            "hash mismatch for skill '{}': pinned {}, found {}",
                            pin.name,
                            pinned,
                            hash
                        );
                    }
                }
                return Ok(Expanded {
                    name: found.name,
                    source: found.source,
                    hash,
                    description: found.description,
                });
            }
            _ => {
                let Some(pinned) = &pin.pinned_hash else {
                    bail!(
                        "two packages with the name '{}' in library root {}; pin by hash to disambiguate",
                        pin.name,
                        root.display()
                    );
                };
                for found in matches {
                    let hash = hash_tree(&found.source)?;
                    if &hash == pinned {
                        return Ok(Expanded {
                            name: found.name,
                            source: found.source,
                            hash,
                            description: found.description,
                        });
                    }
                }
                bail!(
                    "no package matching '{}' found in library root {}",
                    format!("{}@{}", pin.name, pinned),
                    root.display()
                );
            }
        }
    }
    Err(not_found_error(&pin.name, roots))
}

pub(crate) fn not_found_error(name: &str, roots: &[PathBuf]) -> anyhow::Error {
    let searched = roots
        .iter()
        .map(|r| r.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::anyhow!("skill '{name}' not found in any library root (searched: {searched})")
}

struct Expanded {
    name: String,
    source: PathBuf,
    hash: String,
    description: String,
}

pub fn enforce_worker_budget(
    workers: &[crate::run::Worker],
    locked: &[Locked],
    max_menu_tokens: u64,
    fail_on_budget: bool,
) -> Result<()> {
    for worker in workers {
        let menu_tokens: u64 = worker
            .pack
            .iter()
            .filter_map(|name| locked.iter().find(|l| &l.name == name))
            .map(|l| l.description_tokens)
            .sum();
        if menu_tokens > max_menu_tokens {
            if fail_on_budget {
                bail!(
                    "worker '{}': menu_tokens {} exceeds max_menu_tokens {} (fail_on_budget = true)",
                    worker.name,
                    menu_tokens,
                    max_menu_tokens
                );
            }
            eprintln!(
                "warning: worker '{}': menu_tokens {} exceeds max_menu_tokens {} (fail_on_budget = false)",
                worker.name, menu_tokens, max_menu_tokens
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MountMode;
    use std::fs;
    use tempfile::TempDir;

    fn pantry() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("demo-review")).unwrap();
        fs::write(
            dir.path().join("demo-review").join("SKILL.md"),
            "---\nname: demo-review\ndescription: Review staged changes for defects and risks.\n---\nbody\n",
        )
        .unwrap();
        dir
    }

    fn config_with_root(root: &PathBuf) -> Config {
        Config {
            library_paths: vec![root.clone()],
            ..Config::default()
        }
    }

    fn pins(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn duplicate_pantry() -> (TempDir, String, String) {
        let dir = TempDir::new().unwrap();
        for sub in ["copy-a", "copy-b"] {
            fs::create_dir_all(dir.path().join(sub)).unwrap();
            fs::write(
                dir.path().join(sub).join("SKILL.md"),
                format!("---\nname: demo-review\ndescription: {sub} copy\n---\n"),
            )
            .unwrap();
        }
        let hash_a = hash_tree(&dir.path().join("copy-a")).unwrap();
        let hash_b = hash_tree(&dir.path().join("copy-b")).unwrap();
        assert_ne!(hash_a, hash_b);
        (dir, hash_a, hash_b)
    }

    #[test]
    fn duplicate_names_in_one_root_resolve_via_hash_pin() {
        let (dir, hash_a, _hash_b) = duplicate_pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let locked = resolve(&[format!("demo-review@{hash_a}")], &cfg, &[]).unwrap();
        assert_eq!(locked.len(), 1);
        assert_eq!(locked[0].hash, hash_a);
        assert_eq!(locked[0].source, dir.path().join("copy-a"));
        assert_eq!(locked[0].description_tokens, estimate("demo-review", "copy-a copy"));
    }

    #[test]
    fn duplicate_names_with_plain_pin_error() {
        let (dir, _hash_a, _hash_b) = duplicate_pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let err = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap_err().to_string();
        assert!(err.contains("two packages with the name 'demo-review'"), "{err}");
        assert!(err.contains("pin by hash"), "{err}");
    }

    #[test]
    fn duplicate_names_hash_pin_with_no_match_errors() {
        let (dir, _hash_a, _hash_b) = duplicate_pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let bogus = format!("demo-review@sha256:{}", "0".repeat(64));
        let err = resolve(&[bogus], &cfg, &[]).unwrap_err().to_string();
        assert!(err.contains("no package matching"), "{err}");
    }

    #[test]
    fn resolves_plain_pin_with_hash() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let locked = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap();
        assert_eq!(locked.len(), 1);
        assert_eq!(locked[0].name, "demo-review");
        assert!(locked[0].hash.starts_with("sha256:"));
        assert_eq!(locked[0].description_tokens, 14);
    }

    #[test]
    fn resolves_hash_pin_with_matching_hash() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let hash = hash_tree(&dir.path().join("demo-review")).unwrap();
        let pin = format!("demo-review@{hash}");
        let locked = resolve(&[pin], &cfg, &[]).unwrap();
        assert_eq!(locked[0].hash, hash);
    }

    #[test]
    fn hash_mismatch_fails() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let bogus = "sha256:".to_string() + &"0".repeat(64);
        let err = resolve(&[format!("demo-review@{bogus}")], &cfg, &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("hash mismatch"), "{err}");
    }

    #[test]
    fn missing_name_lists_every_root() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let err = resolve(&pins(&["nope"]), &cfg, &[]).unwrap_err().to_string();
        assert!(err.contains("not found"), "{err}");
        assert!(err.contains(&dir.path().display().to_string()), "{err}");
    }

    #[test]
    fn deny_gate_names_the_skill() {
        let dir = pantry();
        let mut cfg = config_with_root(&dir.path().to_path_buf());
        cfg.deny = vec!["demo-review".to_string()];
        let err = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap_err().to_string();
        assert!(err.contains("denied"), "{err}");
        assert!(err.contains("demo-review"), "{err}");
    }

    #[test]
    fn allow_gate_rejects_unlisted() {
        let dir = pantry();
        let mut cfg = config_with_root(&dir.path().to_path_buf());
        cfg.allow = vec!["something-else".to_string()];
        let err = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap_err().to_string();
        assert!(err.contains("allow"), "{err}");
    }

    #[test]
    fn allow_gate_passes_listed() {
        let dir = pantry();
        let mut cfg = config_with_root(&dir.path().to_path_buf());
        cfg.allow = vec!["demo-review".to_string()];
        assert!(resolve(&pins(&["demo-review"]), &cfg, &[]).is_ok());
    }

    #[test]
    fn tag_pin_errors() {
        let err = parse_pin("demo-review@v1.2.0").unwrap_err().to_string();
        assert_eq!(err, "tag pins are not supported yet; pin by hash (got 'demo-review@v1.2.0')");
    }

    #[test]
    fn malformed_hash_pin_is_rejected() {
        assert!(parse_pin("x@sha256:abc").is_err());
        assert!(parse_pin("x@sha256:ABCDEF").is_err());
        assert!(parse_pin("x").is_ok());
    }

    #[test]
    fn scan_command_fails_closed() {
        let dir = pantry();
        let mut cfg = config_with_root(&dir.path().to_path_buf());
        cfg.scan_command = "some-scanner".to_string();
        let err = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap_err().to_string();
        assert!(err.contains("scan_command is configured but not supported"), "{err}");
    }

    #[test]
    fn duplicate_pins_dedupe_same_hash() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let locked = resolve(&pins(&["demo-review", "demo-review"]), &cfg, &[]).unwrap();
        assert_eq!(locked.len(), 1);
    }

    fn default_worker(pack: &[&str]) -> crate::run::Worker {
        crate::run::Worker {
            name: "default".to_string(),
            pack: pins(pack),
            description: None,
        }
    }

    #[test]
    fn budget_hard_fail_names_the_worker() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let locked = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap();
        let workers = vec![default_worker(&["demo-review"])];
        let err = enforce_worker_budget(&workers, &locked, 10, true)
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "worker 'default': menu_tokens 14 exceeds max_menu_tokens 10 (fail_on_budget = true)"
        );
    }

    #[test]
    fn budget_soft_warn_per_worker_still_passes() {
        let dir = pantry();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let locked = resolve(&pins(&["demo-review"]), &cfg, &[]).unwrap();
        let workers = vec![default_worker(&["demo-review"])];
        assert!(enforce_worker_budget(&workers, &locked, 10, false).is_ok());
    }

    #[test]
    fn budget_checks_each_worker_pack_not_the_union() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("alpha")).unwrap();
        fs::write(
            dir.path().join("alpha").join("SKILL.md"),
            "---\nname: alpha\ndescription: d\n---\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("beta")).unwrap();
        fs::write(
            dir.path().join("beta").join("SKILL.md"),
            "---\nname: beta\ndescription: d\n---\n",
        )
        .unwrap();
        let cfg = config_with_root(&dir.path().to_path_buf());
        let locked = resolve(&pins(&["alpha", "beta"]), &cfg, &[]).unwrap();
        let split: Vec<u64> = locked.iter().map(|l| l.description_tokens).collect();
        let cap = split.iter().max().unwrap();
        let workers = vec![
            crate::run::Worker { name: "a".to_string(), pack: pins(&["alpha"]), description: None },
            crate::run::Worker { name: "b".to_string(), pack: pins(&["beta"]), description: None },
        ];
        assert!(enforce_worker_budget(&workers, &locked, *cap, true).is_ok());
        let union_only = vec![crate::run::Worker {
            name: "u".to_string(),
            pack: pins(&["alpha", "beta"]),
            description: None,
        }];
        assert!(enforce_worker_budget(&union_only, &locked, *cap, true).is_err());
    }

    #[test]
    fn extra_roots_are_searched_first() {
        let dir = pantry();
        let other = TempDir::new().unwrap();
        fs::create_dir_all(other.path().join("demo-review")).unwrap();
        fs::write(
            other.path().join("demo-review").join("SKILL.md"),
            "---\nname: demo-review\ndescription: Other copy.\n---\n",
        )
        .unwrap();
        let cfg = Config {
            library_paths: vec![dir.path().to_path_buf()],
            mount_mode: MountMode::Symlink,
            ..Config::default()
        };
        let locked = resolve(&pins(&["demo-review"]), &cfg, &[other.path().to_path_buf()]).unwrap();
        assert_eq!(locked[0].description_tokens, estimate("demo-review", "Other copy."));
    }
}
