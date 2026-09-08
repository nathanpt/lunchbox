pub const SKILL_LISTING_OVERHEAD_CHARS: usize = 204;
pub const MENU_PREAMBLE_TOKENS: u64 = 74;

pub fn estimate(name: &str, description: &str) -> u64 {
    let chars = name.chars().count()
        + 1
        + description.chars().count()
        + SKILL_LISTING_OVERHEAD_CHARS;
    chars.div_ceil(4) as u64
}

pub fn with_preamble(menu_sum: u64) -> u64 {
    if menu_sum == 0 {
        0
    } else {
        menu_sum + MENU_PREAMBLE_TOKENS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fixture_constants() {
        assert_eq!(estimate("demo-review", "Review staged changes for defects and risks."), 65);
        assert_eq!(estimate("demo-scan", "Scan for leaked secrets in the worktree."), 64);
    }

    #[test]
    fn overhead_is_added_after_the_description_chars() {
        let base = SKILL_LISTING_OVERHEAD_CHARS as u64;
        assert_eq!(estimate("", ""), (base + 1).div_ceil(4));
        assert_eq!(estimate("abcd", ""), (base + 1 + 4).div_ceil(4));
        assert_eq!(estimate("abcde", ""), (base + 1 + 5).div_ceil(4));
    }

    #[test]
    fn counts_unicode_scalars_not_bytes() {
        let base = SKILL_LISTING_OVERHEAD_CHARS as u64;
        assert_eq!(estimate("éééé", ""), (base + 1 + 4).div_ceil(4));
    }

    #[test]
    fn preamble_applies_once_and_only_to_nonempty_menus() {
        assert_eq!(with_preamble(0), 0);
        assert_eq!(with_preamble(129), 203);
    }
}
