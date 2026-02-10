//! Procedural macros for a2a-rs stations and guards.
//!
//! This crate provides compile-time code generation for the CONSTRUCT system:
//!
//! - `#[station]` - Auto-implements Station trait from struct + methods
//! - `#[guard]` - Auto-generates Guard trait from predicate functions
//!
//! # Design Philosophy
//!
//! The macros reduce boilerplate while preserving type safety and determinism.
//! They follow the CONSTRUCT principle: domain logic remains hand-written,
//! structural code is generated.
//!
//! # Example: Station Macro
//!
//! ```ignore
//! use a2a_rs_macros::station;
//! use a2a_rs::construct::ontology::OntologyState;
//! use a2a_rs::construct::station::{RefusalReceipt, Station};
//!
//! #[station(method = "custom/operation")]
//! struct CustomStation {
//!     config: String,
//! }
//!
//! impl CustomStation {
//!     fn admit(ontology: &OntologyState, input: &CustomInput) -> Result<(), RefusalReceipt> {
//!         if input.value.is_empty() {
//!             return Err(RefusalReceipt::new(-32602, "Value required".to_string()));
//!         }
//!         Ok(())
//!     }
//!
//!     fn step(
//!         &mut self,
//!         ontology: &mut OntologyState,
//!         input: CustomInput,
//!     ) -> Result<CustomOutput, RefusalReceipt> {
//!         // Implementation here
//!         Ok(CustomOutput { result: input.value })
//!     }
//! }
//! ```
//!
//! # Example: Guard Macro
//!
//! ```ignore
//! use a2a_rs_macros::guard;
//! use a2a_rs::construct::guards::{Guard, RefusalReceipt, RefusalCode};
//!
//! #[guard(name = "NonEmptyString")]
//! fn check_non_empty(input: &serde_json::Value) -> Result<(), String> {
//!     match input.as_str() {
//!         Some(s) if !s.is_empty() => Ok(()),
//!         _ => Err("String must not be empty".to_string()),
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, ExprLit, ItemFn, ItemStruct, Lit, Meta, MetaNameValue, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Auto-implement Station trait for a struct.
///
/// # Attributes
///
/// - `method` (required) - The JSON-RPC method name (e.g., "custom/operation")
///
/// # Requirements
///
/// The struct must implement two methods:
///
/// ```ignore
/// fn admit(ontology: &OntologyState, input: &Input) -> Result<(), RefusalReceipt>
/// fn step(&mut self, ontology: &mut OntologyState, input: Input) -> Result<Output, RefusalReceipt>
/// ```
///
/// The macro will extract Input and Output types from these signatures.
///
/// # Generated Code
///
/// Implements the `Station` trait by delegating to the user-provided methods.
///
/// # Example
///
/// ```ignore
/// #[station(method = "custom/greet")]
/// struct GreetStation;
///
/// impl GreetStation {
///     fn admit(ontology: &OntologyState, input: &GreetInput) -> Result<(), RefusalReceipt> {
///         if input.name.is_empty() {
///             return Err(RefusalReceipt::new(-32602, "Name required".to_string()));
///         }
///         Ok(())
///     }
///
///     fn step(
///         &mut self,
///         ontology: &mut OntologyState,
///         input: GreetInput,
///     ) -> Result<GreetOutput, RefusalReceipt> {
///         Ok(GreetOutput {
///             greeting: format!("Hello, {}!", input.name),
///         })
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn station(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as StationArgs);
    let input_struct = parse_macro_input!(input as ItemStruct);

    let _method_name = args.method; // Reserved for future use (method routing)
    let struct_name = &input_struct.ident;

    // Generate Station implementation
    let expanded = quote! {
        #input_struct

        impl ::a2a_rs::construct::station::Station for #struct_name {
            type Input = <Self as StationImpl>::Input;
            type Output = <Self as StationImpl>::Output;

            fn admit(
                ontology: &::a2a_rs::construct::ontology::OntologyState,
                input: &Self::Input,
            ) -> Result<(), ::a2a_rs::construct::station::RefusalReceipt> {
                <Self as StationImpl>::admit(ontology, input)
            }

            fn step(
                &mut self,
                ontology: &mut ::a2a_rs::construct::ontology::OntologyState,
                input: Self::Input,
            ) -> Result<Self::Output, ::a2a_rs::construct::station::RefusalReceipt> {
                <Self as StationImpl>::step(self, ontology, input)
            }
        }

        // Helper trait to extract Input/Output types from user-provided methods
        trait StationImpl {
            type Input;
            type Output;

            fn admit(
                ontology: &::a2a_rs::construct::ontology::OntologyState,
                input: &Self::Input,
            ) -> Result<(), ::a2a_rs::construct::station::RefusalReceipt>;

            fn step(
                &mut self,
                ontology: &mut ::a2a_rs::construct::ontology::OntologyState,
                input: Self::Input,
            ) -> Result<Self::Output, ::a2a_rs::construct::station::RefusalReceipt>;
        }
    };

    TokenStream::from(expanded)
}

