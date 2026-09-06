pub mod terminal;

#[cfg(feature = "tui-doctor")]
pub mod doctor;

#[cfg(feature = "tui-doctor")]
pub mod preview;

#[cfg(feature = "tui-menu")]
pub mod picker;
#[cfg(feature = "tui-menu")]
pub mod policy;
#[cfg(test)]
pub mod snap;

#[cfg(test)]
pub(crate) mod testkit {
    use std::path::PathBuf;

    pub(crate) fn demo_tree_home() -> tempfile::TempDir {
        let home = tempfile::TempDir::new().unwrap();
        let global = home.path().join(".agents").join("skills");
        for name in ["demo-review", "demo-scan"] {
            let src = std::fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("testdata/skills")
                    .join(name)
                    .join("SKILL.md"),
            )
            .unwrap();
            std::fs::create_dir_all(global.join(name)).unwrap();
            std::fs::write(global.join(name).join("SKILL.md"), src).unwrap();
        }
        home
    }
}
