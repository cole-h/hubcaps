extern crate env_logger;
extern crate hyper;
extern crate hubcaps;
extern crate hyper_native_tls;
extern crate frank_jwt;
extern crate serde_json;

use hyper::Client;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;
use hubcaps::{Credentials, Github, JWTCredentials, InstallationTokenGenerator};
use hubcaps::checks::{CheckRunOptions, Action, Output, Conclusion, Annotation, AnnotationLevel, Image};
use std::env;

fn main() {
    env_logger::init().unwrap();

    let mut keypath = env::current_dir().unwrap();
    keypath.push("private-key.pem");
    let cred = JWTCredentials::new(20500, keypath);

    let github =
        Github::new(
            format!("hubcaps/{}", env!("CARGO_PKG_VERSION")),
            Client::with_connector(HttpsConnector::new(NativeTlsClient::new().unwrap())),
            Credentials::InstallationToken(InstallationTokenGenerator::new(444986, cred))
        );

    let repo = github.repo("grahamc", "notpkgs");
    let checks = repo.checkruns();
    let options = &CheckRunOptions{
        actions: Some(vec![
            Action {
                description: "click to do a thing".to_string(),
                identifier: "the thing".to_string(),
                label: "nix-build -A pkgA".to_string(),
            },
            Action {
                description: "click to do a different thing".to_string(),
                identifier: "the different thing".to_string(),
                label: "nix-build -A pkgB".to_string(),
            }

        ]),
        completed_at: Some("2018-01-01T01:01:01Z".to_string()),
        started_at: Some("2018-08-01T01:01:01Z".to_string()),
        conclusion: Some(Conclusion::Neutral),
        details_url: Some("https://nix.ci/status/hi".to_string()),
        external_id: Some("heyyy".to_string()),
        head_sha: "263376dd4c872fbaa976f4055ec6269ab66e3a73".to_string(),
        name: "nix-build . -A pkgA".to_string(),
        output: Some(Output {
            annotations: Some(vec![
                Annotation {
                    annotation_level: AnnotationLevel::Warning,
                    start_line: 4,
                    end_line: 4,
                    start_column: Some(4),
                    end_column: Some(6),
                    message: "Trailing whitespace".to_string(),
                    path: "bogus".to_string(),
                    raw_details: "".to_string(),
                    title: "Whitespace".to_string(),
                },

                Annotation {
                    annotation_level: AnnotationLevel::Warning,
                    start_line: 7,
                    end_line: 7,
                    start_column: Some(4),
                    end_column: Some(8),
                    message: "not sure you meant this letter".to_string(),
                    path: "bogus".to_string(),
                    raw_details: "rawdeetshere\n  is\n   some\n    text".to_string(),
                    title: "hiiii".to_string(),
                }
            ]),
            images: Some(vec![
                Image{
                    alt: "alt text".to_string(),
                    caption: Some("caption text".to_string()),
                    image_url: "https://nix.ci/nix.ci.svg".to_string(),
                }
            ]),
            summary: "build failed".to_string(),
            text: Some("texthere\n  is\n   some\n    text".to_string()),
            title: "build failed".to_string()
        }),
        status: None,
    };

    println!("{}", serde_json::to_string(options).unwrap());
    println!("{:?}", checks.create(options));
}
