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
use hubcaps::checks::{CheckRunOptions, CheckRunUpdateOptions, Output, Conclusion, CheckRunState};
use std::env;
use std::thread;
use std::time::Duration;

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

    let r = checks.create(&CheckRunOptions {
        name: "live updates! nix-build -A  --argstr system x86_64-linux".to_string(),
        actions: None,
        started_at: None,
        completed_at: None,
        status: Some(CheckRunState::Queued),
        conclusion: None,
        details_url: None,
        external_id: Some("bogus-request-id".to_string()),
        head_sha: "fba11b4caba20e70dd3eb6499a17ea45a796aca4".to_string(), // "abc123".to_string(),
        output: None,
    }).unwrap();
    println!("{:?}", r);
    thread::sleep(Duration::from_secs(15));
    println!("{:?}", checks.update(
        &r.id,
        &CheckRunUpdateOptions {
            status: Some(CheckRunState::InProgress),
            conclusion: None,
            details_url: Some("https://logs.nix.ci/?key=nixos/nixpkgs.2345&attempt_id=foo".to_string()),
            external_id: None,
            output: Some(Output {
                title: "Build Started".to_string(),
                summary: "See streaming logs at https://logs.nix.ci/?key=nixos/nixpkgs.2345&attempt_id=foo".to_string(),
                text: None,
                annotations: None,
                images: None,
            }),
            actions: None,
            completed_at: None,
            name: None,
            started_at: None,
        }));

    thread::sleep(Duration::from_secs(10));
    println!("{:?}", checks.update(
        &r.id,
        &CheckRunUpdateOptions {
            name: None,
            actions: None,
            started_at: None,
            completed_at: Some("2018-01-01T01:01:01Z".to_string()),
            status: Some(CheckRunState::Completed),
            conclusion: Some(Conclusion::Success),
            details_url: None,
            external_id: None,
            output: Some(Output {
                title: "Build Results".to_string(),
                summary: "Attempted: `foo`

The following builds were skipped because they don't evaluate on x86_64-linux: `bar`.".to_string(),
                text: Some("
Partial log from building `foo`:

```
make[2]: Entering directory '/private/tmp/nix-build-gdb-8.1.drv-0/gdb-8.1/readline'
make[2]: Nothing to be done for 'install'.
make[2]: Leaving directory '/private/tmp/nix-build-gdb-8.1.drv-0/gdb-8.1/readline'
make[1]: Nothing to be done for 'install-target'.
make[1]: Leaving directory '/private/tmp/nix-build-gdb-8.1.drv-0/gdb-8.1'
removed '/nix/store/pcja75y9isdvgz5i00pkrpif9rxzxc29-gdb-8.1/share/info/bfd.info'
post-installation fixup
strip is /nix/store/5a88zk3jgimdmzg8rfhvm93kxib3njf9-cctools-binutils-darwin/bin/strip
patching script interpreter paths in /nix/store/pcja75y9isdvgz5i00pkrpif9rxzxc29-gdb-8.1
/nix/store/pcja75y9isdvgz5i00pkrpif9rxzxc29-gdb-8.1
```

".to_string()),
                annotations: None,
                images: None,
            }),
        }
    ));
}
