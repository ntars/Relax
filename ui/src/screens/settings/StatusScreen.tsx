// 服务端状态屏（C5.6）：展示 doctor 摘要 + 五态角标 + 离线横幅联动（手册 7.4 / 14.3）。
// 状态判定在 statusReducer.ts 纯函数里；组件只做 api 桥接与渲染。

import { useReducer, useCallback, useEffect } from 'react';
import { serverStatus, errorCodeOf } from '../../services/api';
import {
    serverStatusReducer, initialServerStatusState,
} from './statusReducer';

const BADGE_TEXT: Record<string, string> = {
    synced: '服务端正常',
    pending: '检查中',
    offline: '服务端离线，内容已存本地',
    gap: '服务端有缺口',
    error: '服务端错误',
};

export function StatusScreen() {
    const [state, dispatch] = useReducer(serverStatusReducer, undefined, initialServerStatusState);

    const load = useCallback(async () => {
        try {
            const info = await serverStatus();
            dispatch({ type: 'loaded', info });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            } else {
                dispatch({ type: 'offline' });
            }
        }
    }, []);

    useEffect(() => { void load(); }, [load]);

    return (
        <section className="server-status-screen">
            <header>
                <h2>服务端状态</h2>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
            </header>

            {state.badge === 'offline' && (
                <div className="banner banner-offline">服务端离线，内容已存本地</div>
            )}
            {state.badge === 'gap' && state.degraded !== null && (
                <div className="banner banner-gap">{state.degraded}</div>
            )}
            {state.errorCode !== null && (
                <div className="banner banner-error">错误码 {state.errorCode}</div>
            )}

            {state.info !== null && (
                <dl className="status-detail">
                    <dt>版本</dt><dd>{state.info.version}</dd>
                    <dt>契约版本</dt><dd>{state.info.contractVersion}</dd>
                    <dt>证书剩余天数</dt>
                    <dd>{state.info.certDaysRemaining === null ? '未配置' : state.info.certDaysRemaining}</dd>
                    <dt>降级项</dt><dd>{state.info.degradedCount}</dd>
                    <dt>自检</dt><dd>{state.info.complete ? '全绿' : '有缺口'}</dd>
                </dl>
            )}

            <button onClick={() => void load()}>重新检查</button>
        </section>
    );
}
