//! Explicit, inform-only release check: one GitHub API request per user click.
//! Nothing is downloaded, installed, scheduled or persisted.
use serde::Deserialize;
use std::{cmp::Ordering, path::Path, sync::Arc, time::Duration};
use ureq::{
    ProxyProtocol,
    tls::{Certificate, RootCerts, TlsConfig, TlsProvider},
};

pub const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/ChrisLauinger77/code-convoy/releases/latest";
pub const HOMEBREW_COMMAND: &str =
    "brew update && brew upgrade --cask ChrisLauinger77/cask/code-convoy";
/// One command per line: Windows PowerShell 5.1, which Scoop supports, cannot
/// parse `&&` pipeline chains.
pub const SCOOP_COMMAND: &str = "scoop update\nscoop update code-convoy";
/// Scoop updates global apps only with `--global` and administrator rights.
pub const SCOOP_GLOBAL_COMMAND: &str = "scoop update\nscoop update code-convoy --global";
/// Covers name resolution, connection, TLS, redirects and the response body.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Release JSON carries notes and asset lists; real responses are far smaller.
const BODY_LIMIT: u64 = 1024 * 1024;
/// GitHub error messages are prose; diagnostics keep one bounded line.
const MESSAGE_LIMIT: usize = 300;
/// Shared by the Homebrew cask token and the Scoop app directory.
const PACKAGE: &str = "code-convoy";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: semver::Version,
    /// The release's GitHub page, opened only by an explicit link activation.
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The latest release is not newer, including when this build is newer.
    UpToDate { latest: semver::Version },
    Available {
        release: Release,
        source: InstallSource,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallSource {
    Homebrew,
    Scoop,
    /// Scoop's default global root, `%ProgramData%\scoop`.
    ScoopGlobal,
    Unknown,
}

impl InstallSource {
    /// Package manager name and its update command, when clearly detected.
    pub fn update(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Homebrew => Some(("Homebrew", HOMEBREW_COMMAND)),
            Self::Scoop => Some(("Scoop", SCOOP_COMMAND)),
            Self::ScoopGlobal => Some(("Scoop", SCOOP_GLOBAL_COMMAND)),
            Self::Unknown => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Network,
    RateLimited,
    Http,
    InvalidResponse,
    InvalidVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    /// Copyable diagnostics. Never includes response bodies that name the client.
    pub detail: String,
}

impl Failure {
    pub fn new(kind: FailureKind, detail: impl AsRef<str>) -> Self {
        Self {
            kind,
            detail: format!("GET {LATEST_RELEASE_API}\n{}", detail.as_ref()),
        }
    }
    pub fn summary(&self) -> &'static str {
        match self.kind {
            FailureKind::Network => {
                "GitHub could not be reached. Check the network connection or proxy, then try again."
            }
            FailureKind::RateLimited => "GitHub's API rate limit was reached. Try again later.",
            FailureKind::Http => "GitHub returned an error. Try again later.",
            FailureKind::InvalidResponse => "GitHub's release information could not be read.",
            FailureKind::InvalidVersion => "The release version could not be compared.",
        }
    }
}

/// Rate-limit response headers, kept verbatim for diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RateLimit {
    pub remaining: Option<String>,
    pub reset: Option<String>,
    pub retry_after: Option<String>,
}

/// Requests the latest release on the calling thread; callers use a worker.
pub fn check(current: &str, executable: Option<&Path>) -> Result<Outcome, Failure> {
    let release = fetch()?;
    let source = executable.map_or(InstallSource::Unknown, |path| {
        install_source(path, cfg!(windows))
    });
    compare(current, release, source)
}

fn fetch() -> Result<Release, Failure> {
    let mut response = agent()?
        .get(LATEST_RELEASE_API)
        .call()
        .map_err(|e| Failure::new(FailureKind::Network, format!("Network error: {e}")))?;
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let rate_limit = RateLimit {
        remaining: header("x-ratelimit-remaining"),
        reset: header("x-ratelimit-reset"),
        retry_after: header("retry-after"),
    };
    match response
        .body_mut()
        .with_config()
        .limit(BODY_LIMIT)
        .read_to_string()
    {
        Ok(body) => interpret(status, &rate_limit, &body),
        // An error status remains more useful than its unreadable body.
        Err(_) if !(200..300).contains(&status) => interpret(status, &rate_limit, ""),
        Err(ureq::Error::BodyExceedsLimit(limit)) => Err(Failure::new(
            FailureKind::InvalidResponse,
            format!("HTTP status: {status}\nRelease response exceeds {limit} bytes."),
        )),
        Err(e) => Err(Failure::new(
            FailureKind::Network,
            format!("HTTP status: {status}\nCould not read the response: {e}"),
        )),
    }
}

