// 日程屏（C4.4）：日程列表 + 农历输入 + 精确闹钟授权降级提示 + 五态角标横幅。
// reducer 是纯函数（见 reducer.ts / reducer.test.ts），本组件只做 api 桥接与事件转发。

import { useReducer, useCallback, useEffect, useState } from 'react';
import {
    scheduleList,
    scheduleCreate,
    schedulePlanAlarms,
    errorCodeOf,
} from '../../services/api';
import {
    schedulesReducer, initialSchedulesState, canCreateSchedule, canReadSchedules,
    validateLunar,
} from './reducer';
import type { Role } from '../../gen/capabilities';
import type { BadgeState } from './reducer';

const BADGE_TEXT: Record<BadgeState, string> = {
    synced: '已同步',
    pending: '同步中',
    offline: '离线，稍后重试',
    gap: '数据缺口，请下拉刷新',
    error: '出错了',
};

export function SchedulesScreen({ role }: { readonly role: Role }) {
    const [state, dispatch] = useReducer(schedulesReducer, undefined, initialSchedulesState);
    const [title, setTitle] = useState('');
    const [startsAt, setStartsAt] = useState('');
    const [lunarYear, setLunarYear] = useState('');
    const [lunarMonth, setLunarMonth] = useState('');
    const [lunarDay, setLunarDay] = useState('');

    const load = useCallback(async () => {
        try {
            const data = await scheduleList();
            const items = data.items.map((o) => ({
                id: o.id,
                title: o.title,
                startsAt: o.startsAt,
                kind: o.kind,
                lunar: o.lunar,
            }));
            dispatch({ type: 'hydrate', items });
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
        void load();
    }, [load]);

    useEffect(() => {
        dispatch({ type: 'setRole', role });
    }, [role]);

    const planAlarms = useCallback(async () => {
        try {
            const result = await schedulePlanAlarms();
            dispatch({ type: 'alarmCapability', exactAllowed: result.exactAllowed });
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
        void planAlarms();
    }, [planAlarms]);

    const onAdd = useCallback(async () => {
        if (title.trim().length === 0) {
            return;
        }
        // 农历输入：三个字段都填了才校验，越界直接报 4005，不落库。
        const hasLunar =
            lunarYear.trim().length > 0 &&
            lunarMonth.trim().length > 0 &&
            lunarDay.trim().length > 0;
        if (hasLunar) {
            const ok = validateLunar(Number(lunarYear), Number(lunarMonth), Number(lunarDay));
            if (!ok) {
                dispatch({ type: 'lunarInvalid' });
                return;
            }
        }
        try {
            await scheduleCreate({
                title,
                startsAt,
                kind: 'event',
                lunar: hasLunar,
            });
            setTitle('');
            setStartsAt('');
            setLunarYear('');
            setLunarMonth('');
            setLunarDay('');
            await load();
            void planAlarms();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [title, startsAt, lunarYear, lunarMonth, lunarDay, load, planAlarms]);

    const showCreate = canCreateSchedule(state.role);
    const showList = canReadSchedules(state.role);
    const degraded = state.alarm.degraded;

    return (
        <div className="schedules-screen">
            <header className="schedules-header">
                <h1>📅 日程提醒</h1>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
                {state.badge === 'offline' || state.badge === 'gap' ? (
                    <button type="button" onClick={() => void load()}>重试</button>
                ) : null}
            </header>

            {degraded !== null ? (
                <p className="alarm-degraded" role="status">{degraded}</p>
            ) : null}

            {showCreate ? (
                <section className="schedule-form" aria-label="新增日程">
                    <input
                        aria-label="日程标题"
                        value={title}
                        onChange={(e) => setTitle(e.target.value)}
                        placeholder="日程标题"
                    />
                    <input
                        aria-label="开始时间"
                        type="datetime-local"
                        value={startsAt}
                        onChange={(e) => setStartsAt(e.target.value)}
                    />
                    <fieldset className="lunar-input">
                        <legend>农历（可选，生日/纪念日）</legend>
                        <input
                            aria-label="农历年"
                            inputMode="numeric"
                            value={lunarYear}
                            onChange={(e) => setLunarYear(e.target.value)}
                            placeholder="年 1900-2100"
                        />
                        <input
                            aria-label="农历月"
                            inputMode="numeric"
                            value={lunarMonth}
                            onChange={(e) => setLunarMonth(e.target.value)}
                            placeholder="月 1-12"
                        />
                        <input
                            aria-label="农历日"
                            inputMode="numeric"
                            value={lunarDay}
                            onChange={(e) => setLunarDay(e.target.value)}
                            placeholder="日 1-30"
                        />
                    </fieldset>
                    {state.lunarError ? (
                        <p className="lunar-error" role="alert">农历日期超出 1900–2100 或月日非法</p>
                    ) : null}
                    <button type="button" onClick={() => void onAdd()}>添加日程</button>
                </section>
            ) : null}

            {showList ? (
                <ul className="schedule-items">
                    {state.items.map((item) => (
                        <li key={item.id}>
                            <span>{item.title}</span>
                            <time>{item.startsAt}</time>
                            {item.lunar ? <span className="lunar-tag">农历</span> : null}
                        </li>
                    ))}
                </ul>
            ) : null}
        </div>
    );
}
