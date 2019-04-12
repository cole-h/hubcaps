//! Hubcaps provides a set of building blocks for interacting with the Github API
//!
//! # Examples
//!
//!  Typical use will require instantiation of a Github client. Which requires
//! a user agent string, a `hyper::Client`, and set of `hubcaps::Credentials`.
//!
//! The hyper client should be configured with tls.
//!
//! ```no_run
//! extern crate hubcaps;
//! extern crate hyper;
//! extern crate hyper_native_tls;
//!
//! use hubcaps::{Credentials, Github};
//! use hyper::Client;
//! use hyper::net::HttpsConnector;
//! use hyper_native_tls::NativeTlsClient;
//!
//! fn main() {
//!   let github = Github::new(
//!     String::from("user-agent-name"),
//!     Client::with_connector(
//!       HttpsConnector::new(
//!         NativeTlsClient::new().unwrap()
//!       )
//!     ),
//!     Credentials::Token(
//!       String::from("personal-access-token")
//!     )
//!   );
//! }
//! ```
//!
//! Github enterprise users will want to create a client with the
//! [Github#host](struct.Github.html#method.host) method
//!
//! Access to various services are provided via methods on instances of the `Github` type.
//!
//! The convention for executing operations typically looks like
//! `github.repo(.., ..).service().operation(OperationOptions)` where operation may be `create`,
//! `delete`, etc.

//! Services and their types are packaged under their own module namespace.
//! A service interface will provide access to operations and operations may access options types
//! that define the various parameter options available for the operation. Most operation option
//! types expose `builder()` methods for a builder oriented style of constructing options.
//!
//! # Errors
//!
//! Operations typically result in a `hubcaps::Result` Type which is an alias for Rust's
//! built-in Result with the Err Type fixed to the
//! [hubcaps::Error](errors/enum.Error.html) enum type.
//!
#![allow(missing_docs)] // todo: make this a deny eventually

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate serde_derive;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate url;
extern crate frank_jwt;

// all the modules!
use serde::de::DeserializeOwned;
pub mod branches;
pub mod git;
pub mod users;
pub mod comments;
pub mod review_comments;
pub mod pull_commits;
pub mod keys;
pub mod gists;
pub mod deployments;
#[allow(deprecated)]
pub mod errors;
pub mod hooks;
pub mod issues;
pub mod labels;
pub mod releases;
pub mod repositories;
pub mod review_requests;
pub mod statuses;
pub mod pulls;
pub mod search;
pub mod teams;
pub mod organizations;
pub mod app;
pub mod checks;

pub use errors::{Error, ErrorKind, Result};
use gists::{Gists, UserGists};
use search::Search;
use hyper::Client;
use hyper::client::RequestBuilder;
use hyper::method::Method;
use hyper::header::{qitem, Accept, Authorization, UserAgent};
use hyper::mime::Mime;
use hyper::status::StatusCode;
use repositories::{Repository, Repositories, UserRepositories, OrganizationRepositories};
use organizations::{Organization, Organizations, UserOrganizations};
use users::Users;
use app::App;
use std::fmt;
use url::Url;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time;
use std::sync::Mutex;
use std::sync::Arc;
use frank_jwt::{Algorithm, encode};


/// Link header type
header! { (Link, "Link") => [String] }

const DEFAULT_HOST: &'static str = "https://api.github.com";
// 9 minutes so we always come below github's 10minmax even if their or
// our clocks are slightly off
const MAX_JWT_TOKEN_LIFE: time::Duration = time::Duration::from_secs(60 * 9);
// 8 minutes so we refresh sooner than it actually expires
const JWT_TOKEN_REFRESH_PERIOD: time::Duration = time::Duration::from_secs(60 * 8);

/// alias for Result that infers hubcaps::Error as Err
// pub type Result<T> = std::result::Result<T, Error>;
/// Github defined Media types
/// See [this doc](https://developer.github.com/v3/media/) for more for more information
#[derive(Clone, Copy)]
pub enum MediaType {
    /// Return json (the default)
    Json,
    /// Return json in preview form
    Preview(&'static str),
}

impl Default for MediaType {
    fn default() -> MediaType {
        MediaType::Json
    }
}

impl From<MediaType> for Mime {
    fn from(media: MediaType) -> Mime {
        match media {
            MediaType::Json => "application/vnd.github.v3+json".parse().unwrap(),
            MediaType::Preview(codename) => {
                format!("application/vnd.github.{}-preview+json", codename)
                    .parse()
                    .unwrap()
            }
        }
    }
}

/// enum representation of Github list sorting options
#[derive(Clone, Debug, PartialEq)]
pub enum SortDirection {
    /// Sort in ascending order (the default)
    Asc,
    /// Sort in descending order
    Desc,
}

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }.fmt(f)
    }
}

