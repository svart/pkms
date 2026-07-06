pub(crate) const INDEX_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>PKMS Search</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f7f7f4;
      --panel: #ffffff;
      --panel-subtle: #f1f5f2;
      --text: #242622;
      --muted: #62675f;
      --line: #d7dbd2;
      --accent: #246b50;
      --accent-strong: #174b38;
      --warn: #8a5a16;
      --error: #9e2f2f;
      --code: #334155;
    }

    * {
      box-sizing: border-box;
    }

    body {
      margin: 0;
      min-height: 100vh;
      background: var(--bg);
      color: var(--text);
      font: 15px/1.5 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    button,
    input,
    select {
      font: inherit;
    }

    .topbar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      min-height: 58px;
      padding: 12px clamp(16px, 4vw, 36px);
      border-bottom: 1px solid var(--line);
      background: var(--panel);
    }

    .brand {
      font-size: 18px;
      font-weight: 700;
    }

    .status-strip {
      display: flex;
      align-items: center;
      gap: 10px;
      min-width: 0;
      color: var(--muted);
      font-size: 13px;
      white-space: nowrap;
    }

    .phase {
      min-width: 86px;
      padding: 4px 8px;
      border: 1px solid var(--line);
      border-radius: 8px;
      background: var(--panel-subtle);
      color: var(--accent-strong);
      text-align: center;
      font-weight: 650;
    }

    .shell {
      width: min(1180px, 100%);
      margin: 0 auto;
      padding: 22px clamp(14px, 4vw, 28px) 34px;
    }

    .query-bar {
      display: grid;
      grid-template-columns: minmax(180px, 1fr) minmax(118px, 150px) minmax(86px, 104px) auto;
      gap: 10px;
      align-items: end;
    }

    label {
      display: grid;
      gap: 5px;
      min-width: 0;
      color: var(--muted);
      font-size: 12px;
      font-weight: 650;
    }

    input,
    select {
      min-height: 42px;
      min-width: 0;
      border: 1px solid var(--line);
      border-radius: 8px;
      background: var(--panel);
      color: var(--text);
      padding: 8px 10px;
    }

    input:focus,
    select:focus {
      outline: 2px solid color-mix(in srgb, var(--accent) 35%, transparent);
      border-color: var(--accent);
    }

    button {
      min-height: 42px;
      border: 1px solid var(--accent-strong);
      border-radius: 8px;
      background: var(--accent);
      color: #ffffff;
      padding: 8px 16px;
      font-weight: 700;
      cursor: pointer;
    }

    button:disabled {
      cursor: wait;
      opacity: 0.7;
    }

    .summary {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin: 14px 0 18px;
      color: var(--muted);
      font-size: 13px;
    }

    .metric {
      border: 1px solid var(--line);
      border-radius: 8px;
      background: var(--panel);
      padding: 5px 8px;
    }

    .metric strong {
      color: var(--text);
    }

    .message {
      min-height: 24px;
      margin: 0 0 12px;
      color: var(--muted);
    }

    .message.error {
      color: var(--error);
    }

    .results {
      display: grid;
      gap: 12px;
    }

    .result {
      display: grid;
      gap: 8px;
      border: 1px solid var(--line);
      border-radius: 8px;
      background: var(--panel);
      padding: 14px;
    }

    .result-header {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 10px;
      align-items: start;
    }

    .title {
      min-width: 0;
      color: var(--text);
      font-size: 16px;
      font-weight: 750;
      text-decoration: none;
      overflow-wrap: anywhere;
    }

    .title-link:hover,
    .title-link:focus {
      color: var(--accent-strong);
      text-decoration: underline;
      text-underline-offset: 3px;
    }

    .score {
      color: var(--accent-strong);
      font-variant-numeric: tabular-nums;
      font-weight: 700;
      white-space: nowrap;
    }

    .source,
    .reason,
    .heading {
      color: var(--muted);
      font-size: 13px;
      overflow-wrap: anywhere;
    }

    .snippet {
      margin: 0;
      color: var(--code);
      white-space: pre-wrap;
      overflow-wrap: anywhere;
    }

    .empty {
      border: 1px dashed var(--line);
      border-radius: 8px;
      padding: 22px;
      color: var(--muted);
      text-align: center;
    }

    @media (max-width: 720px) {
      .topbar {
        align-items: flex-start;
        flex-direction: column;
      }

      .status-strip {
        width: 100%;
        justify-content: space-between;
        white-space: normal;
      }

      .query-bar {
        grid-template-columns: 1fr;
      }

      button {
        width: 100%;
      }

      .result-header {
        grid-template-columns: 1fr;
      }

      .score {
        white-space: normal;
      }
    }
  </style>
