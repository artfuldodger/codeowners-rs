use indoc::indoc;
use predicates::prelude::*;
use std::error::Error;

mod common;
use common::OutputStream;
use common::run_codeowners;

// Alpha owns ruby/app/**/* but excludes beta_owned.rb through unowned_globs, and Beta owns that file
// through an annotation. GitHub applies the last matching CODEOWNERS line, so the annotation line must
// come after Alpha's broader glob for GitHub to agree with for-file.
const FIXTURE: &str = "annotation_in_unowned_team_glob";

#[test]
fn test_validate_accepts_generated_order() -> Result<(), Box<dyn Error>> {
    run_codeowners(FIXTURE, &["validate"], true, OutputStream::Stdout, predicate::eq(""))?;
    Ok(())
}

#[test]
fn test_codeowners_file_agrees_with_for_file() -> Result<(), Box<dyn Error>> {
    run_codeowners(
        FIXTURE,
        &["crosscheck-owners"],
        true,
        OutputStream::Stdout,
        predicate::eq(indoc! {"
            Success! All files match between CODEOWNERS and for-file command.
        "}),
    )?;
    Ok(())
}

#[test]
fn test_for_file_from_codeowners_returns_annotated_owner() -> Result<(), Box<dyn Error>> {
    run_codeowners(
        FIXTURE,
        &["for-file", "--from-codeowners", "ruby/app/services/beta_owned.rb"],
        true,
        OutputStream::Stdout,
        predicate::str::contains("Team: Beta"),
    )?;
    Ok(())
}
