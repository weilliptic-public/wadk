static COLON: char = ':';
static FORWARD_SLASH: char = '/';
static ASTERISK: char = '*';
static TILDA: char = '~';

static INVALID_KEY_CHARS: [char; 4] = [COLON, FORWARD_SLASH, ASTERISK, TILDA];

pub const INVALID_KEY_ERROR: &str =
    "invalid key - contains reserved characters. Must not contain : ~ / *";
    
// checks if the key is valid (does not contain invalid characters)
// this is to safeguard against potential parsing issues downstream 
pub fn is_valid_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| !INVALID_KEY_CHARS.contains(&c))
}