fn agent() -> Result<ureq::Agent, Failure> {
    // Without ureq's SOCKS support, an environment SOCKS proxy would be bypassed
    // with a direct connection. Refuse instead; never log the proxy URL.
    let proxy = ureq::Proxy::try_from_env();
    if proxy.as_ref().is_some_and(unsupported_proxy) {
        return Err(Failure::new(
            FailureKind::Network,
            "A SOCKS proxy is configured by environment variables. The update check supports HTTP(S) proxies only and did not connect directly.",
        ));
    }
    // Operating-system trust roots: no bundled CA list that ages with the
    // build, and roots installed by an administrator keep working.
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        let errors: Vec<_> = native.errors.iter().map(ToString::to_string).collect();
        return Err(Failure::new(
            FailureKind::Network,
            format!(
                "No trusted root certificates were found on this system.\n{}",
                errors.join("\n")
            ),
        ));
    }
    let roots: Vec<_> = native
        .certs
        .iter()
        .map(|der| Certificate::from_der(der.as_ref()).to_owned())
        .collect();
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .root_certs(RootCerts::new_with_certs(&roots))
        // ring avoids aws-lc's CMake/NASM build inputs. It is set for this
        // agent only rather than installed as a process-wide default.
        .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .build();
    Ok(ureq::Agent::config_builder()
        .proxy(proxy)
        .timeout_global(Some(TIMEOUT))
        .https_only(true)
        .http_status_as_error(false)
        .user_agent(format!("CodeConvoy/{}", env!("CARGO_PKG_VERSION")))
        .accept("application/vnd.github+json")
        .tls_config(tls)
        .build()
        .new_agent())
}

fn unsupported_proxy(proxy: &ureq::Proxy) -> bool {
    !matches!(proxy.protocol(), ProxyProtocol::Http | ProxyProtocol::Https)
        && !LATEST_RELEASE_API
            .parse()
            .is_ok_and(|uri| proxy.is_no_proxy(&uri))
}

/// Maps a GitHub API response to a release or a diagnosable failure.
pub fn interpret(status: u16, rate_limit: &RateLimit, body: &str) -> Result<Release, Failure> {
    if (200..300).contains(&status) {
        return parse_release(body);
    }
    let message = github_message(body);
    let limited = status == 429
        || (status == 403
            && (rate_limit.remaining.as_deref() == Some("0")
                || rate_limit.retry_after.is_some()
                || message
                    .as_deref()
                    .is_some_and(|m| m.to_ascii_lowercase().contains("rate limit"))));
    if limited {
        // GitHub's primary-limit message names the client IP address. Omit the
        // body so copied diagnostics do not disclose it.
        let mut detail = format!("HTTP status: {status} (rate limited)");
        for (name, value) in [
            ("x-ratelimit-remaining", &rate_limit.remaining),
            ("x-ratelimit-reset", &rate_limit.reset),
            ("retry-after", &rate_limit.retry_after),
        ] {
            if let Some(value) = value {
                detail.push_str(&format!("\n{name}: {value}"));
            }
        }
        return Err(Failure::new(FailureKind::RateLimited, detail));
    }
    let mut detail = format!("HTTP status: {status}");
    if status == 404 {
        detail.push_str("\nNo published release was found.");
    }
    if let Some(message) = message {
        detail.push_str(&format!("\nGitHub message: {message}"));
    }
    Err(Failure::new(FailureKind::Http, detail))
}

fn github_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct ErrorBody {
        message: Option<String>,
    }
    let message = serde_json::from_str::<ErrorBody>(body).ok()?.message?;
    let line = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        None
    } else if line.chars().count() > MESSAGE_LIMIT {
        Some(line.chars().take(MESSAGE_LIMIT).chain(['…']).collect())
    } else {
        Some(line)
    }
}

