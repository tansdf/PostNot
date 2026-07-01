(function () {
  const root = document.documentElement;
  const themeStorageKey = "postnot-docs-theme";
  const fallbackTheme = "dark";
  const supportedThemes = new Set(["light", "dark", "forest"]);
  const themeButtons = Array.from(document.querySelectorAll("[data-site-theme]"));
  const screenshotButtons = Array.from(document.querySelectorAll("[data-screenshot-src]"));
  const modal = document.querySelector(".screenshot-modal");
  const modalTitle = document.querySelector("#screenshot-modal-title");
  const modalImage = document.querySelector(".screenshot-modal__image");
  const closeButtons = Array.from(document.querySelectorAll("[data-modal-close]"));
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
      // The visual theme still updates if storage is unavailable.
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

    if (options.persist !== false) {
      persistTheme(nextTheme);
    }
  }

  function openScreenshot(button) {
    if (!modal || !modalImage || !modalTitle) {
      return;
    }

    lastFocusedElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    modalImage.src = button.dataset.screenshotSrc || "";
    modalImage.alt = button.dataset.screenshotAlt || "";
    modalTitle.textContent = button.dataset.screenshotTitle || "Screenshot";
    modal.hidden = false;
    document.body.style.overflow = "hidden";

    const closeButton = modal.querySelector(".screenshot-modal__close");
    if (closeButton instanceof HTMLElement) {
      closeButton.focus();
    }
  }

  function closeScreenshot() {
    if (!modal || !modalImage) {
      return;
    }

    modal.hidden = true;
    modalImage.src = "";
    document.body.style.overflow = "";

    if (lastFocusedElement) {
      lastFocusedElement.focus();
      lastFocusedElement = null;
    }
  }

  for (const button of themeButtons) {
    button.addEventListener("click", () => setTheme(button.dataset.siteTheme || "dark"));
  }

  for (const button of screenshotButtons) {
    button.addEventListener("click", () => openScreenshot(button));
  }

  for (const button of closeButtons) {
    button.addEventListener("click", closeScreenshot);
  }

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && modal && !modal.hidden) {
      closeScreenshot();
    }
  });

  setTheme(readStoredTheme(), { persist: false });
})();
