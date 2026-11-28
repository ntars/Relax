import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import {
  announcementReducer,
  initialAnnouncementState,
  type UiAnnouncement,
} from './reducer';

function ann(id: string, read: boolean): UiAnnouncement {
  return { id, title: `公告${id}`, body: '内容', pinned: false, read, attachmentIds: [] };
}

describe('announcementReducer 未读角标', () => {
  it('hydrate 计算未读条数', () => {
    const s = announcementReducer(initialAnnouncementState(), {
      type: 'hydrate',
      items: [ann('a', false), ann('b', true), ann('c', false)],
    });
    expect(s.unreadCount).toBe(2);
    expect(s.badge).toBe('synced');
  });

  it('markRead 只减对应公告的未读，不重复减', () => {
    let s = announcementReducer(initialAnnouncementState(), {
      type: 'hydrate',
      items: [ann('a', false), ann('b', false)],
    });
    s = announcementReducer(s, { type: 'markRead', id: 'a' });
    expect(s.unreadCount).toBe(1);
    s = announcementReducer(s, { type: 'markRead', id: 'a' });
    expect(s.unreadCount).toBe(1);
    expect(s.items[0]?.read).toBe(true);
  });

  it('全部已读 → 未读角标归零', () => {
    let s = announcementReducer(initialAnnouncementState(), {
      type: 'hydrate',
      items: [ann('a', false), ann('b', false)],
    });
    s = announcementReducer(s, { type: 'markRead', id: 'a' });
    s = announcementReducer(s, { type: 'markRead', id: 'b' });
    expect(s.unreadCount).toBe(0);
  });
});

describe('announcementReducer 附件下载', () => {
  it('downloadStart → downloading + pending 角标', () => {
    const s = announcementReducer(initialAnnouncementState(), {
      type: 'downloadStart',
      attachmentId: 'att-1',
    });
    expect(s.downloads['att-1']).toBe('downloading');
    expect(s.badge).toBe('pending');
  });

  it('downloadDone 后无其它下载 → synced', () => {
    let s = announcementReducer(initialAnnouncementState(), {
      type: 'downloadStart',
      attachmentId: 'att-1',
    });
    s = announcementReducer(s, { type: 'downloadDone', attachmentId: 'att-1' });
    expect(s.downloads['att-1']).toBe('done');
    expect(s.badge).toBe('synced');
  });

  it('并行下载：一个完成另一个仍下载 → 保持 pending', () => {
    let s = announcementReducer(initialAnnouncementState(), {
      type: 'downloadStart',
      attachmentId: 'att-1',
    });
    s = announcementReducer(s, { type: 'downloadStart', attachmentId: 'att-2' });
    s = announcementReducer(s, { type: 'downloadDone', attachmentId: 'att-1' });
    expect(s.badge).toBe('pending');
    s = announcementReducer(s, { type: 'downloadDone', attachmentId: 'att-2' });
    expect(s.badge).toBe('synced');
  });

  it('分片/网络失败 → offline 角标 + 错误码', () => {
    const s = announcementReducer(initialAnnouncementState(), {
      type: 'downloadFailed',
      attachmentId: 'att-1',
      code: ERROR_CODES[3001].code,
    });
    expect(s.downloads['att-1']).toBe('failed');
    expect(s.badge).toBe('offline');
    expect(s.errorCode).toBe(3001);
  });
});

describe('announcementReducer 五态角标', () => {
  it('网络/TLS/契约失败 → offline', () => {
    for (const code of [3001, 3002, 3003]) {
      const s = announcementReducer(initialAnnouncementState(), {
        type: 'error',
        code: ERROR_CODES[code as keyof typeof ERROR_CODES].code,
      });
      expect(s.badge).toBe('offline');
    }
  });

  it('游标过期 → gap', () => {
    const s = announcementReducer(initialAnnouncementState(), {
      type: 'error',
      code: ERROR_CODES[3004].code,
    });
    expect(s.badge).toBe('gap');
  });

  it('不可自动恢复错误 → error 态带错误码', () => {
    const code = ERROR_CODES[1006].code;
    const s = announcementReducer(initialAnnouncementState(), { type: 'error', code });
    expect(s.badge).toBe('error');
    expect(s.errorCode).toBe(code);
  });

  it('offline/gap/synced 动作直接切角标', () => {
    expect(announcementReducer(initialAnnouncementState(), { type: 'offline' }).badge).toBe('offline');
    expect(announcementReducer(initialAnnouncementState(), { type: 'gap' }).badge).toBe('gap');
    const off = announcementReducer(initialAnnouncementState(), { type: 'offline' });
    expect(announcementReducer(off, { type: 'synced' }).badge).toBe('synced');
  });
});
