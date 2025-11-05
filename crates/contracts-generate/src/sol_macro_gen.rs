//! SolMacroGen implementation vendored from
//! <https://github.com/foundry-rs/foundry/blob/cc24b6b74978e72b7330ad7d4b39140e9ee33deb/crates/sol-macro-gen/src/sol_macro_gen.rs>
//! to avoid depending on forge-sol-macro-gen.

use {
    alloy_sol_macro_input::SolInput,
    anyhow::{Context, Result},
    proc_macro2::{Span, TokenStream},
    std::path::PathBuf,
};

pub struct SolMacroGen {
    pub path: PathBuf,
    pub name: String,
    pub expansion: Option<TokenStream>,
}

impl SolMacroGen {
    pub fn new(path: PathBuf, name: String) -> Self {
        Self {
            path,
            name,
            expansion: None,
        }
    }

    pub fn get_sol_input(&self) -> Result<SolInput> {
        let path = self.path.to_string_lossy().into_owned();
        let name = proc_macro2::Ident::new(&self.name, Span::call_site());
        let tokens = quote::quote! {
            #[sol(ignore_unlinked)]
            #name,
            #path
        };

        let sol_input: SolInput = syn::parse2(tokens).context("failed to parse input")?;

        Ok(sol_input)
    }
}
