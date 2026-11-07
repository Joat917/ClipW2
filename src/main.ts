import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import DOMPurify from "dompurify";

interface Entry {
  format_name: string;
  format_id: number;
  category: string;
  type_label: string;
  encoding?: string;
  description?: string;
  content?: string | string[];
  width?: number;
  height?: number;
  size?: number;
  preview?: string;
}

interface ClipboardData {
  formats: Entry[];
  error?: string;
}

interface CategoryConfig {
  label: string;
  color: string;
}

const CATEGORY_CONFIG: Record<string, CategoryConfig> = {
  text:   { label: "TEXT",      color: "green" },
  html:   { label: "HTML",      color: "blue" },
  rtf:    { label: "RICH TEXT", color: "purple" },
  image:  { label: "IMAGE",     color: "cyan" },
  files:  { label: "FILES",     color: "yellow" },
  binary: { label: "BINARY",    color: "red" },
  empty:  { label: "EMPTY",     color: "gray" },
  error:  { label: "ERROR",     color: "red" },
};

let activeFilter: string | null = null;
let refreshTimer: ReturnType<typeof setInterval> | null = null;
let previousClipboardHash: string | null = null;

const ICONS = {
  file: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9 1H4a1 1 0 00-1 1v12a1 1 0 001 1h8a1 1 0 001-1V5L9 1z"/><path d="M9 1v4h4"/><path d="M6 8h4M6 11h4"/></svg>`,
  code: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 4L1 8l4 4M11 4l4 4-4 4"/></svg>`,
  text: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3h10M5 3v10M11 3v10M3 7h10M3 11h10"/></svg>`,
  richText: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h12M6 3v10M2 7h6"/></svg>`,
  image: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="1" y="1" width="14" height="14" rx="2"/><circle cx="5" cy="5" r="1.5"/><path d="M15 10l-4-4-6 6"/></svg>`,
  folder: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 4v9a2 2 0 002 2h10a2 2 0 002-2V6a2 2 0 00-2-2H8L6 2H3a2 2 0 00-2 2z"/></svg>`,
  binary: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4 2v12M2 4h4M2 8h4M2 12h4M12 2v12M10 4h4M10 8h4M10 12h4"/></svg>`,
  empty: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="6"/><path d="M5 8h6"/></svg>`,
  error: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M8 1L1 14h14L8 1zM8 6v4M8 12v1"/></svg>`,
  unknown: `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="6"/><path d="M6 6a2 2 0 014 0c0 1-2 2-2 3M8 12v.5"/></svg>`,
};

