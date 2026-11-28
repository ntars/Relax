import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../gen/error-codes';
import {
    statusReducer, initialStatusState, badgeForFailure, badgeText, errorAction,
} from './status';
import type { StatusState } from './status';

describe('五态分类 badgeForFailure（按 name 分支）', () => {
    it('网络/TLS/契约失败 → offline', () => {
        expect(badgeForFailure(ERROR_CODES[3001].code)).toBe('offline');
        expect(badgeForFailure(ERROR_CODES[3002].code)).toBe('offline');
        expect(badgeForFailure(ERROR_CODES[3003].code)).toBe('offline');
    });

    it('游标过期 → gap，其余 → error', () => {
        expect(badgeForFailure(ERROR_CODES[3004].code)).toBe('gap');
        expect(badgeForFailure(ERROR_CODES[5001].code)).toBe('error');
        expect(badgeForFailure(ERROR_CODES[4005].code)).toBe('error');
    });
});

describe('statusReducer 五态', () => {
    it('初始 = synced', () => {
        expect(initialStatusState().badge).toBe('synced');
    });

    it('outbox 非空 → pending + 条数，清零 → synced', () => {
        let s = statusReducer(initialStatusState(), { type: 'outboxChanged', count: 3 });
        expect(s.badge).toBe('pending');
        expect(s.pendingCount).toBe(3);
        s = statusReducer(s, { type: 'outboxChanged', count: 0 });
        expect(s.badge).toBe('synced');
        expect(s.pendingCount).toBe(0);
    });

    it('pending 时 pullSuccess 不覆盖角标', () => {
        const pending = statusReducer(initialStatusState(), { type: 'outboxChanged', count: 2 });
        expect(statusReducer(pending, { type: 'pullSuccess' }).badge).toBe('pending');
    });

    it('离线上报 → offline，文案含已存本地', () => {
        const s = statusReducer(initialStatusState(), { type: 'offline' });
        expect(s.badge).toBe('offline');
        expect(badgeText(s)).toContain('内容已存本地');
    });

    it('有缺口 → gap + degraded 详情', () => {
        const s = statusReducer(initialStatusState(), {
            type: 'gap', degraded: 'photo_timeline 缺页',
        });
        expect(s.badge).toBe('gap');
        expect(s.degraded).toBe('photo_timeline 缺页');
    });

    it('错误分派到对应五态并带错误码', () => {
        const s = statusReducer(initialStatusState(), {
            type: 'error', code: ERROR_CODES[5001].code,
        });
        expect(s.badge).toBe('error');
        expect(s.errorCode).toBe(ERROR_CODES[5001].code);
    });

    it('retry 清除错误回到 synced', () => {
        let s = statusReducer(initialStatusState(), {
            type: 'error', code: ERROR_CODES[5001].code,
        });
        s = statusReducer(s, { type: 'retry' });
        expect(s.badge).toBe('synced');
        expect(s.errorCode).toBeNull();
    });
});

describe('错误码动作按钮', () => {
    it('从 ERROR_CODES.action 取文案，retryable 保留', () => {
        const lunar = errorAction(ERROR_CODES[4005].code);
        expect(lunar?.label).toBe(ERROR_CODES[4005].action);
        expect(lunar?.retryable).toBe(false);
        expect(errorAction(null)).toBeNull();
    });
});

describe('五态文案齐备', () => {
    it('每个状态都有非空文案', () => {
        const states: StatusState['badge'][] = ['synced', 'pending', 'offline', 'gap', 'error'];
        for (const badge of states) {
            expect(badgeText({ ...initialStatusState(), badge }).length).toBeGreaterThan(0);
        }
    });
});
