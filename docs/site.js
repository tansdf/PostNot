(function () {
  const root = document.documentElement;
  const themeButtons = Array.from(document.querySelectorAll("[data-site-theme]"));
  const screenshotButtons = Array.from(document.querySelectorAll("[data-screenshot-src]"));
  const modal = document.querySelector(".screenshot-modal");
  const modalTitle = document.querySelector("#screenshot-modal-title");
  const modalImage = document.querySelector(".screenshot-modal__image");
  const closeButtons = Array.from(document.querySelectorAll("[data-modal-close]"));
  let lastFocusedElement = null;

  function setTheme(theme) {
    root.dataset.siteTheme = theme;

    for (const button of themeButtons) {
      const isActive = button.dataset.siteTheme === theme;
      button.classList.toggle("theme-card--active", isActive);
      button.setAttribute("aria-pressed", String(isActive));
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

  setTheme("dark");
})();
