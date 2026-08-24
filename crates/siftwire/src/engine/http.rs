use std::fmt;
use std::fs;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};
use ureq::{Agent, ResponseExt};
use url::Url;

use super::model::ResolveError;

const USER_AGENT: &str = "siftwire/0";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
// Receipt: receipts/http-response-size.json.
const MAX_DECODED_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct HttpClient {
    agent: Agent,
}

impl HttpClient {
    pub fn new() -> Self {
        let config = Agent::config_builder()
            .timeout_global(Some(HTTP_TIMEOUT))
            .user_agent(USER_AGENT)
            .proxy(None)
            .build();
        Self {
            agent: Agent::with_parts(
                config,
                DefaultConnector::default(),
                PublicResolver::default(),
            ),
        }
    }

    pub fn get(&self, endpoint: &str) -> Result<Vec<u8>> {
        self.get_with_headers(endpoint, &[])
    }

    /// Fetches an endpoint with extra request headers (e.g. Riot's `x-api-key`).
    pub fn get_with_headers(&self, endpoint: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
        if let Ok(parsed) = Url::parse(endpoint)
            && parsed.scheme() == "file"
        {
            return read_eval_file(&parsed);
        }
        let mut request = self.agent.get(endpoint);
        for (header, value) in headers {
            request = request.header(*header, *value);
        }
        let mut response = request
            .call()
            .map_err(|error| request_error(error, endpoint))?;
        read_response(&mut response).with_context(|| format!("read response from {endpoint}"))
    }

    pub fn get_until(
        &self,
        endpoint: &str,
        deadline: Instant,
    ) -> Result<(Vec<u8>, String), ResolveError> {
        let timeout = remaining(deadline)?;
        let mut response = self
            .agent
            .get(endpoint)
            .config()
            .timeout_global(Some(timeout))
            .build()
            .call()
            .map_err(|error| resolve_request_error(&error, endpoint))?;
        let final_url = response.get_uri().to_string();
        let bytes = read_response(&mut response)
            .map_err(|error| ResolveError::Message(error.to_string()))?;
        Ok((bytes, final_url))
    }

    pub fn follow_redirect_until(
        &self,
        endpoint: &str,
        deadline: Instant,
    ) -> Result<String, ResolveError> {
        let timeout = remaining(deadline)?;
        let response = self
            .agent
            .get(endpoint)
            .config()
            .timeout_global(Some(timeout))
            .build()
            .call()
            .map_err(|error| resolve_request_error(&error, endpoint))?;
        Ok(response.get_uri().to_string())
    }

    pub fn post_form_until(
        &self,
        endpoint: &str,
        field: &str,
        value: &str,
        deadline: Instant,
    ) -> Result<Vec<u8>, ResolveError> {
        let timeout = remaining(deadline)?;
        let mut response = self
            .agent
            .post(endpoint)
            .config()
            .timeout_global(Some(timeout))
            .build()
            .send_form([(field, value)])
            .map_err(|error| resolve_request_error(&error, endpoint))?;
        read_response(&mut response).map_err(|error| ResolveError::Message(error.to_string()))
    }
}

fn read_response(response: &mut ureq::http::Response<ureq::Body>) -> Result<Vec<u8>> {
    read_limited(response.body_mut().as_reader(), MAX_DECODED_RESPONSE_BYTES)
        .context("read HTTP response body")
}

fn read_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let read_limit = u64::try_from(limit)
        .context("response size limit exceeds u64")?
        .checked_add(1)
        .context("response size limit exceeds u64")?;
    let mut bytes = Vec::new();
    let _byte_count = reader
        .take(read_limit)
        .read_to_end(&mut bytes)
        .context("read decoded response body")?;
    if bytes.len() > limit {
        bail!("decoded response body exceeds {limit}-byte limit");
    }
    Ok(bytes)
}

struct PublicResolver<R = DefaultResolver> {
    inner: R,
}

