//! RFC 4791 discovery and manual, conditional CalDAV synchronization.
use super::{
    ical,
    store::{Collection, Conflict, Remote, Resource, Store},
};
use crate::storage::Storage;
use anyhow::{Context, Result, bail, ensure};
use quick_xml::{NsReader, events::Event, name::ResolveResult};
use reqwest::{Client, Method, StatusCode, Url};
use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

const DAV: &str = "DAV:";
const CAL: &str = "urn:ietf:params:xml:ns:caldav";
const PROPERTIES: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:resourcetype/><d:displayname/><d:current-user-principal/><c:calendar-home-set/><d:current-user-privilege-set/><c:supported-calendar-component-set/></d:prop></d:propfind>"#;
const QUERY: &str = r#"<?xml version="1.0" encoding="utf-8"?><c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><c:calendar-data/></d:prop><c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT"/></c:comp-filter></c:filter></c:calendar-query>"#;

pub use crate::config::CalDavAccount as Account;

pub fn validate_url(text: &str) -> Result<Url> {
    let url = Url::parse(text).context("Invalid CalDAV URL")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "CalDAV requires HTTPS (HTTP allowed only on localhost)"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.query().is_none(),
        "CalDAV URL cannot contain credentials, query, or fragment"
    );
    Ok(url)
}

impl Account {
    pub fn validate(&self) -> Result<()> {
        validate_url(&self.url)?;
        ensure!(
            self.username.is_empty() == self.password_env.is_empty(),
            "CalDAV username and password_env must be configured together"
        );
        ensure!(
            self.password_env.is_empty()
                || self
                    .password_env
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Invalid CalDAV password_env name"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct Node {
    ns: String,
    name: String,
    text: String,
    attributes: BTreeMap<String, String>,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, ns: &str, name: &str) -> Option<&Self> {
        self.children.iter().find(|c| c.ns == ns && c.name == name)
    }
    fn descendants<'a>(
        &'a self,
        ns: &'a str,
        name: &'a str,
    ) -> Box<dyn Iterator<Item = &'a Self> + 'a> {
        Box::new(self.children.iter().flat_map(move |c| {
            std::iter::once(c)
                .filter(move |c| c.ns == ns && c.name == name)
                .chain(c.descendants(ns, name))
        }))
    }
}

fn xml(source: &str) -> Result<Node> {
    ensure!(
        source.len() <= ical::MAX_BYTES,
        "CalDAV response exceeds 16 MiB"
    );
    let mut reader = NsReader::from_str(source);
    let mut version = quick_xml::XmlVersion::Implicit1_0;
    let mut stack = vec![Node::default()];
    loop {
        let (ns, event) = reader.read_resolved_event()?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let namespace = match ns {
                    ResolveResult::Bound(ns) => String::from_utf8(ns.as_ref().to_vec())?,
                    ResolveResult::Unbound => String::new(),
                    ResolveResult::Unknown(_) => bail!("Undeclared XML namespace"),
                };
                let mut node = Node {
                    ns: namespace,
                    name: String::from_utf8(e.local_name().as_ref().to_vec())?,
                    ..Node::default()
                };
                for attr in e.attributes() {
                    let attr = attr?;
                    node.attributes.insert(
                        String::from_utf8(attr.key.local_name().as_ref().to_vec())?,
                        attr.decoded_and_normalized_value(version, reader.decoder())?
                            .into_owned(),
                    );
                }
                if empty {
                    stack.last_mut().context("Invalid XML")?.children.push(node);
                } else {
                    stack.push(node);
                    ensure!(stack.len() <= 32, "CalDAV XML nesting exceeds safety limit");
                }
            }
            Event::End(_) => {
                ensure!(stack.len() > 1, "Unbalanced CalDAV XML");
                let node = stack.pop().context("Invalid XML")?;
                stack.last_mut().context("Invalid XML")?.children.push(node);
            }
            Event::Text(e) => stack
                .last_mut()
                .context("Invalid XML")?
                .text
                .push_str(&e.xml_content(version)?),
            Event::CData(e) => stack
                .last_mut()
                .context("Invalid XML")?
                .text
                .push_str(&e.decode()?),
            Event::GeneralRef(e) => {
                let escaped = format!("&{};", e.decode()?);
                stack
                    .last_mut()
                    .context("Invalid XML")?
                    .text
                    .push_str(&quick_xml::escape::unescape(&escaped)?);
            }
            Event::DocType(_) => bail!("CalDAV XML document types are not allowed"),
            Event::Decl(e) => version = e.xml_version()?,
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(stack.len() == 1, "Incomplete CalDAV XML");
    let mut root = stack.pop().context("Invalid XML")?;
    ensure!(root.children.len() == 1, "Expected one CalDAV XML document");
    let root = root.children.remove(0);
    ensure!(
        root.ns == DAV && root.name == "multistatus",
        "Expected DAV:multistatus"
    );
    Ok(root)
}

