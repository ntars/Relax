// 文件屏纯函数 reducer（手册 C4.5 / 3.7）：上传进度 + 断点续传 + 回收站 + 能力控制 + 五态角标（7.4）。
// 能力来自 gen/capabilities；错误只按 error.code 的 name 分支，不写三位数字字面量比较。

import { ERROR_CODES } from '../../gen/error-codes';
import { CAPABILITIES, type Role } from '../../gen/capabilities';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface UiFile {
  readonly id: string;
  readonly name: string;
  readonly size: number;
  readonly mime: string;
}

/** 上传任务状态：UPLOAD_INTERRUPTED（3005）后进入 interrupted，可点「续传」只传缺失片。 */
export type UploadStatus = 'idle' | 'uploading' | 'interrupted' | 'done' | 'failed';

export interface UploadTask {
  readonly id: string;
  readonly name: string;
  readonly uploaded: number;
  readonly total: number;
  readonly status: UploadStatus;
}

export interface FilesViewState {
  readonly items: readonly UiFile[];
  readonly recycle: readonly UiFile[];
  readonly uploads: readonly UploadTask[];
  readonly role: Role;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
}

export type FilesAction =
  | { readonly type: 'hydrate'; readonly items: readonly UiFile[] }
  | { readonly type: 'setRole'; readonly role: Role }
  | { readonly type: 'recycleLoaded'; readonly items: readonly UiFile[] }
  | { readonly type: 'uploadStart'; readonly task: UploadTask }
  | { readonly type: 'uploadProgress'; readonly id: string; readonly uploaded: number }
  | { readonly type: 'uploadInterrupted'; readonly id: string }
  | { readonly type: 'uploadDone'; readonly id: string }
  | { readonly type: 'uploadFailed'; readonly id: string; readonly code: number }
  | { readonly type: 'removeUpload'; readonly id: string }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialFilesState(): FilesViewState {
  return {
    items: [],
    recycle: [],
    uploads: [],
    role: 'GUEST',
    badge: 'synced',
    errorCode: null,
  };
}

export function canReadFiles(role: Role): boolean {
  return CAPABILITIES['read_files']?.includes(role) ?? false;
}
export function canUploadFiles(role: Role): boolean {
  return CAPABILITIES['upload_files']?.includes(role) ?? false;
}
export function canClearRecycle(role: Role): boolean {
  return CAPABILITIES['clear_recycle_bin']?.includes(role) ?? false;
}
export function canRestoreAny(role: Role): boolean {
  return CAPABILITIES['restore_any_file']?.includes(role) ?? false;
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

function badgeForFailure(code: number): BadgeState {
  const name = errorName(code);
  // UPLOAD_INTERRUPTED 可续传，不算超时错误；按 retryable 先看它。
  if (name === ERROR_CODES[3005].name) {
    return 'pending';
  }
  if (
    name === ERROR_CODES[3001].name ||
    name === ERROR_CODES[3002].name ||
    name === ERROR_CODES[3003].name
  ) {
    return 'offline';
  }
  if (name === ERROR_CODES[3004].name) {
    return 'gap';
  }
  return 'error';
}

function patchUpload(
  uploads: readonly UploadTask[],
  id: string,
  patch: (task: UploadTask) => UploadTask,
): UploadTask[] {
  return uploads.map((task) => (task.id === id ? patch(task) : task));
}

export function filesReducer(state: FilesViewState, action: FilesAction): FilesViewState {
  switch (action.type) {
    case 'hydrate':
      return { ...state, items: [...action.items], badge: 'synced', errorCode: null };
    case 'setRole':
      return { ...state, role: action.role };
    case 'recycleLoaded':
      return { ...state, recycle: [...action.items] };
    case 'uploadStart':
      return {
        ...state,
        uploads: [...state.uploads.filter((t) => t.id !== action.task.id), action.task],
        badge: 'pending',
      };
    case 'uploadProgress':
      return {
        ...state,
        uploads: patchUpload(state.uploads, action.id, (t) => ({ ...t, uploaded: action.uploaded })),
      };
    case 'uploadInterrupted':
      return {
        ...state,
        uploads: patchUpload(state.uploads, action.id, (t) => ({ ...t, status: 'interrupted' })),
        badge: 'pending',
        errorCode: ERROR_CODES[3005].code,
      };
    case 'uploadDone':
      return {
        ...state,
        uploads: patchUpload(state.uploads, action.id, (t) => ({
          ...t,
          status: 'done',
          uploaded: t.total,
        })),
        badge: 'synced',
      };
    case 'uploadFailed':
      return {
        ...state,
        uploads: patchUpload(state.uploads, action.id, (t) => ({ ...t, status: 'failed' })),
        badge: badgeForFailure(action.code),
        errorCode: action.code,
      };
    case 'removeUpload':
      return { ...state, uploads: state.uploads.filter((t) => t.id !== action.id) };
    case 'offline':
      return { ...state, badge: 'offline' };
    case 'gap':
      return { ...state, badge: 'gap' };
    case 'error':
      return { ...state, badge: badgeForFailure(action.code), errorCode: action.code };
    case 'synced':
      return { ...state, badge: 'synced', errorCode: null };
  }
}
