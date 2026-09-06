#[cfg(feature = "tui-doctor")]
use crate::config::Config;
#[cfg(feature = "tui-doctor")]
use anyhow::{Result, bail};
#[cfg(feature = "tui-doctor")]
use std::path::PathBuf;

#[cfg(feature = "tui-doctor")]
pub struct PreviewSkill {
    pub name: String,
    pub tokens: u64,
}

#[cfg(feature = "tui-doctor")]
pub struct Preview {
    pub skills: Vec<PreviewSkill>,
    pub menu_tokens: u64,
    pub without_menu_tokens: u64,
    pub max_menu_tokens: u64,
    pub over_budget: bool,
}

#[cfg(feature = "tui-doctor")]
pub fn preview(cfg: &Config, roots_extra: &[PathBuf], pins: &[String]) -> Result<Preview> {
    let roots = cfg.search_roots(roots_extra);
    let mut skills = Vec::new();
    for pin in pins {
        let parsed = crate::resolve::parse_pin(pin)?;
        if parsed.pinned_hash.is_some() {
            bail!("tag pins are not supported yet; pin by hash (got '{pin}')");
        }
        let mut found = None;
        for root in &roots {
            let matches = crate::library::find_in_root(root, &parsed.name)?;
            if let Some(skill) = matches.into_iter().next() {
                found = Some(skill);
                break;
            }
        }
        let Some(found) = found else {
            let searched = roots
                .iter()
                .map(|r| r.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "skill '{}' not found in any library root (searched: {})",
                parsed.name,
                searched
            );
        };
        skills.push(PreviewSkill {
            tokens: estimate(&found.name, &found.description),
            name: found.name,
        });
    }
    let menu_tokens = skills.iter().map(|s| s.tokens).sum();
    let adapter = crate::adapter::resolve_adapter(&cfg.default_adapter)?;
    let (without_menu_tokens, _) = crate::adapter::union_menu(adapter.as_ref(), cfg);
    Ok(Preview {
        over_budget: menu_tokens > cfg.max_menu_tokens,
        max_menu_tokens: cfg.max_menu_tokens,
        without_menu_tokens,
        menu_tokens,
        skills,
    })
}

pub fn estimate(name: &str, description: &str) -> u64 {
    let chars = name.chars().count() + 1 + description.chars().count();
    chars.div_ceil(4) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fixture_constants() {
        assert_eq!(
            estimate("demo-review", "Review staged changes for defects and risks."),
            14
        );
        assert_eq!(
            estimate("demo-scan", "Scan for leaked secrets in the worktree."),
            13
        );
    }

    #[test]
    fn rounds_up_partial_tokens() {
        assert_eq!(estimate("a", ""), 1);
        assert_eq!(estimate("abc", ""), 1);
        assert_eq!(estimate("abcd", ""), 2);
        assert_eq!(estimate("abcde", ""), 2);
    }

    #[test]
    fn counts_unicode_scalars_not_bytes() {
        assert_eq!(estimate("éééé", ""), 2);
    }
}
