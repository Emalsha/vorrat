// Vorrat service worker: the app keeps opening without a connection, showing
// the last copy of each page you visited. Changes still need the server.
const CACHE = "vorrat-__VERSION__";
const PRECACHE = [
  "/", "/items", "/plan", "/plan/print", "/plan/power-cut", "/shopping",
  "/static/app.css", "/static/jost.woff2", "/static/app.js", "/static/print.js",
  "/static/icon-192.png", "/static/manifest.webmanifest",
];
// Never cached: live data for scripts, downloads, the calendar feed.
const SKIP = ["/api/", "/export/", "/calendar.ics", "/healthz", "/scan/code", "/login", "/logout"];

self.addEventListener("install", (e) => {
  e.waitUntil(
    caches.open(CACHE)
      .then((c) => Promise.allSettled(PRECACHE.map((u) => c.add(u))))
      .then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

// One cache entry per page, whatever message or undo link it was shown with.
function key(url) {
  const u = new URL(url);
  u.searchParams.delete("msg");
  u.searchParams.delete("undo");
  u.hash = "";
  return u.toString();
}

// Fixed files: cache first. Pages: network first, the saved copy when offline.
self.addEventListener("fetch", (e) => {
  const req = e.request;
  const url = new URL(req.url);
  if (req.method !== "GET" || url.origin !== location.origin) return;
  if (SKIP.some((p) => url.pathname.startsWith(p))) return;

  if (url.pathname.startsWith("/static/") || url.pathname.startsWith("/files/")) {
    e.respondWith(
      caches.match(req).then((hit) => hit || fetch(req).then((res) => {
        if (res.ok) {
          const copy = res.clone();
          caches.open(CACHE).then((c) => c.put(req, copy));
        }
        return res;
      }))
    );
    return;
  }

  e.respondWith(
    fetch(req)
      .then((res) => {
        if (res.ok && res.type === "basic") {
          const copy = res.clone();
          caches.open(CACHE).then((c) => c.put(key(req.url), copy));
        }
        return res;
      })
      .catch(() =>
        caches.match(key(req.url)).then((hit) => hit || caches.match("/")).then((hit) =>
          hit || new Response("Offline, and this page wasn't saved yet.", {
            status: 503,
            headers: { "content-type": "text/plain; charset=utf-8" },
          })
        )
      )
  );
});