function getCategoryIcon(cat: string): string {
  switch (cat) {
    case "text": return ICONS.text;
    case "html": return ICONS.code;
    case "rtf": return ICONS.richText;
    case "image": return ICONS.image;
    case "files": return ICONS.folder;
    case "binary": return ICONS.binary;
    case "empty": return ICONS.empty;
    case "error": return ICONS.error;
    default: return ICONS.unknown;
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  return (bytes / (1024 * 1024)).toFixed(2) + " MB";
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function renderCardContent(item: Entry): string {
  const cat = item.category || "unknown";

  if (cat === "html") {
    const source = typeof item.content === "string" ? item.content : (item.preview || "");
    const htmlIndex = source.search(/<html[\s>]/i);
    let displaySource: string;
    if (htmlIndex > 0) {
      const beforeHtml = source.slice(0, htmlIndex);
      const fromHtml = source.slice(htmlIndex);
      displaySource = `<span class="html-preamble">${escapeHtml(beforeHtml)}</span>${escapeHtml(fromHtml)}`;
    } else {
      displaySource = escapeHtml(source);
    }
    const previewContent = htmlIndex > 0 ? source.slice(htmlIndex) : source;
    const sanitizedPreview = DOMPurify.sanitize(previewContent, {
      USE_PROFILES: { html: true },
      ADD_ATTR: ['target']
    });
    return `<div class="html-card">
      <div class="html-toggle">
        <button class="html-toggle-btn" data-action="source">源码</button>
        <button class="html-toggle-btn active" data-action="preview">预览</button>
      </div>
      <div class="html-source-pane" style="display:none">
        <div class="card-content html-content">${displaySource}</div>
      </div>
      <div class="html-preview-pane">
        <iframe class="html-preview-frame" sandbox srcdoc="${escapeHtml(sanitizedPreview)}"></iframe>
      </div>
      <div class="html-raw-data" style="display:none">${escapeHtml(sanitizedPreview)}</div>
    </div>`;
  }

  if (cat === "text" || cat === "rtf") {
    const text = typeof item.content === "string" ? item.content : (item.preview || "");
    return `<div class="card-content ${cat}-content">${escapeHtml(text)}</div>`;
  }

  if (cat === "image") {
    return `<div class="image-preview" data-format-id="${item.format_id}">
      <div class="image-loading">加载中...</div>
    </div>`;
  }

  if (cat === "files") {
    const files = Array.isArray(item.content) ? item.content : [];
    return `<div class="list-header">${files.length} 个文件</div>
    <ul class="file-list">${files.map((f) =>
      `<li><span class="file-icon">${ICONS.file}</span><span>${escapeHtml(f)}</span></li>`
    ).join("")}</ul>`;
  }

  if (cat === "binary") {
    return `<div class="card-content binary-content">${item.preview || "(binary data)"}</div>`;
  }

  if (cat === "empty") {
    return `<div class="card-content empty-content">(no data)</div>`;
  }

  if (cat === "error") {
    return `<div class="card-content error-content">${escapeHtml(item.preview || "(error)")}</div>`;
  }

  return `<div class="card-content">${escapeHtml(item.preview || "")}</div>`;
}

function renderCard(item: Entry, index: number): string {
  const cat = item.category || "unknown";
  const cfg = CATEGORY_CONFIG[cat] || { label: cat.toUpperCase(), color: "gray" };
  const contentHtml = renderCardContent(item);

  const metaItems: string[] = [];
  if (item.type_label) {
    metaItems.push(`<div class="meta-item"><span class="meta-label">Type:</span><span class="meta-value">${escapeHtml(item.type_label)}</span></div>`);
  }
  if (item.encoding) {
    metaItems.push(`<div class="meta-item"><span class="meta-label">Encoding:</span><span class="meta-value">${escapeHtml(item.encoding)}</span></div>`);
  }
  if (item.size != null) {
    metaItems.push(`<div class="meta-item"><span class="meta-label">Size:</span><span class="meta-value">${formatSize(item.size)}</span></div>`);
  }
  if (item.width && item.height) {
    metaItems.push(`<div class="meta-item"><span class="meta-label">Dimensions:</span><span class="meta-value">${item.width} × ${item.height} px</span></div>`);
  }

  const descAttr = item.description ? ` title="${escapeHtml(item.description)}"` : "";

  return `<div class="card" data-format-id="${item.format_id}">
    <div class="card-header"${descAttr}>
      <div class="card-header-left">
        <div class="card-icon icon-${cfg.color}">${getCategoryIcon(cat)}</div>
        <div>
          <div class="card-title">#${index + 1} ${escapeHtml(item.format_name || "Unknown")}</div>
          <div class="card-format">ID: ${item.format_id}</div>
        </div>
      </div>
      <span class="card-badge badge-${cfg.color}">${cfg.label}</span>
    </div>
    <div class="card-body">
      ${metaItems.length > 0 ? `<div class="card-meta">${metaItems.join("")}</div>` : ""}
      ${contentHtml}
    </div>
  </div>`;
}

function renderStats(formats: Entry[]): void {
  const grouped: Record<string, number> = {};
  for (const f of formats) {
    const cat = f.category || "unknown";
    grouped[cat] = (grouped[cat] || 0) + 1;
  }

  const chips: string[] = [];
  for (const [cat, count] of Object.entries(grouped)) {
    const cfg = CATEGORY_CONFIG[cat] || { label: cat.toUpperCase(), color: "gray" };
    const active = activeFilter === cat ? " active" : "";
    chips.push(`<div class="stat-chip${active}" data-category="${cat}">
      <span class="dot" style="background: var(--${cfg.color})"></span>
      <span>${count} ${cfg.label}</span>
    </div>`);
  }

  document.getElementById("stats")!.innerHTML = chips.join("");
}

function renderContent(data: ClipboardData): void {
  const container = document.getElementById("content")!;

  if (data.error) {
    container.innerHTML = `<div class="error-state">Error: ${escapeHtml(data.error)}</div>`;
    return;
  }

  const formats = data.formats || [];

  if (formats.length === 0) {
    activeFilter = null;
    container.innerHTML = `<div class="empty-state"><p>剪贴板为空</p></div>`;
    return;
  }

  if (activeFilter && !formats.some((f) => f.category === activeFilter)) {
    activeFilter = null;
  }

  renderStats(formats);

  const filtered = activeFilter
    ? formats.filter((f) => f.category === activeFilter)
    : formats;

  if (filtered.length === 0) {
    container.innerHTML = `<div class="empty-state"><p>没有匹配的格式</p></div>`;
    return;
  }

  container.innerHTML = `<div class="cards">${filtered.map((f, i) => renderCard(f, i)).join("")}</div>`;

  container.querySelectorAll(".image-preview[data-format-id]").forEach((el) => {
    const formatId = Number((el as HTMLElement).dataset.formatId);
    if (formatId) {
      loadImagePreview(el as HTMLElement, formatId);
    }
  });
}

async function loadImagePreview(container: HTMLElement, formatId: number): Promise<void> {
  const loading = container.querySelector(".image-loading");
  
  if (formatId === 2) {
    if (loading) loading.textContent = "GDI 位图句柄，无法直接显示";
    return;
  }

  if (loading) loading.textContent = "加载中...";
  
  try {
    const buffer = await invoke<ArrayBuffer>("get_format_raw_data", { formatId });
    const bytes = new Uint8Array(buffer);
    
    let blob: Blob;
    if (formatId === 8 || formatId === 17) {
      const bmp = buildBmpFromDib(bytes);
      if (!bmp) {
        if (loading) loading.textContent = "DIB 数据格式不支持直接渲染";
        return;
      }
      blob = new Blob([bmp.buffer as ArrayBuffer], { type: "image/bmp" });
    } else {
      const mime = detectImageMime(bytes);
      if (!mime) {
        if (loading) loading.textContent = "无法识别的图片格式";
        return;
      }
      blob = new Blob([bytes], { type: mime });
    }

    const url = URL.createObjectURL(blob);
    container.innerHTML = `<img src="${url}" alt="Preview">`;
  } catch (e) {
    if (loading) loading.textContent = `加载失败: ${String(e)}`;
  }
}

function detectImageMime(bytes: Uint8Array): string | null {
  if (bytes.length < 8) return null;
  
  // PNG: 89 50 4E 47 0D 0A 1A 0A
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) {
    return "image/png";
  }
  
  // JPEG: FF D8 FF
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) {
    return "image/jpeg";
  }
  
  // GIF: 47 49 46 38
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) {
    return "image/gif";
  }
  
  // BMP: 42 4D
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) {
    return "image/bmp";
  }
  
  return null;
}

