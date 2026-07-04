(() => {
  const HOVER_DELAY_MS = 450;
  const preview = document.getElementById("note-preview");
  const originalNote = document.querySelector(".note-body");
  const sidePanels = Array.from(document.querySelectorAll(".side-panel"));
  const outlineLinks = Array.from(document.querySelectorAll(".contents-panel a[href^='#h-']"));
  const openButton = document.querySelector(".open-note-button[data-open-url]");

  if (sidePanels.length > 0) {
    const widePanels = window.matchMedia("(min-width: 1361px)");
    for (const panel of sidePanels) {
      panel.open = widePanels.matches;
    }
  }

  if (originalNote && outlineLinks.length > 0) {
    const headings = outlineLinks
      .map((link) => {
        const id = link.getAttribute("href").slice(1);
        const heading = document.getElementById(id);
        return heading ? { id, heading, link } : null;
      })
      .filter(Boolean);
    let activeId = "";
    let ticking = false;

    function setActiveOutline(id) {
      if (id === activeId) {
        return;
      }
      activeId = id;
      for (const item of headings) {
        const active = item.id === id;
        item.link.classList.toggle("active-outline", active);
        if (active) {
          item.link.setAttribute("aria-current", "location");
        } else {
          item.link.removeAttribute("aria-current");
        }
      }
    }

    function updateActiveOutline() {
      ticking = false;
      if (headings.length === 0) {
        return;
      }
      const threshold = Math.min(window.innerHeight * 0.3, 160);
      let current = headings[0];
      for (const item of headings) {
        if (item.heading.getBoundingClientRect().top <= threshold) {
          current = item;
        } else {
          break;
        }
      }
      setActiveOutline(current.id);
    }

    function requestOutlineUpdate() {
      if (ticking) {
        return;
      }
      ticking = true;
      window.requestAnimationFrame(updateActiveOutline);
    }

    window.addEventListener("scroll", requestOutlineUpdate, { passive: true });
    window.addEventListener("resize", requestOutlineUpdate);
    updateActiveOutline();
  }

  if (openButton && window.fetch) {
    openButton.addEventListener("click", async () => {
      const label = openButton.textContent;
      openButton.disabled = true;
      try {
        const response = await fetch(openButton.dataset.openUrl, { method: "POST" });
        openButton.textContent = response.ok ? "Opened" : "Open failed";
      } catch (_error) {
        openButton.textContent = "Open failed";
      } finally {
        window.setTimeout(() => {
          openButton.textContent = label;
          openButton.disabled = false;
        }, 1400);
      }
    });
  }
  if (!preview || !originalNote || !window.fetch) {
    return;
  }

  let hoverTimer = 0;
  let activeController = null;
  const cache = new Map();

  function hidePreview() {
    window.clearTimeout(hoverTimer);
    hoverTimer = 0;
    if (activeController) {
      activeController.abort();
      activeController = null;
    }
    preview.hidden = true;
    preview.innerHTML = "";
    preview.removeAttribute("data-active-preview");
  }

  function positionPreview(anchor) {
    const rect = anchor.getBoundingClientRect();
    preview.style.top = `${Math.max(16, Math.min(rect.top, window.innerHeight * 0.35))}px`;
  }

  async function showPreview(anchor) {
    const uuid = anchor.dataset.previewId;
    if (!uuid) {
      return;
    }
    positionPreview(anchor);
    preview.hidden = false;
    preview.dataset.activePreview = uuid;
    preview.innerHTML = "<p class=\"panel-empty\">Loading...</p>";

    if (cache.has(uuid)) {
      preview.innerHTML = cache.get(uuid);
      return;
    }

    if (activeController) {
      activeController.abort();
    }
    activeController = new AbortController();
    const url = `${preview.dataset.previewUrl}?id=${encodeURIComponent(uuid)}`;
    try {
      const response = await fetch(url, { signal: activeController.signal });
      if (!response.ok) {
        throw new Error(`Preview request failed: ${response.status}`);
      }
      const html = await response.text();
      cache.set(uuid, html);
      if (preview.dataset.activePreview === uuid) {
        preview.innerHTML = html;
      }
    } catch (error) {
      if (error.name !== "AbortError" && preview.dataset.activePreview === uuid) {
        preview.innerHTML = "<p class=\"panel-empty\">Preview unavailable</p>";
      }
    }
  }

  document.addEventListener("mouseover", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target ? target.closest("a[data-preview-id]") : null;
    if (!anchor) {
      return;
    }
    window.clearTimeout(hoverTimer);
    hoverTimer = window.setTimeout(() => showPreview(anchor), HOVER_DELAY_MS);
  });

  document.addEventListener("mouseout", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target ? target.closest("a[data-preview-id]") : null;
    if (!anchor || anchor.contains(event.relatedTarget)) {
      return;
    }
    window.clearTimeout(hoverTimer);
  });

  originalNote.addEventListener("click", (event) => {
    if (!preview.hidden && !preview.contains(event.target)) {
      hidePreview();
    }
  });

  preview.addEventListener("wheel", (event) => {
    event.stopPropagation();
    const lineHeight = 16;
    const pageHeight = preview.clientHeight;
    const delta = event.deltaMode === 1
      ? event.deltaY * lineHeight
      : event.deltaMode === 2
        ? event.deltaY * pageHeight
        : event.deltaY;
    const maxScrollTop = preview.scrollHeight - preview.clientHeight;
    const atTop = preview.scrollTop <= 0;
    const atBottom = preview.scrollTop >= maxScrollTop - 1;
    const targetScrollTop = preview.scrollTop + delta;
    if (delta < 0 && (atTop || targetScrollTop < 0)) {
      event.preventDefault();
      preview.scrollTop = 0;
    } else if (delta > 0 && (atBottom || targetScrollTop > maxScrollTop)) {
      event.preventDefault();
      preview.scrollTop = maxScrollTop;
    }
  }, { passive: false });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      hidePreview();
    }
  });
})();
