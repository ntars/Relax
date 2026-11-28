// 服务端状态屏 reducer 测试（C5.6）：doctor ok/complete/degraded → 五态角标。

import { describe, it, expect } from 'vitest';
import {
    serverStatusReducer, initialServerStatusState,
} from './statusReducer';
import type { ServerStatusInfo } from './statusReducer';

const healthy: ServerStatusInfo = {
    ok: true, complete: true, degradedCount: 0,
    version: '0.1.0', contractVersion: '1.0.0', certDaysRemaining: 300,
};

describe('serverStatusReducer 五态映射', () => {
    it('全绿 doctor → synced', () => {
        const s = serverStatusReducer(initialServerStatusState(), { type: 'loaded', info: healthy });
        expect(s.badge).toBe('synced');
        expect(s.info?.version).toBe('0.1.0');
    });

    it('ok=false → offline（触发离线横幅）', () => {
        const s = serverStatusReducer(initialServerStatusState(), {
            type: 'loaded', info: { ...healthy, ok: false },
        });
        expect(s.badge).toBe('offline');
    });

    it('complete=false → gap 且 degraded 可点开', () => {
        const s = serverStatusReducer(initialServerStatusState(), {
            type: 'loaded', info: { ...healthy, complete: false, degradedCount: 3 },
        });
        expect(s.badge).toBe('gap');
        expect(s.degraded).toContain('3 项降级');
    });

    it('网络错误码 → offline', () => {
        const s = serverStatusReducer(initialServerStatusState(), { type: 'error', code: 3001 });
        expect(s.badge).toBe('offline');
        expect(s.errorCode).toBe(3001);
    });

    it('其他错误码 → error', () => {
        const s = serverStatusReducer(initialServerStatusState(), { type: 'error', code: 5001 });
        expect(s.badge).toBe('error');
    });

    it('显式 offline 动作 → offline', () => {
        const s = serverStatusReducer(initialServerStatusState(), { type: 'offline' });
        expect(s.badge).toBe('offline');
    });

    it('显式 gap 动作带 degraded 文案', () => {
        const s = serverStatusReducer(initialServerStatusState(), { type: 'gap', degraded: '备份>48h' });
        expect(s.badge).toBe('gap');
        expect(s.degraded).toBe('备份>48h');
    });
});
