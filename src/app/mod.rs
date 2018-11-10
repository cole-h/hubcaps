//! Labels interface

extern crate serde_json;

use self::super::{Github, Result, MediaType};

pub struct App<'a> {
    github: &'a Github,
}

impl<'a> App<'a> {
    #[doc(hidden)]
    pub fn new(github: &'a Github) -> App<'a>
    {
        App {
            github: github,
        }
    }

    fn path(&self, more: &str) -> String {
        format!("/app{}", more)
    }

    pub fn make_access_token(&self, installation_id: i32) -> Result<AccessToken> {
        self.github.post_media::<AccessToken>(
            &self.path(&format!("/installations/{}/access_tokens", installation_id)),
            b"",
            MediaType::Preview("machine-man")
        )
    }
}

// representations

#[derive(Debug, Deserialize)]
pub struct AccessToken {
    pub token: String,
    pub expires_at: String,
}
