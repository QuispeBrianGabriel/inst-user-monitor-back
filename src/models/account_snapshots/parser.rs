//! Lectura de una lista de cuentas copiada de Instagram (Ctrl+A, Ctrl+C en
//! la lista de seguidores o de seguidos).
//!
//! El texto tiene una cuenta tras otra: una línea con el usuario y, debajo,
//! su nombre visible. El nombre puede faltar (`angelmeeza` va seguido
//! directamente de otro usuario), así que no basta con leer de dos en dos.
//! Se busca la lectura completa más probable:
//!
//! - Una línea de usuario tiene que ser un usuario de Instagram tal cual
//!   ([`Username::is_valid`]). Un nombre puede ser cualquier texto.
//! - Entre todas las lecturas posibles se elige la de menos cuentas: un texto
//!   que parece usuario debajo de otro usuario suele ser su nombre (`izqmatty`
//!   → `izqmatty`, `duki` → `5202`).
//! - Salvo que tenga `.` o `_`: es raro en un nombre y casi seguro que es la
//!   siguiente cuenta, sin nombre (`l4sov.jpg` → `juliodiaz.go`).
//!
//! Las líneas vacías y los espacios de los extremos no cuentan. Si no hay
//! ninguna lectura completa, el texto no es una lista de Instagram: se
//! rechaza entero, indicando la primera línea que no encaja.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::monitored_accounts::Username;

/// Máximo de cuentas en una lista pegada.
pub const MAX_ENTRIES: usize = 25_000;
/// Máximo de caracteres de un nombre visible.
pub const MAX_FULL_NAME_LEN: usize = 150;

/// Coste de cada cuenta de la lectura: se busca la de menor coste total.
const ENTRY_COST: u32 = 2;
/// Coste extra de leer como nombre un texto que parece otro usuario. Mayor
/// que `ENTRY_COST`: dos cuentas sin nombre salen más baratas que una con él.
const USERNAME_AS_NAME_COST: u32 = 3;

/// Una cuenta de la lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub username: String,
    pub full_name: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("el texto está vacío: pega la lista de cuentas copiada de Instagram")]
    Empty,
    #[error(
        "la línea {line} («{text}») no es un usuario de Instagram. Se espera una cuenta \
         por bloque: su usuario y, en la línea siguiente, su nombre. Copia solo la lista \
         de cuentas y vuelve a intentarlo; no se ha guardado nada"
    )]
    UnexpectedLine { line: usize, text: String },
    #[error(
        "la cuenta «{username}» aparece dos veces (líneas {first} y {second}); \
         no se ha guardado nada"
    )]
    Duplicate {
        username: String,
        first: usize,
        second: usize,
    },
    #[error("la lista no puede tener más de {MAX_ENTRIES} cuentas")]
    TooManyEntries,
}

/// Lee la lista de cuentas de `text`.
///
/// # Errors
///
/// Cuando el texto está vacío, alguna línea no encaja en el formato, una
/// cuenta aparece dos veces o hay demasiadas cuentas.
pub fn parse(text: &str) -> Result<Vec<Entry>, ParseError> {
    // (número de línea desde 1, texto recortado)
    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(ParseError::Empty);
    }
    if lines.len() > MAX_ENTRIES * 2 {
        return Err(ParseError::TooManyEntries);
    }

    let steps = best_reading(&lines).ok_or_else(|| first_unexpected_line(&lines))?;

    let mut entries = Vec::new();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut index = 0;
    while index < lines.len() {
        let (line, username) = lines[index];
        if let Some(&first) = seen.get(username) {
            return Err(ParseError::Duplicate {
                username: username.to_owned(),
                first,
                second: line,
            });
        }
        seen.insert(username, line);

        let with_name = steps[index];
        entries.push(Entry {
            username: username.to_owned(),
            full_name: with_name.then(|| lines[index + 1].1.to_owned()),
        });
        index += if with_name { 2 } else { 1 };
    }
    if entries.len() > MAX_ENTRIES {
        return Err(ParseError::TooManyEntries);
    }
    Ok(entries)
}

