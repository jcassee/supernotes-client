use std::io::Read;
use std::{fs, io};

use anyhow::{anyhow, Context, Result};
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::reqwest::http_client;
use oauth2::{
    AccessToken, AuthUrl, ClientId, ResourceOwnerPassword, ResourceOwnerUsername, TokenResponse,
    TokenUrl,
};
use reqwest::blocking::Response;
use reqwest::Url;
use serde_json::{json, Value};
use structopt::clap::AppSettings;
use structopt::StructOpt;
use uuid::Uuid;

const NAME: &str = "sn";
const BASE_URL: &str = "https://api.supernotes.app/v1/";

#[derive(Debug, StructOpt)]
#[structopt(name = NAME, bin_name = NAME, about = "A Supernotes client")]
#[structopt(setting = AppSettings::VersionlessSubcommands)]
struct Opt {
    /// The username to login with
    #[structopt(short, long, value_name = "USERNAME", env = "SN_USERNAME")]
    username: String,

    /// The password
    #[structopt(
        short,
        long,
        value_name = "PASSWORD",
        env = "SN_PASSWORD",
        hide_env_values = true
    )]
    password: String,

    /// The API base URL
    #[structopt(
        long,
        value_name = "URL",
        env = "SN_BASE_URL",
        default_value = BASE_URL,
        parse(try_from_str = parse_abs_url)
    )]
    base_url: Url,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Creates a new Supernotes card
    #[structopt(alias = "c")]
    Create {
        /// The name of the new card
        #[structopt(value_name = "NAME")]
        name: String,

        /// File with the card body in Markdown  [default: stdin]
        #[structopt(value_name = "FILE")]
        file: Option<String>,
    },
}

fn parse_abs_url(src: &str) -> Result<Url> {
    let mut url = Url::parse(src)?;
    match url.scheme() {
        "http" | "https" => (),
        _ => return Err(anyhow!("scheme must be either http or https")),
    };
    if url.cannot_be_a_base() {
        return Err(anyhow!("not an absolute URL"));
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url));
    };
    Ok(url)
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    match opt.cmd {
        Command::Create { name, file } => create(
            &opt.base_url,
            &opt.username,
            &opt.password,
            &name,
            file.as_deref(),
        ),
    }
}

/// Get an access token.
///
/// Example:
/// ```
/// let response = get_token(Url::parse("https://example.com")?, "username", "password")?;
/// let access_token = response.access_token();
/// ```
fn get_token(base_url: &Url, username: &str, password: &str) -> Result<BasicTokenResponse> {
    let auth_url = Url::parse("http://127.0.0.1/unused")?;
    let token_url = base_url
        .join("user/login")
        .context(format!("invalid URL: {}", base_url))?;

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
/// let result = create(Url::parse("https://example.com")?, &token, "card name", "card.md");
/// ```
fn create(
    base_url: &Url,
    username: &str,
    password: &str,
    name: &str,
    file: Option<&str>,
) -> Result<()> {
    let content = read_content(file)?;

    let token_response = get_token(base_url, username, password)?;
    let token = token_response.access_token();

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
fn create_card(base_url: &Url, token: &AccessToken, name: &str, markup: &str) -> Result<Response> {
    let client = reqwest::blocking::Client::new();
    let url = base_url.join("cards/")?;
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
        Some(f) => fs::read_to_string(f).context(format!("could not read file: {}", f)),
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
    use mockito::{mock, server_url, Matcher};

    use super::*;

    #[test]
    fn test_get_token() -> Result<()> {
        let endpoint = mock("POST", "/user/login")
            .match_header("content-type", "application/x-www-form-urlencoded")
            .match_body("grant_type=password&username=username&password=password")
            .with_body(json!({"access_token": "token", "token_type": "bearer"}).to_string())
            .create();

        let token = get_token(&Url::parse(&server_url())?, "username", "password")?;
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
        create_card(&Url::parse(&server_url())?, &token, "name", "markup")?;

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
//[cfg(test)]
