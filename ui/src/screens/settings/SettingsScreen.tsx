// 设置屏（C5.5）：回收站（OWNER 清空/恢复）+ 审计只读页（OWNER）+ 能力控制 + 五态角标横幅。
// reducer 是纯函数（见 reducer.ts / reducer.test.ts），本组件只做 api 桥接与事件转发。
// 客户端隐藏按钮只是体验——真正的 ACL 在服务端中间件判定。

import { useReducer, useCallback, useEffect } from 'react';
import {
    recycleList, recycleRestore, recycleClear, auditList, errorCodeOf,
} from '../../services/api';
import {
    settingsReducer, initialSettingsState, canClearRecycleBin, canRestoreAnyFile,
    canReadAuditLog,
} from './reducer';
import type { Role } from '../../gen/capabilities';
import type { BadgeState, UiRecycleItem, UiAuditEntry } from './reducer';

const BADGE_TEXT: Record<BadgeState, string> = {
    synced: '已同步',
    pending: '同步中',
    offline: '服务端离线，内容已存本地',
    gap: '有缺口，请查看 degraded',
    error: '出错了',
};

export function SettingsScreen({ role }: { readonly role: Role }) {
    const [state, dispatch] = useReducer(settingsReducer, undefined, initialSettingsState);

    const load = useCallback(async () => {
        try {
            const recycle = await recycleList();
            const items: UiRecycleItem[] = recycle.items.map((it) => ({
                id: it.id, name: it.name, recycledAt: it.recycledAt,
            }));
            dispatch({ type: 'recycleLoaded', items });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            } else {
                dispatch({ type: 'offline' });
            }
        }
    }, []);

    const loadAudit = useCallback(async () => {
        try {
            const data = await auditList();
            const entries: UiAuditEntry[] = data.entries.map((e) => ({
                id: e.id, actorId: e.actorId, action: e.action, targetType: e.targetType,
                targetId: e.targetId, createdAt: e.createdAt,
            }));
            dispatch({ type: 'auditLoaded', entries });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            } else {
                dispatch({ type: 'offline' });
            }
        }
    }, []);

    useEffect(() => {
        dispatch({ type: 'setRole', role });
        if (canRestoreAnyFile(role) || canClearRecycleBin(role)) {
            void load();
        }
        if (canReadAuditLog(role)) {
            void loadAudit();
        }
    }, [role, load, loadAudit]);

    const onRestore = useCallback(async (id: string) => {
        if (!canRestoreAnyFile(role)) return;
        try {
            await recycleRestore(id);
            dispatch({ type: 'restored', id });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) dispatch({ type: 'error', code });
        }
    }, [role]);

    const onClear = useCallback(async () => {
        if (!canClearRecycleBin(role)) return;
        try {
            await recycleClear();
            dispatch({ type: 'cleared' });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) dispatch({ type: 'error', code });
        }
    }, [role]);

    const showRecycle = canRestoreAnyFile(role) || canClearRecycleBin(role);
    const showAudit = canReadAuditLog(role);

    return (
        <section className="settings-screen">
            <header>
                <h2>设置</h2>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
            </header>

            {state.badge === 'offline' && (
                <div className="banner banner-offline">服务端离线，内容已存本地</div>
            )}
            {state.badge === 'gap' && (
                <div className="banner banner-gap">有缺口，请查看 degraded</div>
            )}
            {state.errorCode !== null && (
                <div className="banner banner-error">错误码 {state.errorCode}</div>
            )}
            {state.lastAction !== null && <p className="hint">{state.lastAction}</p>}

            {showRecycle && (
                <section className="recycle-bin">
                    <h3>回收站（保留 30 天）</h3>
                    <ul>
                        {state.recycle.map((it) => (
                            <li key={it.id}>
                                <span>{it.name}</span>
                                <time>{it.recycledAt}</time>
                                {canRestoreAnyFile(role) && (
                                    <button onClick={() => void onRestore(it.id)}>恢复</button>
                                )}
                            </li>
                        ))}
                    </ul>
                    {canClearRecycleBin(role) && state.recycle.length > 0 && (
                        <button onClick={() => void onClear()}>清空回收站</button>
                    )}
                </section>
            )}

            {showAudit && (
                <section className="audit-log">
                    <h3>审计日志（只读，保留 180 天）</h3>
                    <table>
                        <thead>
                            <tr><th>时间</th><th>操作者</th><th>动作</th><th>目标</th></tr>
                        </thead>
                        <tbody>
                            {state.audit.map((e) => (
                                <tr key={e.id}>
                                    <td>{e.createdAt}</td>
                                    <td>{e.actorId}</td>
                                    <td>{e.action}</td>
                                    <td>{e.targetType}/{e.targetId}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </section>
            )}
        </section>
    );
}
