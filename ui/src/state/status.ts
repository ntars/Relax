// 五态统一状态层（手册 7.4）：把散落在各屏的 badgeForFailure / 文案 / 动作收敛到一处。
// 五态：synced 已同步 / pending 待发送 / offline 离线 / gap 有缺口 / error 错误。
// 错误只按 ERROR_CODES[code].name 分支，不写三位数字字面量比较（手册 3.2）。

import { ERROR_CODES } from '../gen/error-codes';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface StatusState {
    readonly badge: BadgeState;
    /** 待发送条数（outbox 非空时为 pending 角标数字）。 */
    readonly pendingCount: number;
    /** 有缺口时的 degraded 详情，可点开查看。 */
    readonly degraded: string | null;
    /** 当前错误码；用于渲染红色错误码 + 动作按钮。 */
    readonly errorCode: number | null;
}

export type StatusAction =
    | { readonly type: 'outboxChanged'; readonly count: number }
    | { readonly type: 'pullSuccess' }
    | { readonly type: 'offline' }
    | { readonly type: 'gap'; readonly degraded: string }
    | { readonly type: 'error'; readonly code: number }
    | { readonly type: 'retry' };

export function initialStatusState(): StatusState {
    return { badge: 'synced', pendingCount: 0, degraded: null, errorCode: null };
}

function errorName(code: number): string | undefined {
    return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

/** 统一五态分类：网络/TLS/契约→offline，游标过期→gap，其余→error。 */
export function badgeForFailure(code: number): BadgeState {
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

/** 五态文案（手册 7.4）。pending 文案由组件追加条数。 */
export function badgeText(state: StatusState): string {
    const base: Record<BadgeState, string> = {
        synced: '已同步',
        pending: '待发送',
        offline: '服务端离线，内容已存本地',
        gap: '有缺口',
        error: '出错了',
    };
    return base[state.badge];
}

/** 错误码动作按钮文案：来自 ERROR_CODES[code].action（手册 7.4）。 */
export function errorAction(code: number | null): { label: string; retryable: boolean } | null {
    if (code === null) {
        return null;
    }
    const entry = ERROR_CODES[code as keyof typeof ERROR_CODES];
    if (entry === undefined) {
        return null;
    }
    return { label: entry.action, retryable: entry.retryable };
}

export function statusReducer(state: StatusState, action: StatusAction): StatusState {
    switch (action.type) {
        case 'outboxChanged':
            if (action.count > 0) {
                return { ...state, badge: 'pending', pendingCount: action.count, errorCode: null };
            }
            return state.badge === 'pending'
                ? { ...state, badge: 'synced', pendingCount: 0 }
                : { ...state, pendingCount: 0 };
        case 'pullSuccess':
            return state.badge === 'pending'
                ? state
                : { ...state, badge: 'synced', errorCode: null, degraded: null };
        case 'offline':
            return { ...state, badge: 'offline' };
        case 'gap':
            return { ...state, badge: 'gap', degraded: action.degraded };
        case 'error':
            return {
                ...state,
                badge: badgeForFailure(action.code),
                errorCode: action.code,
            };
        case 'retry':
            return { ...state, badge: 'synced', errorCode: null, degraded: null };
    }
}
