<!--
  src/lib/components/Toolbar.svelte
  ScreenNote Overlay — Thanh công cu nổi (Floating Toolbar)

  Thiết kế: Floating pill ở bottom-center màn hình.

  Click-Through:
  - Drawing mode = true  → toolbar hiện đầy đủ, có thể tương tác
  - Drawing mode = false → toàn bộ cửa sổ xuyên thấu (OS-level), toolbar ẩn
    → Chỉ hiện badge nhỏ, user dùng Ctrl+Shift+D để quay lại vẽ
-->

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import {
    isDrawingMode,
    currentColor,
    brushSize,
    currentTool,
    isToolbarVisible,
    canUndo,
    canRedo,
    PRESET_COLORS,
    type Tool,
  } from '$lib/stores';

  // Props
  export let onUndo: () => void = () => {};
  export let onRedo: () => void = () => {};
  export let onClear: () => void = () => {};
  // Toggle mode được xử lý từ parent (+page.svelte) để gọi invoke Rust
  export let onToggleMode: () => void = () => {};

  function selectTool(tool: Tool) { currentTool.set(tool); }

  function selectColor(color: string) {
    currentColor.set(color);
    currentTool.set('pen');
  }

  function handleBrushSizeChange(e: Event) {
    const target = e.target as HTMLInputElement;
    brushSize.set(Number(target.value));
  }
</script>

