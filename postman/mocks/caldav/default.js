// RFC 4791 fixture: discovery, calendar-query, conditional event writes, and failures.
const http = require("http");
const prefix = "/caldav";
const calendar = `${prefix}/calendars/default/`;
const seed = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:CalDAV simulation\r\nBEGIN:VEVENT\r\nUID:remote-seed\r\nSUMMARY:Remote meeting\r\nDTSTART:20261023T090000Z\r\nDTEND:20261023T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
const escapeXml = value => value.replace(/[<>&"']/g, c => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&apos;" }[c]));
const multistatus = entries => `<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">${entries.join("")}</d:multistatus>`;
const properties = (href, body) => `<d:response><d:href>${escapeXml(href)}</d:href><d:propstat><d:prop>${body}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>`;

http.createServer(async (req, res) => {
  let path = new URL(req.url, "http://localhost").pathname.replace(/^\/caldav(?=\/|$)/, "");
  if (!path.endsWith("/") && !path.endsWith(".ics") && !path.startsWith("/__")) path += "/";
  let state = await pm.state.get("caldav.data");
  if (!state || (req.method === "POST" && path === "/__reset")) {
    state = { version: 1, fail: false, events: { [`${calendar}seed.ics`]: { source: seed, etag: '"v1"' } } };
    await pm.state.set("caldav.data", state);
  }
  const send = (status, body = "", headers = {}) => { res.writeHead(status, headers); res.end(body); };
  const xml = body => send(207, body, { "Content-Type": "application/xml; charset=utf-8" });
  if (req.method === "POST" && path === "/__reset") return send(204);
  if (req.method === "POST" && path === "/__fail") { state.fail = true; await pm.state.set("caldav.data", state); return send(204); }
  if (req.method === "POST" && path === "/__race") { state.race = true; await pm.state.set("caldav.data", state); return send(204); }
  if (state.fail) return send(503, "<html>Calendar server unavailable</html>", { "Content-Type": "text/html" });
  if (req.method === "PROPFIND") {
    if (req.headers.depth !== "0" && req.headers.depth !== "1") return send(400, "Depth required");
    if (path === "/" || path === "") return xml(multistatus([properties(`${prefix}/`, `<d:current-user-principal><d:href>${prefix}/principals/user/</d:href></d:current-user-principal><d:resourcetype><d:collection/></d:resourcetype>`)]));
    if (path === "/principals/user/") return xml(multistatus([properties(`${prefix}${path}`, `<c:calendar-home-set><d:href>../../calendars/</d:href></c:calendar-home-set>`)]));
    if (path === "/calendars/" || path === "/calendars/default/") return xml(multistatus([properties(calendar, '<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>Work &amp; Home</d:displayname><d:current-user-privilege-set><d:privilege><d:write/></d:privilege></d:current-user-privilege-set><c:supported-calendar-component-set><c:comp name="VEVENT"/></c:supported-calendar-component-set>')]));
    return send(404);
  }
  if (req.method === "REPORT" && path === "/calendars/default/") {
    if (req.headers.depth !== "1") return send(400, "Depth: 1 required");
    return xml(multistatus(Object.entries(state.events).map(([href, event]) => properties(href, `<d:getetag>${escapeXml(event.etag)}</d:getetag><c:calendar-data><![CDATA[${event.source}]]></c:calendar-data>`))));
  }
  if (path.startsWith("/calendars/default/") && path.endsWith(".ics")) {
    const href = `${prefix}${path}`;
    const event = state.events[href];
    if (req.method === "GET") return event ? send(200, event.source, { ETag: event.etag, "Content-Type": "text/calendar" }) : send(404);
    if (req.method === "PUT" || req.method === "DELETE") {
      if (state.race && event) {
        state.race = false;
        event.source = event.source.replace(/SUMMARY:[^\r\n]*/, "SUMMARY:Remote race winner");
        event.etag = `"v${++state.version}"`;
        await pm.state.set("caldav.data", state);
        return send(412);
      }
      if (event ? req.headers["if-match"] !== event.etag : req.headers["if-none-match"] !== "*") return send(412);
      if (req.method === "DELETE") { delete state.events[href]; await pm.state.set("caldav.data", state); return send(204); }
      const source = Buffer.isBuffer(req.body) ? req.body.toString("utf8") : typeof req.body === "string" ? req.body : "";
      if (!source.startsWith("BEGIN:VCALENDAR") || !source.includes("BEGIN:VEVENT")) return send(400, "Valid calendar required");
      const etag = `"v${++state.version}"`;
      state.events[href] = { source, etag };
      await pm.state.set("caldav.data", state);
      return send(event ? 204 : 201, "", { ETag: etag });
    }
  }
  return send(501, "Operation not modeled by this RFC 4791 fixture");
}).listen(process.env.PORT || 49148);
