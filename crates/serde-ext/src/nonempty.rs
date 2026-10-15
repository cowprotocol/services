use {
    serde::{Deserialize, Deserializer},
    std::{collections::HashSet, hash::Hash},
};

/// Deserializes a sequence into a [`Vec`], erroring if it is empty.
pub fn deserialize_nonempty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let vec = Vec::<T>::deserialize(deserializer)?;
    if vec.is_empty() {
        return Err(serde::de::Error::custom("expected at least one element"));
    }
    Ok(vec)
}

/// Deserializes a sequence into a [`Vec`], erroring if it is empty or if it
/// contains duplicate elements.
pub fn deserialize_nonempty_unique_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Eq + Hash,
{
    let vec = deserialize_nonempty_vec(deserializer)?;
    let mut seen = HashSet::with_capacity(vec.len());
    for item in &vec {
        if !seen.insert(item) {
            return Err(serde::de::Error::custom("duplicate element"));
        }
    }
    Ok(vec)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::de::value::{Error, SeqDeserializer},
    };

    #[test]
    fn deserializes_nonempty_sequence() {
        let seq = SeqDeserializer::<_, Error>::new(vec![1u32, 2, 3].into_iter());
        let result: Vec<u32> = deserialize_nonempty_vec::<_, u32>(seq).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn rejects_empty_sequence() {
        let seq = SeqDeserializer::<_, Error>::new(Vec::<u32>::new().into_iter());
        let err: Error = deserialize_nonempty_vec::<_, u32>(seq).unwrap_err();
        assert!(err.to_string().contains("expected at least one element"));
    }

    #[test]
    fn deserializes_nonempty_unique_sequence() {
        let seq = SeqDeserializer::<_, Error>::new(vec![1u32, 2, 3].into_iter());
        let result: Vec<u32> = deserialize_nonempty_unique_vec::<_, u32>(seq).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn rejects_duplicate_sequence() {
        let seq = SeqDeserializer::<_, Error>::new(vec![1u32, 2, 1].into_iter());
        let err: Error = deserialize_nonempty_unique_vec::<_, u32>(seq).unwrap_err();
        assert!(err.to_string().contains("duplicate element"));
    }
}