/// Reads `tag_name` and `html_url` from GitHub's latest-release JSON.
pub fn parse_release(body: &str) -> Result<Release, Failure> {
    #[derive(Deserialize)]
    struct Latest {
        tag_name: Option<String>,
        html_url: Option<String>,
    }
    let invalid = |detail: String| Failure::new(FailureKind::InvalidResponse, detail);
    let latest: Latest =
        serde_json::from_str(body).map_err(|e| invalid(format!("Invalid release JSON: {e}")))?;
    let tag = latest
        .tag_name
        .filter(|tag| !tag.is_empty())
        .ok_or_else(|| invalid("Release response has no tag_name.".into()))?;
    let url = latest
        .html_url
        .filter(|url| !url.is_empty())
        .ok_or_else(|| invalid("Release response has no html_url.".into()))?;
    let version = tag_version(&tag).map_err(|e| {
        Failure::new(
            FailureKind::InvalidVersion,
            format!("Release tag {tag:?} is not a semantic version: {e}"),
        )
    })?;
    if !url.starts_with("https://github.com/") {
        return Err(invalid(format!(
            "Release page is not an https://github.com/ address: {url:?}"
        )));
    }
    Ok(Release { version, url })
}

/// Accepts one optional leading `v`, as used by CodeConvoy release tags.
pub fn tag_version(tag: &str) -> Result<semver::Version, semver::Error> {
    semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag))
}

pub fn compare(current: &str, release: Release, source: InstallSource) -> Result<Outcome, Failure> {
    let running = semver::Version::parse(current).map_err(|e| {
        Failure::new(
            FailureKind::InvalidVersion,
            format!("Running version {current:?} is not a semantic version: {e}"),
        )
    })?;
    // Precedence ignores build metadata; a pre-release sorts before its release.
    Ok(match release.version.cmp_precedence(&running) {
        Ordering::Greater => Outcome::Available { release, source },
        Ordering::Equal | Ordering::Less => Outcome::UpToDate {
            latest: release.version,
        },
    })
}

