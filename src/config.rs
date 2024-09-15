use std::collections::HashMap;
use std::{fs::File, io::Read};

const CONFIG_FILEPATH: &str = ".env";

pub struct Config {
    pub user: Box<str>,
    pub token: Box<str>,
    pub board_id: Box<str>,
    pub host: Box<str>,
}

/// Extract the configuration struct from the environment variables or the `.env` file, giving
/// precedente to the environment variables.
pub fn configuration() -> Result<Config, ()> {
    let mut variables = parse_dotenv()?;

    // Give precedence to the environment variables
    let mut get_variable = |key: &str| {
        std::env::var(key)
            .or_else(|_| variables.remove(key).ok_or(()))
            .map(|value| value.into_boxed_str())
            .map_err(|_| eprintln!("ERROR: Missing variable {key}"))
    };

    let user = get_variable("JIRA_USER")?;
    let token = get_variable("JIRA_TOKEN")?;
    let board_id = get_variable("JIRA_BOARD_ID")?;
    let host = get_variable("JIRA_HOST")?;

    Ok(Config {
        user,
        token,
        board_id,
        host,
    })
}

fn parse_dotenv() -> Result<HashMap<String, String>, ()> {
    let mut variables = HashMap::with_capacity(2);
    let mut content = Vec::new();

    File::open(CONFIG_FILEPATH)
        .and_then(|mut file| file.read_to_end(&mut content))
        .map_err(|err| eprintln!("ERROR: could not read file {CONFIG_FILEPATH}: {err}"))?;

    let mut view = content.as_slice();

    while !view.is_empty() {
        let (variable, value) = parse_variable(&mut view)?;
        variables.insert(variable, value);
        trim_left_whitespaces(&mut view);
    }

    Ok(variables)
}

fn parse_variable(content: &mut &[u8]) -> Result<(String, String), ()> {
    let mut variable_index = 0;
    while variable_index < content.len() && content[variable_index] != b'=' {
        variable_index += 1;
    }

    let mut value_index = variable_index + 1;
    while value_index < content.len() && !content[value_index].is_ascii_whitespace() {
        value_index += 1;
    }

    let variable = String::from_utf8(content[0..variable_index].to_vec())
        .map_err(|err| eprintln!("ERROR: the content is not utf8 encoded: {err}"))?;
    let value = String::from_utf8(content[variable_index + 1..value_index].to_vec())
        .map_err(|err| eprintln!("ERROR: the content is not utf8 encoded: {err}"))?;

    *content = &content[value_index + 1..];

    Ok((variable, value))
}

fn trim_left_whitespaces(content: &mut &[u8]) {
    let mut index = 0;
    while index < content.len() && content[index].is_ascii_whitespace() {
        index += 1;
    }

    *content = &content[index..]
}