impl Default for SortDirection {
    fn default() -> SortDirection {
        SortDirection::Asc
    }
}

/// Various forms of authentication credentials supported by Github
#[derive(Debug, PartialEq)]
pub enum Credentials {
    /// No authentication (the default)
    None,
    /// Oauth token string
    /// https://developer.github.com/v3/#oauth2-token-sent-in-a-header
    Token(String),
    /// Oauth client id and secret
    /// https://developer.github.com/v3/#oauth2-keysecret
    Client(String, String),
    /// JWT token exchange, to be performed transparently in the
    /// background. app-id, key-file.
    /// https://developer.github.com/apps/building-github-apps/authenticating-with-github-apps/
    JWT(JWTCredentials),
    /// JWT-based App Installation Token
    /// https://developer.github.com/apps/building-github-apps/authenticating-with-github-apps/
    InstallationToken(InstallationTokenGenerator),
}

impl Default for Credentials {
    fn default() -> Credentials {
        Credentials::None
    }
}

/// JSON Web Token authentication mechanism
///
/// The GitHub client methods are all &self, but the dynamically
/// generated JWT token changes regularly. The token is also a bit
/// expensive to regenerate, so we do want to have a mutable cache.
///
/// We use a token inside a Mutex so we can have interior mutability
/// even though JWTCredentials is not mutable.
#[derive(Debug, Clone)]
pub struct JWTCredentials {
    pub app_id: i32,
    pub private_key_file: PathBuf,
    cache: Arc<Mutex<ExpiringJWTCredential>>
}

impl JWTCredentials {
    pub fn new<A, P>(app_id: A, private_key_file: P) -> JWTCredentials
    where
        P: Into<PathBuf>,
        A: Into<i32>
    {
        let app_id = app_id.into();
        let private_key_file = private_key_file.into();

        let creds = ExpiringJWTCredential::calculate(
            &app_id,
            &private_key_file
        );

        JWTCredentials {
            app_id: app_id,
            private_key_file: private_key_file.into(),
            cache: Arc::new(Mutex::new(creds)),
        }
    }

    fn is_stale(&self) -> bool {
        self.cache.lock().unwrap().is_stale()
    }

    /// Fetch a valid JWT token, regenerating it if necessary
    pub fn token(&self) -> String {
        let mut expiring = self.cache.lock().unwrap();
        if expiring.is_stale() {
            * expiring = ExpiringJWTCredential::calculate(
                &self.app_id,
                &self.private_key_file,
            );
        }

        expiring.token.clone()
    }
}

impl PartialEq for JWTCredentials {
    fn eq(&self, other: &JWTCredentials) -> bool {
        self.app_id == other.app_id &&
            self.private_key_file == other.private_key_file
    }
}

#[derive(Debug)]
struct ExpiringJWTCredential {
    token: String,
    created_at: time::Instant
}

impl ExpiringJWTCredential {
    fn calculate(app_id: &i32, private_key_file: &PathBuf) -> ExpiringJWTCredential {
        // SystemTime can go backwards, Instant can't, so always use
        // Instant for ensuring regular cycling.
        let created_at = time::Instant::now();
        let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap();
        let expires = now + MAX_JWT_TOKEN_LIFE;

        let payload = json!({
            "iat" : now.as_secs(),
            "exp" : expires.as_secs(),
            "iss": app_id,
        });
        let header = json!({});
        let jwt = encode(header, private_key_file, &payload,
                         Algorithm::RS256)
            .expect(&format!("key {:?} may not exist", private_key_file));

        ExpiringJWTCredential {
            created_at: created_at,
            token: jwt,
        }
    }

    fn is_stale(&self) -> bool {
        self.created_at.elapsed() >= JWT_TOKEN_REFRESH_PERIOD
    }
}

/// A caching token "generator" which contains JWT credentials.
///
/// The authentication mechanism in the GitHub client library
/// determines if the token is stale, and if so, uses the contained
/// JWT credentials to fetch a new installation token.
///
/// The Mutex<Option> access key is for interior mutability.
#[derive(Debug)]
pub struct InstallationTokenGenerator {
    pub installation_id: i32,
    pub jwt_credential: JWTCredentials,
    access_key: Mutex<Option<String>>
}

