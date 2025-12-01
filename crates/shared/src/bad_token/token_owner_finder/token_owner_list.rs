use {
    super::TokenOwnerProposing,
    alloy::primitives::Address,
    anyhow::Result,
    std::collections::HashMap,
};

type Token = Address;
type Owner = Address;

pub struct TokenOwnerList {
    owners: HashMap<Token, Vec<Owner>>,
}

impl TokenOwnerList {
    pub fn new(owners: HashMap<Token, Vec<Owner>>) -> Self {
        Self { owners }
    }
}

#[async_trait::async_trait]
impl TokenOwnerProposing for TokenOwnerList {
    async fn find_candidate_owners(&self, token: Address) -> Result<Vec<Owner>> {
        Ok(self.owners.get(&token).cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn token_owner_list_constructor_empty() {
        let finder = TokenOwnerList::new(Default::default());
        let candidate_owners = finder
            .find_candidate_owners(Address::with_last_byte(10))
            .await;
        assert!(candidate_owners.unwrap().is_empty());
    }

    #[tokio::test]
    async fn token_owner_list_constructor() {
        let token = Address::with_last_byte(1);
        let owners = vec![Address::with_last_byte(2), Address::with_last_byte(3)];
        let finder = TokenOwnerList::new(HashMap::from([(token, owners.clone())]));
        let candidate_owners = finder
            .find_candidate_owners(Address::with_last_byte(1))
            .await
            .unwrap();
        assert_eq!(owners, candidate_owners);
    }
}
