//! Jira's API implementation
use std::iter;

use serde::Deserialize;

pub struct Jira {
    authorization: Box<str>,
    host: Box<str>,
}

#[derive(Deserialize, Debug)]
struct Sprint {
    id: u32,
    name: String,
}

#[derive(Deserialize, Debug)]
struct Issue {
    #[serde(rename(deserialize = "key"))]
    name: String,
    fields: IssueFields,
}

#[derive(Deserialize, Debug)]
struct IssueFields {
    summary: String,
    #[serde(
        rename(deserialize = "issuetype"),
        deserialize_with = "deserialize_type"
    )]
    kind: String,
    #[serde(deserialize_with = "deserialize_assigne")]
    assignee: Option<String>,
    #[serde(deserialize_with = "deserialize_status")]
    status: Option<String>,
}

fn deserialize_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Outer {
        name: String,
    }

    Outer::deserialize(deserializer).map(|o| o.name)
}

fn deserialize_assigne<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Outer {
        #[serde(rename(deserialize = "displayName"))]
        display_name: String,
    }

    Option::<Outer>::deserialize(deserializer).map(|o| match o {
        Some(v) => Some(v.display_name),
        None => None,
    })
}

fn deserialize_status<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Outer {
        name: String,
    }

    Option::<Outer>::deserialize(deserializer).map(|o| match o {
        Some(v) => Some(v.name),
        None => None,
    })
}

impl Jira {
    pub fn new(user: &str, token: &str, host: Box<str>) -> Self {
        Self {
            authorization: basic_authentication_header(user, token),
            host,
        }
    }
    pub fn get_sprint_issues(&self, board_id: &str, sprint_id: &str) {
        #[derive(Deserialize)]
        struct Response {
            issues: Vec<Issue>,
        }

        let a: Response = ureq::get(&format!(
            "{}rest/agile/1.0/board/{board_id}/sprint/{sprint_id}/issue",
            self.host.as_ref()
        ))
        .set("Authorization", &self.authorization.as_ref())
        .query("fields", "summary, status, labels, assignee, issuetype")
        .call()
        .unwrap()
        .into_json()
        .unwrap();

        println!("{:?}", a.issues);
    }

    pub fn get_board_active_sprints(&self, board_id: &str) {
        #[derive(Deserialize)]
        struct Response {
            values: Vec<Sprint>,
        }

        let a: Response = ureq::get(&format!(
            "{}rest/agile/1.0/board/{board_id}/sprint",
            self.host.as_ref()
        ))
        .set("Authorization", &self.authorization.as_ref())
        .query("state", "active")
        .call()
        .unwrap()
        .into_json()
        .unwrap();

        println!("{:?}", a.values);
    }
}

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
fn basic_authentication_header(user: &str, token: &str) -> Box<str> {
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
    if input_lenght % 3 != 0 {
        let mut n = 0;
        let mut index = 0;
        for byte in iterator {
            n |= (byte as usize) << (16 - 8 * (index % chunk_size));
            index += 1;
        }

        for index in 0..index + 1 {
            header.push(BASE64TABLE[n >> (6 * (chunk_size - index)) & 63]);
        }

        // Padding to fill the end
        header.extend(iter::repeat(b'=').take(output_lenght - header.len()));
    }

    // SAFETY: The header is made of two parts: the 'Basic ' prefix and the Base64 encoded string,
    // both are UTF-8
    unsafe { String::from_utf8_unchecked(header).into_boxed_str() }
}

#[cfg(test)]
mod test {
    use super::basic_authentication_header;

    #[test]
    fn encode_test() {
        let result = basic_authentication_header("user", "password");
        assert_eq!(result.as_ref(), "Basic dXNlcjpwYXNzd29yZA==")
    }

    #[test]
    fn encode_longer() {
        let result = basic_authentication_header("user", "difficult password");
        assert_eq!(result.as_ref(), "Basic dXNlcjpkaWZmaWN1bHQgcGFzc3dvcmQ=")
    }

    #[test]
    fn encode_strange() {
        let result = basic_authentication_header("user", "$7r4n/ge$741ng");
        assert_eq!(result.as_ref(), "Basic dXNlcjokN3I0bi9nZSQ3NDFuZw==")
    }
}
