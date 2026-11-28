import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import {
    schedulesReducer, initialSchedulesState, canCreateSchedule, canReadSchedules,
    resolveAlarm, validateLunar,
} from './reducer';
import type { UiOccurrence } from './reducer';

const item: UiOccurrence = {
    id: 's1', title: '周会', startsAt: '2026-01-02T08:00:00Z', kind: 'event', lunar: false,
};

describe('schedulesReducer 能力控制', () => {
    it('create_schedules 仅 OWNER/ADMIN/MEMBER', () => {
        expect(canCreateSchedule('OWNER')).toBe(true);
        expect(canCreateSchedule('ADMIN')).toBe(true);
        expect(canCreateSchedule('MEMBER')).toBe(true);
        expect(canCreateSchedule('CHILD')).toBe(false);
        expect(canCreateSchedule('GUEST')).toBe(false);
    });

    it('read_schedules 含 CHILD，GUEST 无读权限', () => {
        expect(canReadSchedules('CHILD')).toBe(true);
        expect(canReadSchedules('GUEST')).toBe(false);
    });
});

describe('resolveAlarm 精确/降级', () => {
    it('授权精确则不降级', () => {
        const cap = resolveAlarm(true);
        expect(cap.exactAllowed).toBe(true);
        expect(cap.mode).toBe('exact');
        expect(cap.degraded).toBeNull();
    });

    it('拒绝精确则降级并记录 degraded', () => {
        const cap = resolveAlarm(false);
        expect(cap.exactAllowed).toBe(false);
        expect(cap.mode).toBe('setAndAllowWhileIdle');
        expect(cap.degraded).toContain('EXACT_ALARM_DENIED');
        expect(cap.degraded).toContain('15 分钟');
    });

    it('alarmCapability action 更新状态', () => {
        const s = schedulesReducer(initialSchedulesState(), {
            type: 'alarmCapability', exactAllowed: true,
        });
        expect(s.alarm.mode).toBe('exact');
    });
});

describe('validateLunar 农历输入', () => {
    it('1900-2100 年月日在范围内通过', () => {
        expect(validateLunar(1900, 1, 1)).toBe(true);
        expect(validateLunar(2100, 12, 30)).toBe(true);
        expect(validateLunar(2026, 6, 15)).toBe(true);
    });

    it('越界年份/月份/日期与非整数被拒', () => {
        expect(validateLunar(1899, 1, 1)).toBe(false);
        expect(validateLunar(2101, 1, 1)).toBe(false);
        expect(validateLunar(2026, 0, 1)).toBe(false);
        expect(validateLunar(2026, 13, 1)).toBe(false);
        expect(validateLunar(2026, 1, 31)).toBe(false);
        expect(validateLunar(2026.5, 1, 1)).toBe(false);
    });

    it('lunarInvalid action 置 4005 与 error 角标', () => {
        const s = schedulesReducer(initialSchedulesState(), { type: 'lunarInvalid' });
        expect(s.lunarError).toBe(true);
        expect(s.errorCode).toBe(ERROR_CODES[4005].code);
        expect(s.badge).toBe('error');
    });
});

describe('schedulesReducer 五态角标', () => {
    it('hydrate 载入实例并回到 synced', () => {
        const s = schedulesReducer(initialSchedulesState(), { type: 'hydrate', items: [item] });
        expect(s.items).toEqual([item]);
        expect(s.badge).toBe('synced');
    });

    it('网络类错误 → offline（按 name，不比数字）', () => {
        for (const code of [3001, 3002, 3003]) {
            const s = schedulesReducer(initialSchedulesState(), { type: 'error', code });
            expect(s.badge).toBe('offline');
        }
    });

    it('游标过期 → gap', () => {
        const s = schedulesReducer(initialSchedulesState(), {
            type: 'error', code: ERROR_CODES[3004].code,
        });
        expect(s.badge).toBe('gap');
    });

    it('农历越界与其他业务错误 → error', () => {
        const lunar = schedulesReducer(initialSchedulesState(), {
            type: 'error', code: ERROR_CODES[4005].code,
        });
        expect(lunar.badge).toBe('error');
        expect(lunar.lunarError).toBe(true);
        const other = schedulesReducer(initialSchedulesState(), {
            type: 'error', code: ERROR_CODES[5005].code,
        });
        expect(other.badge).toBe('error');
        expect(other.lunarError).toBe(false);
    });

    it('offline/gap/synced 动作切换角标', () => {
        let s = schedulesReducer(initialSchedulesState(), { type: 'offline' });
        expect(s.badge).toBe('offline');
        s = schedulesReducer(s, { type: 'gap' });
        expect(s.badge).toBe('gap');
        s = schedulesReducer(s, { type: 'synced' });
        expect(s.badge).toBe('synced');
        expect(s.errorCode).toBeNull();
    });
});