struct Response {
    href: String,
    properties: Node,
}

fn responses(source: &str) -> Result<Vec<Response>> {
    let root = xml(source)?;
    ensure!(
        root.descendants(DAV, "error").next().is_none()
            && root
                .children
                .iter()
                .all(|n| n.ns == DAV
                    && matches!(n.name.as_str(), "response" | "responsedescription")),
        "Unexpected CalDAV multistatus content; no deletions applied"
    );
    let mut output = Vec::new();
    for response in root
        .children
        .iter()
        .filter(|r| r.ns == DAV && r.name == "response")
    {
        if let Some(status) = response.child(DAV, "status") {
            ensure!(
                status.text.split_whitespace().nth(1) == Some("200"),
                "CalDAV response contains a failed resource"
            );
        }
        let mut properties = Node::default();
        for propstat in response
            .children
            .iter()
            .filter(|p| p.ns == DAV && p.name == "propstat")
        {
            let status = propstat
                .child(DAV, "status")
                .context("Missing CalDAV property status")?;
            if status.text.split_whitespace().nth(1) == Some("200") {
                properties.children.extend(
                    propstat
                        .child(DAV, "prop")
                        .context("Missing CalDAV properties")?
                        .children
                        .clone(),
                );
            }
        }
        output.push(Response {
            href: response
                .child(DAV, "href")
                .context("Missing CalDAV href")?
                .text
                .trim()
                .into(),
            properties,
        });
    }
    Ok(output)
}

fn supports_events(item: &Response) -> bool {
    item.properties
        .child(CAL, "supported-calendar-component-set")
        .is_none_or(|set| {
            set.children.iter().any(|c| {
                c.ns == CAL
                    && c.name == "comp"
                    && c.attributes
                        .get("name")
                        .is_some_and(|name| name == "VEVENT")
            })
        })
}

fn report_resources(collection: &Url, source: &str) -> Result<BTreeMap<String, Remote>> {
    let mut output = BTreeMap::new();
    let mut uids = HashSet::new();
    for item in responses(source)? {
        let href = resource_url(collection, &item.href)?.to_string();
        let source = item
            .properties
            .child(CAL, "calendar-data")
            .context("Incomplete calendar-query: missing calendar-data; no deletions applied")?
            .text
            .clone();
        let etag = item
            .properties
            .child(DAV, "getetag")
            .context("Incomplete calendar-query: missing ETag; no deletions applied")?
            .text
            .trim()
            .to_owned();
        validate_etag(&etag)?;
        ensure!(
            uids.insert(ical::uid(&ical::parse(&source)?)?),
            "Duplicate CalDAV event UID; no deletions applied"
        );
        ensure!(
            output
                .insert(href.clone(), Remote { href, source, etag })
                .is_none(),
            "Duplicate CalDAV href"
        );
    }
    Ok(output)
}

fn resolve(origin: &Url, base: &Url, href: &str) -> Result<Url> {
    let url = base.join(href).context("Invalid CalDAV href")?;
    validate_url(url.as_str())?;
    ensure!(
        url.origin() == origin.origin(),
        "Cross-origin CalDAV href rejected; credentials were not sent"
    );
    Ok(url)
}

fn resource_url(collection: &Url, href: &str) -> Result<Url> {
    let url = resolve(collection, collection, href)?;
    ensure!(
        url.path().starts_with(collection.path()) && url.path() != collection.path(),
        "CalDAV event is outside its calendar collection"
    );
    Ok(url)
}

struct Connection {
    client: Client,
    origin: Url,
    username: String,
    password: Option<String>,
}

