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
    use std::collections::{BTreeMap, BTreeSet};

    /// Reads every request source file so the scanners below see the same
    /// text the macro's JSON manifests are supposed to describe.
    fn request_sources() -> Vec<String> {
        let requests_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/requests");
        let entries = std::fs::read_dir(&requests_dir).unwrap_or_else(|error| {
            panic!("failed to read request sources at {requests_dir:?}: {error}")
        });

        let mut sources = Vec::new();
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                sources.push(
                    std::fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}")),
                );
            }
        }

        assert!(
            sources.len() >= 6,
            "expected at least 6 request source files under {requests_dir:?}, found {}",
            sources.len(),
        );

        sources
    }

    /// The fingerprint-bearing subset must match, exactly, the gated request
    /// types that also carry a `fingerprint` field. If this fails the macro
    /// either prompts for the active wallet while acting on a named one (wrong
    /// password handed to the keychain, and a hard failure when logged out), or
    /// reads a `req.fingerprint` that does not exist (a compile error).
    #[test]
    fn fingerprint_gated_set_matches_request_types_with_fingerprint_field() {
        let fingerprint_gated: BTreeSet<String> =
            serde_json::from_str(include_str!("../password-gated-fingerprint.json")).unwrap();
        let gated: BTreeSet<String> =
            serde_json::from_str(include_str!("../password-gated.json")).unwrap();

        assert!(
            fingerprint_gated.is_subset(&gated),
            "password-gated-fingerprint.json must be a subset of password-gated.json; \
             extra entries: {:?}",
            fingerprint_gated.difference(&gated).collect::<Vec<_>>(),
        );

        let mut discovered = BTreeSet::new();
        for source in &request_sources() {
            for (name, has_fingerprint) in password_structs(source) {
                if has_fingerprint {
                    discovered.insert(name);
                }
            }
        }

        assert_eq!(
            fingerprint_gated,
            discovered,
            "password-gated-fingerprint.json is out of sync with the request types.\n\
             Only in password-gated-fingerprint.json: {:?}\n\
             Only in request types: {:?}",
            fingerprint_gated
                .difference(&discovered)
                .collect::<Vec<_>>(),
            discovered
                .difference(&fingerprint_gated)
                .collect::<Vec<_>>(),
        );
    }

    /// The gated set must match, exactly, the request types carrying a
    /// `password` field. If this fails you either added a signing endpoint
    /// without gating it (a security hole) or gated one that takes no
    /// password (a spurious prompt).
    #[test]
    fn gated_set_matches_request_types_with_password_field() {
        let gated: BTreeSet<String> =
            serde_json::from_str(include_str!("../password-gated.json")).unwrap();

        let mut discovered = BTreeSet::new();
        for source in &request_sources() {
            discovered.extend(password_structs(source).into_keys());
        }

        assert_eq!(
            gated,
            discovered,
            "password-gated.json is out of sync with the request types.\n\
             Only in password-gated.json: {:?}\n\
             Only in request types: {:?}",
            gated.difference(&discovered).collect::<Vec<_>>(),
            discovered.difference(&gated).collect::<Vec<_>>(),
        );
    }

    /// Scans Rust source for `pub struct Name {` blocks containing a
    /// `pub password: Option<String>` field, returning `snake_case` names
    /// mapped to whether the same struct also carries a `fingerprint` field.
    fn password_structs(source: &str) -> BTreeMap<String, bool> {
        let mut found = BTreeMap::new();
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
            let mut has_fingerprint = false;
            while cursor < lines.len() && lines[cursor] != "}" {
                let field = lines[cursor].trim();
                if field == "pub password: Option<String>," {
                    has_password = true;
                }
                if field == "pub fingerprint: u32," {
                    has_fingerprint = true;
                }
                cursor += 1;
            }

            if has_password {
                found.insert(to_snake_case(name), has_fingerprint);
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
