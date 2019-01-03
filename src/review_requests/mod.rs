//! Review requests interface
extern crate serde_json;

use super::{Github, Result};
use pulls::Pull;
use teams::Team;
use users::User;

/// A structure for interfacing with a review comments
pub struct ReviewRequests<'a> {
    github: &'a Github,
    owner: String,
    repo: String,
    number: u64,
}

impl<'a> ReviewRequests<'a> {
    #[doc(hidden)]
    pub fn new<O, R>(github: &'a Github, owner: O, repo: R, number: u64) -> Self
    where
        O: Into<String>,
        R: Into<String>,
    {
        ReviewRequests {
            github,
            owner: owner.into(),
            repo: repo.into(),
            number,
        }
    }

    /// list requested reviews
    pub fn get(&self) -> Result<ReviewRequest> {
        self.github.get::<ReviewRequest>(&self.path())
    }

    /// Add new requested reviews
    pub fn create(&self, review_request: &ReviewRequestOptions) -> Result<Pull> {
        let data = serde_json::to_string(&review_request)?;
        self.github.post(&self.path(), data.as_bytes())
    }

    /// Delete a review request
    pub fn delete(&self, review_request: &ReviewRequestOptions) -> Result<()> {
        let data = serde_json::to_string(&review_request)?;
        self.github
            .delete_message(&self.path(), data.as_bytes())
    }

    fn path(&self) -> String {
        format!(
            "/repos/{}/{}/pulls/{}/requested_reviewers",
            self.owner, self.repo, self.number
        )
    }
}

// representations (todo: replace with derive_builder)

#[derive(Default, Serialize)]
pub struct ReviewRequestOptions {
    /// An array of user `logins` that will be requested.
    /// Note, each login must be a collaborator.
    pub reviewers: Vec<String>,
    /// An array of team `slugs` that will be requested.
    pub team_reviewers: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    pub users: Vec<User>,
    pub teams: Vec<Team>,
}
