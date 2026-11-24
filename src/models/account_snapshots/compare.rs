//! Comparación entre capturas: qué cambió desde la anterior.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::parser::Entry;

/// Cambios de una lista respecto de la captura anterior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Changes {
    /// Diferencia en la cantidad de cuentas (puede ser negativa).
    pub total_delta: i64,
    /// Cuentas nuevas, en el orden de la lista actual.
    pub added: Vec<Entry>,
    /// Cuentas que ya no están, en el orden de la lista anterior.
    pub removed: Vec<Entry>,
    /// Cuentas que siguen pero cambiaron de nombre visible.
    pub renamed: Vec<Rename>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rename {
    pub username: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl Changes {
    #[must_use]
    pub fn between(previous: &[Entry], current: &[Entry]) -> Self {
        let before: HashMap<&str, &Entry> = previous
            .iter()
            .map(|entry| (entry.username.as_str(), entry))
            .collect();
        let after: HashSet<&str> = current
            .iter()
            .map(|entry| entry.username.as_str())
            .collect();

        let mut added = Vec::new();
        let mut renamed = Vec::new();
        for entry in current {
            match before.get(entry.username.as_str()) {
                None => added.push(entry.clone()),
                Some(old) if old.full_name != entry.full_name => renamed.push(Rename {
                    username: entry.username.clone(),
                    before: old.full_name.clone(),
                    after: entry.full_name.clone(),
                }),
                Some(_) => {}
            }
        }
        let removed = previous
            .iter()
            .filter(|entry| !after.contains(entry.username.as_str()))
            .cloned()
            .collect();

        Self {
            total_delta: len_i64(current) - len_i64(previous),
            added,
            removed,
            renamed,
        }
    }
}

fn len_i64(entries: &[Entry]) -> i64 {
    i64::try_from(entries.len()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(username: &str, full_name: Option<&str>) -> Entry {
        Entry {
            username: username.to_owned(),
            full_name: full_name.map(str::to_owned),
        }
    }

    #[test]
    fn changes_between_snapshots() {
        let previous = [
            entry("duki", Some("5202")),
            entry("hettacs", Some("hetta")),
            entry("angelmeeza", None),
        ];
        let current = [
            entry("nuevo.uno", Some("Nuevo")),
            entry("duki", Some("Duki")),
            entry("angelmeeza", None),
            entry("nuevo.dos", None),
        ];

        let changes = Changes::between(&previous, &current);

        assert_eq!(changes.total_delta, 1);
        assert_eq!(
            changes.added,
            [entry("nuevo.uno", Some("Nuevo")), entry("nuevo.dos", None)]
        );
        assert_eq!(changes.removed, [entry("hettacs", Some("hetta"))]);
        assert_eq!(
            changes.renamed,
            [Rename {
                username: "duki".to_owned(),
                before: Some("5202".to_owned()),
                after: Some("Duki".to_owned()),
            }]
        );
    }

    #[test]
    fn no_changes_between_equal_lists() {
        let list = [entry("duki", Some("5202"))];
        let changes = Changes::between(&list, &list);
        assert_eq!(changes.total_delta, 0);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        assert!(changes.renamed.is_empty());
    }
}
