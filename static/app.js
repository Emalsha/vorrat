// Offline support: register the service worker and say when we're offline.
if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch(() => {});
}
function showOffline() {
  document.body.classList.toggle("offline", !navigator.onLine);
}
addEventListener("online", showOffline);
addEventListener("offline", showOffline);
showOffline();
