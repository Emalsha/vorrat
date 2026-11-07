// Print button on the checklist page (no inline scripts: the CSP forbids them).
document.querySelectorAll("[data-print]").forEach((b) =>
  b.addEventListener("click", () => window.print())
);
