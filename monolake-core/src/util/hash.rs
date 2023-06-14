use sha2::{Digest, Sha256};

pub fn sha256(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::sha256;

    #[test]
    fn test_hash_with_sha256() {
        assert_eq!(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
            sha256("hello world")
        );
        assert_eq!(
            "8a5edab282632443219e051e4ade2d1d5bbc671c781051bf1437897cbdfea0f1",
            sha256("/")
        );
        assert_eq!(
            "439b41782a6650352640cb3ab790a1151d23dd093f4f49577799c6b67f8d195c",
            sha256("/ping")
        );
    }
}
