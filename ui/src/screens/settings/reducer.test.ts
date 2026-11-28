// 设置屏 reducer 测试（C5.5）：回收站/审计能力控制 + 五态 + 只读语义。

import { describe, it, expect } from 'vitest';
import {
  settingsReducer, initialSettingsState, canClearRecycleBin, canRestoreAnyFile,
  canReadAuditLog,
} from './reducer';

describe('C5.5 能力控制', () => {
  it('清空回收站仅 OWNER', () => {
    expect(canClearRecycleBin('OWNER')).toBe(true);
    expect(canClearRecycleBin('ADMIN')).toBe(false);
    expect(canClearRecycleBin('MEMBER')).toBe(false);
    expect(canClearRecycleBin('GUEST')).toBe(false);
  });

  it('恢复任意文件 OWNER+ADMIN', () => {
    expect(canRestoreAnyFile('OWNER')).toBe(true);
    expect(canRestoreAnyFile('ADMIN')).toBe(true);
    expect(canRestoreAnyFile('MEMBER')).toBe(false);
    expect(canRestoreAnyFile('GUEST')).toBe(false);
  });

  it('读审计日志仅 OWNER（只读页）', () => {
    expect(canReadAuditLog('OWNER')).toBe(true);
    expect(canReadAuditLog('ADMIN')).toBe(false);
    expect(canReadAuditLog('MEMBER')).toBe(false);
    expect(canReadAuditLog('CHILD')).toBe(false);
    expect(canReadAuditLog('GUEST')).toBe(false);
  });
});

describe('C5.5 回收站 reducer', () => {
  const items = [
    { id: 'f1', name: 'a.jpg', recycledAt: '2026-11-01T00:00:00Z' },
    { id: 'f2', name: 'b.jpg', recycledAt: '2026-11-02T00:00:00Z' },
  ];

  it('加载回收站', () => {
    const s = settingsReducer(initialSettingsState(), { type: 'recycleLoaded', items });
    expect(s.recycle).toHaveLength(2);
    expect(s.badge).toBe('synced');
  });

  it('恢复后从列表移除并记动作', () => {
    let s = settingsReducer(initialSettingsState(), { type: 'recycleLoaded', items });
    s = settingsReducer(s, { type: 'restored', id: 'f1' });
    expect(s.recycle.map((i) => i.id)).toEqual(['f2']);
    expect(s.lastAction).toBe('已恢复');
  });

  it('清空后列表为空', () => {
    let s = settingsReducer(initialSettingsState(), { type: 'recycleLoaded', items });
    s = settingsReducer(s, { type: 'cleared' });
    expect(s.recycle).toHaveLength(0);
    expect(s.lastAction).toBe('回收站已清空');
  });
});

describe('C5.5 审计只读页 + 五态', () => {
  it('审计条目只读加载，不提供写入 action', () => {
    const entries = [
      { id: 1, actorId: 'owner', action: 'backup', targetType: 'system', targetId: 'b1', createdAt: '2026-11-01T00:00:00Z' },
    ];
    const s = settingsReducer(initialSettingsState(), { type: 'auditLoaded', entries });
    expect(s.audit).toEqual(entries);
    // reducer 的 action 联合类型里没有审计写入分支——只读语义由类型系统保证。
  });

  it('离线/缺口/错误/同步五态', () => {
    let s = settingsReducer(initialSettingsState(), { type: 'offline' });
    expect(s.badge).toBe('offline');
    s = settingsReducer(s, { type: 'gap' });
    expect(s.badge).toBe('gap');
    s = settingsReducer(s, { type: 'error', code: 5005 });
    expect(s.badge).toBe('error');
    expect(s.errorCode).toBe(5005);
    s = settingsReducer(s, { type: 'synced' });
    expect(s.badge).toBe('synced');
    expect(s.errorCode).toBeNull();
  });

  it('网络类错误码归入 offline', () => {
    const s = settingsReducer(initialSettingsState(), { type: 'error', code: 3001 });
    expect(s.badge).toBe('offline');
  });
});
