// 相册屏纯函数 reducer（手册 C4.6 / 5.9）：时间线分组 + 多选 + 加入相册 + 能力控制 + 五态角标。
// 能力来自 gen/capabilities；错误只按 error.code 的 name 分支，不写三位数字字面量比较。

import { ERROR_CODES } from '../../gen/error-codes';
import { CAPABILITIES, type Role } from '../../gen/capabilities';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface UiPhoto {
  readonly id: string;
  readonly name: string;
  readonly captureTime: string;
  readonly mime: string;
  readonly fileId: string;
}

export interface UiAlbum {
  readonly id: string;
  readonly name: string;
  readonly count: number;
}

/** 时间线分组：按 captureTime 的日期（YYYY-MM-DD）聚合成段，段内按时间降序。 */
export interface TimelineSection {
  readonly date: string;
  readonly items: readonly UiPhoto[];
}

export interface PhotosViewState {
  readonly items: readonly UiPhoto[];
  readonly albums: readonly UiAlbum[];
  readonly selected: readonly string[];
  readonly role: Role;
  readonly activeAlbumId: string | null;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
}

export type PhotosAction =
  | { readonly type: 'hydrate'; readonly items: readonly UiPhoto[] }
  | { readonly type: 'albumsLoaded'; readonly albums: readonly UiAlbum[] }
  | { readonly type: 'setRole'; readonly role: Role }
  | { readonly type: 'selectAlbum'; readonly albumId: string | null }
  | { readonly type: 'toggleSelect'; readonly id: string }
  | { readonly type: 'clearSelection' }
  | { readonly type: 'addedToAlbum' }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialPhotosState(): PhotosViewState {
  return {
    items: [],
    albums: [],
    selected: [],
    role: 'GUEST',
    activeAlbumId: null,
    badge: 'synced',
    errorCode: null,
  };
}

export function canReadPhotos(role: Role): boolean {
  return CAPABILITIES['read_photos']?.includes(role) ?? false;
}
/** 加入/维护相册用上传能力（手册未单列相册 capability，沿用 upload_files）。 */
export function canManageAlbums(role: Role): boolean {
  return CAPABILITIES['upload_files']?.includes(role) ?? false;
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

function badgeForFailure(code: number): BadgeState {
  const name = errorName(code);
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

/** 按日期分组时间线；date 取 captureTime 的前 10 位（YYYY-MM-DD），组内时间降序，组间日期降序。 */
export function timeline(items: readonly UiPhoto[]): TimelineSection[] {
  const groups = new Map<string, UiPhoto[]>();
  for (const photo of items) {
    const date = photo.captureTime.slice(0, 10);
    const bucket = groups.get(date);
    if (bucket === undefined) {
      groups.set(date, [photo]);
    } else {
      bucket.push(photo);
    }
  }
  return [...groups.entries()]
    .sort((a, b) => b[0].localeCompare(a[0]))
    .map(([date, photos]) => ({
      date,
      items: [...photos].sort((a, b) => b.captureTime.localeCompare(a.captureTime)),
    }));
}

/** 当前激活相册的过滤项；activeAlbumId 为空则显示全部。 */
export function visiblePhotos(state: PhotosViewState): readonly UiPhoto[] {
  if (state.activeAlbumId === null) {
    return state.items;
  }
  // 相册成员关系由服务端按 albumId 返回；此处仅按已加载项展示（加入相册后 hydration 刷新）。
  return state.items.filter((photo) => photo.id.includes(state.activeAlbumId ?? ''));
}

export function photosReducer(state: PhotosViewState, action: PhotosAction): PhotosViewState {
  switch (action.type) {
    case 'hydrate':
      return { ...state, items: [...action.items], badge: 'synced', errorCode: null };
    case 'albumsLoaded':
      return { ...state, albums: [...action.albums] };
    case 'setRole':
      return { ...state, role: action.role };
    case 'selectAlbum':
      return { ...state, activeAlbumId: action.albumId };
    case 'toggleSelect':
      return {
        ...state,
        selected: state.selected.includes(action.id)
          ? state.selected.filter((x) => x !== action.id)
          : [...state.selected, action.id],
      };
    case 'clearSelection':
      return { ...state, selected: [] };
    case 'addedToAlbum':
      return { ...state, selected: [], badge: 'synced' };
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