impl Default for PublicResolver {
    fn default() -> Self {
        Self {
            inner: DefaultResolver::default(),
        }
    }
}

impl<R: fmt::Debug> fmt::Debug for PublicResolver<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PublicResolver").finish()
    }
}

impl<R: Resolver> Resolver for PublicResolver<R> {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let resolved = self.inner.resolve(uri, config, timeout)?;
        let mut public = self.empty();
        for address in resolved
            .iter()
            .copied()
            .filter(|address| is_public_ip(address.ip()))
        {
            public.push(address);
        }
        if public.is_empty() {
            Err(ureq::Error::HostNotFound)
        } else {
            Ok(public)
        }
    }
}

const fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

const fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, third, _fourth] = address.octets();
    !(matches!(first, 0 | 10 | 127 | 224..=u8::MAX)
        || (first == 100 && matches!(second, 64..=127))
        || (first == 169 && second == 254)
        || (first == 172 && matches!(second, 16..=31))
        || (first == 192
            && ((second == 0 && matches!(third, 0 | 2))
                || (second == 88 && third == 99)
                || second == 168))
        || (first == 198 && matches!(second, 18..=19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113))
}

const fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let [
        first,
        second,
        _third,
        _fourth,
        _fifth,
        _sixth,
        _seventh,
        _eighth,
    ] = address.segments();
    matches!(first, 0x2000..=0x3fff)
        && !(first == 0x2001 && second <= 0x01ff)
        && !(first == 0x2001 && second == 0x0db8)
        && first != 0x2002
        && first != 0x3ffe
        && !(first == 0x3fff && second <= 0x0fff)
}

fn read_eval_file(url: &Url) -> Result<Vec<u8>> {
    if std::env::var("SIFTWIRE_EVAL_ALLOW_FILE_URLS").as_deref() != Ok("1") {
        bail!("file URLs are only available for isolated eval fixtures");
    }
    let path = url
        .to_file_path()
        .map_err(|()| anyhow!("file URL must include a path"))?;
    fs::read(path).context("read eval feed fixture")
}

fn remaining(deadline: Instant) -> Result<Duration, ResolveError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(ResolveError::Timeout)
    } else {
        Ok(remaining)
    }
}

fn request_error(error: ureq::Error, endpoint: &str) -> anyhow::Error {
    if let ureq::Error::StatusCode(code) = error {
        anyhow!("HTTP {code} for {endpoint}")
    } else {
        anyhow!(error)
    }
}

