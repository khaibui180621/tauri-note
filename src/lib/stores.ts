// ============================================================
// src/lib/stores.ts
// ScreenNote Overlay — Svelte Stores (State Management)
//
// Tất cả trạng thái toàn cục của ứng dụng được quản lý tập trung
// tại đây để tách biệt logic khỏi UI components.
// ============================================================

import { writable, derived, get } from 'svelte/store';

// ============================================================
// TYPES
// ============================================================

export type Tool = 'pen' | 'eraser';

export interface DrawingState {
  imageData: ImageData | null;
}

// ============================================================
// CORE STORES
// ============================================================

/**
 * isDrawingMode: true = đang vẽ, false = click-through mode
 * Khi false, canvas sẽ có pointer-events: none (CSS)
 * và Rust sẽ không nhận cursor events (hybrid approach)
 */
export const isDrawingMode = writable<boolean>(true);

/**
 * Màu hiện tại đang được chọn (hex string)
 */
export const currentColor = writable<string>('#FF4136');

/**
 * Kích thước nét bút (pixel)
 */
export const brushSize = writable<number>(4);

/**
 * Công cụ hiện tại: bút vẽ hoặc cục tẩy
 */
export const currentTool = writable<Tool>('pen');

/**
 * Hiển thị/ẩn thanh toolbar
 */
export const isToolbarVisible = writable<boolean>(true);

// ============================================================
// DERIVED STORES
// ============================================================

/**
 * CSS pointer-events value cho canvas:
 *   - 'auto' khi đang ở drawing mode (nhận chuột bình thường)
 *   - 'none' khi ở click-through mode (chuột xuyên qua canvas, đến toolbar hoặc OS)
 */
export const canvasPointerEvents = derived(
  isDrawingMode,
  ($isDrawingMode) => ($isDrawingMode ? 'auto' : 'none')
);

/**
 * Cursor style cho canvas dựa theo tool đang chọn và drawing mode.
 */
export const canvasCursor = derived(
  [isDrawingMode, currentTool],
  ([$isDrawingMode, $currentTool]) => {
    if (!$isDrawingMode) return 'default';
    if ($currentTool === 'eraser') return 'cell';
    return 'crosshair';
  }
);

// ============================================================
// UNDO/REDO HISTORY STACK
// ============================================================

/**
 * Stack lưu trạng thái canvas (ImageData snapshots).
 * Tối đa MAX_HISTORY bước để tránh tràn RAM.
 */
const MAX_HISTORY = 20;

export const undoStack = writable<DrawingState[]>([]);
export const redoStack = writable<DrawingState[]>([]);

/** True khi có thể Undo */
export const canUndo = derived(undoStack, ($stack) => $stack.length > 0);

/** True khi có thể Redo */
export const canRedo = derived(redoStack, ($stack) => $stack.length > 0);

/**
 * Lưu trạng thái hiện tại vào undo stack.
 * @param snapshot ImageData của canvas tại thời điểm gọi hàm
 */
export function pushUndoState(snapshot: ImageData): void {
  undoStack.update((stack) => {
    const newStack = [...stack, { imageData: snapshot }];
    // Giới hạn tối đa MAX_HISTORY phần tử (FIFO: xóa phần tử cũ nhất)
    if (newStack.length > MAX_HISTORY) {
      newStack.shift();
    }
    return newStack;
  });
  // Khi thực hiện action mới, xóa redo stack
  redoStack.set([]);
}

/**
 * Lấy trạng thái Undo (pop từ stack).
 * @returns ImageData của trạng thái trước đó, hoặc null nếu stack rỗng
 */
export function popUndoState(currentSnapshot: ImageData): ImageData | null {
  const stack = get(undoStack);
  if (stack.length === 0) return null;

  const previousState = stack[stack.length - 1];

  // Push trạng thái hiện tại vào redo stack
  redoStack.update((rStack) => {
    const newStack = [...rStack, { imageData: currentSnapshot }];
    if (newStack.length > MAX_HISTORY) newStack.shift();
    return newStack;
  });

  // Pop từ undo stack
  undoStack.update((s) => s.slice(0, -1));

  return previousState.imageData;
}

/**
 * Lấy trạng thái Redo (pop từ redo stack).
 */
export function popRedoState(currentSnapshot: ImageData): ImageData | null {
  const stack = get(redoStack);
  if (stack.length === 0) return null;

  const nextState = stack[stack.length - 1];

  // Push trạng thái hiện tại vào undo stack
  undoStack.update((uStack) => {
    const newStack = [...uStack, { imageData: currentSnapshot }];
    if (newStack.length > MAX_HISTORY) newStack.shift();
    return newStack;
  });

  // Pop từ redo stack
  redoStack.update((s) => s.slice(0, -1));

  return nextState.imageData;
}

/**
 * Xóa toàn bộ history (khi Clear canvas)
 */
export function clearHistory(): void {
  undoStack.set([]);
  redoStack.set([]);
}

// ============================================================
// PRESET COLORS
// ============================================================

export const PRESET_COLORS = [
  { label: 'Đỏ',    value: '#FF4136' },
  { label: 'Cam',   value: '#FF851B' },
  { label: 'Vàng',  value: '#FFDC00' },
  { label: 'Xanh lá', value: '#2ECC40' },
  { label: 'Xanh dương', value: '#0074D9' },
  { label: 'Tím',   value: '#B10DC9' },
  { label: 'Trắng', value: '#FFFFFF' },
  { label: 'Đen',   value: '#111111' },
] as const;
