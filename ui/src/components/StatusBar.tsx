// 五态角标条（手册 7.4）：灰云勾/橙时钟+条数/离线红条/橙感叹号/红错误码+动作按钮。
// 纯展示组件，所有状态来自 state/status 的 statusReducer，不自己判定五态。

import {
    badgeText, errorAction,
} from '../state/status';
import type { StatusAction, StatusState } from '../state/status';

export function StatusBar({
    state, dispatch,
}: {
    readonly state: StatusState;
    readonly dispatch: (action: StatusAction) => void;
}) {
    const action = errorAction(state.errorCode);
    const label = state.badge === 'pending'
        ? `${badgeText(state)} ${state.pendingCount}`
        : badgeText(state);

    return (
        <div className={`status-bar badge-${state.badge}`} role="status" aria-live="polite">
            <span className="status-badge" data-badge={state.badge}>{label}</span>
            {state.badge === 'gap' && state.degraded !== null ? (
                <details className="gap-detail">
                    <summary>查看缺口</summary>
                    <span>{state.degraded}</span>
                </details>
            ) : null}
            {state.badge === 'error' && action !== null ? (
                <span className="error-actions">
                    <span className="error-code">{state.errorCode}</span>
                    <button type="button" onClick={() => dispatch({ type: 'retry' })}>
                        {action.label}
                    </button>
                </span>
            ) : null}
        </div>
    );
}
