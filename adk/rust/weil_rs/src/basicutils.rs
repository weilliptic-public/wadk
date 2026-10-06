static HYPHEN: char = '-';
static COLON: char = ':';
static FORWARD_SLASH: char = '/';

static INVALID_KEY_CHARS: [char; 2] = [COLON, FORWARD_SLASH];

pub const INVALID_KEY_ERROR: &str =
    "invalid key - contains reserved characters. Must not contain : or /";
    
// checks if the key is valid (does not contain invalid characters)
// this is to safeguard against potential parsing issues downstream 
pub fn is_valid_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| !INVALID_KEY_CHARS.contains(&c))
}