impl InstallationTokenGenerator {
    pub fn new(installation_id: i32, creds: JWTCredentials) -> InstallationTokenGenerator {
        InstallationTokenGenerator {
            installation_id: installation_id,
            jwt_credential: creds,
            access_key: Mutex::new(None),
        }
    }

    fn set_token(&self, token: app::AccessToken) {
        *self.access_key.lock().unwrap() = Some(token.token);
    }

    /// Fetch the contained token
    ///
    /// Panics if the token is stale: always check staleness before
    /// fetching the token.
    fn token(&self) -> String {
        if self.is_stale() {
            panic!("Token is stale!");
        }
        let keylock = self.access_key.lock().unwrap().clone();
        keylock.unwrap().clone()
    }

    fn is_stale(&self) -> bool {
        self.jwt_credential.is_stale()
            || self.access_key.lock().unwrap().is_none()
    }
}

impl PartialEq for InstallationTokenGenerator {
    fn eq(&self, other: &InstallationTokenGenerator) -> bool {
        self.installation_id == other.installation_id &&
            self.jwt_credential == other.jwt_credential
    }
}

/// Entry point interface for interacting with Github API
pub struct Github {
    host: String,
    agent: String,
    client: Client,
    credentials: Mutex<Option<Credentials>>,
}

impl Github {
    /// Create a new Github instance. This will typically be how you interface with all
    /// other operations
    pub fn new<A>(agent: A, client: Client, credentials: Credentials) -> Github
    where
        A: Into<String>,
    {
        Github::host(DEFAULT_HOST, agent, client, credentials)
    }

    /// Create a new Github instance hosted at a custom location.
    /// Useful for github enterprise installations ( yourdomain.com/api/v3/ )
    pub fn host<H, A>(host: H, agent: A, client: Client, credentials: Credentials) -> Github
    where
        H: Into<String>,
        A: Into<String>,
    {
        Github {
            host: host.into(),
            agent: agent.into(),
            client: client,
            credentials: Mutex::new(Some(credentials)),
        }
    }

    /// Return a reference to a Github repository
    pub fn repo<O, R>(&self, owner: O, repo: R) -> Repository
    where
        O: Into<String>,
        R: Into<String>,
    {
        Repository::new(self, owner, repo)
    }

    /// Return a reference to the collection of repositories owned by and
    /// associated with an owner
    pub fn user_repos<S>(&self, owner: S) -> UserRepositories
    where
        S: Into<String>,
    {
        UserRepositories::new(self, owner)
    }

    /// Return a reference to the collection of repositories owned by the user
    /// associated with the current authentication credentials
    pub fn repos(&self) -> Repositories {
        Repositories::new(self)
    }

    pub fn org<O>(&self, org: O) -> Organization
    where
        O: Into<String>,
    {
        Organization::new(self, org)
    }

    /// Return a reference to the collection of organizations that the user
    /// associated with the current authentication credentials is in
    pub fn orgs(&self) -> Organizations {
        Organizations::new(self)
    }

    /// Return a reference to an interface that provides access
    /// to user information.
    pub fn users(&self) -> Users {
        Users::new(self)
    }

    /// Return a reference to the collection of organizations a user
    /// is publicly associated with
    pub fn user_orgs<U>(&self, user: U) -> UserOrganizations
    where
        U: Into<String>,
    {
        UserOrganizations::new(self, user)
    }

    /// Return a reference to an interface that provides access to a user's gists
    pub fn user_gists<O>(&self, owner: O) -> UserGists
    where
        O: Into<String>,
    {
        UserGists::new(self, owner)
    }

    /// Return a reference to an interface that provides access to the
    /// gists belonging to the owner of the token used to configure this client
    pub fn gists(&self) -> Gists {
        Gists::new(self)
    }

    /// Return a reference to an interface that provides access to search operations
    pub fn search(&self) -> Search {
        Search::new(self)
    }

    /// Return a reference to the collection of repositories owned by and
    /// associated with an organization
    pub fn org_repos<O>(&self, org: O) -> OrganizationRepositories
    where
        O: Into<String>,
    {
        OrganizationRepositories::new(self, org)
    }

    /// Return a reference to GitHub Apps
    pub fn app(&self) -> App
    {
        App::new(self)
    }

