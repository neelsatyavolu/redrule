//! Keys for read-only share links. A link's id is the SHA-256 of its owner key, so only the key's holder can publish,
//! change or remove it, and the service stores no key. Links made before owner keys have random ids.
use super::folders::{random_key, sha256_hex};

/// A new link's id and owner key.
pub fn new_share() -> (String, String) {
    let key = random_key();
    (share_id(&key), key)
}

/// The id of the link an owner key controls, as the service computes it.
pub fn share_id(owner_key: &str) -> String {
    sha256_hex(owner_key.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folders::valid_key;

    #[test]
    fn ids_are_the_sha256_of_the_owner_key() {
        // Node's sha256 of the same text, which sharing/api/share.js compares with the id.
        assert_eq!(share_id(&"c".repeat(64)), "52b6419d27bd7f547cee3b92f8c17a908b8a49601ecbec161e5030de1dfe9e0a");
    }

    #[test]
    fn new_shares_are_random_and_consistent() {
        let (id, key) = new_share();
        assert!(valid_key(&id) && valid_key(&key));
        assert_eq!(share_id(&key), id);
        assert_ne!(new_share().1, key);
    }
}