impl Connection {
    fn new(account: &Account) -> Result<Self> {
        account.validate()?;
        let password = if account.password_env.is_empty() {
            None
        } else {
            let value = std::env::var(&account.password_env).with_context(|| {
                format!(
                    "Set the {} environment variable before starting clin",
                    account.password_env
                )
            })?;
            ensure!(
                !value.is_empty(),
                "CalDAV password environment variable is empty"
            );
            Some(value)
        };
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .user_agent("clin-calendar")
                .build()?,
            origin: validate_url(&account.url)?,
            username: account.username.clone(),
            password,
        })
    }

    fn request(&self, method: &str, url: &Url) -> Result<reqwest::RequestBuilder> {
        resolve(&self.origin, url, url.as_str())?;
        let request = self
            .client
            .request(Method::from_bytes(method.as_bytes())?, url.clone());
        Ok(if let Some(password) = &self.password {
            request.basic_auth(&self.username, Some(password))
        } else {
            request
        })
    }

    async fn text(response: reqwest::Response) -> Result<String> {
        ensure!(
            response
                .content_length()
                .is_none_or(|n| n <= ical::MAX_BYTES as u64),
            "CalDAV response exceeds 16 MiB"
        );
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(reqwest::Error::without_url)?
        {
            ensure!(
                bytes.len() + chunk.len() <= ical::MAX_BYTES,
                "CalDAV response exceeds 16 MiB"
            );
            bytes.extend_from_slice(&chunk);
        }
        String::from_utf8(bytes).context("CalDAV response is not UTF-8")
    }

    async fn propfind(&self, url: &Url, depth: &str) -> Result<Vec<Response>> {
        let response = self
            .request("PROPFIND", url)?
            .header("Depth", depth)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(PROPERTIES)
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        ensure!(
            response.status().as_u16() == 207,
            "CalDAV discovery returned HTTP {}",
            response.status().as_u16()
        );
        responses(&Self::text(response).await?)
    }

    async fn discover(&self, account: &Account) -> Result<Vec<Collection>> {
        let initial = self.propfind(&self.origin, "0").await?;
        let first = initial
            .first()
            .context("CalDAV discovery returned no resources")?;
        if first
            .properties
            .child(DAV, "resourcetype")
            .is_some_and(|r| r.child(CAL, "calendar").is_some())
        {
            ensure!(
                supports_events(first),
                "This calendar does not support VEVENT"
            );
            return Ok(vec![self.collection(account, &self.origin, first)?]);
        }
        let mut home_base = self.origin.clone();
        let mut home = first
            .properties
            .child(CAL, "calendar-home-set")
            .and_then(|n| n.child(DAV, "href"))
            .map(|n| n.text.clone());
        if home.is_none()
            && let Some(principal) = first
                .properties
                .child(DAV, "current-user-principal")
                .and_then(|n| n.child(DAV, "href"))
        {
            let url = resolve(&self.origin, &self.origin, principal.text.trim())?;
            let principal = self.propfind(&url, "0").await?;
            home_base = url;
            home = principal.iter().find_map(|r| {
                r.properties
                    .child(CAL, "calendar-home-set")
                    .and_then(|n| n.child(DAV, "href"))
                    .map(|n| n.text.clone())
            });
        }
        let home = home.map_or_else(
            || Ok(self.origin.clone()),
            |href| resolve(&self.origin, &home_base, href.trim()),
        )?;
        let listing = self.propfind(&home, "1").await?;
        let mut calendars = Vec::new();
        for item in listing {
            if item
                .properties
                .child(DAV, "resourcetype")
                .is_some_and(|r| r.child(CAL, "calendar").is_some())
                && supports_events(&item)
            {
                calendars.push(self.collection(account, &home, &item)?);
            }
        }
        ensure!(
            !calendars.is_empty(),
            "No VEVENT calendars discovered; configure a calendar collection URL"
        );
        calendars.sort_by(|a, b| a.url.cmp(&b.url));
        calendars.dedup_by(|a, b| a.url == b.url);
        Ok(calendars)
    }

    fn collection(&self, account: &Account, base: &Url, item: &Response) -> Result<Collection> {
        let mut url = resolve(&self.origin, base, &item.href)?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        let privileges = item.properties.child(DAV, "current-user-privilege-set");
        let writable = privileges.is_none_or(|p| {
            p.descendants(DAV, "write").next().is_some()
                || p.descendants(DAV, "all").next().is_some()
        });
        let name = item
            .properties
            .child(DAV, "displayname")
            .map_or_else(|| url.path().to_owned(), |n| n.text.clone());
        Ok(Collection {
            account: account.url.clone(),
            url: url.to_string(),
            name: if account.name.is_empty() {
                name
            } else {
                format!("{} / {name}", account.name)
            },
            writable,
        })
    }

    async fn report(&self, collection: &Url) -> Result<BTreeMap<String, Remote>> {
        let response = self
            .request("REPORT", collection)?
            .header("Depth", "1")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(QUERY)
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        ensure!(
            response.status().as_u16() == 207,
            "CalDAV calendar-query returned HTTP {}",
            response.status().as_u16()
        );
        report_resources(collection, &Self::text(response).await?)
    }

    async fn get(&self, href: &Url) -> Result<Remote> {
        let response = self
            .request("GET", href)?
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        ensure!(
            response.status() == StatusCode::OK,
            "CalDAV GET returned HTTP {}",
            response.status().as_u16()
        );
        let etag = response
            .headers()
            .get("ETag")
            .context("CalDAV GET omitted ETag")?
            .to_str()?
            .to_owned();
        validate_etag(&etag)?;
        let source = Self::text(response).await?;
        ical::uid(&ical::parse(&source)?)?;
        Ok(Remote {
            href: href.to_string(),
            source,
            etag,
        })
    }
}