/// Lectura de menor coste: para cada línea en la que empieza una cuenta,
/// `true` si la siguiente es su nombre. `None` si no hay lectura completa.
///
/// Programación dinámica de atrás hacia delante: `cost[i]` es el coste mínimo
/// de leer las líneas `i..`; `None` si no se pueden leer.
fn best_reading(lines: &[(usize, &str)]) -> Option<Vec<bool>> {
    let n = lines.len();
    let mut cost: Vec<Option<u32>> = vec![None; n + 1];
    let mut with_name = vec![false; n];
    cost[n] = Some(0);

    for i in (0..n).rev() {
        let username = lines[i].1;
        if !Username::is_valid(username) {
            continue;
        }
        let alone = cost[i + 1].map(|rest| rest + ENTRY_COST);
        let paired = lines
            .get(i + 1)
            .filter(|(_, name)| is_full_name(name))
            .and_then(|(_, name)| {
                cost[i + 2].map(|rest| rest + ENTRY_COST + name_cost(username, name))
            });
        // A igual coste, con nombre.
        (cost[i], with_name[i]) = match (alone, paired) {
            (alone, Some(paired)) if alone.is_none_or(|alone| paired <= alone) => {
                (Some(paired), true)
            }
            (alone, _) => (alone, false),
        };
    }
    cost[0].map(|_| with_name)
}

fn is_full_name(text: &str) -> bool {
    text.chars().count() <= MAX_FULL_NAME_LEN
}

/// Coste extra de leer `name` como el nombre de `username`.
fn name_cost(username: &str, name: &str) -> u32 {
    let looks_like_another_account =
        name != username && Username::is_valid(name) && name.contains(['.', '_']);
    if looks_like_another_account {
        USERNAME_AS_NAME_COST
    } else {
        0
    }
}

/// Error para un texto sin lectura completa: la línea más avanzada a la que
/// se llega leyendo cuentas y que no es un usuario.
fn first_unexpected_line(lines: &[(usize, &str)]) -> ParseError {
    let n = lines.len();
    let mut reachable = vec![false; n + 1];
    reachable[0] = true;
    let mut stuck = 0;
    for i in 0..n {
        if !reachable[i] {
            continue;
        }
        if Username::is_valid(lines[i].1) {
            reachable[i + 1] = true;
            if i + 1 < n && is_full_name(lines[i + 1].1) {
                reachable[i + 2] = true;
            }
        } else {
            stuck = i;
        }
    }
    let (line, text) = lines[stuck];
    ParseError::UnexpectedLine {
        line,
        text: preview(text),
    }
}

