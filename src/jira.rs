//! Jira's API implementation
use std::iter;

const BASE64TABLE: [u8; 64] = [
    b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N', b'O', b'P',
    b'Q', b'R', b'S', b'T', b'U', b'V', b'W', b'X', b'Y', b'Z', b'a', b'b', b'c', b'd', b'e', b'f',
    b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v',
    b'w', b'x', b'y', b'z', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'+', b'/',
];

/// The basic authentication is a method for an HTTP user agent to provide an username and
/// a password when making a request. To perform it the agent should include a header in the
/// form of Authorization: Basic <credentials>, where <credentials> is the Base64 encoding of
/// '<user>:<password>'.
fn basic_authentication_header(user: &str, token: &str) -> String {
    // The output length is calculated by the 'Basic ' prefix size (6 chars) added to the encoded
    // credentials, which yields 4 characters for every triplet (including incomplete triplets)
    // after encoded in Base64.
    let output_lenght = 10 + 4 * ((user.len() + token.len()) / 3);
    let mut header = Vec::with_capacity(output_lenght);
    header.extend_from_slice(b"Basic ");

    let input_lenght = user.len() + token.len() + 1;
    let chunk_size = 3;
    let chunk_count = input_lenght / chunk_size;

    let mut iterator = user.bytes().chain(":".bytes()).chain(token.bytes());

    for _ in 0..chunk_count {
        // SAFETY: We pre-calculated that this iterator have at least this amount of elements or
        // more
        let n = unsafe {
            (iterator.next().unwrap_unchecked() as usize) << 16
                | (iterator.next().unwrap_unchecked() as usize) << 8
                | (iterator.next().unwrap_unchecked() as usize)
        };

        header.extend([
            BASE64TABLE[(n >> 18) & 63],
            BASE64TABLE[(n >> 12) & 63],
            BASE64TABLE[(n >> 6) & 63],
            BASE64TABLE[n & 63],
        ]);
    }

    // Remaining, if it exists
    let mut n = 0;
    for (index, byte) in iterator.enumerate() {
        n |= (byte as usize) << (16 - 8 * (index % chunk_size));
    }

    for index in 0..input_lenght - chunk_count * chunk_size + 1 {
        header.push(BASE64TABLE[n >> (6 * (chunk_size - index)) & 63]);
    }

    // Padding to fill the end
    header.extend(iter::repeat(b'=').take(header.capacity() - header.len()));

    // SAFETY: The header is made of two parts: the 'Basic ' prefix and the Base64 encoded string,
    // both are UTF-8
    unsafe { String::from_utf8_unchecked(header) }
}

#[cfg(test)]
mod test {
    use super::basic_authentication_header;

    #[test]
    fn encode_test() {
        let result = basic_authentication_header("user", "password");
        assert_eq!(result.as_str(), "Basic dXNlcjpwYXNzd29yZA==")
    }

    #[test]
    fn encode_longer() {
        let result = basic_authentication_header("user", "difficult password");
        assert_eq!(result.as_str(), "Basic dXNlcjpkaWZmaWN1bHQgcGFzc3dvcmQ=")
    }

    #[test]
    fn encode_strange() {
        let result = basic_authentication_header("user", "$7r4n/ge$741ng");
        assert_eq!(result.as_str(), "Basic dXNlcjokN3I0bi9nZSQ3NDFuZw==")
    }
}
