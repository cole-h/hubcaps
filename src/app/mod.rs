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

    pub fn find_repo_installation<O, R>(&self, owner: O, repo: R) -> Result<Installation>
    where
        O: Into<String>,
        R: Into<String> {
        self.github.get_media::<Installation>(
            &format!("/repos/{}/{}/installation", owner.into(), repo.into()),
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

#[derive(Debug, Deserialize)]
pub struct Installation {
    pub id: i32,
    // account: Account
    pub access_tokens_url: String,
    pub repositories_url: String,
    pub html_url: String,
    pub app_id: i32,
    pub target_id: i32,
    pub target_type: String,
    // permissions: Permissions
    pub events: Vec<String>,
    // created_at, updated_at
    pub single_file_name: Option<String>,
    pub repository_selection: String,
}
