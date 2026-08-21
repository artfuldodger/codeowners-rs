//! Parity between `validate` / `gv` with an explicit file list and the same command with
//! no file list.
//!
//! Passing paths routes through `validate_files()` instead of `validate_all()`
//! (`runner.rs:124`). Both resolve ownership through the mappers, so both catch an
//! invalid team annotation and a file owned two ways; `validate_files` simply scopes the
//! per-file checks to the supplied paths.
//!
//! Only the staleness check differs, and unavoidably so: it compares the whole generated
//! CODEOWNERS against the whole on-disk one, so it cannot be scoped to a subset.
//! `gv <paths>` makes it moot by regenerating first.
//!
//! These previously all failed. `validate_files` used to answer only "does this path have
//! an owner in the CODEOWNERS file", which could not see a file owned two ways — generation
//! picks one winner, so the file looked owned and the command exited 0. They are kept as
//! regression guards against reintroducing that shortcut.
//!
//! One test remains `#[ignore]`d for a separate, still-unfixed bug. Run it with:
//!
//! ```sh
//! cargo test --test validate_files_parity_test -- --ignored
//! ```

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

mod common;

use common::*;

/// The `invalid_project` fixture carries one defect of each class. Full `validate` reports
/// all of them; see `tests/invalid_project_test.rs`.
const FIXTURE: &str = "tests/fixtures/invalid_project";

#[test]
fn test_gv_with_paths_detects_dual_ownership_via_codeowner_file() -> Result<(), Box<dyn Error>> {
    // `ruby/app/services/multi_owned.rb` is owned twice: a `@team Payments` annotation and
    // `ruby/app/services/.codeowner` naming Payroll. Full `gv` reports "Code ownership
    // should only be defined for each file in one way".
    //
    // Regression guard. This used to exit 0 with empty output: `gv` regenerates first,
    // writing the file into CODEOWNERS under @PaymentTeam, so a check that read CODEOWNERS
    // back found an owner and passed. Regeneration concealed the defect.
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
fn test_gv_with_paths_detects_dual_ownership_via_owned_gems() -> Result<(), Box<dyn Error>> {
    // Same class, different source: `gems/payroll_calculator/calculator.rb` has a
    // `@team Payments` annotation while Payroll claims it through `owned_gems`.
    //
    // Regression guard, same false pass. Kept separate because the two travel through
    // different mappers, so a regression could reappear in one and not the other.
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
fn test_gv_with_paths_names_the_invalid_team() -> Result<(), Box<dyn Error>> {
    // `ruby/app/models/blockchain.rb` is annotated `@team Web3`, which is not a team. Full
    // `gv` reports "is referencing an invalid team - 'Web3'".
    //
    // Regression guard. This used to fail, but for the wrong reason: an invalid team yields
    // no owner, so the file was absent from the generated CODEOWNERS and reported as merely
    // "unowned", sending the developer after missing ownership instead of a typo'd team.
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
fn test_gv_with_every_path_matches_gv_with_no_paths() -> Result<(), Box<dyn Error>> {
    // The differential check: handing over every owned file should be equivalent to handing
    // over none. This is the general form of the three tests above -- it needs no knowledge
    // of which defects the fixture contains, so it keeps working as fixtures change.
    //
    // Regression guard, and the most valuable of the set: it needs no knowledge of the
    // fixture's contents, so it keeps working as fixtures change. The all-paths run used to
    // report neither the dual ownership nor the invalid team.
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

    // Compare the defects each run found rather than byte-for-byte output. The two now
    // share a report format, but the no-paths run legitimately reports more (staleness,
    // and files outside the supplied list), so only the shared substance is claimed here.
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
#[ignore = "separate pre-existing bug: owned_globs filter drops non-canonical absolute paths"]
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
