//! Parity between `validate` / `gv` with an explicit file list and the same command with
//! no file list.
//!
//! Passing paths swaps `validate_all()` for `validate_files()` (`runner.rs:124`).
//! `validate_all` runs three checks -- `validate_invalid_team`, `validate_file_ownership`,
//! `validate_codeowners_file` (`validator.rs:40-57`). `validate_files` runs none of them;
//! it only asks whether each path resolves to a team when reading the CODEOWNERS file.
//!
//! `gv <paths>` regenerates before validating, which cures staleness by construction, but
//! not the other two. Worse, regenerating makes a dual-owned file *appear* owned, so the
//! per-path check waves it through.
//!
//! EVERY TEST IN THIS FILE CURRENTLY FAILS, so all are `#[ignore]`d to keep the suite green.
//! They assert the behavior we want and exist to document the gap. Run them with:
//!
//! ```sh
//! cargo test --test validate_files_parity_test -- --ignored
//! ```
//!
//! Remove the `#[ignore]` attributes as each is fixed.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

mod common;

use common::*;

/// The `invalid_project` fixture carries one defect of each class. Full `validate` reports
/// all of them; see `tests/invalid_project_test.rs`.
const FIXTURE: &str = "tests/fixtures/invalid_project";

#[test]
#[ignore = "documents the validate/gv <paths> parity gap; remove when fixed"]
fn test_gv_with_paths_detects_dual_ownership_via_codeowner_file() -> Result<(), Box<dyn Error>> {
    // `ruby/app/services/multi_owned.rb` is owned twice: a `@team Payments` annotation and
    // `ruby/app/services/.codeowner` naming Payroll. Full `gv` reports "Code ownership
    // should only be defined for each file in one way".
    //
    // BUG: `gv` regenerates first, which writes the file into CODEOWNERS as @PaymentTeam.
    // The per-path check then finds an owner and exits 0. A false pass -- the commit is
    // waved through with genuinely ambiguous ownership.
    let temp_dir = setup_fixture_repo(std::path::Path::new(FIXTURE));
    let project_root = temp_dir.path();
    git_add_all_files(project_root);

    Command::cargo_bin("codeowners")?
        .arg("--project-root")
        .arg(project_root)
        .arg("--no-cache")
        .arg("gv")
        .arg("ruby/app/services/multi_owned.rb")
        .assert()
        .failure()
        .stdout(predicate::str::contains("multi_owned.rb").and(predicate::str::contains("one way")));

    Ok(())
}

#[test]
#[ignore = "documents the validate/gv <paths> parity gap; remove when fixed"]
fn test_gv_with_paths_detects_dual_ownership_via_owned_gems() -> Result<(), Box<dyn Error>> {
    // Same class, different source: `gems/payroll_calculator/calculator.rb` has a
    // `@team Payments` annotation while Payroll claims it through `owned_gems`.
    //
    // BUG: same false pass. Included separately because the two travel through different
    // mappers, so a fix could plausibly catch one and miss the other.
    let temp_dir = setup_fixture_repo(std::path::Path::new(FIXTURE));
    let project_root = temp_dir.path();
    git_add_all_files(project_root);

    Command::cargo_bin("codeowners")?
        .arg("--project-root")
        .arg(project_root)
        .arg("--no-cache")
        .arg("gv")
        .arg("gems/payroll_calculator/calculator.rb")
        .assert()
        .failure()
        .stdout(predicate::str::contains("calculator.rb").and(predicate::str::contains("one way")));

    Ok(())
}

#[test]
#[ignore = "documents the validate/gv <paths> parity gap; remove when fixed"]
fn test_gv_with_paths_names_the_invalid_team() -> Result<(), Box<dyn Error>> {
    // `ruby/app/models/blockchain.rb` is annotated `@team Web3`, which is not a team. Full
    // `gv` reports "is referencing an invalid team - 'Web3'".
    //
    // BUG: this one does fail, but for the wrong reason. An invalid team yields no owner, so
    // the file is absent from the generated CODEOWNERS and gets reported as merely "unowned".
    // The actual fault -- a typo'd team name -- is never named, so the developer goes looking
    // for missing ownership instead of fixing the annotation.
    let temp_dir = setup_fixture_repo(std::path::Path::new(FIXTURE));
    let project_root = temp_dir.path();
    git_add_all_files(project_root);

    Command::cargo_bin("codeowners")?
        .arg("--project-root")
        .arg(project_root)
        .arg("--no-cache")
        .arg("gv")
        .arg("ruby/app/models/blockchain.rb")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Web3"));

    Ok(())
}

