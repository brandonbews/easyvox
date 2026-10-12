//! Check for Updates: one GET of [`LATEST_URL`], only when the person asks, through the
//! NSURLSession transport model downloads use (`download/macos.rs`). Nothing is downloaded or
//! installed: the app offers to open the release's page.
//!
//! **The file.** Each release commits it to the site as `site/public/latest.json`
//! (`.github/workflows/release.yml`, once the release is the latest):
//!
//! ```json
//! {"version": "0.1.1", "url": "https://github.com/brandonbews/easyvox/releases/tag/v0.1.1"}
//! ```
//!
//! Both fields are required; any others are ignored, so the file can grow without breaking the
//! apps already out. It's at most [`MAX_BYTES`]. Before the first release there's no file, and
//! the check fails with a 404.
//!
//! **Versions** compare by semver precedence, build metadata ignored. A pre-release sorts
//! before its release, so someone on 0.2.0-beta.1 is offered 0.2.0. A pre-release in the file
//! is never offered: the release workflow only writes vX.Y.Z, and the check reads one there as
//! "up to date".
//!
//! **Addresses.** The file and the page it names must be https on easyvox.dev or github.com
//! ([`is_trusted_url`]). A redirect anywhere else is refused.

use crate::download::{self, CancelToken, FetchError, Fetched, Request, sentence};
use std::cmp::Ordering;
use std::fmt;
use std::time::Duration;

/// Where the check reads the latest version.
pub const LATEST_URL: &str = "https://easyvox.dev/latest.json";
/// The most the check reads. The file is about 100 bytes; this leaves room for more fields.
pub const MAX_BYTES: usize = 16 * 1024;
/// The whole check gives up after this long.
const TIMEOUT: Duration = Duration::from_secs(10);
/// The only hosts the check reads from or links to.
const TRUSTED_HOSTS: [&str; 2] = ["easyvox.dev", "github.com"];

/// A newer version to offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// As the file has it: "0.1.1".
    pub version: String,
    /// Its download page, checked with [`is_trusted_url`].
    pub url: String,
}

/// Why a check failed. `Display` gives the sentences the app shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    /// No connection to the server (offline, DNS, refused).
    Offline(String),
    /// Another network failure: a timeout, a dropped connection, TLS.
    Network(String),
    /// An error status. 404 until the first release is out; a 3xx is a redirect that was
    /// refused.
    Http(u16),
    /// The answer was longer than [`MAX_BYTES`].
    TooLarge,
    /// The answer isn't the file described above, for this reason.
    Invalid(String),
    /// The page it names isn't https on easyvox.dev or github.com.
    UntrustedLink(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Couldn't check for updates. ")?;
        match self {
            Self::Offline(e) | Self::Network(e) => write!(f, "{}", sentence(e)),
            Self::Http(404) => write!(f, "No release is listed on easyvox.dev yet."),
            Self::Http(code @ 300..=399) => {
                write!(f, "The check was sent to an address EasyVox doesn't accept (HTTP {code}).")
            }
            Self::Http(code) => write!(f, "The server answered with an error (HTTP {code})."),
            Self::TooLarge => write!(f, "The answer from easyvox.dev was larger than expected."),
            Self::Invalid(why) => write!(f, "The answer from easyvox.dev isn't valid: {why}."),
            Self::UntrustedLink(_) => write!(f, "The download link isn't on easyvox.dev or github.com."),
        }
    }
}

impl std::error::Error for UpdateError {}

/// Blocking, for up to 10 s: call it off the UI thread. Asks [`LATEST_URL`] for the latest
/// version and compares it with `current`, the app's own. `Ok(None)` means up to date.
pub fn check(current: &str) -> Result<Option<Release>, UpdateError> {
    let body = fetch(LATEST_URL, is_trusted_url)?;
    newer(current, &body)
}

/// Whether `url` is https on easyvox.dev or github.com, with no port and no user name. Never
/// panics: the transport calls it on each redirect, from an Objective-C callback.
pub fn is_trusted_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    TRUSTED_HOSTS.iter().any(|h| host.eq_ignore_ascii_case(h))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\')
}

/// The release in `body` if it's newer than `current` and not a pre-release.
fn newer(current: &str, body: &[u8]) -> Result<Option<Release>, UpdateError> {
    let (latest, release) = parse(body)?;
    // Cargo takes only semver versions, so the app's own always parses.
    let offered = Version::parse(current).is_some_and(|current| latest.pre.is_empty() && latest > current);
    Ok(offered.then_some(release))
}