function buildBmpFromDib(dib: Uint8Array): Uint8Array | null {
  if (dib.length < 40) return null;

  const headerSize = dib[0] | (dib[1] << 8) | (dib[2] << 16) | (dib[3] << 24);
  if (headerSize !== 40 && headerSize !== 124) return null;

  const bitCount = dib[14] | (dib[15] << 8);
  const compression = dib[16] | (dib[17] << 8) | (dib[18] << 16) | (dib[19] << 24);

  let colorTableSize = 0;
  if (headerSize === 40) {
    if (bitCount <= 8) {
      const numColors = compression === 3 ? 3 : (1 << bitCount);
      colorTableSize = numColors * 4;
    } else if (bitCount === 16 || bitCount === 32) {
      if (compression === 3) colorTableSize = 12;
    }
  }

  const pixelOffset = 14 + headerSize + colorTableSize;
  const fileSize = 14 + dib.length;

  const bmp = new Uint8Array(fileSize);
  bmp[0] = 0x42; bmp[1] = 0x4d;
  bmp[2] = fileSize & 0xff;
  bmp[3] = (fileSize >> 8) & 0xff;
  bmp[4] = (fileSize >> 16) & 0xff;
  bmp[5] = (fileSize >> 24) & 0xff;
  bmp[10] = pixelOffset & 0xff;
  bmp[11] = (pixelOffset >> 8) & 0xff;
  bmp[12] = (pixelOffset >> 16) & 0xff;
  bmp[13] = (pixelOffset >> 24) & 0xff;
  bmp.set(dib, 14);

  return bmp;
}

