// 聊天屏纯函数 reducer（手册 7.4 五态角标 / 8.4 每屏一个 useReducer）。
// 离线发送：写本地 outbox 后乐观入列（pending 态）；恢复同步后转 synced。
// 只按 error.code 对应的契约 name 分支，不读 message（手册 3.2 硬规则），
// 也不写三位数字字面量比较（check-ui 门禁）。

import { ERROR_CODES } from '../../gen/error-codes';

/** 手册 7.4 五态：已同步/待发送/离线/有缺口/错误。 */
export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface UiMessage {
  readonly id: string;
  readonly content: string;
  readonly pending: boolean;
}

export interface ChatViewState {
  readonly messages: readonly UiMessage[];
  readonly pendingCount: number;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
}

export type ChatAction =
  | { readonly type: 'enqueue'; readonly id: string; readonly content: string }
  | { readonly type: 'sent'; readonly id: string }
  | { readonly type: 'failed'; readonly id: string; readonly code: number }
  | { readonly type: 'hydrate'; readonly messages: readonly UiMessage[]; readonly pendingCount: number }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialChatState(): ChatViewState {
  return { messages: [], pendingCount: 0, badge: 'synced', errorCode: null };
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

/** 离线/契约/不可达类错误 → 离线横幅；游标过期 → 有缺口；其余 → 错误态。 */
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

export function chatReducer(state: ChatViewState, action: ChatAction): ChatViewState {
  switch (action.type) {
    case 'enqueue':
      return {
        messages: [...state.messages, { id: action.id, content: action.content, pending: true }],
        pendingCount: state.pendingCount + 1,
        badge: 'pending',
        errorCode: null,
      };
    case 'sent': {
      const messages = state.messages.map((m) =>
        m.id === action.id ? { ...m, pending: false } : m,
      );
      const pendingCount = Math.max(0, state.pendingCount - 1);
      return {
        messages,
        pendingCount,
        badge: pendingCount > 0 ? 'pending' : 'synced',
        errorCode: null,
      };
    }
    case 'failed':
      return {
        ...state,
        badge: badgeForFailure(action.code),
        errorCode: action.code,
      };
    case 'hydrate':
      return {
        messages: [...action.messages],
        pendingCount: action.pendingCount,
        badge: action.pendingCount > 0 ? 'pending' : 'synced',
        errorCode: null,
      };
    case 'offline':
      return { ...state, badge: 'offline' };
    case 'gap':
      return { ...state, badge: 'gap' };
    case 'error':
      return { ...state, badge: 'error', errorCode: action.code };
    case 'synced':
      return { ...state, badge: 'synced', errorCode: null };
  }
}