#[test]
#[ignore = "documents the validate/gv <paths> parity gap; remove when fixed"]
fn test_gv_with_every_path_matches_gv_with_no_paths() -> Result<(), Box<dyn Error>> {
    // The differential check: handing over every owned file should be equivalent to handing
    // over none. This is the general form of the three tests above -- it needs no knowledge
    // of which defects the fixture contains, so it keeps working as fixtures change.
    //
    // BUG: the no-paths run reports dual ownership and the invalid team; the all-paths run
    // reports neither.
    let temp_dir = setup_fixture_repo(std::path::Path::new(FIXTURE));
    let project_root = temp_dir.path();
    git_add_all_files(project_root);

    // owned_globs for this fixture is `**/*.{rb,tsx}`.
    let tracked = Command::new("git").arg("ls-files").current_dir(project_root).output()?;
    let owned_files: Vec<String> = String::from_utf8(tracked.stdout)?
        .lines()
        .filter(|line| line.ends_with(".rb") || line.ends_with(".tsx"))
        .map(str::to_string)
        .collect();
    assert!(!owned_files.is_empty(), "fixture should contain owned files");

    let no_paths = Command::cargo_bin("codeowners")?
        .arg("--project-root")
        .arg(project_root)
        .arg("--no-cache")
        .arg("gv")
        .output()?;

    let all_paths = Command::cargo_bin("codeowners")?
        .arg("--project-root")
        .arg(project_root)
        .arg("--no-cache")
        .arg("gv")
        .args(&owned_files)
        .output()?;

    // Compare the defects each run found, not byte-for-byte output: the two use different
    // report formats, and only the substance is being claimed here.
    let no_paths_out = String::from_utf8_lossy(&no_paths.stdout);
    let all_paths_out = String::from_utf8_lossy(&all_paths.stdout);

    for defect in ["one way", "Web3"] {
        assert_eq!(
            no_paths_out.contains(defect),
            all_paths_out.contains(defect),
            "`gv` with no paths and `gv` with every path disagree about {:?}.\n\
             \n--- no paths (exit {:?}) ---\n{}\n--- every path (exit {:?}) ---\n{}",
            defect,
            no_paths.status.code(),
            no_paths_out,
            all_paths.status.code(),
            all_paths_out,
        );
    }

    Ok(())
}

#[test]
#[ignore = "documents the validate/gv <paths> parity gap; remove when fixed"]
fn test_validate_does_not_silently_skip_absolute_paths() -> Result<(), Box<dyn Error>> {
    // Unrelated to the parity gap above, and the most dangerous of the set because it is
    // completely silent.
    //
    // `cli.rs` canonicalizes `--project-root`. On macOS the temp dir is under `/var`, which
    // canonicalizes to `/private/var`, so a caller-supplied `/var/...` path fails
    // `strip_prefix`, stays absolute, and is then rejected by the `owned_globs` filter --
    // dropped before any ownership query runs. Exit 0, no output, file never checked.
    //
    // valid_project is used here because its owned_globs are directory-anchored
    // (`{gems,config,javascript,ruby,components}/**`). With a `**`-leading glob the same path
    // survives the filter and is reported spuriously unowned instead, so the symptom is
    // config-dependent while the cause is the same.
    let temp_dir = setup_fixture_repo(std::path::Path::new("tests/fixtures/valid_project"));
    let project_root = temp_dir.path();
    git_add_all_files(project_root);

    // Deliberately NOT canonicalized -- that is the bug.
    let absolute = project_root.join("ruby/app/unowned.rb");

    Command::cargo_bin("codeowners")?
        .arg("--project-root")
        .arg(project_root)
        .arg("--no-cache")
        .arg("validate")
        .arg(absolute.to_str().unwrap())
        .assert()
        .failure()
        .stdout(predicate::str::contains("unowned.rb"));

    Ok(())
}
