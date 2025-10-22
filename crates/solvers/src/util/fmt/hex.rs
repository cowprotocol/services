use std::fmt::{self, Debug, Display, Formatter};

pub struct Hex<'a>(pub &'a [u8]);

impl Debug for Hex<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl Display for Hex<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(&const_hex::encode_prefixed(self.0))
    }
}
