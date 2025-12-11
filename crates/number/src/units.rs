use alloy::primitives::{
    U256,
    utils::{ParseUnits, Unit, parse_units},
};

pub trait EthUnit: std::marker::Sized {
    /// Returns the current wei amount.
    fn wei(self) -> U256;

    /// Returns the current Mwei amount as wei (i.e. 1e6 wei).
    fn mwei(self) -> U256 {
        self.wei() * Unit::MWEI.wei()
    }

    /// Returns the current Gwei amount as wei (i.e. 1e9 wei).
    fn gwei(self) -> U256 {
        self.wei() * Unit::GWEI.wei()
    }

    /// Returns the current Eth amount as wei (i.e. 1e18 wei).
    fn eth(self) -> U256 {
        self.wei() * Unit::ETHER.wei()
    }
}

impl EthUnit for u64 {
    fn wei(self) -> U256 {
        U256::from(self)
    }
}

impl EthUnit for u128 {
    fn wei(self) -> U256 {
        U256::from(self)
    }
}

impl EthUnit for f64 {
    fn wei(self) -> U256 {
        match parse_units(&self.to_string(), "wei").unwrap() {
            ParseUnits::U256(val) => val,
            _ => panic!("could not parse number as u256: {self}"),
        }
    }

    fn mwei(self) -> U256 {
        match parse_units(&self.to_string(), "mwei").unwrap() {
            ParseUnits::U256(val) => val,
            _ => panic!("could not parse number as u256: {self}"),
        }
    }

    fn gwei(self) -> U256 {
        match parse_units(&self.to_string(), "gwei").unwrap() {
            ParseUnits::U256(val) => val,
            _ => panic!("could not parse number as u256: {self}"),
        }
    }

    fn eth(self) -> U256 {
        match parse_units(&self.to_string(), "ether").unwrap() {
            ParseUnits::U256(val) => val,
            _ => panic!("could not parse number as u256: {self}"),
        }
    }
}
