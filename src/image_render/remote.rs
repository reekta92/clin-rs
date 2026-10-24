//! Credential-free HTTP(S) images. Validate every destination and DNS answer before connecting.
use anyhow::{Context, Result, bail};
use reqwest::{
    Url,
    dns::{Addrs, Name, Resolve, Resolving},
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

struct PublicDns;
impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addresses: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if addresses.is_empty()
                || addresses
                    .iter()
                    .any(|address| !public_address(address.ip()))
            {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "remote image DNS must resolve only to public addresses",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}
fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || a == 0
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && (b == 18 || b == 19)))
}
fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public_v4(ip);
            }
            let s = ip.segments();
            s[0] & 0xe000 == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001
                    && (s[1] == 0
                        || s[1] == 2
                        || s[1] == 0xdb8
                        || s[1] & 0xfff0 == 0x10
                        || s[1] & 0xfff0 == 0x20))
        }
    }
}
pub(crate) fn validate_url(source: &str) -> Result<Url> {
    if source.len() > 64 * 1024 {
        bail!("remote image URL exceeds 64 KiB limit");
    }
    let mut url = Url::parse(source).context("invalid remote image URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        bail!("remote images require credential-free HTTP(S) URLs");
    }
    let host = url.host_str().context("remote image URL has no host")?;
    if host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
        || host.ends_with(".local")
    {
        bail!("remote images cannot access local hosts");
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>()
        && !public_address(ip)
    {
        bail!("remote images cannot access private or reserved addresses");
    }
    url.set_fragment(None);
    Ok(url)
}
fn public_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .referer(false)
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(PublicDns))
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .build()?)
}
async fn transfer(source: &str, client: &reqwest::Client) -> Result<Vec<u8>> {
    let mut url = validate_url(source)?;
    for redirect in 0..=3 {
        let response = client
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "image/*")
            .send()
            .await
            .context("remote image request failed")?;
        if response.status().is_redirection() {
            if redirect == 3 {
                bail!("remote image exceeds three redirects");
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .context("remote image redirect has no Location")?
                .to_str()?;
            url = validate_url(url.join(location)?.as_str())?;
            continue;
        }
        response
            .error_for_status_ref()
            .context("remote image returned an error")?;
        return crate::http::read_bounded(response, 8 * 1024 * 1024).await;
    }
    unreachable!("redirect loop returns at bound")
}
pub(crate) fn load_remote_image(source: &str, cancelled: &AtomicBool) -> Result<Vec<u8>> {
    validate_url(source)?;
    crate::http::run(cancelled, Duration::from_secs(8), async {
        transfer(source, &public_client()?).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_images_reject_private_reserved_credentials_and_non_http_targets() {
        for url in [
            "file:///etc/passwd",
            "ftp://example.com/a.png",
            "http://user:pass@example.com/a.png",
            "http://127.1/a",
            "http://0.0.0.0/a",
            "http://10.1.2.3/a",
            "http://169.254.169.254/a",
            "http://192.168.1.1/a",
            "http://100.64.0.1/a",
            "http://198.18.0.1/a",
            "http://192.0.2.1/a",
            "http://[::1]/a",
            "http://[::ffff:127.0.0.1]/a",
            "http://[fc00::1]/a",
            "http://[2002:7f00:1::]/a",
            "http://localhost/a",
            "https://service.local/a",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700::1111"] {
            assert!(public_address(ip.parse().unwrap()), "{ip}");
        }
        assert_eq!(
            validate_url("https://example.com/a.png#ignored")
                .unwrap()
                .as_str(),
            "https://example.com/a.png"
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(
            load_remote_image(
                &format!(
                    "http://127.0.0.1:{}/a",
                    listener.local_addr().unwrap().port()
                ),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn streamed_http_images_enforce_status_redirect_and_body_limits() {
        use std::io::{Read, Write};
        for (response,expected) in [
            (b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nPNG".to_vec(),"PNG"),
            (b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),"private or reserved"),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 8388609\r\nConnection: close\r\n\r\n".to_vec(),"8388608 bytes"),
            (b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),"returned an error"),
        ] {
            let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
            let server=std::thread::spawn(move || {let (mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();let mut request=[0;4096];let _=stream.read(&mut request).unwrap();stream.write_all(&response).unwrap();});
            let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let result=runtime.block_on(async {
                // Trusted test-only DNS override reaches disposable transport fixture.
                let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).resolve_to_addrs("images.example",&[address]).timeout(Duration::from_secs(2)).build().unwrap();
                transfer(&format!("http://images.example:{}/cover.png",address.port()),&client).await
            });server.join().unwrap();
            if expected=="PNG" {assert_eq!(result.unwrap(),b"PNG");} else {assert!(format!("{:#}",result.unwrap_err()).contains(expected),"{expected}");}
        }
    }
    #[test]
    fn dns_answers_are_checked_before_connecting() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let name = "localhost".parse::<Name>().unwrap();
        let result = runtime.block_on(PublicDns.resolve(name));
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("public addresses")
        );
    }
    #[test]
    fn cancelled_remote_images_never_start_a_request() {
        assert!(
            load_remote_image("https://example.com/a.png", &AtomicBool::new(true))
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }
}