    fn authenticate(&self, method: Method, url: String) -> RequestBuilder {
        // We complete remove the credentials from the client, because
        // if we are using the Installation token method we may need
        // to replace the client's credentials and issue another
        // request.
        //
        // At the end of this function, we will put the credentials
        // back.
        //
        // I hope this is safe.
        let mut creds_lock = self.credentials.lock().unwrap();
        let creds_orig = creds_lock.take().unwrap();
        drop(creds_lock);

        let ret;

        match creds_orig {
            Credentials::Token(ref token) => {
                ret = self.client.request(method, &url).header(Authorization(
                    format!("token {}", token),
                ));
            }
            Credentials::Client(ref id, ref secret) => {
                let mut parsed = Url::parse(&url).unwrap();
                parsed
                    .query_pairs_mut()
                    .append_pair("client_id", id)
                    .append_pair("client_secret", secret);
                ret = self.client.request(method, parsed);
            }
            Credentials::JWT(ref jwt) => {
                let header = format!("Bearer {}", jwt.token());
                ret = self.client.request(method, &url).header(Authorization(
                    header
                ));
            }
            Credentials::InstallationToken(ref apptoken) => {
                if apptoken.is_stale() {
                    // Plunk in the JWT credentials for refreshing our
                    // installation token.
                    debug!("App token is stale, refreshing");
                    let mut creds = self.credentials.lock().unwrap();
                    *creds = Some(Credentials::JWT(apptoken.jwt_credential.clone()));
                    drop(creds);

                    let token = self.app().make_access_token(apptoken.installation_id).unwrap();

                    apptoken.set_token(token);
                }

                ret = self.client.request(method, &url).header(Authorization(
                    format!("token {}", apptoken.token()),
                ));
            }

            Credentials::None => {
                ret = self.client.request(method, &url);
            }
        }

        let mut creds = self.credentials.lock().unwrap();
        *creds = Some(creds_orig);

        return ret;
    }


    fn iter<'a, D, I>(&'a self, uri: String, into_items: fn(D) -> Vec<I>) -> Result<Iter<'a, D, I>>
    where
        D: DeserializeOwned,
    {
        self.iter_media(uri, into_items, MediaType::Json)
    }

    fn iter_media<'a, D, I>(
        &'a self,
        uri: String,
        into_items: fn(D) -> Vec<I>,
        media_type: MediaType,
    ) -> Result<Iter<'a, D, I>>
    where
        D: DeserializeOwned,
    {
        Iter::new(self, self.host.clone() + &uri, into_items, media_type)
    }

    fn request<D>(
        &self,
        method: Method,
        uri: String,
        body: Option<&[u8]>,
        media_type: MediaType,
    ) -> Result<(Option<Links>, D)>
    where
        D: DeserializeOwned,
    {
        let builder = self.authenticate(method, uri)
            .header(UserAgent(self.agent.to_owned()))
            .header(Accept(vec![qitem(From::from(media_type))]));

        let res = (match body {
                       Some(ref bod) => builder.body(*bod).send(),
                       _ => builder.send(),
                   })?;

        let links = res.headers.get::<Link>().map(|&Link(ref value)| {
            Links::new(value.to_owned())
        });

        debug!("rec response {:?}", res);
        match res.status {
            StatusCode::Conflict |
            StatusCode::BadRequest |
            StatusCode::UnprocessableEntity |
            StatusCode::Unauthorized |
            StatusCode::NotFound |
            StatusCode::Forbidden => {
                Err(
                    ErrorKind::Fault {
                        code: res.status,
                        error: serde_json::from_reader(res)?,
                    }.into(),
                )
            }
            _ => Ok((links, serde_json::from_reader(res)?)),
        }
    }

    fn request_entity<D>(
        &self,
        method: Method,
        uri: String,
        body: Option<&[u8]>,
        media_type: MediaType,
    ) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.request(method, uri, body, media_type).map(
            |(_, entity)| {
                entity
            },
        )
    }

    fn get<D>(&self, uri: &str) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.get_media(uri, MediaType::Json)
    }

    fn get_media<D>(&self, uri: &str, media: MediaType) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.request_entity(Method::Get, self.host.clone() + uri, None, media)
    }

    fn delete(&self, uri: &str) -> Result<()> {
        match self.request_entity::<()>(
            Method::Delete,
            self.host.clone() + uri,
            None,
            MediaType::Json,
        ) {
            Err(Error(ErrorKind::Codec(_), _)) => Ok(()),
            otherwise => otherwise,
        }
    }

    fn delete_message(&self, uri: &str, message: &[u8]) -> Result<()> {
        match self.request_entity::<()>(
            Method::Delete,
            self.host.clone() + uri,
            Some(message),
            MediaType::Json,
        ) {
            Err(Error(ErrorKind::Codec(_), _)) => Ok(()),
            otherwise => otherwise,
        }
    }

    fn post<D>(&self, uri: &str, message: &[u8]) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.post_media(
            uri,
            message,
            MediaType::Json,
        )
    }

    fn post_media<D>(&self, uri: &str, message: &[u8], media: MediaType) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.request_entity(
            Method::Post,
            self.host.clone() + uri,
            Some(message),
            media,
        )
    }

    fn patch_media<D>(&self, uri: &str, message: &[u8], media: MediaType) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.request_entity(Method::Patch, self.host.clone() + uri, Some(message), media)
    }

    fn patch<D>(&self, uri: &str, message: &[u8]) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.patch_media(uri, message, MediaType::Json)
    }


    fn put_no_response(&self, uri: &str, message: &[u8]) -> Result<()> {
        match self.put(uri, message) {
            Err(Error(ErrorKind::Codec(_), _)) => Ok(()),
            otherwise => otherwise,
        }
    }

    fn put<D>(&self, uri: &str, message: &[u8]) -> Result<D>
    where
        D: DeserializeOwned,
    {
        self.request_entity(
            Method::Put,
            self.host.clone() + uri,
            Some(message),
            MediaType::Json,
        )
    }
}

