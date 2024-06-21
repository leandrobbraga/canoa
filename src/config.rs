use std::collections::HashMap;
use std::{fs::File, io::Read};

pub struct Config {
    pub user: String,
    pub token: String,
}

/// Extract the configuration struct from the environment variables or the `.env` file, giving
/// precedente to the environment variables.
pub fn configuration() -> Result<Config, ()> {
    match (std::env::var("JIRA_USER"), std::env::var("JIRA_TOKEN)")) {
        (Ok(user), Ok(token)) => Ok(Config { user, token }),
        (Ok(user), Err(_)) => {
            let mut variables = parse_dotenv()?;

            let token = variables.remove("JIRA_TOKEN").ok_or(())?;

            Ok(Config { user, token })
        }
        (Err(_), Ok(token)) => {
            let mut variables = parse_dotenv()?;

            let user = variables.remove("JIRA_USER").ok_or(())?;

            Ok(Config { user, token })
        }
        (Err(_), Err(_)) => {
            let mut variables = parse_dotenv()?;

            let user = variables.remove("JIRA_USER").ok_or(())?;
            let token = variables.remove("JIRA_TOKEN").ok_or(())?;

            Ok(Config { user, token })
        }
    }
}

fn parse_dotenv() -> Result<HashMap<String, String>, ()> {
    let mut variables = HashMap::with_capacity(2);
    let mut content = Vec::new();
    let config_filepath = "./.env";

    File::open(config_filepath)
        .and_then(|mut file| file.read_to_end(&mut content))
        .map_err(|err| eprintln!("ERROR: could not read file {config_filepath}: {err}"))?;

    let mut view = content.as_slice();

    while !view.is_empty() {
        let (variable, value) = parse_variable(&mut view)?;
        variables.insert(variable, value);
    }

    Ok(variables)
}

fn parse_variable(content: &mut &[u8]) -> Result<(String, String), ()> {
    if content.is_empty() {
        eprintln!("ERROR: the config file does not contain the user and token.");
        return Err(());
    }

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
