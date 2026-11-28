// 作息屏（C4.3）：家庭/个人切换 + 覆盖优先级 + 能力控制 + 五态角标横幅。
// reducer 是纯函数（见 reducer.ts / reducer.test.ts），本组件只做 api 桥接与事件转发。

import { useReducer, useCallback, useEffect, useState } from 'react';
import {
    routineList,
    routineCreate,
    errorCodeOf,
} from '../../services/api';
import {
    routinesReducer, initialRoutinesState, canReadRoutines, canManageFamily,
    canEditPersonal, visibleItems,
} from './reducer';
import type { Role } from '../../gen/capabilities';

const BADGE_TEXT = {
    synced: '已同步',
    pending: '同步中',
    offline: '离线，稍后重试',
    gap: '数据缺口，请下拉刷新',
    error: '出错了',
} as const;

export function RoutinesScreen({ role, selfId }: { readonly role: Role; readonly selfId: string }) {
    const [state, dispatch] = useReducer(routinesReducer, undefined, initialRoutinesState);
    const [title, setTitle] = useState('');
    const [kind, setKind] = useState('wake');

    const load = useCallback(async () => {
        try {
            const data = await routineList();
            dispatch({ type: 'hydrate', family: data.family, personal: data.personal });
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
    }, [role]);

    useEffect(() => {
        void load();
    }, [load]);

    const onCreate = useCallback(async () => {
        if (title.trim().length === 0) {
            return;
        }
        const isFamily = state.scope === 'family';
        try {
            await routineCreate({
                memberId: isFamily ? null : selfId,
                title,
                kind,
                daysMask: 127,
                startMinute: 420,
                endMinute: 430,
            });
            setTitle('');
            await load();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [title, kind, state.scope, selfId, load]);

    const canCreate = state.scope === 'family'
        ? canManageFamily(role)
        : canEditPersonal(role);
    const readable = canReadRoutines(role);
    const items = visibleItems(state);

    if (!readable) {
        return <div className="routines-screen"><p>无作息读取权限</p></div>;
    }

    return (
        <div className="routines-screen">
            <header className="routines-header">
                <h1>⏰ 作息安排</h1>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
                <div className="scope-switch" role="tablist" aria-label="作息范围">
                    <button
                        type="button"
                        role="tab"
                        aria-selected={state.scope === 'family'}
                        onClick={() => dispatch({ type: 'setScope', scope: 'family' })}
                    >
                        家庭
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected={state.scope === 'personal'}
                        onClick={() => dispatch({ type: 'setScope', scope: 'personal' })}
                    >
                        个人
                    </button>
                </div>
            </header>

            {canCreate ? (
                <section className="routine-create">
                    <input
                        aria-label="作息标题"
                        value={title}
                        onChange={(e) => setTitle(e.target.value)}
                        placeholder="作息标题"
                    />
                    <input
                        aria-label="作息类型"
                        value={kind}
                        onChange={(e) => setKind(e.target.value)}
                        placeholder="类型（wake/sleep/...）"
                    />
                    <button type="button" onClick={() => void onCreate()}>添加</button>
                </section>
            ) : null}

            <ul className="routine-items">
                {items.map((item) => (
                    <li key={item.id} className={item.active ? 'routine-active' : 'routine-inactive'}>
                        <span>{item.title}</span>
                        {item.overridesFamily ? <span className="override-tag">覆盖家庭</span> : null}
                    </li>
                ))}
            </ul>
        </div>
    );
}
