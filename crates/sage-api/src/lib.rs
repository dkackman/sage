mod events;
mod records;
mod requests;
mod types;

#[cfg(feature = "openapi")]
mod openapi_metadata;

pub use events::*;
pub use records::*;
pub use requests::*;
pub use types::*;

#[cfg(feature = "openapi")]
pub use openapi_metadata::*;

// Re-export the openapi attribute macro
#[cfg(feature = "openapi")]
pub use sage_api_macro::openapi as openapi_attr;

#[cfg(test)]
mod password_gate_drift {
    use std::collections::BTreeSet;

    /// The gated set must match, exactly, the request types carrying a
    /// `password` field. If this fails you either added a signing endpoint
    /// without gating it (a security hole) or gated one that takes no
    /// password (a spurious prompt).
    #[test]
    fn gated_set_matches_request_types_with_password_field() {
        let gated: BTreeSet<String> =
            serde_json::from_str(include_str!("../password-gated.json")).unwrap();

        let requests_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/requests");
        let entries = std::fs::read_dir(&requests_dir).unwrap_or_else(|error| {
            panic!("failed to read request sources at {requests_dir:?}: {error}")
        });

        let mut sources = Vec::new();
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                sources.push(std::fs::read_to_string(&path).unwrap_or_else(|error| {
                    panic!("failed to read {path:?}: {error}")
                }));
            }
        }

        assert!(
            sources.len() >= 6,
            "expected at least 6 request source files under {requests_dir:?}, found {}",
            sources.len(),
        );

        let mut discovered = BTreeSet::new();
        for source in &sources {
            discovered.extend(structs_with_password_field(source));
        }

        assert_eq!(
            gated, discovered,
            "password-gated.json is out of sync with the request types.\n\
             Only in password-gated.json: {:?}\n\
             Only in request types: {:?}",
            gated.difference(&discovered).collect::<Vec<_>>(),
            discovered.difference(&gated).collect::<Vec<_>>(),
        );
    }

    /// Scans Rust source for `pub struct Name {` blocks containing a
    /// `pub password: Option<String>` field, returning snake_case names.
    fn structs_with_password_field(source: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut index = 0;

        while index < lines.len() {
            let line = lines[index].trim_end();
            let Some(rest) = line.strip_prefix("pub struct ") else {
                index += 1;
                continue;
            };
            let Some(name) = rest.strip_suffix(" {") else {
                index += 1;
                continue;
            };

            let mut cursor = index + 1;
            let mut has_password = false;
            while cursor < lines.len() && lines[cursor] != "}" {
                if lines[cursor].trim() == "pub password: Option<String>," {
                    has_password = true;
                }
                cursor += 1;
            }

            if has_password {
                found.insert(to_snake_case(name));
            }
            index = cursor + 1;
        }

        found
    }

    fn to_snake_case(name: &str) -> String {
        let mut out = String::new();
        for (position, character) in name.char_indices() {
            if character.is_uppercase() && position != 0 {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        }
        out
    }
}
