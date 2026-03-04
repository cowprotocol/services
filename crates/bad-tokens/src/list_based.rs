use {
    alloy::primitives::Address,
    std::{collections::HashSet, sync::Arc},
};

/// Explicitly deny listed tokens.
#[derive(Default, Clone)]
pub struct DenyListedTokens(Arc<Inner>);

#[derive(Default)]
struct Inner {
    deny_list: HashSet<Address>,
}

impl DenyListedTokens {
    pub fn new(deny_list: Vec<Address>) -> Self {
        let deny_list = deny_list.into_iter().collect();
        Self(Arc::new(Inner { deny_list }))
    }
}

impl DenyListedTokens {
    pub fn contains(&self, token: &Address) -> bool {
        self.0.deny_list.contains(token)
    }
}
