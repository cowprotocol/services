use {super::TokenOwnerProposing, anyhow::Result, ethcontract::H160, std::collections::HashMap};

type Token = H160;
type Owner = H160;

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
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<Owner>> {
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
            .find_candidate_owners(H160::from_low_u64_be(10))
            .await;
        assert!(candidate_owners.unwrap().is_empty());
    }

    #[tokio::test]
    async fn token_owner_list_constructor() {
        let token = H160::from_low_u64_be(1);
        let owners = vec![H160::from_low_u64_be(2), H160::from_low_u64_be(3)];
        let finder = TokenOwnerList::new(HashMap::from([(token, owners.clone())]));
        let candidate_owners = finder
            .find_candidate_owners(H160::from_low_u64_be(1))
            .await
            .unwrap();
        assert_eq!(owners, candidate_owners);
    }
}