/// Recorta un texto largo para mostrarlo en un mensaje.
fn preview(text: &str) -> String {
    const MAX: usize = 60;
    if text.chars().count() <= MAX {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(MAX).collect();
        format!("{cut}…")
    }
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
    fn reads_pairs_of_username_and_name() {
        let text = "izqmatty\nizqmatty\ncreativehub.py\nCreative Hub\nduki\n5202\n";
        assert_eq!(
            parse(text).unwrap(),
            [
                entry("izqmatty", Some("izqmatty")),
                entry("creativehub.py", Some("Creative Hub")),
                entry("duki", Some("5202")),
            ]
        );
    }

    #[test]
    fn reads_accounts_without_name() {
        let text = "setinzinho\n0.001%\nangelmeeza\njer.silva.green\nJeremy Silva\n\
                    l4sov.jpg\njuliodiaz.go\nJulio Diaz";
        assert_eq!(
            parse(text).unwrap(),
            [
                entry("setinzinho", Some("0.001%")),
                entry("angelmeeza", None),
                entry("jer.silva.green", Some("Jeremy Silva")),
                entry("l4sov.jpg", None),
                entry("juliodiaz.go", Some("Julio Diaz")),
            ]
        );
    }

    #[test]
    fn prefers_names_without_dots_or_underscores() {
        // `juliodiaz.go` podría ser el nombre de `l4sov.jpg`, y `julio` el
        // de nadie; pero un nombre con punto es casi siempre otra cuenta.
        let text = "l4sov.jpg\njuliodiaz.go\njulio";
        assert_eq!(
            parse(text).unwrap(),
            [
                entry("l4sov.jpg", None),
                entry("juliodiaz.go", Some("julio"))
            ]
        );
    }

    #[test]
    fn reads_consecutive_accounts_without_name() {
        assert_eq!(
            parse("a.x\nb.y\nc.z\nd_w").unwrap(),
            [
                entry("a.x", None),
                entry("b.y", None),
                entry("c.z", None),
                entry("d_w", None)
            ]
        );
    }

    #[test]
    fn last_account_may_have_no_name() {
        assert_eq!(
            parse("duki\n5202\nangelmeeza").unwrap(),
            [entry("duki", Some("5202")), entry("angelmeeza", None)]
        );
        assert_eq!(parse("angelmeeza").unwrap(), [entry("angelmeeza", None)]);
    }

    #[test]
    fn ignores_blank_lines_and_surrounding_spaces() {
        let text = "\r\n  danisanchez13ds \r\nSacjo \r\n\r\n\tduki\n5202\n\n";
        assert_eq!(
            parse(text).unwrap(),
            [
                entry("danisanchez13ds", Some("Sacjo")),
                entry("duki", Some("5202"))
            ]
        );
    }

    #[test]
    fn reads_the_example_paste() {
        let entries = parse(include_str!("../../../tests/fixtures/followers.txt")).unwrap();
        assert_eq!(entries.len(), 222);
        assert_eq!(entries[0], entry("izqmatty", Some("izqmatty")));
        assert!(entries.contains(&entry("angelmeeza", None)));
        assert!(entries.contains(&entry("l4sov.jpg", None)));
        assert!(entries.contains(&entry("living.d4adgirl", Some("danaqiu"))));
        assert_eq!(
            entries.last(),
            Some(&entry("heidijimenez2017", Some("Heidi Jiménez Steppuhn")))
        );
    }

    #[test]
    fn rejects_empty_text() {
        assert_eq!(parse(""), Err(ParseError::Empty));
        assert_eq!(parse(" \n\t\n"), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_text_that_is_not_a_list() {
        assert_eq!(
            parse("Seguidores\nduki\n5202"),
            Err(ParseError::UnexpectedLine {
                line: 1,
                text: "Seguidores".to_owned()
            })
        );
        // Un botón copiado en medio de la lista.
        assert_eq!(
            parse("duki\nDuki\nSeguir\nangelmeeza"),
            Err(ParseError::UnexpectedLine {
                line: 3,
                text: "Seguir".to_owned()
            })
        );
        // Dos nombres seguidos.
        assert_eq!(
            parse("\nduki\nDuki\nNadia Boggiano\n"),
            Err(ParseError::UnexpectedLine {
                line: 4,
                text: "Nadia Boggiano".to_owned()
            })
        );
    }

    #[test]
    fn shortens_long_lines_in_errors() {
        let long = "X".repeat(200);
        let Err(ParseError::UnexpectedLine { text, .. }) = parse(&long) else {
            panic!("debía fallar");
        };
        assert_eq!(text.chars().count(), 61);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn rejects_duplicates() {
        assert_eq!(
            parse("duki\n5202\nangelmeeza\nduki\nDuki"),
            Err(ParseError::Duplicate {
                username: "duki".to_owned(),
                first: 1,
                second: 4
            })
        );
    }

    #[test]
    fn rejects_too_many_entries() {
        let text: String = (0..=MAX_ENTRIES).map(|i| format!("user.{i}\n")).collect();
        assert_eq!(parse(&text), Err(ParseError::TooManyEntries));
    }
}
