use super::TokenOwnerProposing;
use anyhow::Result;
use ethcontract::H160;

pub struct TokenOwnerList {
    owners: Vec<H160>,
}

impl TokenOwnerList {
    pub fn new(owners: Vec<H160>) -> Self {
        Self { owners }
    }
}

#[async_trait::async_trait]
impl TokenOwnerProposing for TokenOwnerList {
    async fn find_candidate_owners(&self, _: H160) -> Result<Vec<H160>> {
        Ok(self.owners.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seasolver_finder_constructor_empty() {
        let finder = TokenOwnerList::new(vec![]);
        let candidate_owners = finder
            .find_candidate_owners(H160::from_low_u64_be(10))
            .await;
        assert!(candidate_owners.unwrap().is_empty());
    }

    #[tokio::test]
    async fn seasolver_finder_constructor() {
        let owners = vec![H160::from_low_u64_be(1), H160::from_low_u64_be(2)];
        let finder = TokenOwnerList::new(owners.clone());
        let candidate_owners = finder
            .find_candidate_owners(H160::from_low_u64_be(10))
            .await
            .unwrap();
        assert_eq!(owners, candidate_owners);
    }
}
