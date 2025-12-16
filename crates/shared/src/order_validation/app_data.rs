//! App data parsing and validation
//!
//! This module is responsible for parsing and validating order app data.
//! It's a **first-class operation** that runs before strategy selection and
//! before any checks, ensuring app data is available to all subsequent validations.
//!
//! # Why app_data is special
//!
//! App data parsing is not a regular check because:
//! - It must happen **once, upfront**, before any strategy selection
//! - It's a **prerequisite** for strategy selection (wrappers are in app_data)
//! - All strategies and checks receive **pre-parsed app_data**
//! - It validates the shape/format, not behavioral properties
//!
//! # Flow
//!
//! ```text
//! OrderCreation
//!     ↓
//! parse_app_data() → OrderAppData [only operation before strategy selection]
//!     ↓
//! Select strategy based on wrappers and signature
//!     ↓
//! Pass pre-parsed OrderAppData to strategy via ValidationContext
//! ```

use {
    crate::order_validation::types::*,
    anyhow::anyhow,
    app_data::{Validator, ValidatedAppData},
    model::order::{OrderCreationAppData, Interactions},
    model::interaction::InteractionData,
    contracts::alloy::HooksTrampoline,
};

/// Parses and validates app data from the order creation request
///
/// This is a **synchronous, first-class operation** that happens before
/// strategy selection. It must succeed before any other validation can proceed.
///
/// # Arguments
/// - `app_data`: The app data to validate (hash, full, or both)
/// - `full_app_data_override`: Optional full app data to use if only hash provided
/// - `validator`: App data validator from the shared module
/// - `hooks`: HooksTrampoline instance for rendering interactions
///
/// # Returns
/// - `Ok(OrderAppData)` - Successfully parsed and validated app data
/// - `Err(ValidationError)` - App data is malformed or invalid
///
/// # Errors
/// - Mismatch between provided app data hash and actual data hash
/// - Invalid JSON format
/// - Invalid interaction specifications
pub fn parse_app_data(
    app_data: &OrderCreationAppData,
    full_app_data_override: &Option<String>,
    validator: &Validator,
    hooks: &HooksTrampoline::Instance,
) -> Result<OrderAppData, ValidationError> {
    let validate = |app_data: &str| -> Result<ValidatedAppData, ValidationError> {
        let app_data = validator
            .validate(app_data.as_bytes())
            .map_err(|err| ValidationError::AppData(AppDataValidationError::Invalid(err)))?;
        Ok(app_data)
    };

    let validated_app_data = match app_data {
        OrderCreationAppData::Both { full, expected } => {
            let validated = validate(full)?;
            if validated.hash != *expected {
                return Err(ValidationError::AppData(
                    AppDataValidationError::Mismatch {
                        provided: *expected,
                        actual: validated.hash,
                    },
                ));
            }
            validated
        }
        OrderCreationAppData::Hash { hash } => {
            // Use the override if provided, otherwise error
            let protocol = if let Some(full) = full_app_data_override {
                validate(full)?.protocol
            } else {
                return Err(ValidationError::AppData(
                    AppDataValidationError::Invalid(anyhow!(
                        "Unknown pre-image for app data hash {:?}",
                        hash,
                    )),
                ));
            };

            ValidatedAppData {
                hash: *hash,
                document: String::new(),
                protocol,
            }
        }
        OrderCreationAppData::Full { full } => validate(full)?,
    };

    // Convert hooks to interactions
    let interactions = custom_interactions(&validated_app_data.protocol.hooks, hooks);

    Ok(OrderAppData {
        inner: validated_app_data,
        interactions,
    })
}

/// Converts app data hooks into pre/post settlement interactions
fn custom_interactions(
    hooks: &app_data::Hooks,
    hooks_trampoline: &HooksTrampoline::Instance,
) -> Interactions {
    let mut pre_interactions = Vec::new();
    let mut post_interactions = Vec::new();

    // Pre-hooks
    if !hooks.pre.is_empty() {
        let data = hooks_trampoline
            .encode_execute_with(hooks.pre.iter().cloned())
            .unwrap_or_default();
        pre_interactions.push(InteractionData {
            target: hooks_trampoline.address(),
            value: 0.into(),
            call_data: data,
        });
    }

    // Post-hooks
    if !hooks.post.is_empty() {
        let data = hooks_trampoline
            .encode_execute_with(hooks.post.iter().cloned())
            .unwrap_or_default();
        post_interactions.push(InteractionData {
            target: hooks_trampoline.address(),
            value: 0.into(),
            call_data: data,
        });
    }

    Interactions {
        pre: pre_interactions,
        post: post_interactions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_parsing_stub() {
        // Tests will be implemented when full integration is ready
    }
}
