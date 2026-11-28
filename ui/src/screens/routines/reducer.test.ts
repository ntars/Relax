import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import {
    routinesReducer, initialRoutinesState, canReadRoutines, canManageFamily,
    canEditPersonal, visibleItems,
} from './reducer';
import type { UiRoutine } from './reducer';

const familyItem: UiRoutine = {
    id: 'f1', title: '起床', kind: 'wake', daysMask: 127, startMinute: 420,
    endMinute: 430, active: true, isFamily: true, overridesFamily: false,
};
const personalItem: UiRoutine = {
    id: 'p1', title: '早起', kind: 'wake', daysMask: 31, startMinute: 400,
    endMinute: 410, active: true, isFamily: false, overridesFamily: true,
};

describe('routinesReducer 家庭/个人切换', () => {
    it('hydrate 载入家庭与个人两套', () => {
        const s = routinesReducer(initialRoutinesState(), {
            type: 'hydrate', family: [familyItem], personal: [personalItem],
        });
        expect(visibleItems(s)).toEqual([familyItem]);
        const p = routinesReducer(s, { type: 'setScope', scope: 'personal' });
        expect(visibleItems(p)).toEqual([personalItem]);
    });

    it('upsert 家庭项只进家庭、个人项只进个人', () => {
        let s = routinesReducer(initialRoutinesState(), {
            type: 'upsert', item: familyItem,
        });
        expect(s.familyItems).toHaveLength(1);
        expect(s.personalItems).toHaveLength(0);
        s = routinesReducer(s, { type: 'upsert', item: personalItem });
        expect(s.familyItems).toHaveLength(1);
        expect(s.personalItems).toHaveLength(1);
    });

    it('upsert 同 id 覆盖而非追加', () => {
        let s = routinesReducer(initialRoutinesState(), { type: 'upsert', item: familyItem });
        s = routinesReducer(s, {
            type: 'upsert', item: { ...familyItem, title: '改标题' },
        });
        expect(s.familyItems).toHaveLength(1);
        expect(s.familyItems[0]?.title).toBe('改标题');
    });
});

describe('routinesReducer 能力控制', () => {
    it('GUEST 无读权限；家庭管理仅 OWNER/ADMIN', () => {
        expect(canReadRoutines('GUEST')).toBe(false);
        expect(canReadRoutines('MEMBER')).toBe(true);
        expect(canManageFamily('OWNER')).toBe(true);
        expect(canManageFamily('ADMIN')).toBe(true);
        expect(canManageFamily('MEMBER')).toBe(false);
    });

    it('个人作息编辑 OWNER/ADMIN/MEMBER/CHILD 可见，GUEST 不可见', () => {
        expect(canEditPersonal('CHILD')).toBe(true);
        expect(canEditPersonal('GUEST')).toBe(false);
    });
});

describe('routinesReducer 五态角标', () => {
    it('网络/TLS/契约失败 → offline', () => {
        for (const code of [3001, 3002, 3003]) {
            const s = routinesReducer(initialRoutinesState(), {
                type: 'error', code: ERROR_CODES[code as keyof typeof ERROR_CODES].code,
            });
            expect(s.badge).toBe('offline');
        }
    });

    it('游标过期 → gap；其余错误 → error 态带码', () => {
        expect(
            routinesReducer(initialRoutinesState(), {
                type: 'error', code: ERROR_CODES[3004].code,
            }).badge,
        ).toBe('gap');
        const s = routinesReducer(initialRoutinesState(), {
            type: 'error', code: ERROR_CODES[1006].code,
        });
        expect(s.badge).toBe('error');
        expect(s.errorCode).toBe(1006);
    });

    it('offline/synced 动作直接切角标', () => {
        const off = routinesReducer(initialRoutinesState(), { type: 'offline' });
        expect(off.badge).toBe('offline');
        expect(routinesReducer(off, { type: 'synced' }).badge).toBe('synced');
    });
});
