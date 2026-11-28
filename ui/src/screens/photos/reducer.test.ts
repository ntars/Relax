import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import {
    photosReducer, initialPhotosState, canReadPhotos, canManageAlbums, timeline, visiblePhotos,
} from './reducer';
import type { UiPhoto, UiAlbum } from './reducer';

const p1: UiPhoto = { id: 'p1', name: 'a.jpg', captureTime: '2026-01-02T10:00:00Z', mime: 'image/jpeg', fileId: 'f1' };
const p2: UiPhoto = { id: 'p2', name: 'b.jpg', captureTime: '2026-01-02T08:00:00Z', mime: 'image/jpeg', fileId: 'f2' };
const p3: UiPhoto = { id: 'p3', name: 'c.jpg', captureTime: '2025-12-31T20:00:00Z', mime: 'image/jpeg', fileId: 'f3' };
const album: UiAlbum = { id: 'alb1', name: '旅行', count: 2 };

describe('photosReducer 能力控制', () => {
    it('read_photos 全员含 GUEST', () => {
        expect(canReadPhotos('GUEST')).toBe(true);
        expect(canReadPhotos('CHILD')).toBe(true);
        expect(canReadPhotos('OWNER')).toBe(true);
    });

    it('加入相册沿用 upload_files，不含 CHILD/GUEST', () => {
        expect(canManageAlbums('MEMBER')).toBe(true);
        expect(canManageAlbums('CHILD')).toBe(false);
        expect(canManageAlbums('GUEST')).toBe(false);
    });
});

describe('timeline 时间线分组', () => {
    it('按日期分组，组内时间降序，组间日期降序', () => {
        const sections = timeline([p1, p2, p3]);
        expect(sections.map((s) => s.date)).toEqual(['2026-01-02', '2025-12-31']);
        expect(sections[0]?.items.map((x) => x.id)).toEqual(['p1', 'p2']);
        expect(sections[1]?.items.map((x) => x.id)).toEqual(['p3']);
    });

    it('空列表返回空时间线', () => {
        expect(timeline([])).toEqual([]);
    });
});

describe('photosReducer 多选与相册', () => {
    it('hydrate 与 albumsLoaded', () => {
        let s = photosReducer(initialPhotosState(), { type: 'hydrate', items: [p1, p2, p3] });
        expect(s.items).toHaveLength(3);
        s = photosReducer(s, { type: 'albumsLoaded', albums: [album] });
        expect(s.albums).toEqual([album]);
    });

    it('toggleSelect 多选切换，clearSelection 清空', () => {
        let s = photosReducer(initialPhotosState(), { type: 'toggleSelect', id: 'p1' });
        s = photosReducer(s, { type: 'toggleSelect', id: 'p2' });
        expect(s.selected).toEqual(['p1', 'p2']);
        s = photosReducer(s, { type: 'toggleSelect', id: 'p1' }); // 再点取消
        expect(s.selected).toEqual(['p2']);
        s = photosReducer(s, { type: 'clearSelection' });
        expect(s.selected).toEqual([]);
    });

    it('addedToAlbum 清空选中并回 synced', () => {
        let s = photosReducer(initialPhotosState(), { type: 'toggleSelect', id: 'p1' });
        s = photosReducer(s, { type: 'addedToAlbum' });
        expect(s.selected).toEqual([]);
        expect(s.badge).toBe('synced');
    });

    it('selectAlbum 过滤，null 恢复全部', () => {
        let s = photosReducer(initialPhotosState(), { type: 'hydrate', items: [p1] });
        s = photosReducer(s, { type: 'selectAlbum', albumId: 'alb1' });
        expect(s.activeAlbumId).toBe('alb1');
        expect(visiblePhotos(s)).toBeDefined();
        s = photosReducer(s, { type: 'selectAlbum', albumId: null });
        expect(visiblePhotos(s)).toHaveLength(1);
    });
});

describe('photosReducer 五态角标', () => {
    it('网络类错误 → offline（按 name）', () => {
        for (const code of [3001, 3002, 3003]) {
            expect(photosReducer(initialPhotosState(), { type: 'error', code }).badge).toBe('offline');
        }
    });

    it('游标过期 → gap，其余 → error', () => {
        expect(photosReducer(initialPhotosState(), {
            type: 'error', code: ERROR_CODES[3004].code,
        }).badge).toBe('gap');
        expect(photosReducer(initialPhotosState(), {
            type: 'error', code: ERROR_CODES[5001].code,
        }).badge).toBe('error');
    });

    it('offline/gap/synced 动作', () => {
        let s = photosReducer(initialPhotosState(), { type: 'offline' });
        expect(s.badge).toBe('offline');
        s = photosReducer(s, { type: 'gap' });
        expect(s.badge).toBe('gap');
        s = photosReducer(s, { type: 'synced' });
        expect(s.badge).toBe('synced');
    });
});
