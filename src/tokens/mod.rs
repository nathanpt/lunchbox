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