/// Recognizes only this application's Homebrew Caskroom (macOS/Linux) or Scoop
/// app directory (Windows). Anything else, including a cask-installed app moved
/// to `/Applications`, is Unknown so only the release link is shown. Scoop apps
/// under `ProgramData\scoop` are global; other `scoop` roots are per-user. A
/// custom global root named `scoop` makes Scoop itself ask for `--global`.
pub fn install_source(executable: &Path, windows: bool) -> InstallSource {
    let path = executable.to_string_lossy();
    let separators: &[char] = if windows { &['/', '\\'] } else { &['/'] };
    let parts: Vec<&str> = path.split(separators).filter(|p| !p.is_empty()).collect();
    let contains = |expected: &[&str]| {
        parts.windows(expected.len()).any(|window| {
            window
                .iter()
                .zip(expected)
                .all(|(part, name)| part.eq_ignore_ascii_case(name))
        })
    };
    if windows {
        if contains(&["ProgramData", "scoop", "apps", PACKAGE]) {
            return InstallSource::ScoopGlobal;
        }
        if contains(&["scoop", "apps", PACKAGE]) {
            return InstallSource::Scoop;
        }
    } else if contains(&["Caskroom", PACKAGE]) {
        return InstallSource::Homebrew;
    }
    InstallSource::Unknown
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FIXTURE: &str = include_str!("../tests/fixtures/github-latest-release.json");
    const RATE_LIMIT_FIXTURE: &str = include_str!("../tests/fixtures/github-rate-limit.json");

    fn release(tag: &str) -> Release {
        Release {
            version: tag_version(tag).unwrap(),
            url: format!("https://github.com/ChrisLauinger77/code-convoy/releases/tag/{tag}"),
        }
    }
    fn outcome(current: &str, tag: &str) -> Outcome {
        compare(current, release(tag), InstallSource::Unknown).unwrap()
    }
    fn available(current: &str, tag: &str) -> bool {
        matches!(outcome(current, tag), Outcome::Available { .. })
    }
    fn release_json(tag: &str) -> String {
        FIXTURE.replace("\"v1.3.0\"", &serde_json::to_string(tag).unwrap())
    }

    #[test]
    fn api_endpoint_matches_the_package_repository() {
        let repository = env!("CARGO_PKG_REPOSITORY");
        assert_eq!(
            LATEST_RELEASE_API,
            format!(
                "{}/releases/latest",
                repository.replacen("https://github.com/", "https://api.github.com/repos/", 1)
            )
        );
        assert!(
            Failure::new(FailureKind::Network, "x")
                .detail
                .starts_with(&format!("GET {LATEST_RELEASE_API}\n"))
        );
    }

    #[test]
    fn fixture_release_parses_tag_and_page() {
        let parsed = parse_release(FIXTURE).unwrap();
        assert_eq!(parsed.version, semver::Version::new(1, 3, 0));
        assert_eq!(
            parsed.url,
            "https://github.com/ChrisLauinger77/code-convoy/releases/tag/v1.3.0"
        );
        assert_eq!(interpret(200, &RateLimit::default(), FIXTURE), Ok(parsed));
    }

    #[test]
    fn newer_equal_and_older_releases() {
        let newer = compare("1.2.0", release("v1.3.0"), InstallSource::Scoop).unwrap();
        assert_eq!(
            newer,
            Outcome::Available {
                release: release("v1.3.0"),
                source: InstallSource::Scoop
            }
        );
        assert!(available("1.2.0", "v1.2.1"));
        assert!(available("1.2.9", "v1.10.0"));
        assert_eq!(
            outcome("1.2.0", "v1.2.0"),
            Outcome::UpToDate {
                latest: semver::Version::new(1, 2, 0)
            }
        );
        // A local build ahead of the latest release is not offered a downgrade.
        assert_eq!(
            outcome("1.4.0", "v1.3.0"),
            Outcome::UpToDate {
                latest: semver::Version::new(1, 3, 0)
            }
        );
    }

    #[test]
    fn one_leading_v_is_optional() {
        assert_eq!(
            tag_version("v1.2.3").unwrap(),
            semver::Version::new(1, 2, 3)
        );
        assert_eq!(tag_version("1.2.3").unwrap(), semver::Version::new(1, 2, 3));
        assert!(available("1.2.0", "1.3.0"));
        for tag in ["vv1.2.3", "V1.2.3", " v1.2.3", "v"] {
            assert!(tag_version(tag).is_err(), "{tag}");
        }
    }

    #[test]
    fn pre_releases_use_semantic_precedence() {
        assert!(available("1.2.0", "v1.3.0-rc.1"));
        assert!(available("1.3.0-rc.1", "v1.3.0"));
        assert!(available("1.3.0-rc.1", "v1.3.0-rc.2"));
        assert!(available("1.3.0-alpha", "v1.3.0-beta"));
        assert!(!available("1.3.0", "v1.3.0-rc.1"));
        assert!(!available("1.3.0-rc.2", "v1.3.0-rc.1"));
        // Build metadata does not affect precedence.
        assert!(!available("1.3.0", "v1.3.0+build.7"));
        assert!(!available("1.3.0+local", "v1.3.0"));
    }

    #[test]
    fn invalid_tags_and_running_versions_are_errors() {
        for tag in [
            "latest",
            "v1.2",
            "1.2.0.0",
            "release-1.2.0",
            "v01.2.0",
            "nightly",
        ] {
            let error = parse_release(&release_json(tag)).unwrap_err();
            assert_eq!(error.kind, FailureKind::InvalidVersion, "{tag}");
            assert!(
                error.detail.contains(&format!("{tag:?}")),
                "{}",
                error.detail
            );
        }
        let error = compare("unknown", release("v1.3.0"), InstallSource::Unknown).unwrap_err();
        assert_eq!(error.kind, FailureKind::InvalidVersion);
        assert!(error.detail.contains("Running version \"unknown\""));
    }

    #[test]
    fn missing_fields_and_invalid_json_are_errors() {
        let without = |field: &str| {
            let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
            value.as_object_mut().unwrap().remove(field);
            value.to_string()
        };
        for (body, expected) in [
            (without("tag_name"), "no tag_name"),
            (without("html_url"), "no html_url"),
            (release_json(""), "no tag_name"),
            (FIXTURE.replace("\"v1.3.0\",", "null,"), "no tag_name"),
            ("{}".into(), "no tag_name"),
            ("".into(), "Invalid release JSON"),
            ("not json".into(), "Invalid release JSON"),
            ("[]".into(), "Invalid release JSON"),
            (FIXTURE[..FIXTURE.len() / 2].into(), "Invalid release JSON"),
            (
                FIXTURE.replace("\"v1.3.0\",", "130,"),
                "Invalid release JSON",
            ),
        ] {
            let error = parse_release(&body).unwrap_err();
            assert_eq!(error.kind, FailureKind::InvalidResponse, "{body}");
            assert!(error.detail.contains(expected), "{}", error.detail);
        }
    }

    #[test]
    fn release_pages_must_be_on_github() {
        for url in [
            "http://github.com/ChrisLauinger77/code-convoy/releases/tag/v1.3.0",
            "https://github.com.example/releases/tag/v1.3.0",
            "https://example.com/https://github.com/",
            "javascript:alert(1)",
        ] {
            let body = FIXTURE.replace(
                "https://github.com/ChrisLauinger77/code-convoy/releases/tag/v1.3.0",
                url,
            );
            let error = parse_release(&body).unwrap_err();
            assert_eq!(error.kind, FailureKind::InvalidResponse, "{url}");
        }
    }

    #[test]
    fn rate_limits_are_reported_without_the_client_address() {
        let exhausted = RateLimit {
            remaining: Some("0".into()),
            reset: Some("1793610000".into()),
            retry_after: None,
        };
        for (status, rate_limit) in [
            (403, exhausted.clone()),
            (429, exhausted.clone()),
            (429, RateLimit::default()),
            (403, RateLimit::default()),
            (
                403,
                RateLimit {
                    retry_after: Some("60".into()),
                    ..Default::default()
                },
            ),
        ] {
            let error = interpret(status, &rate_limit, RATE_LIMIT_FIXTURE).unwrap_err();
            assert_eq!(error.kind, FailureKind::RateLimited, "{status}");
            assert!(
                error
                    .detail
                    .contains(&format!("HTTP status: {status} (rate limited)"))
            );
            assert!(!error.detail.contains("203.0.113.7"), "{}", error.detail);
        }
        let error = interpret(403, &exhausted, RATE_LIMIT_FIXTURE).unwrap_err();
        assert!(error.detail.contains("x-ratelimit-remaining: 0"));
        assert!(error.detail.contains("x-ratelimit-reset: 1793610000"));
        assert!(error.summary().contains("rate limit"));
    }

    #[test]
    fn other_http_errors_keep_status_and_bounded_message() {
        let error =
            interpret(404, &RateLimit::default(), r#"{"message":"Not Found"}"#).unwrap_err();
        assert_eq!(error.kind, FailureKind::Http);
        assert!(error.detail.contains("HTTP status: 404"));
        assert!(error.detail.contains("No published release was found."));
        assert!(error.detail.contains("GitHub message: Not Found"));

        let blocked = r#"{"message":"Repository access blocked"}"#;
        let error = interpret(403, &RateLimit::default(), blocked).unwrap_err();
        assert_eq!(error.kind, FailureKind::Http);
        assert!(error.detail.contains("Repository access blocked"));

        let error = interpret(502, &RateLimit::default(), "<html>Bad gateway</html>").unwrap_err();
        assert_eq!(error.kind, FailureKind::Http);
        assert!(error.detail.ends_with("HTTP status: 502"));

        let long = serde_json::json!({ "message": format!("line\n{}", "x".repeat(400)) });
        let error = interpret(500, &RateLimit::default(), &long.to_string()).unwrap_err();
        let message = error.detail.lines().last().unwrap();
        assert!(message.starts_with("GitHub message: line x"));
        assert_eq!(
            message.chars().count(),
            "GitHub message: ".len() + MESSAGE_LIMIT + 1
        );
    }

    #[test]
    fn install_source_detects_only_clear_package_locations() {
        for (path, windows, expected) in [
            // Linux
            ("/usr/bin/codeconvoy", false, InstallSource::Unknown),
            (
                "/tmp/.mount_CodeCoAb12/usr/bin/codeconvoy",
                false,
                InstallSource::Unknown,
            ),
            (
                "/home/user/Applications/CodeConvoy.AppImage",
                false,
                InstallSource::Unknown,
            ),
            (
                "/home/linuxbrew/.linuxbrew/Caskroom/code-convoy/1.2.0/codeconvoy",
                false,
                InstallSource::Homebrew,
            ),
            (
                "/home/user/scoop/apps/code-convoy/current/codeconvoy",
                false,
                InstallSource::Unknown,
            ),
            // macOS
            (
                "/Applications/CodeConvoy.app/Contents/MacOS/codeconvoy",
                false,
                InstallSource::Unknown,
            ),
            (
                "/opt/homebrew/Caskroom/code-convoy/1.2.0/CodeConvoy.app/Contents/MacOS/codeconvoy",
                false,
                InstallSource::Homebrew,
            ),
            (
                "/usr/local/Caskroom/code-convoy/1.2.0/CodeConvoy.app/Contents/MacOS/codeconvoy",
                false,
                InstallSource::Homebrew,
            ),
            (
                "/opt/homebrew/Caskroom/other-app/1.0/CodeConvoy.app/Contents/MacOS/codeconvoy",
                false,
                InstallSource::Unknown,
            ),
            (
                "/Users/me/Caskroom-code-convoy/codeconvoy",
                false,
                InstallSource::Unknown,
            ),
            // Windows
            (
                r"C:\Users\me\scoop\apps\code-convoy\current\CodeConvoy.exe",
                true,
                InstallSource::Scoop,
            ),
            (
                r"C:\ProgramData\scoop\apps\code-convoy\1.2.0\CodeConvoy.exe",
                true,
                InstallSource::ScoopGlobal,
            ),
            (
                r"\\?\C:\programdata\Scoop\apps\code-convoy\current\CodeConvoy.exe",
                true,
                InstallSource::ScoopGlobal,
            ),
            (
                r"D:\scoop\apps\code-convoy\current\CodeConvoy.exe",
                true,
                InstallSource::Scoop,
            ),
            (
                r"C:\ProgramData\Tools\scoop\apps\code-convoy\current\CodeConvoy.exe",
                true,
                InstallSource::Scoop,
            ),
            (
                r"\\?\C:\Users\me\Scoop\Apps\Code-Convoy\current\CodeConvoy.exe",
                true,
                InstallSource::Scoop,
            ),
            (
                r"C:\Users\me\AppData\Local\Programs\CodeConvoy\CodeConvoy.exe",
                true,
                InstallSource::Unknown,
            ),
            (
                r"D:\Tools\apps\code-convoy\CodeConvoy.exe",
                true,
                InstallSource::Unknown,
            ),
            (
                r"C:\Users\me\scoop\apps\other\CodeConvoy.exe",
                true,
                InstallSource::Unknown,
            ),
            (
                r"C:\Users\me\Downloads\CodeConvoy\CodeConvoy.exe",
                true,
                InstallSource::Unknown,
            ),
            (
                r"C:\Users\me\Caskroom\code-convoy\CodeConvoy.exe",
                true,
                InstallSource::Unknown,
            ),
        ] {
            assert_eq!(
                install_source(&PathBuf::from(path), windows),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn socks_proxies_are_refused_unless_github_is_excluded() {
        for (proxy, refused) in [
            ("http://proxy.example:3128", false),
            ("https://proxy.example:3129", false),
            ("socks5://127.0.0.1:1080", true),
            ("socks5h://127.0.0.1:1080", true),
            ("socks4://127.0.0.1:1080", true),
        ] {
            assert_eq!(
                unsupported_proxy(&ureq::Proxy::new(proxy).unwrap()),
                refused,
                "{proxy}"
            );
        }
        let excluded = ureq::Proxy::builder(ProxyProtocol::Socks5)
            .host("127.0.0.1")
            .no_proxy("api.github.com")
            .build()
            .unwrap();
        assert!(!unsupported_proxy(&excluded));
    }

    #[test]
    fn update_commands_are_offered_only_for_detected_package_managers() {
        assert_eq!(
            InstallSource::Homebrew.update(),
            Some(("Homebrew", HOMEBREW_COMMAND))
        );
        assert_eq!(
            InstallSource::Scoop.update(),
            Some(("Scoop", SCOOP_COMMAND))
        );
        assert_eq!(
            InstallSource::ScoopGlobal.update(),
            Some(("Scoop", SCOOP_GLOBAL_COMMAND))
        );
        assert_eq!(InstallSource::Unknown.update(), None);
        // Windows PowerShell 5.1 rejects `&&`; each Scoop command has its own line.
        for command in [SCOOP_COMMAND, SCOOP_GLOBAL_COMMAND] {
            assert!(!command.contains("&&"), "{command}");
            assert_eq!(command.lines().next(), Some("scoop update"));
        }
        assert_eq!(
            SCOOP_GLOBAL_COMMAND.lines().last(),
            Some("scoop update code-convoy --global")
        );
    }
}
