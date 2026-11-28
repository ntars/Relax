import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import { newsReducer, initialNewsState, canManageFeeds } from './reducer';

describe('newsReducer 源管理入口可见性（ADMIN/OWNER）', () => {
  it('OWNER/ADMIN 可见，MEMBER/CHILD/GUEST 不可见', () => {
    expect(canManageFeeds('OWNER')).toBe(true);
    expect(canManageFeeds('ADMIN')).toBe(true);
    expect(canManageFeeds('MEMBER')).toBe(false);
    expect(canManageFeeds('CHILD')).toBe(false);
    expect(canManageFeeds('GUEST')).toBe(false);
  });
});

describe('newsReducer 新闻与源列表', () => {
  it('hydrate 载入新闻与源', () => {
    const s = newsReducer(initialNewsState(), {
      type: 'hydrate',
      items: [{ id: 'n1', title: '新闻', sourceUrl: 'https://x', read: false }],
      feeds: [{ id: 'f1', title: '源', url: 'https://x/feed', enabled: true }],
    });
    expect(s.items).toHaveLength(1);
    expect(s.feeds).toHaveLength(1);
    expect(s.badge).toBe('synced');
  });

  it('markRead 只改对应项', () => {
    let s = newsReducer(initialNewsState(), {
      type: 'hydrate',
      items: [
        { id: 'n1', title: 'a', sourceUrl: '', read: false },
        { id: 'n2', title: 'b', sourceUrl: '', read: false },
      ],
      feeds: [],
    });
    s = newsReducer(s, { type: 'markRead', id: 'n1' });
    expect(s.items[0]?.read).toBe(true);
    expect(s.items[1]?.read).toBe(false);
  });

  it('addFeed / removeFeed 增删源', () => {
    let s = newsReducer(initialNewsState(), {
      type: 'addFeed',
      feed: { id: 'f1', title: '源', url: 'https://x', enabled: true },
    });
    expect(s.feeds).toHaveLength(1);
    s = newsReducer(s, { type: 'removeFeed', id: 'f1' });
    expect(s.feeds).toHaveLength(0);
  });

  it('setRole 切换源管理可见性依据', () => {
    let s = newsReducer(initialNewsState(), { type: 'setRole', role: 'ADMIN' });
    expect(canManageFeeds(s.role)).toBe(true);
    s = newsReducer(s, { type: 'setRole', role: 'MEMBER' });
    expect(canManageFeeds(s.role)).toBe(false);
  });
});

describe('newsReducer 五态角标', () => {
  it('源不可达 4004 → offline 横幅', () => {
    const s = newsReducer(initialNewsState(), {
      type: 'error',
      code: ERROR_CODES[4004].code,
    });
    expect(s.badge).toBe('offline');
    expect(s.errorCode).toBe(4004);
  });

  it('网络/TLS/契约失败 → offline', () => {
    for (const code of [3001, 3002, 3003]) {
      const s = newsReducer(initialNewsState(), {
        type: 'error',
        code: ERROR_CODES[code as keyof typeof ERROR_CODES].code,
      });
      expect(s.badge).toBe('offline');
    }
  });

  it('游标过期 → gap；源解析失败 → error 态', () => {
    expect(
      newsReducer(initialNewsState(), { type: 'error', code: ERROR_CODES[3004].code }).badge,
    ).toBe('gap');
    expect(
      newsReducer(initialNewsState(), { type: 'error', code: ERROR_CODES[5005].code }).badge,
    ).toBe('error');
  });

  it('offline/gap/synced 动作直接切角标', () => {
    expect(newsReducer(initialNewsState(), { type: 'offline' }).badge).toBe('offline');
    expect(newsReducer(initialNewsState(), { type: 'gap' }).badge).toBe('gap');
    const off = newsReducer(initialNewsState(), { type: 'offline' });
    expect(newsReducer(off, { type: 'synced' }).badge).toBe('synced');
  });
});