fn resolve_request_error(error: &ureq::Error, endpoint: &str) -> ResolveError {
    if matches!(error, ureq::Error::Timeout(_)) {
        return ResolveError::Timeout;
    }
    if let ureq::Error::StatusCode(code) = error {
        return ResolveError::Message(format!("HTTP {code} for {endpoint}"));
    }
    ResolveError::Message(error.to_string())
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::panic_in_result_fn,
        reason = "HTTP behavior tests return Result for setup while assertions report contract failures"
    )]

    use std::io::Cursor;
    use std::net::SocketAddr;

    use anyhow::{Context, Result, bail};
    use ureq::Timeout;
    use ureq::config::Config;
    use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
    use ureq::unversioned::transport::NextTimeout;
    use ureq::unversioned::transport::time::Duration;

    use super::{HttpClient, PublicResolver, is_public_ip, read_limited};

    #[derive(Debug)]
    struct StaticResolver {
        addresses: Vec<SocketAddr>,
    }

    impl Resolver for StaticResolver {
        fn resolve(
            &self,
            _uri: &ureq::http::Uri,
            _config: &Config,
            _timeout: NextTimeout,
        ) -> std::result::Result<ResolvedSocketAddrs, ureq::Error> {
            let mut resolved = self.empty();
            for address in &self.addresses {
                resolved.push(*address);
            }
            Ok(resolved)
        }
    }

    #[derive(Debug)]
    struct RedirectResolver;

    impl Resolver for RedirectResolver {
        fn resolve(
            &self,
            uri: &ureq::http::Uri,
            _config: &Config,
            _timeout: NextTimeout,
        ) -> std::result::Result<ResolvedSocketAddrs, ureq::Error> {
            let address = match uri.host() {
                Some("public.example") => "93.184.216.34:80",
                Some("redirect.example") => "169.254.169.254:80",
                _ => return Err(ureq::Error::HostNotFound),
            }
            .parse()
            .map_err(|_error| ureq::Error::HostNotFound)?;
            let mut resolved = self.empty();
            resolved.push(address);
            Ok(resolved)
        }
    }

    fn timeout() -> NextTimeout {
        NextTimeout {
            after: Duration::NotHappening,
            reason: Timeout::Resolve,
        }
    }

    #[test]
    fn private_literal_cannot_connect() -> Result<()> {
        let Err(error) = HttpClient::new().get("http://127.0.0.1:9/private") else {
            bail!("private literal unexpectedly connected");
        };
        assert!(
            error.to_string().contains("host not found"),
            "private literal returned the wrong error: {error}"
        );
        Ok(())
    }

    #[test]
    fn resolver_keeps_only_public_dns_results() -> Result<()> {
        let resolver = PublicResolver {
            inner: StaticResolver {
                addresses: [
                    "10.0.0.1:80",
                    "169.254.169.254:80",
                    "93.184.216.34:80",
                    "[2606:4700:4700::1111]:443",
                ]
                .into_iter()
                .map(str::parse)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            },
        };
        let uri = "https://feed.example/".parse()?;
        let filtered = resolver.resolve(&uri, &Config::default(), timeout())?;
        let addresses = filtered.iter().copied().collect::<Vec<_>>();
        assert_eq!(
            addresses,
            [
                "93.184.216.34:80".parse()?,
                "[2606:4700:4700::1111]:443".parse()?
            ],
            "resolver did not remove every private address"
        );
        Ok(())
    }

    #[test]
    fn redirect_target_resolution_filters_metadata_address() -> Result<()> {
        let resolver = PublicResolver {
            inner: RedirectResolver,
        };
        let initial = "http://public.example/start".parse()?;
        assert!(
            resolver
                .resolve(&initial, &Config::default(), timeout())
                .is_ok(),
            "public redirect source was filtered"
        );
        let target = "http://redirect.example/latest/meta-data".parse()?;
        let Err(error) = resolver.resolve(&target, &Config::default(), timeout()) else {
            bail!("private redirect target was accepted");
        };
        assert!(
            matches!(error, ureq::Error::HostNotFound),
            "private redirect target returned the wrong error: {error}"
        );
        Ok(())
    }

    #[test]
    fn public_address_classifier_rejects_non_public_ranges() -> Result<()> {
        for address in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.0.0.1",
            "192.168.0.1",
            "198.18.0.1",
            "224.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "fc00::1",
            "fe80::1",
        ] {
            let parsed = address
                .parse()
                .with_context(|| format!("parse test IP address {address}"))?;
            assert!(!is_public_ip(parsed), "accepted non-public IP {address}");
        }
        for address in ["8.8.8.8", "93.184.216.34", "2606:4700:4700::1111"] {
            let parsed = address
                .parse()
                .with_context(|| format!("parse test IP address {address}"))?;
            assert!(is_public_ip(parsed), "filtered public IP {address}");
        }
        Ok(())
    }

    #[test]
    fn decoded_body_overflow_is_rejected() -> Result<()> {
        let Err(error) = read_limited(Cursor::new(b"12345"), 4) else {
            bail!("oversized decoded body was accepted");
        };
        assert!(
            error
                .to_string()
                .contains("decoded response body exceeds 4-byte limit"),
            "decoded overflow returned the wrong error: {error}"
        );
        Ok(())
    }
}