async function loadClipboard(manualRefresh = false): Promise<void> {
  const btn = document.getElementById("refresh-btn")!;
  btn.classList.add("loading");

  try {
    const currentHash = await invoke<string>("get_clipboard_hash");
    
    if (!manualRefresh && currentHash === previousClipboardHash) {
      btn.classList.remove("loading");
      return;
    }
    
    previousClipboardHash = currentHash;
    
    const jsonStr = await invoke<string>("read_clipboard");
    const data: ClipboardData = JSON.parse(jsonStr);
    renderContent(data);
  } catch (e) {
    document.getElementById("content")!.innerHTML =
      `<div class="error-state">Failed to load: ${escapeHtml(String(e))}</div>`;
  } finally {
    btn.classList.remove("loading");
  }
}

function setFilter(category: string): void {
  activeFilter = activeFilter === category ? null : category;
  previousClipboardHash = null;
  loadClipboard();
}

function handleHtmlToggle(btn: HTMLElement): void {
  const card = btn.closest(".html-card");
  if (!card) return;

  const action = btn.dataset.action;
  const buttons = card.querySelectorAll(".html-toggle-btn");
  const sourcePane = card.querySelector(".html-source-pane") as HTMLElement;
  const previewPane = card.querySelector(".html-preview-pane") as HTMLElement;
  if (!sourcePane || !previewPane) return;

  buttons.forEach((b) => b.classList.remove("active"));
  btn.classList.add("active");

  if (action === "preview") {
    sourcePane.style.display = "none";
    previewPane.style.display = "";
  } else {
    sourcePane.style.display = "";
    previewPane.style.display = "none";
  }
}

window.addEventListener("DOMContentLoaded", () => {
  const refreshBtn = document.getElementById("refresh-btn")!;
  const autoRefreshCb = document.getElementById("auto-refresh") as HTMLInputElement;
  const themeToggle = document.getElementById("theme-toggle")!;

  const savedTheme = localStorage.getItem("theme") || "dark";
  document.documentElement.setAttribute("data-theme", savedTheme);

  themeToggle.addEventListener("click", () => {
    const currentTheme = document.documentElement.getAttribute("data-theme") || "dark";
    const newTheme = currentTheme === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", newTheme);
    localStorage.setItem("theme", newTheme);
  });

  refreshBtn.addEventListener("click", () => loadClipboard(true));

  const updateAutoRefresh = () => {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
    if (autoRefreshCb.checked) {
      refreshTimer = setInterval(() => loadClipboard(), 2000);
    }
  };

  autoRefreshCb.addEventListener("change", updateAutoRefresh);
  updateAutoRefresh();

  document.getElementById("stats")!.addEventListener("click", (e) => {
    const chip = (e.target as HTMLElement).closest(".stat-chip") as HTMLElement | null;
    if (chip?.dataset.category) {
      setFilter(chip.dataset.category);
    }
  });

  document.getElementById("content")!.addEventListener("click", (e) => {
    const toggleBtn = (e.target as HTMLElement).closest(".html-toggle-btn") as HTMLElement | null;
    if (toggleBtn) {
      handleHtmlToggle(toggleBtn);
    }
  });

  loadClipboard();
});