/// An abstract type used for iterating over result sets
pub struct Iter<'a, D, I> {
    github: &'a Github,
    next_link: Option<String>,
    into_items: fn(D) -> Vec<I>,
    items: Vec<I>,
    media_type: MediaType,
}

impl<'a, D, I> Iter<'a, D, I>
where
    D: DeserializeOwned,
{
    /// creates a new instance of an Iter
    pub fn new(
        github: &'a Github,
        uri: String,
        into_items: fn(D) -> Vec<I>,
        media_type: MediaType,
    ) -> Result<Iter<'a, D, I>> {
        let (links, payload) = github.request::<D>(Method::Get, uri, None, media_type)?;
        let mut items = into_items(payload);
        items.reverse(); // we pop from the tail
        Ok(Iter {
            github: github,
            next_link: links.and_then(|l| l.next()),
            into_items: into_items,
            items: items,
            media_type: media_type,
        })
    }

    fn set_next(&mut self, next: Option<String>) {
        self.next_link = next;
    }
}

impl<'a, D, I> Iterator for Iter<'a, D, I>
where
    D: DeserializeOwned,
{
    type Item = I;
    fn next(&mut self) -> Option<I> {
        self.items.pop().or_else(|| {
            self.next_link.clone().and_then(|ref next_link| {
                self.github
                    .request::<D>(Method::Get, next_link.to_owned(), None, self.media_type)
                    .ok()
                    .and_then(|(links, payload)| {
                        let mut next_items = (self.into_items)(payload);
                        next_items.reverse(); // we pop() from the tail
                        self.set_next(links.and_then(|l| l.next()));
                        self.items = next_items;
                        self.next()
                    })
            })
        })
    }
}

/// An abstract collection of Link header urls
/// Exposes interfaces to access link relations github typically
/// sends as headers
#[derive(Debug)]
pub struct Links {
    values: HashMap<String, String>,
}

impl Links {
    /// Creates a new Links instance given a raw header string value
    pub fn new<V>(value: V) -> Links
    where
        V: Into<String>,
    {
        let values = value
            .into()
            .split(",")
            .map(|link| {
                let parts = link.split(";").collect::<Vec<_>>();
                (
                    parts[1].to_owned().replace(" rel=\"", "").replace("\"", ""),
                    parts[0]
                        .to_owned()
                        .replace("<", "")
                        .replace(">", "")
                        .replace(" ", ""),
                )
            })
            .fold(HashMap::new(), |mut acc, (rel, link)| {
                acc.insert(rel, link);
                acc
            });
        Links { values: values }
    }

    /// Returns next link url, when available
    pub fn next(&self) -> Option<String> {
        self.values.get("next").map(|s| s.to_owned())
    }

    /// Returns prev link url, when available
    pub fn prev(&self) -> Option<String> {
        self.values.get("prev").map(|s| s.to_owned())
    }

    /// Returns last link url, when available
    pub fn last(&self) -> Option<String> {
        self.values.get("last").map(|s| s.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_links() {
        let links = Links::new(r#"<linknext>; rel="next", <linklast>; rel="last""#);
        assert_eq!(links.next(), Some("linknext".to_owned()));
        assert_eq!(links.last(), Some("linklast".to_owned()));
    }

    #[test]
    fn default_sort_direction() {
        let default: SortDirection = Default::default();
        assert_eq!(default, SortDirection::Asc)
    }

    #[test]
    fn default_credentials() {
        let default: Credentials = Default::default();
        assert_eq!(default, Credentials::None)
    }
}
