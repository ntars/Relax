// 离线横幅（手册 7.4）：顶部红条「服务端离线，内容已存本地」+ 重试动作按钮。
// 状态来自 state/status 的 statusReducer；离线/offline 由网络/TLS/契约失败触发。

import { badgeText } from '../state/status';
import type { StatusAction, StatusState } from '../state/status';

export function Banner({
    state, dispatch,
}: {
    readonly state: StatusState;
    readonly dispatch: (action: StatusAction) => void;
}) {
    if (state.badge !== 'offline') {
        return null;
    }
    return (
        <div className="offline-banner" role="alert">
            <span className="banner-text">{badgeText(state)}</span>
            <button type="button" onClick={() => dispatch({ type: 'retry' })}>重试</button>
        </div>
    );
}
