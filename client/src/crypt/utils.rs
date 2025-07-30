use base64::DecodeError;

pub fn blake3_hash(input: &str) -> String {
    let output = blake3::hash(input.as_bytes());
    output.to_hex().to_string()
}
pub fn base64_encode(input: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, input)
}

pub fn base64_decode(input: &str) -> Result<Vec<u8>, DecodeError> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
}
