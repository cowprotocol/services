use {super::Quality, crate::domain::eth};

#[derive(Default)]
pub struct Detector;

impl Detector {
    pub fn get_quality(&self, _token: eth::TokenAddress) -> Option<Quality> {
        None
    }
}