fn validate_etag(etag: &str) -> Result<()> {
    ensure!(
        etag.starts_with('"')
            && etag.ends_with('"')
            && etag.len() >= 2
            && !etag.contains(['\r', '\n']),
        "CalDAV requires a strong ETag for safe writes"
    );
    Ok(())
}

/// Pull first, preserve conflicts, then conditionally publish each local edit with a durable checkpoint.
pub async fn sync(storage: &Storage, accounts: &[Account]) -> Result<String> {
    let _guard = storage.calendar_guard()?;
    let mut store = Store::load(storage)?;
    let mut conflicts = 0;
    let mut collections = 0;
    for account in accounts {
        let connection = Connection::new(account)?;
        for collection in connection.discover(account).await? {
            let url = validate_url(&collection.url)?;
            let mut remote = connection.report(&url).await?;
            let mut data = (*store.data).clone();
            data.collections.retain(|c| c.url != collection.url);
            data.collections.push(collection.clone());
            let mut remove = HashSet::new();
            for r in data
                .resources
                .iter_mut()
                .filter(|r| r.collection.as_deref() == Some(&collection.url))
            {
                let incoming = if let Some(baseline) = &r.remote {
                    remote.remove(&baseline.href)
                } else {
                    let uid = ical::uid(&ical::parse(&r.source)?)?;
                    let href = remote.iter().find_map(|(href, item)| {
                        (ical::uid(&ical::parse(&item.source).ok()?).ok()? == uid)
                            .then(|| href.clone())
                    });
                    href.and_then(|href| remote.remove(&href))
                };
                match (&r.remote, incoming) {
                    (Some(old), Some(new)) if old.etag == new.etag => {
                        r.conflict = None;
                    }
                    (_, Some(new)) => {
                        if r.dirty()
                            && ical::parse(&r.source)?.to_string()
                                != ical::parse(&new.source)?.to_string()
                        {
                            r.conflict = Some(Conflict { remote: Some(new) });
                            conflicts += 1;
                        } else {
                            r.source.clone_from(&new.source);
                            r.remote = Some(new);
                            r.conflict = None;
                        }
                    }
                    (Some(_), None) if !r.dirty() || r.deleted => {
                        remove.insert(r.id.clone());
                    }
                    (Some(_), None) => {
                        r.conflict = Some(Conflict { remote: None });
                        conflicts += 1;
                    }
                    (None, None) => {}
                }
            }
            data.resources.retain(|r| !remove.contains(&r.id));
            for incoming in remote.into_values() {
                let mut r = Resource::new(incoming.source.clone(), Some(collection.url.clone()));
                r.remote = Some(incoming);
                data.resources.push(r);
            }
            store.save_locked(storage, data)?;
            if collection.writable {
                let pending: Vec<_> = store
                    .data
                    .resources
                    .iter()
                    .filter(|r| {
                        r.collection.as_deref() == Some(&collection.url)
                            && r.dirty()
                            && r.conflict.is_none()
                    })
                    .cloned()
                    .collect();
                for r in pending {
                    let href = if let Some(remote) = &r.remote {
                        resource_url(&url, &remote.href)?
                    } else {
                        url.join(&format!("{}.ics", r.id))?
                    };
                    let mut request =
                        connection.request(if r.deleted { "DELETE" } else { "PUT" }, &href)?;
                    if let Some(remote) = &r.remote {
                        request = request.header("If-Match", &remote.etag);
                    } else {
                        request = request.header("If-None-Match", "*");
                    }
                    if !r.deleted {
                        request = request
                            .header("Content-Type", "text/calendar; charset=utf-8")
                            .body(r.source.clone());
                    }
                    let response = request.send().await.map_err(reqwest::Error::without_url)?;
                    let status = response.status();
                    let mut data = (*store.data).clone();
                    if status == StatusCode::PRECONDITION_FAILED {
                        let incoming = connection.get(&href).await?;
                        data.resources
                            .iter_mut()
                            .find(|item| item.id == r.id)
                            .context("Missing sync resource")?
                            .conflict = Some(Conflict {
                            remote: Some(incoming),
                        });
                        conflicts += 1;
                    } else if r.deleted
                        && (status.is_success()
                            || matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE))
                    {
                        data.resources.retain(|item| item.id != r.id);
                    } else {
                        ensure!(
                            status.is_success(),
                            "CalDAV {} returned HTTP {}; local edit retained",
                            if r.deleted { "DELETE" } else { "PUT" },
                            status.as_u16()
                        );
                        let incoming = connection.get(&href).await?;
                        let saved = data
                            .resources
                            .iter_mut()
                            .find(|item| item.id == r.id)
                            .context("Missing sync resource")?;
                        saved.source.clone_from(&incoming.source);
                        saved.remote = Some(incoming);
                    }
                    store.save_locked(storage, data)?;
                }
            }
            collections += 1;
        }
    }
    Ok(if conflicts == 0 {
        format!("Synced {collections} calendar(s)")
    } else {
        format!(
            "Synced {collections} calendar(s); {conflicts} conflict(s) retained. Select event and press c to resolve."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn multistatus(body: &str) -> String {
        format!(
            r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">{body}</d:multistatus>"#
        )
    }

    fn response(href: &str, props: &str) -> String {
        format!(
            "<d:response><d:href>{href}</d:href><d:propstat><d:prop>{props}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
        )
    }

    #[test]
    fn parses_namespaces_entities_cdata_and_component_capabilities() {
        let items = responses(&multistatus(&response("/calendar/", r#"<d:displayname>Work &amp; Home</d:displayname><c:calendar-data><![CDATA[raw & text]]></c:calendar-data><c:supported-calendar-component-set><c:comp name="VTODO"/></c:supported-calendar-component-set>"#))).unwrap();
        assert_eq!(
            items[0].properties.child(DAV, "displayname").unwrap().text,
            "Work & Home"
        );
        assert_eq!(
            items[0]
                .properties
                .child(CAL, "calendar-data")
                .unwrap()
                .text,
            "raw & text"
        );
        assert!(!supports_events(&items[0]));
        let events = responses(&multistatus(&response("/calendar/", r#"<c:supported-calendar-component-set><c:comp name="VEVENT"/></c:supported-calendar-component-set>"#))).unwrap();
        assert!(supports_events(&events[0]));
    }

    #[test]
    fn incomplete_or_failed_reports_never_look_like_remote_deletions() {
        let url = validate_url("https://example.org/calendar/").unwrap();
        for source in [
            multistatus("<d:error><c:valid-filter/></d:error>"),
            multistatus("<d:unexpected/>"),
            multistatus(&response(
                "/calendar/a.ics",
                "<d:getetag>&quot;v1&quot;</d:getetag>",
            )),
            multistatus(
                "<d:response><d:href>/calendar/a.ics</d:href><d:status>HTTP/1.1 403 Forbidden</d:status></d:response>",
            ),
            "<!DOCTYPE xml><d:multistatus xmlns:d=\"DAV:\"/>".into(),
            "<d:multistatus xmlns:d=\"DAV:\">".into(),
        ] {
            assert!(report_resources(&url, &source).is_err(), "{source}");
        }
        assert!(report_resources(&url, &multistatus("")).unwrap().is_empty());
        let props = "<d:getetag>&quot;v1&quot;</d:getetag><c:calendar-data><![CDATA[BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:duplicate\r\nDTSTART:20261023T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n]]></c:calendar-data>";
        let duplicate = multistatus(&format!(
            "{}{}",
            response("a.ics", props),
            response("b.ics", props)
        ));
        assert!(report_resources(&url, &duplicate).is_err());
    }

    #[test]
    fn hrefs_and_etags_cannot_escape_write_boundaries() {
        let url = validate_url("https://example.org/calendar/").unwrap();
        assert!(resource_url(&url, "a.ics").is_ok());
        for href in [
            "https://elsewhere.org/calendar/a.ics",
            "../a.ics",
            "/calendar-other/a.ics",
            "?secret",
            "https://user:password@example.org/calendar/a.ics",
        ] {
            assert!(resource_url(&url, href).is_err(), "{href}");
        }
        for etag in ["W/\"weak\"", "unquoted", "\"bad\r\nvalue\""] {
            assert!(validate_etag(etag).is_err());
        }
    }
}