/// Reads `latest.json`.
fn parse(body: &[u8]) -> Result<(Version, Release), UpdateError> {
    let invalid = |why: &str| UpdateError::Invalid(why.into());
    if body.len() > MAX_BYTES {
        return Err(UpdateError::TooLarge);
    }
    let json: serde_json::Value = serde_json::from_slice(body).map_err(|_| invalid("it isn't JSON"))?;
    let field = |name: &str| json.get(name).ok_or_else(|| invalid(&format!("\"{name}\" is missing")));
    let version_text = field("version")?.as_str().ok_or_else(|| invalid("\"version\" isn't text"))?;
    let version =
        Version::parse(version_text).ok_or_else(|| invalid("\"version\" isn't a version number like 0.1.1"))?;
    let url = field("url")?.as_str().ok_or_else(|| invalid("\"url\" isn't text"))?;
    if !is_trusted_url(url) {
        return Err(UpdateError::UntrustedLink(url.into()));
    }
    Ok((version, Release { version: version_text.into(), url: url.into() }))
}

/// One GET of `url`, following only the redirects `follow` accepts, read whole into memory:
/// at most [`MAX_BYTES`].
fn fetch(url: &str, follow: fn(&str) -> bool) -> Result<Vec<u8>, UpdateError> {
    let request = Request { url, offset: 0, idle_timeout: TIMEOUT, total_timeout: Some(TIMEOUT), follow };
    let mut body = Body::default();
    // Nothing cancels a check: it ends on its own within the timeout.
    match download::fetch(&request, &CancelToken::new(), &mut |chunk| body.take(chunk)) {
        Ok(()) => Ok(body.0),
        Err(FetchError::Sink(e)) => Err(e),
        Err(FetchError::Http(code)) => Err(UpdateError::Http(code)),
        Err(FetchError::Offline(e)) => Err(UpdateError::Offline(e)),
        Err(FetchError::Transient(e) | FetchError::Fatal(e)) => Err(UpdateError::Network(e)),
        Err(FetchError::Cancelled) => Err(UpdateError::Network("The check was cancelled.".into())),
    }
}

/// The response, collected as it arrives, refused as soon as it's too large.
#[derive(Default)]
struct Body(Vec<u8>);

impl Body {
    fn take(&mut self, chunk: Fetched) -> Result<(), UpdateError> {
        match chunk {
            Fetched::Start { total: Some(total), .. } if total > MAX_BYTES as u64 => Err(UpdateError::TooLarge),
            Fetched::Start { offset, .. } if offset > 0 => {
                Err(UpdateError::Network(format!("The server sent the file from byte {offset}.")))
            }
            Fetched::Start { .. } => {
                self.0.clear();
                Ok(())
            }
            Fetched::Data(bytes) if self.0.len() + bytes.len() > MAX_BYTES => Err(UpdateError::TooLarge),
            Fetched::Data(bytes) => {
                self.0.extend_from_slice(bytes);
                Ok(())
            }
        }
    }
}

/// A semver 2.0 version, without its build metadata, which doesn't count in comparisons.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    /// The pre-release identifiers ("beta.1" is two); empty for a release.
    pre: Vec<Identifier>,
}

/// A pre-release identifier. Numbers sort before words, so the variants' order matters.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Identifier {
    Number(u64),
    Word(String),
}

impl Version {
    /// `MAJOR.MINOR.PATCH`, then an optional `-pre.release` and `+build`, exactly as semver
    /// writes them: no "v", no spaces, no leading zeros.
    fn parse(text: &str) -> Option<Self> {
        let (text, build) = match text.split_once('+') {
            Some((text, build)) => (text, Some(build)),
            None => (text, None),
        };
        if let Some(build) = build
            && !build.split('.').all(|id| !id.is_empty() && id.bytes().all(is_identifier_byte))
        {
            return None;
        }
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut numbers = core.split('.').map(number);
        let (major, minor, patch) = (numbers.next()??, numbers.next()??, numbers.next()??);
        if numbers.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => vec![],
            Some(pre) => pre.split('.').map(identifier).collect::<Option<_>>()?,
        };
        Some(Self { major, minor, patch, pre })
    }
}

fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
}

/// A number with no sign and no leading zero.
fn number(text: &str) -> Option<u64> {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    (digits && (text == "0" || !text.starts_with('0'))).then(|| text.parse().ok()).flatten()
}

