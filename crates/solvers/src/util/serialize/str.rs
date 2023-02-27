use {serde::Serializer, serde_with::SerializeAs};

/// Serializes a slice of strings as a comma-separated list.
pub struct CommaSeparated;

impl SerializeAs<Vec<String>> for CommaSeparated {
    fn serialize_as<S: Serializer>(source: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&source.join(","))
    }
}
