use error_stack::Report;
use lazy_static::lazy_static;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::{
    cache::{Cache, Caching},
    project::{Error, ProjectFile},
};

pub struct ProjectFileBuilder<'a> {
    global_cache: &'a Cache,
}

lazy_static! {
    static ref TEAM_REGEX: Regex =
        Regex::new(r#"^(?:#|//|<!--|<%#)\s*(?:@?team:?\s*)(.*?)\s*(?:-->|%>)?$"#).expect("error compiling regular expression");
}

impl<'a> ProjectFileBuilder<'a> {
    pub fn new(global_cache: &'a Cache) -> Self {
        Self { global_cache }
    }

    pub(crate) fn build(&self, path: PathBuf) -> ProjectFile {
        if let Ok(Some(cached_project_file)) = self.get_project_file_from_cache(&path) {
            return cached_project_file;
        }

        let project_file = build_project_file_without_cache(&path);

        self.save_project_file_to_cache(&path, &project_file);

        project_file
    }

    fn get_project_file_from_cache(&self, path: &Path) -> Result<Option<ProjectFile>, Report<Error>> {
        self.global_cache.get_file_owner(path).map(|entry| {
            entry.map(|e| ProjectFile {
                path: path.to_path_buf(),
                owner: e.owner,
            })
        })
    }

    fn save_project_file_to_cache(&self, path: &Path, project_file: &ProjectFile) {
        self.global_cache.write_file_owner(path, project_file.owner.clone());
    }
}

pub(crate) fn build_project_file_without_cache(path: &Path) -> ProjectFile {
    // Only the first line can carry a `@team` annotation, so read only that line.
    // This used to `read_to_string` the entire file: on a large repo that means
    // reading every byte of every source file to look at one line of each.
    let owner = first_line(path).and_then(|line| {
        TEAM_REGEX
            .captures(&line)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
    });

    ProjectFile {
        path: path.to_path_buf(),
        owner,
    }
}

/// The first line of `path`, without its line terminator.
///
/// Returns `None` if the file cannot be opened or its first line is not valid
/// UTF-8, matching the previous behavior of treating an unreadable file as
/// simply having no annotation.
fn first_line(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut line = String::new();
    BufReader::new(file).read_line(&mut line).ok()?;
    // `str::lines()` strips a trailing \r\n as well as \n; match that.
    let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    type FirstLine = &'static str;
    type Owner = &'static str;

    #[test]
    fn test_team_regex() {
        let mut map: HashMap<FirstLine, Owner> = HashMap::new();
        map.insert("// @team Foo", "Foo");
        map.insert("// @team Foo Bar", "Foo Bar");
        map.insert("// @team Zoo", "Zoo");
        map.insert("// @team: Zoo Foo", "Zoo Foo");
        map.insert("# @team: Bap", "Bap");
        map.insert("# @team: Bap Hap", "Bap Hap");
        map.insert("<!-- @team: Zoink -->", "Zoink");
        map.insert("<!-- @team: Zoink Err -->", "Zoink Err");
        map.insert("<%# @team: Zap %>", "Zap");
        map.insert("<%# @team: Zap Zip%>", "Zap Zip");
        map.insert("<!-- @team Blast -->", "Blast");
        map.insert("<!-- @team Blast Off -->", "Blast Off");

        // New team: format (without @ symbol)
        map.insert("# team: MyTeam", "MyTeam");
        map.insert("// team: MyTeam", "MyTeam");
        map.insert("<!-- team: MyTeam -->", "MyTeam");
        map.insert("<%# team: MyTeam %>", "MyTeam");

        for (key, value) in map {
            let owner = TEAM_REGEX.captures(key).and_then(|cap| cap.get(1)).map(|m| m.as_str());
            assert_eq!(owner, Some(value));
        }
    }

    fn write_and_build(name: &str, bytes: &[u8]) -> Option<String> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        build_project_file_without_cache(&path).owner
    }

    #[test]
    fn test_reads_annotation_from_first_line() {
        assert_eq!(
            write_and_build("a.rb", b"# @team Payments\nclass A; end\n"),
            Some("Payments".to_string())
        );
    }

    #[test]
    fn test_handles_crlf_line_endings() {
        assert_eq!(
            write_and_build("b.rb", b"# @team Payments\r\nclass B; end\r\n"),
            Some("Payments".to_string())
        );
    }

    #[test]
    fn test_no_annotation_on_first_line() {
        assert_eq!(write_and_build("c.rb", b"class C; end\n# @team Payments\n"), None);
    }

    #[test]
    fn test_empty_and_unterminated_files() {
        assert_eq!(write_and_build("d.rb", b""), None);
        // No trailing newline at all.
        assert_eq!(write_and_build("e.rb", b"# @team Payments"), Some("Payments".to_string()));
    }

    #[test]
    fn test_invalid_utf8_after_a_valid_first_line() {
        // Reading only the first line means a file whose *later* bytes are not
        // valid UTF-8 still yields its annotation. Reading the whole file used to
        // fail outright and report no owner. This is the one intentional behavior
        // change from reading line-wise.
        let mut bytes = b"# @team Payments\n".to_vec();
        bytes.extend_from_slice(&[0xff, 0xfe, 0x00]);
        assert_eq!(write_and_build("f.rb", &bytes), Some("Payments".to_string()));
    }

    #[test]
    fn test_invalid_utf8_within_the_first_line() {
        let bytes = [0xff, 0xfe, b'\n'];
        assert_eq!(write_and_build("g.rb", &bytes), None);
    }
}
