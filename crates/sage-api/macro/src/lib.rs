mod openapi;

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use quote::quote;
use syn::parse_macro_input;

#[proc_macro]
pub fn impl_endpoints(input: TokenStream) -> TokenStream {
    generate(&input, false)
}

#[proc_macro]
pub fn impl_endpoints_tauri(input: TokenStream) -> TokenStream {
    generate(&input, true)
}

/// Attribute macro for `OpenAPI` metadata
///
/// Usage:
/// ```ignore
/// #[openapi(
///     tag = "Authentication & Keys",
///     description = "Authenticate and log into a wallet..."
/// )]
/// pub struct Login { ... }
/// ```
#[proc_macro_attribute]
pub fn openapi(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as openapi::OpenApiArgs);
    let input = parse_macro_input!(input as syn::DeriveInput);

    openapi::impl_openapi_metadata(&args, &input).into()
}

/// Generates schema registrations from endpoints.json
///
/// Automatically reads endpoints.json and generates `.schema_from::<Type>()`
/// calls for all endpoint request/response types.
#[proc_macro]
pub fn register_openapi_types(input: TokenStream) -> TokenStream {
    openapi::impl_openapi_registration(input)
}

/// Generates endpoint metadata match arms from endpoints.json
///
/// Automatically generates match arms that retrieve tag and description
/// from `OpenApiMetadata` trait implementations.
#[proc_macro]
pub fn endpoint_metadata(input: TokenStream) -> TokenStream {
    openapi::impl_endpoint_metadata(input)
}

/// Generates request schema match arms from endpoints.json
///
/// Automatically generates match arms mapping endpoints to their request schemas.
#[proc_macro]
pub fn request_schemas(input: TokenStream) -> TokenStream {
    openapi::impl_request_schemas(input)
}

/// Generates response schema match arms from endpoints.json
///
/// Automatically generates match arms mapping endpoints to their response schemas.
#[proc_macro]
pub fn response_schemas(input: TokenStream) -> TokenStream {
    openapi::impl_response_schemas(input)
}

fn generate(input: &TokenStream, tauri: bool) -> TokenStream {
    let mut endpoints: IndexMap<String, bool> =
        serde_json::from_str(include_str!("../../endpoints.json")).expect("Invalid endpoint file");

    if tauri {
        let tauri_endpoints: IndexMap<String, bool> =
            serde_json::from_str(include_str!("../../endpoints-tauri.json"))
                .expect("Invalid endpoint file");

        endpoints.extend(tauri_endpoints);
    }

    let password_gated: std::collections::BTreeSet<String> =
        serde_json::from_str(include_str!("../../password-gated.json"))
            .expect("Invalid password-gated endpoint file");

    // The subset of the gated endpoints whose request type carries a
    // `fingerprint` field. These act on a *named* wallet rather than the
    // active one, so the gate must verify against that wallet.
    let fingerprint_gated: std::collections::BTreeSet<String> =
        serde_json::from_str(include_str!("../../password-gated-fingerprint.json"))
            .expect("Invalid fingerprint-gated endpoint file");

    let gating = Gating {
        password_gated: &password_gated,
        fingerprint_gated: &fingerprint_gated,
    };

    let mut output = proc_macro2::TokenStream::new();

    for token in input.clone() {
        convert(token, &endpoints, gating, None, &mut output);
    }

    output.into()
}

/// Which endpoints the password gate applies to, and how.
#[derive(Clone, Copy)]
struct Gating<'a> {
    password_gated: &'a std::collections::BTreeSet<String>,
    fingerprint_gated: &'a std::collections::BTreeSet<String>,
}

fn convert(
    tree: TokenTree,
    endpoints: &IndexMap<String, bool>,
    gating: Gating<'_>,
    endpoint: Option<&str>,
    output: &mut proc_macro2::TokenStream,
) {
    match &tree {
        TokenTree::Ident(old) => {
            let Some(endpoint) = endpoint else {
                output.extend(proc_macro2::TokenStream::from(TokenStream::from(tree)));
                return;
            };
            let is_async = endpoints[endpoint];

            let ident = old.to_string();

            if ident == "endpoint_string" {
                output.extend(quote!(#endpoint));
            } else if ident == "maybe_async" {
                if is_async {
                    output.extend(quote!(async));
                }
            } else if ident == "maybe_await" {
                if is_async {
                    output.extend(quote!(.await));
                }
            } else if ident == "maybe_unlock" {
                if gating.fingerprint_gated.contains(endpoint) {
                    // Acts on `req.fingerprint`, which may be a wallet other
                    // than the active one -- and may run with no wallet
                    // active at all, from the logged-out wallet list.
                    output.extend(quote!(
                        req.password = sage_password_gate::resolve_for_fingerprint(&app_handle, state.inner(), gate.inner(), req.fingerprint).await?;
                    ));
                } else if gating.password_gated.contains(endpoint) {
                    output.extend(quote!(
                        req.password = sage_password_gate::resolve(&app_handle, state.inner(), gate.inner()).await?;
                    ));
                }
            } else if ident.is_case(Case::Snake) {
                let ident = proc_macro2::Ident::new(
                    &ident.replace("endpoint", &endpoint.to_case(Case::Snake)),
                    old.span().into(),
                );
                output.extend(quote!(#ident));
            } else if ident.is_case(Case::Pascal) {
                let ident = proc_macro2::Ident::new(
                    &ident.replace("Endpoint", &endpoint.to_case(Case::Pascal)),
                    old.span().into(),
                );
                output.extend(quote!(#ident));
            } else {
                output.extend(proc_macro2::TokenStream::from(TokenStream::from(tree)));
            }
        }
        TokenTree::Literal(..) | TokenTree::Punct(..) => {
            output.extend(proc_macro2::TokenStream::from(TokenStream::from(tree)));
        }
        TokenTree::Group(group) => {
            let mut stream = group.stream().into_iter().peekable();

            let repeat = stream.peek().is_some_and(|token| {
                if let TokenTree::Ident(ident) = &token {
                    ident.to_string() == "repeat" && group.delimiter() == Delimiter::Parenthesis
                } else {
                    false
                }
            });

            if repeat {
                stream.next();
            }

            if repeat {
                for endpoint in endpoints.keys() {
                    for tree in stream.clone() {
                        convert(tree, endpoints, gating, Some(endpoint), output);
                    }
                }
            } else {
                let mut inner = proc_macro2::TokenStream::new();

                for tree in stream {
                    convert(tree, endpoints, gating, endpoint, &mut inner);
                }

                output.extend(proc_macro2::TokenStream::from(TokenStream::from(
                    TokenTree::Group(Group::new(group.delimiter(), inner.into())),
                )));
            }
        }
    }
}