/// Auto-generate Guard trait implementation from a predicate function.
///
/// # Attributes
///
/// - `name` (required) - The guard name for audit trails
/// - `code` (optional) - The RefusalCode to use (defaults to PreconditionViolation)
///
/// # Requirements
///
/// The function must have signature:
///
/// ```ignore
/// fn check_something(input: &serde_json::Value) -> Result<(), String>
/// ```
///
/// The error string is used as the refusal reason.
///
/// # Generated Code
///
/// Creates a struct with the guard name and implements the `Guard` trait.
///
/// # Example
///
/// ```ignore
/// #[guard(name = "PositiveNumber", code = "ValueOutOfRange")]
/// fn check_positive(input: &serde_json::Value) -> Result<(), String> {
///     match input.as_f64() {
///         Some(n) if n > 0.0 => Ok(()),
///         _ => Err("Number must be positive".to_string()),
///     }
/// }
///
/// // Usage:
/// let guard = PositiveNumber;
/// guard.check(&serde_json::json!(42), "value", 1)?;
/// ```
#[proc_macro_attribute]
pub fn guard(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as GuardArgs);
    let input_fn = parse_macro_input!(input as ItemFn);

    let guard_name_str = args.name;
    let guard_code = args
        .code
        .unwrap_or_else(|| "PreconditionViolation".to_string());

    let guard_name = format_ident!("{}", guard_name_str);
    let fn_name = &input_fn.sig.ident;

    // Generate Guard implementation
    let expanded = quote! {
        // Keep the original function
        #input_fn

        /// Auto-generated guard struct
        #[derive(Debug, Clone)]
        pub struct #guard_name;

        impl ::a2a_rs::construct::guards::Guard for #guard_name {
            fn check(
                &self,
                input: &serde_json::Value,
                context: &str,
                policy_epoch: u64,
            ) -> Result<(), ::a2a_rs::construct::guards::RefusalReceipt> {
                match #fn_name(input) {
                    Ok(()) => Ok(()),
                    Err(reason) => {
                        let code = ::a2a_rs::construct::guards::RefusalCode::#guard_code;
                        let input_hash = format!("hash-{:x}", {
                            let json_str = serde_json::to_string(input)
                                .unwrap_or_else(|_| "null".to_string());
                            // Simple FNV-1a hash
                            let mut hash: u64 = 0xcbf29ce484222325;
                            for byte in json_str.bytes() {
                                hash ^= byte as u64;
                                hash = hash.wrapping_mul(0x100000001b3);
                            }
                            hash
                        });

                        Err(::a2a_rs::construct::guards::RefusalReceipt::new(
                            code,
                            #guard_name_str.to_string(),
                            input_hash,
                            policy_epoch,
                            reason,
                        ))
                    }
                }
            }

            fn name(&self) -> &str {
                #guard_name_str
            }

            fn description(&self) -> String {
                format!("Guard: {}", #guard_name_str)
            }
        }
    };

    TokenStream::from(expanded)
}

// ==================== Helper Types for Parsing ====================

/// Arguments for #[station(...)]
struct StationArgs {
    method: String,
}

impl Parse for StationArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let meta = input.parse::<Meta>()?;

        if let Meta::NameValue(MetaNameValue {
            ref path,
            ref value,
            ..
        }) = meta
        {
            if path.is_ident("method") {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit_str),
                    ..
                }) = value
                {
                    return Ok(StationArgs {
                        method: lit_str.value(),
                    });
                }
            }
        }

        Err(syn::Error::new_spanned(
            meta,
            "Expected #[station(method = \"...\")]",
        ))
    }
}

/// Arguments for #[guard(...)]
struct GuardArgs {
    name: String,
    code: Option<String>,
}

impl Parse for GuardArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        let mut name = None;
        let mut code = None;

        for meta in metas {
            if let Meta::NameValue(MetaNameValue { path, value, .. }) = meta {
                if path.is_ident("name") {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(lit_str),
                        ..
                    }) = value
                    {
                        name = Some(lit_str.value());
                    }
                } else if path.is_ident("code") {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(lit_str),
                        ..
                    }) = value
                    {
                        code = Some(lit_str.value());
                    }
                }
            }
        }

        let name = name.ok_or_else(|| {
            syn::Error::new(input.span(), "Expected #[guard(name = \"...\")] attribute")
        })?;

        Ok(GuardArgs { name, code })
    }
}

#[cfg(test)]
mod tests {
    // Tests for proc macros must be in integration tests
    // because proc macros can't be tested in their own crate
}
