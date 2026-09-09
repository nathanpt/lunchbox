// Tool token estimates, probe-pinned per ADR-0008: provider payload per tool
// ({name, description, parameters} as JSON, ceil(chars/4)).
// Probed 2026-09-08: pi 0.84.4 (registered builtins, default-selected
// read/bash/edit/write) and omp 18.1.14 (wire capture via the
// before_provider_request extension hook, default + --tools-forced runs).
// omp's goal, init_experiment, run_experiment, log_experiment, and
// update_notes definitions were not cleanly extractable and are omitted;
// selected names missing from a table count as unestimated at run time.
pub struct ToolEstimate {
    pub name: &'static str,
    pub tokens: u64,
}

const PI_TOOLS: &[ToolEstimate] = &[
    ToolEstimate { name: "bash", tokens: 128 },
    ToolEstimate { name: "edit", tokens: 287 },
    ToolEstimate { name: "find", tokens: 147 },
    ToolEstimate { name: "grep", tokens: 252 },
    ToolEstimate { name: "ls", tokens: 111 },
    ToolEstimate { name: "powershell", tokens: 131 },
    ToolEstimate { name: "read", tokens: 164 },
    ToolEstimate { name: "write", tokens: 100 },
];

const OMP_TOOLS: &[ToolEstimate] = &[
    ToolEstimate { name: "ast_edit", tokens: 755 },
    ToolEstimate { name: "bash", tokens: 553 },
    ToolEstimate { name: "debug", tokens: 915 },
    ToolEstimate { name: "edit", tokens: 1418 },
    ToolEstimate { name: "eval", tokens: 3651 },
    ToolEstimate { name: "glob", tokens: 439 },
    ToolEstimate { name: "grep", tokens: 318 },
    ToolEstimate { name: "hub", tokens: 2698 },
    ToolEstimate { name: "lsp", tokens: 539 },
    ToolEstimate { name: "read", tokens: 849 },
    ToolEstimate { name: "task", tokens: 1268 },
    ToolEstimate { name: "todo", tokens: 1203 },
    ToolEstimate { name: "web_search", tokens: 290 },
    ToolEstimate { name: "write", tokens: 272 },
];

pub fn table(adapter: &str) -> Option<&'static [ToolEstimate]> {
    match adapter {
        "pi" => Some(PI_TOOLS),
        "omp" => Some(OMP_TOOLS),
        _ => None,
    }
}

pub fn estimate(adapter: &str, selected: &[String]) -> (u64, usize) {
    let Some(table) = table(adapter) else {
        return (0, selected.len());
    };
    let mut tokens = 0;
    let mut unmatched = 0;
    for tool in selected {
        match table.iter().find(|entry| entry.name == tool.as_str()) {
            Some(entry) => tokens += entry.tokens,
            None => unmatched += 1,
        }
    }
    (tokens, unmatched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_sums_known_and_counts_unknown() {
        let (tokens, unknown) = estimate("pi", &["read".to_string(), "bash".to_string(), "nope".to_string()]);
        assert_eq!(tokens, 128 + 164);
        assert_eq!(unknown, 1);
    }

    #[test]
    fn adapter_without_table_counts_everything_unestimated() {
        assert_eq!(estimate("none", &["read".to_string()]), (0, 1));
    }

    #[test]
    fn empty_selection_estimates_zero() {
        assert_eq!(estimate("omp", &[]), (0, 0));
    }

    #[test]
    fn table_names_are_unique_and_tokens_positive() {
        for adapter in ["pi", "omp"] {
            let table = table(adapter).unwrap();
            assert!(!table.is_empty(), "{adapter}");
            let mut names: Vec<&str> = table.iter().map(|entry| entry.name).collect();
            names.sort_unstable();
            let count = names.len();
            names.dedup();
            assert_eq!(names.len(), count, "{adapter} duplicate tool name");
            for entry in table {
                assert!(entry.tokens > 0, "{adapter}/{}", entry.name);
            }
        }
    }

    #[test]
    fn unknown_adapter_has_no_table() {
        assert!(table("none").is_none());
        assert!(table("zzz").is_none());
    }
}
