use {
    ethcontract::{transaction::TransactionBuilder, Bytes},
    model::interaction::InteractionData,
    primitive_types::{H160, U256},
    web3::Transport,
};

pub trait Interaction: std::fmt::Debug + Send + Sync {
    // TODO: not sure if this should return a result.
    // Write::write returns a result but we know we write to a vector in memory so
    // we know it will never fail. Then the question becomes whether
    // interactions should be allowed to fail encoding for other reasons.
    fn encode(&self) -> EncodedInteraction;
}

impl Interaction for Box<dyn Interaction> {
    fn encode(&self) -> EncodedInteraction {
        self.as_ref().encode()
    }
}

pub type EncodedInteraction = (
    H160,           // target
    U256,           // value
    Bytes<Vec<u8>>, // callData
);

impl Interaction for EncodedInteraction {
    fn encode(&self) -> EncodedInteraction {
        self.clone()
    }
}

impl Interaction for InteractionData {
    fn encode(&self) -> EncodedInteraction {
        (self.target, *self.value, Bytes(self.call_data.clone()))
    }
}

pub fn for_transaction<T>(tx: TransactionBuilder<T>) -> InteractionData
where
    T: Transport,
{
    InteractionData {
        target: tx.to.unwrap(),
        value: tx.value.unwrap_or_default().into(),
        call_data: tx.data.unwrap().0,
    }
}
