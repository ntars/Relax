import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import { chatReducer, initialChatState } from './reducer';

describe('chatReducer 五态角标', () => {
  it('enqueue 乐观入列 → pending 态 + 待发送计数 +1', () => {
    const s = chatReducer(initialChatState(), { type: 'enqueue', id: 'op-1', content: 'hi' });
    expect(s.badge).toBe('pending');
    expect(s.pendingCount).toBe(1);
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]?.pending).toBe(true);
  });

  it('多条 enqueue 累加待发送条数', () => {
    let s = chatReducer(initialChatState(), { type: 'enqueue', id: 'op-1', content: 'a' });
    s = chatReducer(s, { type: 'enqueue', id: 'op-2', content: 'b' });
    expect(s.pendingCount).toBe(2);
  });

  it('sent 清单条 pending，全部发出后回到 synced', () => {
    let s = chatReducer(initialChatState(), { type: 'enqueue', id: 'op-1', content: 'a' });
    s = chatReducer(s, { type: 'sent', id: 'op-1' });
    expect(s.badge).toBe('synced');
    expect(s.pendingCount).toBe(0);
    expect(s.messages[0]?.pending).toBe(false);
  });

  it('部分 sent 仍保持 pending 态', () => {
    let s = chatReducer(initialChatState(), { type: 'enqueue', id: 'op-1', content: 'a' });
    s = chatReducer(s, { type: 'enqueue', id: 'op-2', content: 'b' });
    s = chatReducer(s, { type: 'sent', id: 'op-1' });
    expect(s.badge).toBe('pending');
    expect(s.pendingCount).toBe(1);
  });

  it('网络/TLS/契约失败 → offline 横幅', () => {
    for (const code of [3001, 3002, 3003]) {
      const s = chatReducer(initialChatState(), { type: 'failed', id: 'op-1', code });
      expect(s.badge).toBe('offline');
      expect(s.errorCode).toBe(code);
    }
  });

  it('游标过期 → 有缺口态', () => {
    const s = chatReducer(initialChatState(), {
      type: 'failed',
      id: 'op-1',
      code: ERROR_CODES[3004].code,
    });
    expect(s.badge).toBe('gap');
  });

  it('不可自动恢复错误 → error 态带错误码', () => {
    const code = ERROR_CODES[1002].code;
    const s = chatReducer(initialChatState(), { type: 'failed', id: 'op-1', code });
    expect(s.badge).toBe('error');
    expect(s.errorCode).toBe(code);
  });

  it('hydrate 恢复同步载入消息与待发计数', () => {
    const s = chatReducer(initialChatState(), {
      type: 'hydrate',
      messages: [{ id: 'm-1', content: 'remote', pending: false }],
      pendingCount: 2,
    });
    expect(s.badge).toBe('pending');
    expect(s.messages).toHaveLength(1);
    expect(s.pendingCount).toBe(2);
  });

  it('offline / gap / synced 动作直接切角标', () => {
    expect(chatReducer(initialChatState(), { type: 'offline' }).badge).toBe('offline');
    expect(chatReducer(initialChatState(), { type: 'gap' }).badge).toBe('gap');
    const off = chatReducer(initialChatState(), { type: 'offline' });
    expect(chatReducer(off, { type: 'synced' }).badge).toBe('synced');
  });
});
