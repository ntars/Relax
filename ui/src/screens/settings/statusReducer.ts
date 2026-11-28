// 服务端状态屏 reducer（C5.6，手册 7.4 五态 / 14.3 doctor 12 项）。
// doctor 的 ok/complete/degraded 映射到五态角标：
// - ok=false / 网络失败 → offline（横幅联动）
// - ok=true 且 complete=false → gap（有缺口，degraded 可点开）
// - ok=true 且 complete=true → synced

import { ERROR_CODES } from '../../gen/error-codes';
import type { BadgeState } from '../../state/status';

export interface ServerStatusInfo {
    readonly ok: boolean;
    readonly complete: boolean;
    readonly degradedCount: number;
    readonly version: string;
    readonly contractVersion: string;
    readonly certDaysRemaining: number | null;
}

export interface ServerStatusState {
    readonly badge: BadgeState;
    readonly info: ServerStatusInfo | null;
    readonly degraded: string | null;
    readonly errorCode: number | null;
}

export type ServerStatusAction =
    | { readonly type: 'loaded'; readonly info: ServerStatusInfo }
    | { readonly type: 'offline' }
    | { readonly type: 'gap'; readonly degraded: string }
    | { readonly type: 'error'; readonly code: number };

export function initialServerStatusState(): ServerStatusState {
    return { badge: 'synced', info: null, degraded: null, errorCode: null };
}

/** 网络/证书类错误码 → 离线（与统一状态层 badgeForFailure 一致）。 */
function isOfflineCode(code: number): boolean {
    const name = ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
    return (
        name === ERROR_CODES[3001].name ||
        name === ERROR_CODES[3002].name ||
        name === ERROR_CODES[3003].name
    );
}

export function serverStatusReducer(
    state: ServerStatusState,
    action: ServerStatusAction,
): ServerStatusState {
    switch (action.type) {
        case 'loaded': {
            const { info } = action;
            if (!info.ok) {
                return { ...state, badge: 'offline', info, errorCode: null };
            }
            if (!info.complete) {
                return {
                    ...state,
                    badge: 'gap',
                    info,
                    degraded: `${info.degradedCount} 项降级，运行 relax doctor 查看明细`,
                };
            }
            return { ...state, badge: 'synced', info, degraded: null, errorCode: null };
        }
        case 'offline':
            return { ...state, badge: 'offline', errorCode: null };
        case 'gap':
            return { ...state, badge: 'gap', degraded: action.degraded };
        case 'error':
            return {
                ...state,
                badge: isOfflineCode(action.code) ? 'offline' : 'error',
                errorCode: action.code,
            };
        default:
            return state;
    }
}
