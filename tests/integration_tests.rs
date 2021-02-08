use anyhow::Result;
use assert_cmd::Command;
use mockito::{mock, server_url, Matcher};
use serde_json::json;

#[test]
fn test_sn_create() -> Result<()> {
    let login_endpoint = mock("POST", "/user/login")
        .match_header("content-type", "application/x-www-form-urlencoded")
        .match_body("grant_type=password&username=username&password=password")
        .with_body(json!({"access_token": "secret", "token_type": "bearer"}).to_string())
        .create();
    let cards_endpoint = mock("POST", "/cards/")
        .match_header("content-type", "application/json")
        .match_header("authorization", "Bearer secret")
        .match_body(Matcher::PartialJson(json!({
            "card": {
                "name": "Card title",
                "markup": "Card body",
                "html": "<p>Card body</p>\n",
            }
        })))
        .create();

    Command::cargo_bin("sn")?
        .env("SN_BASE_URL", &server_url())
        .env("SN_USERNAME", "username")
        .env("SN_PASSWORD", "password")
        .arg("create")
        .arg("Card title")
        .write_stdin("Card body")
        .assert()
        .success();

    login_endpoint.assert();
    cards_endpoint.assert();

    Ok(())
}
