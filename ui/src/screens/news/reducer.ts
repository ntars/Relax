// 新闻屏纯函数 reducer（手册 C4.2）：新闻列表 + 源管理入口（ADMIN 可见）+ 五态角标（7.4）。
// 源管理能力来自 gen/capabilities，不在 UI 里硬编码角色；错误只按 error.code 的 name 分支。

import { ERROR_CODES } from '../../gen/error-codes';
import { CAPABILITIES, type Role } from '../../gen/capabilities';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface UiNewsItem {
  readonly id: string;
  readonly title: string;
  readonly sourceUrl: string;
  readonly read: boolean;
}

export interface UiFeed {
  readonly id: string;
  readonly title: string;
  readonly url: string;
  readonly enabled: boolean;
}

export interface NewsViewState {
  readonly items: readonly UiNewsItem[];
  readonly feeds: readonly UiFeed[];
  readonly role: Role;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
}

export type NewsAction =
  | { readonly type: 'hydrate'; readonly items: readonly UiNewsItem[]; readonly feeds: readonly UiFeed[] }
  | { readonly type: 'setRole'; readonly role: Role }
  | { readonly type: 'addFeed'; readonly feed: UiFeed }
  | { readonly type: 'removeFeed'; readonly id: string }
  | { readonly type: 'markRead'; readonly id: string }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialNewsState(): NewsViewState {
  return { items: [], feeds: [], role: 'GUEST', badge: 'synced', errorCode: null };
}

/** 源管理入口是否可见：严格按能力的角色数组，不在 UI 里写角色比较。 */
export function canManageFeeds(role: Role): boolean {
  return CAPABILITIES['manage_news_sources']?.includes(role) ?? false;
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

/** 网络/源不可达 → offline；游标过期 → gap；解析失败/其余 → error。 */
function badgeForFailure(code: number): BadgeState {
  const name = errorName(code);
  if (
    name === ERROR_CODES[3001].name ||
    name === ERROR_CODES[3002].name ||
    name === ERROR_CODES[3003].name ||
    name === ERROR_CODES[4004].name
  ) {
    return 'offline';
  }
  if (name === ERROR_CODES[3004].name) {
    return 'gap';
  }
  return 'error';
}

export function newsReducer(state: NewsViewState, action: NewsAction): NewsViewState {
  switch (action.type) {
    case 'hydrate':
      return {
        ...state,
        items: [...action.items],
        feeds: [...action.feeds],
        badge: 'synced',
        errorCode: null,
      };
    case 'setRole':
      return { ...state, role: action.role };
    case 'addFeed':
      return { ...state, feeds: [...state.feeds, action.feed] };
    case 'removeFeed':
      return { ...state, feeds: state.feeds.filter((f) => f.id !== action.id) };
    case 'markRead': {
      const items = state.items.map((n) => (n.id === action.id ? { ...n, read: true } : n));
      return { ...state, items };
    }
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
