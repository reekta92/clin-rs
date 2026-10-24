// HTTP fixtures for article imports: redirects, errors, and bounded transfers.
const http = require("http");
const article = `<html><head><title>Clipping fixture</title></head><body>
<nav>NAVMARKER <a href="/">Home</a> <a href="/subscribe">Subscribe</a></nav>
<article><h1>Clipping fixture</h1>
<p>ARTICLEMARKER A neighbourhood planning team studied ways to reduce summer heat in busy streets. The team measured shade, pavement temperatures, and access to public drinking water during a long survey of the local area.</p>
<p>The final report recommends planting trees near schools and adding shaded seating at bus stops. Residents asked for improvements that can be maintained with the existing city budget. The team will publish all measurements after the consultation.</p>
<p>The council will review the findings at its next meeting. Read the <a href="/imports/full-report">full report</a> for the measurements. A second survey is planned for next summer to compare the results under different conditions.</p>
<table><tr><th>Location</th><th>Shade</th></tr><tr><td>School</td><td>Low</td></tr></table>
<pre><code>CODEMARKER let count = 42;</code></pre></article>
<aside>ADMARKER Buy our unrelated product.</aside><footer>FOOTERMARKER Privacy policy.</footer>
</body></html>`;
http.createServer((req, res) => {
  const path = new URL(req.url, "http://localhost").pathname.replace(/^\/imports(?=\/|$)/, "");
  if (req.method !== "GET") { res.writeHead(501); return res.end(); }
  if (path === "/article.html") { res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" }); return res.end(article); }
  if (path === "/redirect" || path === "/loop" || path === "/unsafe") {
    res.writeHead(302, { Location: path === "/redirect" ? "/imports/article.html" : path === "/loop" ? "/imports/loop" : "file:///etc/passwd" });
    return res.end();
  }
  if (path === "/failed") { res.writeHead(500, { "Content-Type": "text/html" }); return res.end("<html>Upstream failure</html>"); }
  if (path === "/oversized") { res.writeHead(200, { "Content-Length": "16777217" }); return res.end("a".repeat(16777217)); }
  if (path === "/chunked") { res.writeHead(200, { "Transfer-Encoding": "chunked" }); return res.end("a".repeat(16777217)); }
  if (path === "/slow") { return setTimeout(() => { res.writeHead(200); res.end(article); }, 2000); }
  res.writeHead(501); res.end("Endpoint not defined");
}).listen(process.env.PORT);