</head>
<body>
  <header class="topbar">
    <div class="brand">PKMS Search</div>
    <div class="status-strip">
      <span id="phase" class="phase">idle</span>
      <span id="index-message">Index status pending</span>
    </div>
  </header>
  <main class="shell">
    <form id="search-form" class="query-bar">
      <label>
        Query
        <input id="query" name="query" type="search" autocomplete="off" required autofocus>
      </label>
      <label>
        Mode
        <select id="mode" name="mode">
          <option value="hybrid" selected>Hybrid</option>
          <option value="bm25">BM25</option>
          <option value="dense">Dense</option>
        </select>
      </label>
      <label>
        Limit
        <input id="limit" name="limit" type="number" min="1" max="50" value="8">
      </label>
      <button id="submit" type="submit">Search</button>
    </form>
    <div id="summary" class="summary" aria-live="polite"></div>
    <p id="message" class="message" aria-live="polite"></p>
    <section id="results" class="results" aria-live="polite"></section>
  </main>
  <script src="/ui.js"></script>
</body>
</html>"##;

pub(crate) const UI_JS: &str = r##"const phaseEl = document.querySelector("#phase");
const indexMessageEl = document.querySelector("#index-message");
const summaryEl = document.querySelector("#summary");
const messageEl = document.querySelector("#message");
const resultsEl = document.querySelector("#results");
const formEl = document.querySelector("#search-form");
const submitEl = document.querySelector("#submit");
const queryEl = document.querySelector("#query");
const modeEl = document.querySelector("#mode");
const limitEl = document.querySelector("#limit");
const numberFormat = new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 });
let noteViewerAvailable = false;

function setMessage(text, level = "info") {
  messageEl.textContent = text;
  messageEl.classList.toggle("error", level === "error");
}

function text(value) {
  return value === null || value === undefined ? "" : String(value);
}

function noteHref(noteId) {
  return `/?id=${encodeURIComponent(text(noteId))}`;
}

function addMetric(label, value) {
  const item = document.createElement("span");
  item.className = "metric";
  const strong = document.createElement("strong");
  strong.textContent = numberFormat.format(value || 0);
  item.append(strong, ` ${label}`);
  summaryEl.append(item);
}

async function refreshStatus() {
  const [indexResponse, statusResponse] = await Promise.all([
    fetch("/index/status"),
    fetch("/status")
  ]);
  if (!indexResponse.ok) {
    throw new Error(`index status ${indexResponse.status}`);
  }
  if (!statusResponse.ok) {
    throw new Error(`status ${statusResponse.status}`);
  }
  const indexBody = await indexResponse.json();
  const statusBody = await statusResponse.json();
  noteViewerAvailable = statusBody.note_viewer_available === true;
  phaseEl.textContent = indexBody.phase || "unknown";
  indexMessageEl.textContent = indexBody.message || indexBody.current_step || "idle";
  summaryEl.replaceChildren();
  addMetric("notes", statusBody.notes);
  addMetric("chunks", statusBody.chunks);
  addMetric("embeddings", statusBody.embeddings);
  addMetric("stale", statusBody.stale_chunks);
}

function renderEmpty(textValue) {
  const empty = document.createElement("div");
  empty.className = "empty";
  empty.textContent = textValue;
  resultsEl.replaceChildren(empty);
}

function renderResults(response) {
  resultsEl.replaceChildren();
  const results = response.results || [];
  if (results.length === 0) {
    renderEmpty("No results");
    return;
  }
  for (const entry of results) {
    const item = entry.result || entry;
    const article = document.createElement("article");
    article.className = "result";

    const header = document.createElement("div");
    header.className = "result-header";
    const title = document.createElement(noteViewerAvailable && item.note_id ? "a" : "div");
    title.className = title instanceof HTMLAnchorElement ? "title title-link" : "title";
    if (title instanceof HTMLAnchorElement) {
      title.href = noteHref(item.note_id);
    }
    title.textContent = item.title || item.note_id || "Untitled";
    const score = document.createElement("div");
    score.className = "score";
    score.textContent = numberFormat.format(item.scores?.final || 0);
    header.append(title, score);

    const source = document.createElement("div");
    source.className = "source";
    source.textContent = `${text(item.path)}:${item.start_line || 0}-${item.end_line || 0}`;

    const heading = document.createElement("div");
    heading.className = "heading";
    heading.textContent = (item.heading_path || []).join(" / ");
    heading.hidden = !heading.textContent;

    const reason = document.createElement("div");
    reason.className = "reason";
    reason.textContent = entry.reason || "";
    reason.hidden = !reason.textContent;

    const snippet = document.createElement("pre");
    snippet.className = "snippet";
    snippet.textContent = item.text || "";

    article.append(header, source, heading, reason, snippet);
    resultsEl.append(article);
  }
}

async function retrieve(event) {
  event.preventDefault();
  const query = queryEl.value.trim();
  if (!query) {
    queryEl.focus();
    return;
  }
  submitEl.disabled = true;
  setMessage("Searching");
  try {
    const response = await fetch("/retrieve", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        query,
        mode: modeEl.value,
        limit: Number(limitEl.value) || 8
      })
    });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.detail || `retrieve ${response.status}`);
    }
    renderResults(body);
    setMessage(`${body.results.length} result${body.results.length === 1 ? "" : "s"}`);
  } catch (error) {
    renderEmpty("Search failed");
    setMessage(error.message, "error");
  } finally {
    submitEl.disabled = false;
  }
}

formEl.addEventListener("submit", retrieve);
refreshStatus().catch((error) => setMessage(error.message, "error"));
setInterval(() => refreshStatus().catch(() => {}), 2500);
"##;
