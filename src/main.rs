use std::io::Read;
use std::{fs, io};

use anyhow::{Context, Result};
use clap::{crate_version, App, AppSettings, Arg, ArgSettings, SubCommand};
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::reqwest::http_client;
use oauth2::{
    AccessToken, AuthUrl, ClientId, ResourceOwnerPassword, ResourceOwnerUsername, TokenResponse,
    TokenUrl,
};
use reqwest::blocking::Response;
use reqwest::Url;
use serde_json::{json, Value};
use uuid::Uuid;

const NAME: &str = "sn";
const VERSION: &str = crate_version!();
const BASE_URL: &str = "https://api.supernotes.app/v1/";

fn main() -> Result<()> {
    let matches = App::new(NAME)
        .version(VERSION)
        .about("SuperNotes client")
        .setting(AppSettings::SubcommandRequired)
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::GlobalVersion)
        .arg(
            Arg::with_name("username")
                .short("u")
                .long("username")
                .value_name("USERNAME")
                .env("SN_USERNAME")
                .takes_value(true)
                .required(true)
                .help("The username to login with"),
        )
        .arg(
            Arg::with_name("password")
                .short("p")
                .long("password")
                .value_name("PASSWORD")
                .env("SN_PASSWORD")
                .set(ArgSettings::HideEnvValues)
                .takes_value(true)
                .required(true)
                .help("The password"),
        )
        .arg(
            Arg::with_name("base_url")
                .long("base-url")
                .value_name("URL")
                .env("SN_BASE_URL")
                .default_value(BASE_URL)
                .help("The API base URL"),
        )
        .subcommand(
            SubCommand::with_name("create")
                .alias("c")
                .about("Creates a new note [short: c]")
                .arg(
                    Arg::with_name("name")
                        .value_name("NAME")
                        .required(true)
                        .help("The name of the new note"),
                )
                .arg(
                    Arg::with_name("file")
                        .value_name("FILE")
                        .help("File containing the Markdown content; omit for stdin"),
                ),
        )
        .get_matches();

    let username = matches.value_of("username").unwrap();
    let password = matches.value_of("password").unwrap();
    let mut base_url = matches.value_of("base_url").unwrap().to_string();
    if !base_url.ends_with('/') {
        base_url = format!("{}/", base_url);
    }

    let token_response = get_token(&base_url, username, password)?;
    let token = token_response.access_token();

    match matches.subcommand() {
        ("create", Some(submatches)) => {
            let name = submatches.value_of("name").unwrap();
            let file = submatches.value_of("file");
            create(&base_url, token, name, file).context("error creating card")
        }
        _ => unreachable!(),
    }
}

/// Get an access token.
///
/// Example:
/// ```
/// let response = get_token("https://example.com", "username", "password")?;
/// let access_token = response.access_token();
/// ```
fn get_token(base_url: &str, username: &str, password: &str) -> Result<BasicTokenResponse> {
    let auth_url = Url::parse("http://127.0.0.1/unused").unwrap();
    let token_url = Url::parse(base_url)?.join("user/login")?;

    let client = BasicClient::new(
        ClientId::new(String::from("unused")),
        None,
        AuthUrl::from_url(auth_url),
        Some(TokenUrl::from_url(token_url)),
    );
    client
        .exchange_password(
            &ResourceOwnerUsername::new(String::from(username)),
            &ResourceOwnerPassword::new(String::from(password)),
        )
        .request(http_client)
        .context("error getting access token")
}

/// Command to create a new card.
///
/// Reads a file or stdin as Markdown content and create a new card from it.
///
/// Example:
/// ```
/// let token = AccessToken::new(String::from("secret"));
/// let result = create("https://example.com", &token, "card name", "card.md");
/// ```
fn create(base_url: &str, token: &AccessToken, name: &str, file: Option<&str>) -> Result<()> {
    let content = read_content(file)?;
    create_card(base_url, &token, name, &content)?;
    Ok(())
}

/// Create a new card.
///
/// Example:
/// ```
/// let token = AccessToken::new(String::from("secret"));
/// let result = create_card("https://example.com", &token, "card name", "card content");
/// ```
fn create_card(base_url: &str, token: &AccessToken, name: &str, markup: &str) -> Result<Response> {
    let client = reqwest::blocking::Client::new();
    let url = Url::parse(base_url)?.join("cards/")?;
    let data = card_data(name, markup);

    let response = client
        .post(url)
        .bearer_auth(token.secret())
        .json(&data)
        .send()?;
    Ok(response)
}

/// Return data suitable for creating a new card from.
///
/// Generates HTML from the Markdown content. Id's are generated automatically.
///
/// Example:
/// ```
/// let data = card_data("Card name", "Markdown content");
/// ```
fn card_data(name: &str, markup: &str) -> Value {
    json!({
        "id": Uuid::new_v4(),
        "card": {
            "id": Uuid::new_v4(),
            "name": name,
            "markup": markup,
            "html": &markdown::to_html(markup),
        },
    })
}

fn read_content(file: Option<&str>) -> Result<String> {
    match file {
        Some(f) => fs::read_to_string(f).context(format!("could not read {}", f)),
        None => {
            let mut content = String::new();
            io::stdin()
                .read_to_string(&mut content)
                .context("could not read stdin")?;
            Ok(content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, server_url, Matcher};

    #[test]
    fn test_get_token() -> Result<()> {
        let endpoint = mock("POST", "/user/login")
            .match_header("content-type", "application/x-www-form-urlencoded")
            .match_body("grant_type=password&username=username&password=password")
            .with_body(json!({"access_token": "token", "token_type": "bearer"}).to_string())
            .create();

        let token = get_token(&server_url(), "username", "password")?;
        assert_eq!(token.access_token().secret(), "token");

        endpoint.assert();
        Ok(())
    }

    #[test]
    fn test_create_card() -> Result<()> {
        let endpoint = mock("POST", "/cards/")
            .match_header("content-type", "application/json")
            .match_header("authorization", "Bearer secret")
            .match_body(Matcher::PartialJson(json!({
                "card": {
                    "name": "name",
                    "markup": "markup",
                }
            })))
            .create();

        let token = AccessToken::new(String::from("secret"));
        create_card(&server_url(), &token, "name", "markup")?;

        endpoint.assert();
        Ok(())
    }

    #[test]
    fn test_card_data() {
        let card = card_data("card name", "* item");
        assert!(card["id"].is_string());
        assert!(card["card"]["id"].is_string());
        assert_eq!(card["card"]["name"], "card name");
        assert_eq!(card["card"]["markup"], "* item");
        assert_eq!(card["card"]["html"], "<ul>\n<li>item</li>\n</ul>\n");
    }
}
