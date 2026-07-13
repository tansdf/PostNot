(function () {
  const root = document.documentElement;
  root.classList.add("js");

  const themeStorageKey = "postnot-docs-theme";
  const fallbackTheme = "dark";
  const supportedThemes = new Set(["light", "dark", "forest"]);
  const themeButtons = Array.from(document.querySelectorAll("[data-site-theme]"));
  const screenshotButtons = Array.from(document.querySelectorAll("[data-screenshot-src]"));
  const modal = document.querySelector(".screenshot-modal");
  const modalTitle = document.querySelector("#screenshot-modal-title");
  const modalImage = document.querySelector(".screenshot-modal__image");
  const closeButtons = Array.from(document.querySelectorAll("[data-modal-close]"));
  const menuButton = document.querySelector("[data-menu-toggle]");
  const siteNav = document.querySelector("[data-site-nav]");
  const pageRegions = Array.from(document.querySelectorAll("body > header, body > main, body > footer"));
  let lastFocusedElement = null;

  function readStoredTheme() {
    try {
      const storedTheme = window.localStorage.getItem(themeStorageKey);
      return supportedThemes.has(storedTheme) ? storedTheme : fallbackTheme;
    } catch (_error) {
      return fallbackTheme;
    }
  }

  function persistTheme(theme) {
    try {
      window.localStorage.setItem(themeStorageKey, theme);
    } catch (_error) {
      // Theme selection remains usable when storage is blocked.
    }
  }

  function setTheme(theme, options = {}) {
    const nextTheme = supportedThemes.has(theme) ? theme : fallbackTheme;
    root.dataset.siteTheme = nextTheme;

    for (const button of themeButtons) {
      const isActive = button.dataset.siteTheme === nextTheme;
      button.classList.toggle("theme-card--active", isActive);
      button.setAttribute("aria-pressed", String(isActive));
    }

    const themeMeta = document.querySelector('meta[name="theme-color"]');
    if (themeMeta) {
      themeMeta.setAttribute("content", nextTheme === "light" ? "#f2efe7" : nextTheme === "forest" ? "#101713" : "#111917");
    }

    if (options.persist !== false) persistTheme(nextTheme);
  }

  function setMenuOpen(isOpen, options = {}) {
    if (!menuButton || !siteNav) return;
    menuButton.setAttribute("aria-expanded", String(isOpen));
    siteNav.classList.toggle("site-nav--open", isOpen);
    if (!isOpen && options.restoreFocus) menuButton.focus();
  }

  function getFocusableElements(container) {
    return Array.from(
      container.querySelectorAll(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
      )
    ).filter((element) => !element.hasAttribute("hidden") && element.getClientRects().length > 0);
  }

  function setPageInert(isInert) {
    for (const region of pageRegions) region.inert = isInert;
  }

  function openScreenshot(button) {
    if (!modal || !modalImage || !modalTitle) return;
    lastFocusedElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    modalImage.src = button.dataset.screenshotSrc || "";
    modalImage.alt = button.dataset.screenshotAlt || "";
    modalTitle.textContent = button.dataset.screenshotTitle || "Screenshot";
    modal.hidden = false;
    document.body.style.overflow = "hidden";
    setPageInert(true);

    const closeButton = modal.querySelector(".screenshot-modal__close");
    if (closeButton instanceof HTMLElement) closeButton.focus();
  }

  function closeScreenshot() {
    if (!modal || !modalImage || modal.hidden) return;
    modal.hidden = true;
    modalImage.src = "";
    document.body.style.overflow = "";
    setPageInert(false);

    if (lastFocusedElement) {
      lastFocusedElement.focus();
      lastFocusedElement = null;
    }
  }

  function trapModalFocus(event) {
    if (event.key !== "Tab" || !modal || modal.hidden) return;
    const focusable = getFocusableElements(modal);
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return "Size unavailable";
    const units = ["B", "KB", "MB", "GB"];
    const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / 1024 ** unitIndex;
    return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
  }

  function formatReleaseDate(value) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "Date unavailable";
    return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(date);
  }

  function preferDetectedPlatform() {
    const platform = `${navigator.userAgentData?.platform || ""} ${navigator.platform || ""} ${navigator.userAgent || ""}`.toLowerCase();
    const detected = platform.includes("win") ? "windows" : platform.includes("mac") ? "macos" : platform.includes("linux") ? "linux" : "";
    if (!detected) return;
    const card = document.querySelector(`[data-download-platform="${detected}"]`);
    if (card) card.classList.add("download-card--preferred");
  }

  function addSignatureLink(assetRow, signatureAsset) {
    if (!signatureAsset || assetRow.parentElement?.querySelector(`[data-signature-for="${CSS.escape(signatureAsset.name)}"]`)) return;
    const signatureLink = document.createElement("a");
    signatureLink.className = "asset-signature";
    signatureLink.dataset.signatureFor = signatureAsset.name;
    signatureLink.href = signatureAsset.browser_download_url;
    signatureLink.rel = "noopener noreferrer";
    signatureLink.textContent = "Download updater signature";
    assetRow.insertAdjacentElement("afterend", signatureLink);
  }

  function hydrateRelease(release) {
    const version = release.tag_name || release.name || "Latest stable";
    const versionElement = document.querySelector("[data-release-version]");
    const dateElement = document.querySelector("[data-release-date]");
    const notesLink = document.querySelector("[data-release-notes]");
    if (versionElement) versionElement.textContent = version;
    if (dateElement) dateElement.textContent = formatReleaseDate(release.published_at);
    if (notesLink && release.html_url) notesLink.href = release.html_url;

    const assets = Array.isArray(release.assets) ? release.assets : [];
    for (const assetRow of document.querySelectorAll("[data-asset-match]")) {
      let matcher;
      try {
        matcher = new RegExp(assetRow.dataset.assetMatch, "i");
      } catch (_error) {
        continue;
      }
      const asset = assets.find((candidate) => matcher.test(candidate.name || ""));
      if (!asset) continue;

      assetRow.href = asset.browser_download_url;
      const meta = assetRow.querySelector("[data-asset-meta]");
      if (meta) {
        const digest = typeof asset.digest === "string" ? asset.digest.replace(/^sha256:/, "") : "";
        meta.textContent = digest ? `${formatBytes(asset.size)} · SHA-256 ${digest.slice(0, 12)}…` : formatBytes(asset.size);
        if (digest) meta.title = `SHA-256 ${digest}`;
      }
      const signature = assets.find((candidate) => candidate.name === `${asset.name}.sig`);
      addSignatureLink(assetRow, signature);
    }

    const status = document.querySelector("[data-release-status]");
    if (status) status.textContent = "Installer sizes, digests, and signature links were loaded from the current public GitHub release.";

    const structuredData = document.querySelector('script[type="application/ld+json"]');
    if (structuredData) {
      try {
        const data = JSON.parse(structuredData.textContent);
        data.softwareVersion = version.replace(/^v/, "");
        structuredData.textContent = JSON.stringify(data);
      } catch (_error) {
        // Static structured data remains valid if runtime enhancement fails.
      }
    }
  }

  async function loadLatestRelease() {
    if (!document.querySelector("[data-download-grid]")) return;
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 8000);
    try {
      const response = await fetch("https://api.github.com/repos/tansdf/PostNot/releases/latest", {
        headers: { Accept: "application/vnd.github+json" },
        signal: controller.signal
      });
      if (!response.ok) throw new Error(`GitHub release request failed with ${response.status}`);
      hydrateRelease(await response.json());
    } catch (_error) {
      const versionElement = document.querySelector("[data-release-version]");
      const dateElement = document.querySelector("[data-release-date]");
      const status = document.querySelector("[data-release-status]");
      if (versionElement) versionElement.textContent = "Latest stable release";
      if (dateElement) dateElement.textContent = "Available on GitHub";
      if (status) status.textContent = "Live installer details are unavailable right now. All download links safely fall back to the latest GitHub release.";
    } finally {
      window.clearTimeout(timeoutId);
    }
  }

  function initializeDocumentationToc() {
    const mobileToc = document.querySelector("[data-doc-mobile-toc]");
    if (mobileToc) {
      mobileToc.addEventListener("change", () => {
        const target = document.querySelector(mobileToc.value);
        if (target) target.scrollIntoView();
      });
    }

    const tocLinks = Array.from(document.querySelectorAll(".doc-toc a[href^='#']"));
    const sections = tocLinks.map((link) => document.querySelector(link.getAttribute("href"))).filter(Boolean);
    if (!("IntersectionObserver" in window) || sections.length === 0) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((entry) => entry.isIntersecting).sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
        if (!visible) return;
        for (const link of tocLinks) {
          const isCurrent = link.getAttribute("href") === `#${visible.target.id}`;
          if (isCurrent) link.setAttribute("aria-current", "location");
          else link.removeAttribute("aria-current");
        }
        if (mobileToc) mobileToc.value = `#${visible.target.id}`;
      },
      { rootMargin: "-20% 0px -65%", threshold: [0, 0.25, 0.75] }
    );
    for (const section of sections) observer.observe(section);
  }

  for (const button of themeButtons) button.addEventListener("click", () => setTheme(button.dataset.siteTheme || fallbackTheme));
  for (const button of screenshotButtons) button.addEventListener("click", () => openScreenshot(button));
  for (const button of closeButtons) button.addEventListener("click", closeScreenshot);

  if (menuButton && siteNav) {
    menuButton.addEventListener("click", () => setMenuOpen(menuButton.getAttribute("aria-expanded") !== "true"));
    siteNav.addEventListener("click", (event) => {
      if (event.target.closest("a")) setMenuOpen(false);
    });
    document.addEventListener("pointerdown", (event) => {
      if (menuButton.getAttribute("aria-expanded") === "true" && !event.target.closest("[data-site-header]")) setMenuOpen(false);
    });
  }

  document.addEventListener("keydown", (event) => {
    trapModalFocus(event);
    if (event.key !== "Escape") return;
    if (modal && !modal.hidden) closeScreenshot();
    else if (menuButton?.getAttribute("aria-expanded") === "true") setMenuOpen(false, { restoreFocus: true });
  });

  setTheme(readStoredTheme(), { persist: false });
  preferDetectedPlatform();
  initializeDocumentationToc();
  loadLatestRelease();
})();
