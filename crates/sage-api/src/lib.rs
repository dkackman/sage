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

    /// The three ways the host-layer gate can treat an endpoint. Mirrors the
    /// enum the macro deserialises `password-gating.json` into.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum GateMode {
        /// Prompt every call, verifying against the active wallet.
        Always,
        /// Prompt only when `req.auto_submit` is set. These endpoints build a
        /// transaction for confirmation first and touch no secret until the
        /// caller asks for it to be signed and submitted.
        AutoSubmit,
        /// Prompt every call, verifying against `req.fingerprint` rather than
        /// the active wallet.
        Fingerprint,
    }

    /// Reads the single gating manifest, rejecting any mode string the macro
    /// would not understand.
    fn gate_modes() -> BTreeMap<String, GateMode> {
        let raw: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../password-gating.json")).unwrap();

        raw.into_iter()
            .map(|(endpoint, mode)| {
                let mode = match mode.as_str() {
                    "always" => GateMode::Always,
                    "auto_submit" => GateMode::AutoSubmit,
                    "fingerprint" => GateMode::Fingerprint,
                    other => panic!(
                        "password-gating.json: unknown mode {other:?} for {endpoint:?}; \
                         expected \"always\", \"auto_submit\", or \"fingerprint\"",
                    ),
                };
                (endpoint, mode)
            })
            .collect()
    }

    fn endpoints_with_mode(mode: GateMode) -> BTreeSet<String> {
        gate_modes()
            .into_iter()
            .filter(|(_, actual)| *actual == mode)
            .map(|(endpoint, _)| endpoint)
            .collect()
    }

    /// Reads every request source file so the scanners below see the same
    /// text the macro's JSON manifest is supposed to describe.
    fn request_sources() -> Vec<String> {
        let requests_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/requests");
        read_rust_sources(&requests_dir, 6)
    }

    /// Reads the `sage` crate's endpoint implementations. The gate lives in the
    /// Tauri host layer, but the mode each endpoint needs is a property of how
    /// its implementation consumes the password, so the invariant can only be
    /// checked against that source.
    fn endpoint_implementation_sources() -> Vec<String> {
        let endpoints_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sage/src/endpoints");
        read_rust_sources(&endpoints_dir, 4)
    }

    fn read_rust_sources(dir: &std::path::Path, minimum: usize) -> Vec<String> {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("failed to read sources at {dir:?}: {error}"));

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
            sources.len() >= minimum,
            "expected at least {minimum} Rust source files under {dir:?}, found {}",
            sources.len(),
        );

        sources
    }

    /// The manifest's keys must match, exactly, the request types carrying a
    /// `password` field. If this fails you either added a signing endpoint
    /// without gating it (a security hole) or gated one that takes no
    /// password (a spurious prompt).
    #[test]
    fn gating_map_matches_request_types_with_password_field() {
        let gated: BTreeSet<String> = gate_modes().into_keys().collect();

        let mut discovered = BTreeSet::new();
        for source in &request_sources() {
            discovered.extend(password_structs(source).into_keys());
        }

        assert_eq!(
            gated,
            discovered,
            "password-gating.json is out of sync with the request types.\n\
             Only in password-gating.json: {:?}\n\
             Only in request types: {:?}",
            gated.difference(&discovered).collect::<Vec<_>>(),
            discovered.difference(&gated).collect::<Vec<_>>(),
        );
    }

    /// The `fingerprint` mode must match, exactly, the gated request types that
    /// carry a `fingerprint` field. If this fails the macro either prompts for
    /// the active wallet while acting on a named one (wrong password handed to
    /// the keychain, and a hard failure when logged out), or reads a
    /// `req.fingerprint` that does not exist (a compile error).
    #[test]
    fn fingerprint_mode_matches_request_types_with_fingerprint_field() {
        let fingerprint_gated = endpoints_with_mode(GateMode::Fingerprint);

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
            "the \"fingerprint\" entries in password-gating.json are out of sync with the \
             request types.\n\
             Only in password-gating.json: {:?}\n\
             Only in request types: {:?}",
            fingerprint_gated
                .difference(&discovered)
                .collect::<Vec<_>>(),
            discovered
                .difference(&fingerprint_gated)
                .collect::<Vec<_>>(),
        );
    }

    /// `auto_submit` mode expands to `if req.auto_submit { ... }`, so the field
    /// has to exist. Without this check a mis-filed endpoint is a compile error
    /// in generated code, which points at the macro rather than the manifest.
    #[test]
    fn auto_submit_mode_entries_have_an_auto_submit_field() {
        let auto_submit = endpoints_with_mode(GateMode::AutoSubmit);

        let mut discovered = BTreeSet::new();
        for source in &request_sources() {
            for (name, _) in password_structs(source) {
                if struct_has_auto_submit(source, &name) {
                    discovered.insert(name);
                }
            }
        }

        let missing: Vec<_> = auto_submit.difference(&discovered).collect();
        assert!(
            missing.is_empty(),
            "these endpoints are marked \"auto_submit\" in password-gating.json but their \
             request types have no `auto_submit` field: {missing:?}",
        );
    }

    /// The mode has to match how the endpoint actually consumes the password.
    /// An endpoint that reaches the keychain directly (`extract_secrets`, or
    /// `self.sign`) needs the password on every call; one that only forwards it
    /// to `Sage::transact`/`transact_with` uses it solely when `auto_submit` is
    /// set, and prompting unconditionally there means asking the user for a
    /// password that gets discarded -- once to build the transaction, and again
    /// after they confirm it.
    #[test]
    fn gate_modes_match_how_endpoints_consume_the_password() {
        let sources = endpoint_implementation_sources();

        for (endpoint, mode) in gate_modes() {
            let Some(body) = endpoint_body(&sources, &endpoint) else {
                panic!("no `fn {endpoint}` found in the sage crate's endpoint implementations");
            };

            let signs_directly = body.contains("extract_secrets") || body.contains("self.sign(");
            let expected_conditional = !signs_directly;
            let marked_conditional = mode == GateMode::AutoSubmit;

            assert_eq!(
                marked_conditional,
                expected_conditional,
                "password-gating.json marks `{endpoint}` as {mode:?}, but its implementation \
                 {}. Endpoints that reach a secret directly must be \"always\" (or \
                 \"fingerprint\"); endpoints that only forward the password to \
                 `transact`/`transact_with` must be \"auto_submit\".",
                if signs_directly {
                    "reaches the keychain on every call"
                } else {
                    "only uses the password when `auto_submit` is set"
                },
            );
        }
    }

    /// Extracts the body of `fn <endpoint>(` from the endpoint implementations,
    /// up to the closing brace at method indentation.
    fn endpoint_body(sources: &[String], endpoint: &str) -> Option<String> {
        let needle = format!("fn {endpoint}(");

        for source in sources {
            let lines: Vec<&str> = source.lines().collect();
            let Some(start) = lines.iter().position(|line| {
                let trimmed = line.trim_start();
                (trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub async fn ")
                    || trimmed.starts_with("pub(crate) fn ")
                    || trimmed.starts_with("pub(crate) async fn "))
                    && trimmed.contains(&needle)
            }) else {
                continue;
            };

            let mut body = String::new();
            for line in &lines[start..] {
                body.push_str(line);
                body.push('\n');
                if *line == "    }" {
                    break;
                }
            }
            return Some(body);
        }

        None
    }

    /// Whether the `pub struct` for a `snake_case` endpoint name carries a
    /// `pub auto_submit: bool` field.
    fn struct_has_auto_submit(source: &str, endpoint: &str) -> bool {
        let pascal: String = endpoint
            .split('_')
            .map(|word| {
                let mut characters = word.chars();
                match characters.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                    None => String::new(),
                }
            })
            .collect();

        let header = format!("pub struct {pascal} {{");
        let lines: Vec<&str> = source.lines().collect();
        let Some(start) = lines.iter().position(|line| line.trim_end() == header) else {
            return false;
        };

        lines[start + 1..]
            .iter()
            .take_while(|line| **line != "}")
            .any(|line| line.trim() == "pub auto_submit: bool,")
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
