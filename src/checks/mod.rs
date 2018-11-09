//! Checks interface
// see: https://developer.github.com/v3/checks/suites/

extern crate serde_json;

use self::super::{Github, MediaType, Result};

pub struct CheckRuns<'a> {
    github: &'a Github,
    owner: String,
    repo: String,
}

impl<'a> CheckRuns<'a> {
    #[doc(hidden)]
    pub fn new<O, R>(github: &'a Github, owner: O, repo: R) -> CheckRuns<'a>
    where
        O: Into<String>,
        R: Into<String>,
    {
        CheckRuns {
            github: github,
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    fn path(&self, more: &str) -> String {
        format!("/repos/{}/{}/check-runs{}", self.owner, self.repo, more)
    }

    pub fn create(&self, check_run_options: &CheckRunOptions) -> Result<CheckRun> {
        let data = serde_json::to_string(&check_run_options)?;
        self.github.post_media::<CheckRun>(
            &self.path(""), data.as_bytes(),
            MediaType::Preview("antiope")
        )
    }

    pub fn update(&self, check_run_id: &str, check_run_options: &CheckRunOptions) -> Result<CheckRun> {
        let data = serde_json::to_string(&check_run_options)?;
        self.github.patch_media::<CheckRun>(
            &self.path(&format!("/{}", check_run_id)),
            data.as_bytes(),
            MediaType::Preview("antiope")
        )
    }

    pub fn list_for_suite(&self, suite_id: &str) -> Result<Vec<CheckRun>> {
        // !!! does this actually work?
        // https://developer.github.com/v3/checks/runs/#list-check-runs-in-a-check-suite
        self.github.get_media::<Vec<CheckRun>>(
            &self.path(&format!("/{}/check-runs", suite_id)),
            MediaType::Preview("antiope")
        )
    }
}

// representations

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckRunState {
    Queued,
    InProgress,
    Completed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    TimedOut,
    ActionRequired,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationLevel {
    Notice,
    Warning,
    Failure,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<Annotation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<Image>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Action {
    pub label: String,
    pub description: String,
    pub identifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Annotation {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
    pub annotation_level: AnnotationLevel,
    pub message: String,
    pub title: String,
    pub raw_details: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Image {
    pub alt: String,
    pub image_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckRunOptions {
    pub name: String,
    pub head_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<CheckRunState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<Conclusion>,
   #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
   #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Output>,
   #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<Action>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub head_sha: String,
    pub url: String,
    pub check_suite: CheckSuite,
    pub details_url: Option<String>,
    pub external_id: Option<String>,
    pub status: Option<CheckRunState>,
    pub started_at: Option<String>,
    pub conclusion: Option<Conclusion>,
    pub completed_at: Option<String>,
    pub output: Option<Output>,
    pub actions: Option<Vec<Action>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckSuite {
    pub id: u32,
}
