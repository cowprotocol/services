use std::fmt::{self, Debug, Display, Formatter};

pub struct Hex<'a>(pub &'a [u8]);

impl Debug for Hex<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl Display for Hex<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str("0x")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
