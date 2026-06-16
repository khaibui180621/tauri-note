<!--
  src/routes/+page.svelte
  ScreenNote Overlay — Trang chính: Canvas + Toolbar Integration

  Kiến trúc:
  ┌─────────────────────────────────────────────────────────┐
  │  <canvas> (100vw × 100vh, z-index: 1)                   │
  │    - pointer-events: auto / none (theo isDrawingMode)   │
  │    - Vẽ tự do, requestAnimationFrame để mượt            │
  │                                                         │
  │  <Toolbar> (fixed, bottom-center, z-index: 9999)        │
  │    - pointer-events: auto LUÔN LUÔN                     │
  └─────────────────────────────────────────────────────────┘

  Luồng IPC Tauri ↔ Svelte:
    Rust → Svelte: event "toggle-draw-mode", "tray-toggle-toolbar", "shortcut-register-failed"
    Svelte → Rust: invoke("set_click_through", { enabled })
-->

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import Toolbar from '$lib/components/Toolbar.svelte';
  import {
    isDrawingMode,
    currentColor,
    brushSize,
    currentTool,
    isToolbarVisible,
    canvasPointerEvents,
    canvasCursor,
    pushUndoState,
    popUndoState,
    popRedoState,
    clearHistory,
  } from '$lib/stores';

  // ============================================================
  // CANVAS REFS & CONTEXT
  // ============================================================

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;

  // ============================================================
  // DRAWING STATE (local, không cần store vì chỉ dùng trong file này)
  // ============================================================

  let isPointerDown = false;          // Đang giữ chuột/pen
  let lastX = 0;                      // Tọa độ X của điểm vẽ trước
  let lastY = 0;                      // Tọa độ Y của điểm vẽ trước
  let animFrameId: number | null = null; // RAF handle để cancel khi cần

  // Điểm cần vẽ trong RAF (buffer để tránh lag khi mouse quá nhanh)
  let pendingX = 0;
  let pendingY = 0;
  let hasPendingDraw = false;

  // Toast notification state
  let toastMessage = '';
  let toastVisible = false;
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  // ============================================================
  // TAURI EVENT LISTENERS
  // ============================================================

  let unlistenToggle: UnlistenFn | null = null;
  let unlistenTray: UnlistenFn | null = null;
  let unlistenShortcutError: UnlistenFn | null = null;

  // ============================================================
  // LIFECYCLE
  // ============================================================

  onMount(async () => {
    // Khởi tạo canvas context
    initCanvas();

    // Lắng nghe phím tắt từ Rust backend (Cmd+Shift+D trên macOS / Ctrl+Shift+D trên Windows)
    // Khi Rust emit "toggle-draw-mode", ta đảo trạng thái
    unlistenToggle = await listen('toggle-draw-mode', () => {
      toggleDrawMode();
    });

    // Lắng nghe sự kiện từ System Tray (toggle toolbar)
    unlistenTray = await listen('tray-toggle-toolbar', () => {
      isToolbarVisible.update((v) => !v);
    });

    // Lắng nghe lỗi đăng ký phím tắt từ Rust
    unlistenShortcutError = await listen<string>('shortcut-register-failed', (event) => {
      showToast(`⚠️ ${event.payload}`, 5000);
    });

    // Resize canvas khi cửa sổ thay đổi kích thước
    window.addEventListener('resize', handleResize);

    // Keyboard shortcuts (frontend-level)
    window.addEventListener('keydown', handleKeyboard);
  });

  onDestroy(() => {
    // Cleanup tất cả listeners để tránh memory leak
    unlistenToggle?.();
    unlistenTray?.();
    unlistenShortcutError?.();
    window.removeEventListener('resize', handleResize);
    window.removeEventListener('keydown', handleKeyboard);
    if (animFrameId !== null) cancelAnimationFrame(animFrameId);
    if (toastTimer !== null) clearTimeout(toastTimer);
  });

  // ============================================================
  // CANVAS INITIALIZATION
  // ============================================================

  function initCanvas() {
    if (!canvas) return;
    ctx = canvas.getContext('2d', { willReadFrequently: true })!;

    // Đặt kích thước canvas bằng kích thước màn hình thực
    resizeCanvas();

    // Cấu hình mặc định context
    ctx.lineCap = 'round';    // Đầu nét tròn, trông mượt hơn
    ctx.lineJoin = 'round';   // Góc nét tròn
  }

  function resizeCanvas() {
    if (!canvas || !ctx) return;

    // Lưu nội dung hiện tại trước khi resize
    const imageData = canvas.width > 0
      ? ctx.getImageData(0, 0, canvas.width, canvas.height)
      : null;

    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    // Khôi phục nội dung sau resize
    if (imageData) {
      ctx.putImageData(imageData, 0, 0);
    }

    // Re-apply context settings (reset sau khi resize)
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
  }

  function handleResize() {
    resizeCanvas();
  }

  // ============================================================
  // DRAWING EVENT HANDLERS
  // ============================================================

  /**
   * mousedown / pointerdown: bắt đầu nét vẽ mới.
   * Lưu snapshot vào undo stack trước khi bắt đầu vẽ.
   */
  function handlePointerDown(e: PointerEvent) {
    if (!$isDrawingMode) return;

    isPointerDown = true;

    const { x, y } = getCanvasPos(e);
    lastX = x;
    lastY = y;
    pendingX = x;
    pendingY = y;

    // Lưu snapshot TRƯỚC khi vẽ (để Undo về trạng thái này)
    const snapshot = ctx.getImageData(0, 0, canvas.width, canvas.height);
    pushUndoState(snapshot);

    // Bắt đầu path mới
    ctx.beginPath();
    ctx.moveTo(x, y);

    // Vẽ một chấm tại điểm bắt đầu (cho phép click đơn tạo chấm)
    drawDot(x, y);

    // Bắt requestAnimationFrame loop
    scheduleFrame();
  }

  /**
   * mousemove / pointermove: cập nhật điểm đích để RAF vẽ.
   * Ta KHÔNG vẽ trực tiếp ở đây để tránh lag — RAF sẽ xử lý.
   */
  function handlePointerMove(e: PointerEvent) {
    if (!isPointerDown || !$isDrawingMode) return;

    const { x, y } = getCanvasPos(e);
    pendingX = x;
    pendingY = y;
    hasPendingDraw = true;
  }

  /**
   * mouseup / pointerup: kết thúc nét vẽ.
   */
  function handlePointerUp() {
    if (!isPointerDown) return;
    isPointerDown = false;
    hasPendingDraw = false;

    // Cancel RAF nếu đang chạy
    if (animFrameId !== null) {
      cancelAnimationFrame(animFrameId);
      animFrameId = null;
    }
  }

  /**
   * mouseleave: xử lý khi chuột rời khỏi canvas trong khi đang vẽ.
   */
  function handlePointerLeave() {
    if (isPointerDown) {
      handlePointerUp();
    }
  }

  // ============================================================
  // requestAnimationFrame DRAWING LOOP
  // ============================================================

  /**
   * Lập lịch frame vẽ tiếp theo.
   * Kỹ thuật: dùng RAF thay vì vẽ trực tiếp trong mousemove
   * để đồng bộ với refresh rate của màn hình → mượt hơn.
   */
  function scheduleFrame() {
    if (animFrameId !== null) return; // Đã có frame đang chờ
    animFrameId = requestAnimationFrame(drawFrame);
  }

  /**
   * Frame callback: thực sự vẽ lên canvas.
   * Chỉ vẽ khi có điểm mới (hasPendingDraw).
   */
  function drawFrame() {
    animFrameId = null;

    if (hasPendingDraw && isPointerDown) {
      drawSegment(lastX, lastY, pendingX, pendingY);
      lastX = pendingX;
      lastY = pendingY;
      hasPendingDraw = false;
    }

    // Tiếp tục loop nếu đang vẽ
    if (isPointerDown) {
      scheduleFrame();
    }
  }

  // ============================================================
  // DRAW HELPERS
  // ============================================================

  /**
   * Vẽ một đoạn thẳng từ (x1,y1) đến (x2,y2).
   * Xử lý cả bút vẽ và tẩy.
   */
  function drawSegment(x1: number, y1: number, x2: number, y2: number) {
    if (!ctx) return;

    if ($currentTool === 'eraser') {
      // TẨY: dùng destination-out để "xóa" pixel (trả về trong suốt)
      // destination-out: vẽ với alpha=0, làm trong suốt phần được vẽ đè lên
      ctx.globalCompositeOperation = 'destination-out';
      ctx.strokeStyle = 'rgba(0,0,0,1)'; // Màu không quan trọng, alpha mới là chìa khóa
      ctx.lineWidth = $brushSize * 3; // Tẩy thường to hơn bút
    } else {
      // BÚT VẼ: vẽ bình thường
      ctx.globalCompositeOperation = 'source-over';
      ctx.strokeStyle = $currentColor;
      ctx.lineWidth = $brushSize;
    }

    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();

    // QUAN TRỌNG: Reset globalCompositeOperation về mặc định sau mỗi thao tác
    // để tránh ảnh hưởng đến các thao tác vẽ tiếp theo
    ctx.globalCompositeOperation = 'source-over';
  }

  /**
   * Vẽ một chấm tại điểm (x,y) — dùng khi click đơn không di chuyển.
   */
  function drawDot(x: number, y: number) {
    if (!ctx) return;

    if ($currentTool === 'eraser') {
      ctx.globalCompositeOperation = 'destination-out';
      ctx.fillStyle = 'rgba(0,0,0,1)';
    } else {
      ctx.globalCompositeOperation = 'source-over';
      ctx.fillStyle = $currentColor;
    }

    ctx.beginPath();
    ctx.arc(x, y, ($currentTool === 'eraser' ? $brushSize * 1.5 : $brushSize / 2), 0, Math.PI * 2);
    ctx.fill();
    ctx.globalCompositeOperation = 'source-over';
  }

  /**
   * Chuyển đổi tọa độ chuột sang tọa độ canvas (xử lý DPI scaling).
   */
  function getCanvasPos(e: PointerEvent): { x: number; y: number } {
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    return {
      x: (e.clientX - rect.left) * scaleX,
      y: (e.clientY - rect.top) * scaleY,
    };
  }

  // ============================================================
  // UNDO / REDO / CLEAR
  // ============================================================

  function handleUndo() {
    if (!ctx) return;
    const currentSnapshot = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const previousState = popUndoState(currentSnapshot);
    if (previousState) {
      ctx.putImageData(previousState, 0, 0);
    }
  }

  function handleRedo() {
    if (!ctx) return;
    const currentSnapshot = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const nextState = popRedoState(currentSnapshot);
    if (nextState) {
      ctx.putImageData(nextState, 0, 0);
    }
  }

  function handleClear() {
    if (!ctx) return;
    // Lưu snapshot để có thể undo việc clear
    const snapshot = ctx.getImageData(0, 0, canvas.width, canvas.height);
    pushUndoState(snapshot);
    // Xóa toàn bộ canvas (trong suốt)
    ctx.clearRect(0, 0, canvas.width, canvas.height);
  }

  // ============================================================
  // DRAWING MODE TOGGLE
  // ============================================================

  /**
   * Toggle giữa Drawing mode và Click-through mode.
   *
   * Cơ chế đầy đủ (OS-level click-through):
   *   - Drawing mode ON  → set_ignore_cursor_events(false) → cửa sổ nhận chuột bình thường
   *   - Drawing mode OFF → set_ignore_cursor_events(true)  → cửa sổ trong suốt với chuột,
   *     tất cả click đi xuyên qua tới ứng dụng bên dưới như bình thường.
   *
   * Khi pass-through: toolbar bị ẩn, user dùng Cmd+Shift+D (global shortcut) để quay lại.
   */
  async function toggleDrawMode() {
    const newMode = !$isDrawingMode;
    isDrawingMode.set(newMode);

    try {
      // newMode = true  → đang vẽ   → KHÔNG xuyên qua (ignore = false)
      // newMode = false → pass-through → CÓ xuyên qua (ignore = true)
      await invoke('set_click_through', { enabled: !newMode });
    } catch (e) {
      console.error('[ScreenNote] Lỗi set_click_through:', e);
    }
  }

  // ============================================================
  // SCREENSHOT
  // ============================================================

  async function handleScreenshot() {
    // Ẩn tạm thời toolbar để không bị chụp dính vào ảnh
    isDrawingMode.set(false);
    
    // Đợi UI cập nhật (100ms)
    await new Promise(resolve => setTimeout(resolve, 100));

    try {
      const path = await invoke('take_screenshot');
      showToast(`Đã lưu ảnh chụp màn hình tại: ${path}`);
    } catch (e) {
      console.error(e);
      showToast(`Lỗi chụp ảnh: ${e}`);
    } finally {
      // Hiện lại toolbar
      isDrawingMode.set(true);
    }
  }

  // ============================================================
  // KEYBOARD SHORTCUTS (Frontend-level)
  // ============================================================

  function handleKeyboard(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;

    // Ctrl+Z: Undo
    if (ctrl && e.key === 'z' && !e.shiftKey) {
      e.preventDefault();
      handleUndo();
    }

    // Ctrl+Y hoặc Ctrl+Shift+Z: Redo
    if ((ctrl && e.key === 'y') || (ctrl && e.shiftKey && e.key === 'z')) {
      e.preventDefault();
      handleRedo();
    }

    // Escape: Tắt drawing mode
    if (e.key === 'Escape' && $isDrawingMode) {
      toggleDrawMode();
    }

    // P: Chọn bút
    if (e.key === 'p' || e.key === 'P') {
      currentTool.set('pen');
    }

    // E: Chọn tẩy
    if (e.key === 'e' || e.key === 'E') {
      currentTool.set('eraser');
    }
  }

  // ============================================================
  // TOAST NOTIFICATION
  // ============================================================

  function showToast(message: string, duration = 3000) {
    toastMessage = message;
    toastVisible = true;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastVisible = false;
    }, duration);
  }
