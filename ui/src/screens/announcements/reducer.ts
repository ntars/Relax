// 公告屏纯函数 reducer（手册 C4.1）：未读角标 + 附件下载状态 + 五态角标（7.4）。
// 只按 error.code 对应的契约 name 分支（手册 3.2），不读 message，不写三位数字字面量比较。

import { ERROR_CODES } from '../../gen/error-codes';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

/** 附件下载子态：idle → downloading → done / failed。 */
export type DownloadState = 'idle' | 'downloading' | 'done' | 'failed';

export interface UiAnnouncement {
  readonly id: string;
  readonly title: string;
  readonly body: string;
  readonly pinned: boolean;
  readonly read: boolean;
  readonly attachmentIds: readonly string[];
}

export interface AnnouncementViewState {
  readonly items: readonly UiAnnouncement[];
  readonly unreadCount: number;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
  /** 当前附件下载：attachmentId → 子态。 */
  readonly downloads: Readonly<Record<string, DownloadState>>;
}

export type AnnouncementAction =
  | { readonly type: 'hydrate'; readonly items: readonly UiAnnouncement[] }
  | { readonly type: 'markRead'; readonly id: string }
  | { readonly type: 'downloadStart'; readonly attachmentId: string }
  | { readonly type: 'downloadDone'; readonly attachmentId: string }
  | { readonly type: 'downloadFailed'; readonly attachmentId: string; readonly code: number }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialAnnouncementState(): AnnouncementViewState {
  return { items: [], unreadCount: 0, badge: 'synced', errorCode: null, downloads: {} };
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

/** 网络/TLS/契约 → offline 横幅；游标过期 → 有缺口；其余 → 错误态。 */
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

function countUnread(items: readonly UiAnnouncement[]): number {
  return items.reduce((n, a) => n + (a.read ? 0 : 1), 0);
}

export function announcementReducer(
  state: AnnouncementViewState,
  action: AnnouncementAction,
): AnnouncementViewState {
  switch (action.type) {
    case 'hydrate':
      return {
        items: [...action.items],
        unreadCount: countUnread(action.items),
        badge: 'synced',
        errorCode: null,
        downloads: {},
      };
    case 'markRead': {
      const items = state.items.map((a) => (a.id === action.id ? { ...a, read: true } : a));
      return { ...state, items, unreadCount: countUnread(items) };
    }
    case 'downloadStart':
      return {
        ...state,
        downloads: { ...state.downloads, [action.attachmentId]: 'downloading' },
        badge: 'pending',
        errorCode: null,
      };
    case 'downloadDone': {
      const downloads = { ...state.downloads, [action.attachmentId]: 'done' as DownloadState };
      const hasDownloading = Object.values(downloads).some((d) => d === 'downloading');
      return { ...state, downloads, badge: hasDownloading ? 'pending' : 'synced' };
    }
    case 'downloadFailed':
      return {
        ...state,
        downloads: { ...state.downloads, [action.attachmentId]: 'failed' },
        badge: badgeForFailure(action.code),
        errorCode: action.code,
      };
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