{#if $isDrawingMode}
  <!-- Full toolbar: chỉ hiện khi đang ở drawing mode -->
  <div class="toolbar" role="toolbar" aria-label="Drawing toolbar">

    <!-- === DRAWING MODE TOGGLE === -->
    <div class="toolbar-group">
      <button
        id="btn-toggle-draw"
        class="btn-toggle active"
        on:click={onToggleMode}
        title="Tắt vẽ — Chuyển sang chế độ click-through (Ctrl+Shift+D)"
        aria-pressed={true}
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="18" height="18">
          <path d="M20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.84 1.83 3.75 3.75L20.71 7.04zM3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25z"/>
        </svg>
        <span>Đang vẽ</span>
      </button>
    </div>

    <div class="divider" aria-hidden="true"></div>

    <!-- === COLOR PRESETS === -->
    <div class="toolbar-group" role="group" aria-label="Bảng màu">
      {#each PRESET_COLORS as color}
        <button
          class="color-dot"
          class:selected={$currentColor === color.value}
          style="background-color: {color.value};"
          title={color.label}
          aria-label="Chọn màu {color.label}"
          aria-pressed={$currentColor === color.value}
          on:click={() => selectColor(color.value)}
        ></button>
      {/each}
      <label class="color-picker-label" title="Màu tùy chỉnh" aria-label="Màu tùy chỉnh">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="14" height="14">
          <path d="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9c.83 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-.99 0-.83.67-1.5 1.5-1.5H16c2.76 0 5-2.24 5-5 0-4.42-4.03-8-9-8zm-5.5 9c-.83 0-1.5-.67-1.5-1.5S5.67 9 6.5 9 8 9.67 8 10.5 7.33 12 6.5 12zm3-4C8.67 8 8 7.33 8 6.5S8.67 5 9.5 5s1.5.67 1.5 1.5S10.33 8 9.5 8zm5 0c-.83 0-1.5-.67-1.5-1.5S13.67 5 14.5 5s1.5.67 1.5 1.5S15.33 8 14.5 8zm3 4c-.83 0-1.5-.67-1.5-1.5S16.67 9 17.5 9s1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/>
        </svg>
        <input
          type="color"
          bind:value={$currentColor}
          on:change={() => currentTool.set('pen')}
          aria-label="Chọn màu tùy chỉnh"
        />
      </label>
    </div>

    <div class="divider" aria-hidden="true"></div>

    <!-- === BRUSH SIZE === -->
    <div class="toolbar-group brush-size-group" role="group" aria-label="Kích thước nét">
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="14" height="14" style="opacity:0.7">
        <circle cx="12" cy="12" r="4"/>
      </svg>
      <input
        id="brush-size-slider"
        type="range"
        min="1"
        max="40"
        value={$brushSize}
        on:input={handleBrushSizeChange}
        aria-label="Kích thước nét bút: {$brushSize}px"
        title="Kích thước nét: {$brushSize}px"
        class="brush-slider"
      />
      <span class="brush-size-label">{$brushSize}px</span>
    </div>

    <div class="divider" aria-hidden="true"></div>

    <!-- === TOOLS === -->
    <div class="toolbar-group" role="group" aria-label="Công cụ">
      <button
        id="btn-pen"
        class="btn-tool"
        class:active={$currentTool === 'pen'}
        on:click={() => selectTool('pen')}
        title="Bút vẽ (P)"
        aria-pressed={$currentTool === 'pen'}
        aria-label="Bút vẽ"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.84 1.83 3.75 3.75 1.84-1.83z"/>
        </svg>
      </button>
      <button
        id="btn-eraser"
        class="btn-tool"
        class:active={$currentTool === 'eraser'}
        on:click={() => selectTool('eraser')}
        title="Cục tẩy (E)"
        aria-pressed={$currentTool === 'eraser'}
        aria-label="Cục tẩy"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <path d="M15.14 3c-.51 0-1.02.2-1.41.59L2.59 14.73c-.78.77-.78 2.04 0 2.83L5.03 20h7.66l8.72-8.72c.78-.77.78-2.04 0-2.83l-4.86-4.86C16.16 3.2 15.65 3 15.14 3zM6.21 19l-2.1-2.1 5-5 2.12 2.12L6.21 19z"/>
        </svg>
      </button>
    </div>

    <div class="divider" aria-hidden="true"></div>

    <!-- === UNDO / REDO / CLEAR === -->
    <div class="toolbar-group" role="group" aria-label="Lịch sử">
      <button
        id="btn-undo"
        class="btn-action"
        on:click={onUndo}
        disabled={!$canUndo}
        title="Hoàn tác (Ctrl+Z)"
        aria-label="Hoàn tác"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <path d="M12.5 8c-2.65 0-5.05.99-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z"/>
        </svg>
      </button>
      <button
        id="btn-redo"
        class="btn-action"
        on:click={onRedo}
        disabled={!$canRedo}
        title="Làm lại (Ctrl+Y)"
        aria-label="Làm lại"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <path d="M18.4 10.6C16.55 8.99 14.15 8 11.5 8c-4.65 0-8.58 3.03-9.96 7.22L3.9 16c1.05-3.19 4.05-5.5 7.6-5.5 1.95 0 3.73.72 5.12 1.88L13 16h9V7l-3.6 3.6z"/>
        </svg>
      </button>
      <button
        id="btn-clear"
        class="btn-action btn-clear"
        on:click={onClear}
        title="Xóa toàn bộ"
        aria-label="Xóa toàn bộ canvas"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
        </svg>
      </button>
    </div>

    <!-- Keyboard shortcut hint -->
    <div class="shortcut-hint" aria-live="polite">
      <span>Ctrl+Shift+D</span>
    </div>
  </div>
{/if}

<style>
  /* ==========================================================
     TOOLBAR — Floating pill, bottom-center
     Luôn có pointer-events: auto kể cả khi canvas bị disabled
     ========================================================== */
  .toolbar {
    /* Vị trí: bottom-center, floating */
    position: fixed;
    bottom: 28px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 9999;

    /* Layout: horizontal flex */
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 16px;

    /* Glassmorphism dark design */
    background: rgba(18, 18, 24, 0.85);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 50px;
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.5),
      0 2px 8px rgba(0, 0, 0, 0.3),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);

    /* QUAN TRỌNG: Toolbar luôn nhận chuột dù canvas bị disabled */
    pointer-events: auto !important;

    /* Animation khi xuất hiện */
    animation: toolbar-appear 0.3s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  @keyframes toolbar-appear {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(20px) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateX(-50%) translateY(0) scale(1);
    }
  }

  /* === GROUPS === */
  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .divider {
    width: 1px;
    height: 24px;
    background: rgba(255, 255, 255, 0.15);
    margin: 0 4px;
    flex-shrink: 0;
  }

  /* === TOGGLE BUTTON (Draw mode) === */
  .btn-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.3px;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
  }

  .btn-toggle:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .btn-toggle.active {
    background: rgba(99, 102, 241, 0.3);
    border-color: rgba(99, 102, 241, 0.6);
    color: #a5b4fc;
    box-shadow: 0 0 12px rgba(99, 102, 241, 0.3);
  }

  .btn-toggle.active:hover {
    background: rgba(99, 102, 241, 0.4);
  }

  /* === COLOR DOTS === */
  .color-dot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    transition: all 0.15s ease;
    flex-shrink: 0;
    box-shadow: inset 0 1px 2px rgba(0,0,0,0.3);
  }

  .color-dot:hover {
    transform: scale(1.2);
    border-color: rgba(255, 255, 255, 0.5);
  }

  .color-dot.selected {
    border-color: white;
    transform: scale(1.15);
    box-shadow:
      0 0 0 2px rgba(255,255,255,0.3),
      inset 0 1px 2px rgba(0,0,0,0.3);
  }

  /* === CUSTOM COLOR PICKER === */
  .color-picker-label {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255,255,255,0.1);
    border: 1.5px dashed rgba(255,255,255,0.3);
    cursor: pointer;
    transition: all 0.15s ease;
    color: rgba(255,255,255,0.6);
    position: relative;
    overflow: hidden;
  }

  .color-picker-label:hover {
    background: rgba(255,255,255,0.2);
    border-color: rgba(255,255,255,0.6);
    color: white;
  }

  .color-picker-label input[type="color"] {
    position: absolute;
    opacity: 0;
    width: 100%;
    height: 100%;
    cursor: pointer;
    inset: 0;
  }

  /* === BRUSH SLIDER === */
  .brush-size-group {
    gap: 8px;
  }

  .brush-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 80px;
    height: 4px;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }

  .brush-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #a5b4fc;
    cursor: pointer;
    box-shadow: 0 0 6px rgba(99, 102, 241, 0.6);
    transition: transform 0.1s ease;
  }

  .brush-slider::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  .brush-size-label {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
    min-width: 28px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* === TOOL BUTTONS === */
  .btn-tool {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-tool:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .btn-tool.active {
    background: rgba(99, 102, 241, 0.25);
    color: #a5b4fc;
    border: 1px solid rgba(99, 102, 241, 0.4);
  }

  /* === ACTION BUTTONS (Undo, Redo, Clear) === */
  .btn-action {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-action:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .btn-action:disabled {
    opacity: 0.25;
    cursor: not-allowed;
  }

  .btn-clear:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.2);
    color: #fca5a5;
  }

  /* === SHORTCUT HINT === */
  .shortcut-hint {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.25);
    padding-left: 4px;
    letter-spacing: 0.5px;
    white-space: nowrap;
  }

  /* === GLOBAL BUTTON RESET === */
  button {
    font-family: inherit;
    outline: none;
    -webkit-app-region: no-drag;
  }

  button:focus-visible {
    outline: 2px solid rgba(99, 102, 241, 0.8);
    outline-offset: 2px;
  }
</style>
