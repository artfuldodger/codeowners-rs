use std::path::Path;

use super::mapper::{OwnerMatcher, Source, TeamName};

#[derive(Debug)]
pub struct Owner {
    pub sources: Vec<Source>,
    pub team_name: TeamName,
}

pub struct FileOwnerFinder<'a> {
    pub owner_matchers: &'a [OwnerMatcher],
}

impl FileOwnerFinder<'_> {
    pub fn find(&self, relative_path: &Path) -> Vec<Owner> {
        // A Vec rather than a HashMap: this runs once per file in the repo, and the
        // number of owning teams for a single file is almost always 0 or 1 (more
        // than 1 is a validation error). At that size a linear scan beats hashing,
        // and an unowned file allocates nothing at all.
        let mut team_sources: Vec<(&TeamName, Vec<Source>)> = Vec::new();
        let mut directory_overrider = DirectoryOverrider::default();

        for owner_matcher in self.owner_matchers {
            let (owner, source) = owner_matcher.owner_for(relative_path);

            if let Some(team_name) = owner {
                match source {
                    Source::Directory(_) => {
                        directory_overrider.process(team_name, source);
                    }
                    _ => {
                        push_source(&mut team_sources, team_name, source);
                    }
                }
            }
        }

        // Add most specific directory owner if it exists
        if let Some((team_name, source)) = directory_overrider.specific_directory_owner() {
            push_source(&mut team_sources, team_name, source);
        }

        team_sources
            .into_iter()
            .map(|(team_name, sources)| Owner {
                sources,
                team_name: team_name.clone(),
            })
            .collect()
    }
}

/// Appends `source` to `team_name`'s entry, creating it if this is the first
/// source seen for that team.
fn push_source<'a>(team_sources: &mut Vec<(&'a TeamName, Vec<Source>)>, team_name: &'a TeamName, source: &Source) {
    match team_sources.iter_mut().find(|(name, _)| *name == team_name) {
        Some((_, sources)) => sources.push(source.clone()),
        None => team_sources.push((team_name, vec![source.clone()])),
    }
}

/// DirectoryOverrider is used to override the owner of a directory if a more specific directory owner is found.
#[derive(Debug, Default)]
pub struct DirectoryOverrider<'a> {
    specific_directory_owner: Option<(&'a TeamName, &'a Source)>,
}

impl<'a> DirectoryOverrider<'a> {
    fn process(&mut self, team_name: &'a TeamName, source: &'a Source) {
        if self
            .specific_directory_owner
            .is_none_or(|(_, current_source)| current_source.len() < source.len())
        {
            self.specific_directory_owner = Some((team_name, source));
        }
    }

    fn specific_directory_owner(&self) -> Option<(&TeamName, &Source)> {
        self.specific_directory_owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_overrider() {
        let mut directory_overrider = DirectoryOverrider::default();
        assert_eq!(directory_overrider.specific_directory_owner(), None);
        let team_name_1 = "team1".to_string();
        let source_1 = Source::Directory("src/**".to_string());
        directory_overrider.process(&team_name_1, &source_1);
        assert_eq!(directory_overrider.specific_directory_owner(), Some((&team_name_1, &source_1)));

        let team_name_longest = "team2".to_string();
        let source_longest = Source::Directory("source/subdir/**".to_string());
        directory_overrider.process(&team_name_longest, &source_longest);
        assert_eq!(
            directory_overrider.specific_directory_owner(),
            Some((&team_name_longest, &source_longest))
        );

        let team_name_3 = "team3".to_string();
        let source_3 = Source::Directory("source/**".to_string());
        directory_overrider.process(&team_name_3, &source_3);
        assert_eq!(
            directory_overrider.specific_directory_owner(),
            Some((&team_name_longest, &source_longest))
        );
    }
}