fn identifier(text: &str) -> Option<Identifier> {
    if text.is_empty() || !text.bytes().all(is_identifier_byte) {
        return None;
    }
    if text.bytes().all(|b| b.is_ascii_digit()) {
        return number(text).map(Identifier::Number);
    }
    Some(Identifier::Word(text.into()))
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // A release comes after its pre-releases.
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                // Identifier by identifier; with all equal, the longer list is the later one.
                (false, false) => self.pre.cmp(&other.pre),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|| panic!("{text} should parse"))
    }

    fn latest(version: &str) -> Vec<u8> {
        format!(
            r#"{{"version": "{version}", "url": "https://github.com/brandonbews/easyvox/releases/tag/v{version}"}}"#
        )
        .into_bytes()
    }

    #[test]
    fn versions_compare_by_semver_precedence() {
        // The order in the semver spec's own example, then the numeric fields as numbers.
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.2.0",
            "1.10.0",
            "2.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(v("0.1.1+build.5").cmp(&v("0.1.1")), Ordering::Equal, "build metadata doesn't count");
    }

    #[test]
    fn only_semver_parses() {
        for good in ["0.1.0", "10.20.30", "1.0.0-0.3.7", "1.0.0-x-y-z.--", "1.0.0+20130313144700", "1.0.0-a+b.c"] {
            assert!(Version::parse(good).is_some(), "{good}");
        }
        for bad in [
            "",
            "v0.1.1",
            " 0.1.1",
            "0.1.1 ",
            "0.1",
            "0.1.1.1",
            "01.1.1",
            "0.01.1",
            "0.1.-1",
            "0.1.1-",
            "0.1.1-01",
            "0.1.1-beta..1",
            "0.1.1+",
            "0.1.1+a..b",
            "0.1.1-é",
            "1.x.0",
            "99999999999999999999.0.0",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn a_newer_release_is_offered_and_nothing_else() {
        let offered = newer("0.1.0", &latest("0.1.1")).unwrap();
        assert_eq!(
            offered,
            Some(Release {
                version: "0.1.1".into(),
                url: "https://github.com/brandonbews/easyvox/releases/tag/v0.1.1".into()
            })
        );
        assert_eq!(newer("0.1.1", &latest("0.1.1")), Ok(None), "the same version");
        assert_eq!(newer("0.2.0", &latest("0.1.1")), Ok(None), "a build newer than the release");
        // Pre-releases: one in the file is never offered; someone on one is offered its release.
        assert_eq!(newer("0.1.0", &latest("0.2.0-beta.1")), Ok(None));
        assert!(newer("0.2.0-beta.1", &latest("0.2.0")).unwrap().is_some());
        assert_eq!(newer("0.2.0-beta.1", &latest("0.1.1")), Ok(None));
        assert!(Version::parse(env!("CARGO_PKG_VERSION")).is_some(), "the app's own version parses");
    }

    #[test]
    fn missing_or_malformed_fields_are_refused() {
        let invalid = |body: &str| match parse(body.as_bytes()) {
            Err(UpdateError::Invalid(why)) => why,
            other => panic!("{body}: {other:?}"),
        };
        assert_eq!(invalid(""), "it isn't JSON");
        assert_eq!(invalid("<html>Not found</html>"), "it isn't JSON");
        assert_eq!(invalid(r#"{"version": "0.1.1""#), "it isn't JSON");
        assert_eq!(invalid("[]"), "\"version\" is missing");
        assert_eq!(invalid(r#"{"url": "https://easyvox.dev/"}"#), "\"version\" is missing");
        assert_eq!(invalid(r#"{"version": 0.1, "url": "https://easyvox.dev/"}"#), "\"version\" isn't text");
        assert_eq!(
            invalid(r#"{"version": "v0.1.1", "url": "https://easyvox.dev/"}"#),
            "\"version\" isn't a version number like 0.1.1"
        );
        assert_eq!(invalid(r#"{"version": "0.1.1"}"#), "\"url\" is missing");
        assert_eq!(invalid(r#"{"version": "0.1.1", "url": null}"#), "\"url\" isn't text");
        assert!(matches!(
            parse(br#"{"version": "0.1.1", "url": "https://example.com/easyvox.dmg"}"#),
            Err(UpdateError::UntrustedLink(_))
        ));
        // Other fields are ignored, so the file can grow.
        let more = br#"{"version": "0.1.1", "url": "https://easyvox.dev/download", "notes": "Fixes."}"#;
        assert_eq!(parse(more).map(|(_, r)| r.version), Ok("0.1.1".into()));
    }

    #[test]
    fn an_oversized_answer_is_refused() {
        let mut body = Body::default();
        assert_eq!(
            body.take(Fetched::Start { offset: 0, total: Some(MAX_BYTES as u64 + 1) }),
            Err(UpdateError::TooLarge)
        );
        // No length announced: refused as the bytes pass the limit.
        let mut body = Body::default();
        assert_eq!(body.take(Fetched::Start { offset: 0, total: None }), Ok(()));
        let chunk = vec![b' '; 4096];
        for _ in 0..MAX_BYTES / chunk.len() {
            assert_eq!(body.take(Fetched::Data(&chunk)), Ok(()));
        }
        assert_eq!(body.take(Fetched::Data(b"x")), Err(UpdateError::TooLarge));
        assert_eq!(parse(&vec![b' '; MAX_BYTES + 1]), Err(UpdateError::TooLarge));
    }

    #[test]
    fn a_response_from_the_middle_of_the_file_is_refused() {
        let mut body = Body::default();
        assert!(matches!(body.take(Fetched::Start { offset: 10, total: None }), Err(UpdateError::Network(_))));
    }

    #[test]
    fn only_https_on_easyvox_dev_or_github_com_is_trusted() {
        for good in [
            LATEST_URL,
            "https://github.com/brandonbews/easyvox/releases/tag/v0.1.1",
            "https://GitHub.com/brandonbews/easyvox/releases",
            "https://easyvox.dev",
            "https://easyvox.dev?from=app",
            "https://easyvox.dev#download",
        ] {
            assert!(is_trusted_url(good), "{good}");
        }
        for bad in [
            "http://easyvox.dev/latest.json",
            "HTTPS://easyvox.dev/",
            "ftp://github.com/",
            "//github.com/",
            "github.com/brandonbews/easyvox",
            "https://www.easyvox.dev/",
            "https://easyvox.dev.example.com/",
            "https://evil.github.com.example.com/",
            "https://easyvox.dev./",
            "https://easyvox.dev:8443/",
            "https://github.com@example.com/",
            "https://user@github.com/",
            "https://example.com/github.com",
            "https://example.com\\@github.com/",
            "https://github.com\\@example.com/",
            "https://github.com/a b",
            "https://github.com/\n",
            "https://raw.githubusercontent.com/brandonbews/easyvox/main/latest.json",
            "",
        ] {
            assert!(!is_trusted_url(bad), "{bad:?}");
        }
    }

    #[test]
    fn errors_read_as_sentences() {
        let offline = UpdateError::Offline("The Internet connection appears to be offline.".into());
        assert_eq!(offline.to_string(), "Couldn't check for updates. The Internet connection appears to be offline.");
        assert_eq!(UpdateError::Network("timed out".into()).to_string(), "Couldn't check for updates. Timed out.");
        assert_eq!(
            UpdateError::Http(404).to_string(),
            "Couldn't check for updates. No release is listed on easyvox.dev yet."
        );
        assert_eq!(
            UpdateError::Http(302).to_string(),
            "Couldn't check for updates. The check was sent to an address EasyVox doesn't accept (HTTP 302)."
        );
        assert_eq!(
            UpdateError::Http(503).to_string(),
            "Couldn't check for updates. The server answered with an error (HTTP 503)."
        );
        assert_eq!(
            UpdateError::Invalid("\"url\" is missing".into()).to_string(),
            "Couldn't check for updates. The answer from easyvox.dev isn't valid: \"url\" is missing."
        );
    }

    /// The real transport against a server on 127.0.0.1 (no internet): the body, an error
    /// status, an oversized answer, and a redirect `follow` refuses.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_transport_reads_the_file_and_keeps_to_the_limits() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        fn serve(response: Vec<u8>) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/latest.json", listener.local_addr().unwrap());
            std::thread::spawn(move || {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let _ = (&stream).write_all(&response);
            });
            url
        }
        fn answer(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
            let mut r = format!("HTTP/1.1 {status}\r\nConnection: close\r\n{headers}\r\n").into_bytes();
            r.extend_from_slice(body);
            r
        }
        let any = |_: &str| true;

        let file = latest("0.1.1");
        let ok = answer("200 OK", &format!("Content-Length: {}\r\n", file.len()), &file);
        assert_eq!(fetch(&serve(ok), any), Ok(file));

        let missing = answer("404 Not Found", "Content-Length: 0\r\n", b"");
        assert_eq!(fetch(&serve(missing), any), Err(UpdateError::Http(404)));

        let big = vec![b' '; MAX_BYTES + 1];
        let announced = answer("200 OK", &format!("Content-Length: {}\r\n", big.len()), &big);
        assert_eq!(fetch(&serve(announced), any), Err(UpdateError::TooLarge));
        let unannounced = answer("200 OK", "", &big);
        assert_eq!(fetch(&serve(unannounced), any), Err(UpdateError::TooLarge));

        let redirect = answer("302 Found", "Location: http://127.0.0.2/elsewhere\r\nContent-Length: 0\r\n", b"");
        let not_there = |url: &str| !url.contains("127.0.0.2");
        assert_eq!(fetch(&serve(redirect), not_there), Err(UpdateError::Http(302)));
    }
}