</script>

<svelte:head>
  <title>ScreenNote Overlay</title>
  <meta name="description" content="Vẽ và ghi chú trực tiếp lên màn hình máy tính" />
</svelte:head>

<!--
  Canvas: phủ toàn màn hình
  pointer-events phụ thuộc vào $canvasPointerEvents (từ store)
  Khi isDrawingMode = false → pointer-events: none → chuột đi xuyên qua xuống toolbar
-->
<canvas
  bind:this={canvas}
  id="drawing-canvas"
  style="pointer-events: {$canvasPointerEvents}; cursor: {$canvasCursor};"
  on:pointerdown={handlePointerDown}
  on:pointermove={handlePointerMove}
  on:pointerup={handlePointerUp}
  on:pointerleave={handlePointerLeave}
  aria-label="Drawing canvas"
></canvas>

<!-- Toolbar: chỉ render khi ở drawing mode, tự ẩn khi pass-through -->
<Toolbar
  onUndo={handleUndo}
  onRedo={handleRedo}
  onClear={handleClear}
  onScreenshot={handleScreenshot}
  onToggleMode={toggleDrawMode}
/>

<!-- Toast notification (ví dụ: cảnh báo phím tắt bị chiếm) -->
{#if toastVisible}
  <div class="toast" role="alert" aria-live="assertive">
    {toastMessage}
  </div>
{/if}


<style>
  /* ==========================================================
     CANVAS
     ========================================================== */
  canvas {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    z-index: 1;
    /* Canvas trong suốt — background do OS xử lý */
    background: transparent;
    /* Touch action none để hỗ trợ stylus/bút cảm ứng */
    touch-action: none;
  }

  /* ==========================================================
     TOAST NOTIFICATION
     ========================================================== */
  .toast {
    position: fixed;
    top: 24px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10000;
    padding: 10px 20px;
    background: rgba(30, 30, 40, 0.92);
    backdrop-filter: blur(12px);
    color: #fbbf24;
    border: 1px solid rgba(251, 191, 36, 0.3);
    border-radius: 12px;
    font-size: 13px;
    font-weight: 500;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
    pointer-events: none;
    animation: toast-in 0.25s ease both;
  }

  @keyframes toast-in {
    from { opacity: 0; transform: translateX(-50%) translateY(-10px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }

</style>
