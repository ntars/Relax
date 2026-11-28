import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import {
    filesReducer, initialFilesState, canReadFiles, canUploadFiles, canClearRecycle, canRestoreAny,
} from './reducer';
import type { UiFile, UploadTask } from './reducer';

const file: UiFile = { id: 'f1', name: 'a.png', size: 100, mime: 'image/png' };
const task: UploadTask = {
    id: 'u1', name: 'a.png', uploaded: 0, total: 100, status: 'uploading',
};

describe('filesReducer 能力控制', () => {
    it('read_files 全员含 GUEST，upload_files 不含 CHILD/GUEST', () => {
        expect(canReadFiles('GUEST')).toBe(true);
        expect(canUploadFiles('MEMBER')).toBe(true);
        expect(canUploadFiles('CHILD')).toBe(false);
        expect(canUploadFiles('GUEST')).toBe(false);
    });

    it('clear_recycle_bin 仅 OWNER，restore_any_file OWNER/ADMIN', () => {
        expect(canClearRecycle('OWNER')).toBe(true);
        expect(canClearRecycle('ADMIN')).toBe(false);
        expect(canRestoreAny('ADMIN')).toBe(true);
        expect(canRestoreAny('MEMBER')).toBe(false);
    });
});

describe('filesReducer 上传与断点续传', () => {
    it('hydrate 载入文件列表', () => {
        const s = filesReducer(initialFilesState(), { type: 'hydrate', items: [file] });
        expect(s.items).toEqual([file]);
        expect(s.badge).toBe('synced');
    });

    it('uploadStart 置 pending，progress 只更新指定任务', () => {
        let s = filesReducer(initialFilesState(), { type: 'uploadStart', task });
        expect(s.badge).toBe('pending');
        const task2: UploadTask = { ...task, id: 'u2' };
        s = filesReducer(s, { type: 'uploadStart', task: task2 });
        s = filesReducer(s, { type: 'uploadProgress', id: 'u1', uploaded: 60 });
        expect(s.uploads.find((t) => t.id === 'u1')?.uploaded).toBe(60);
        expect(s.uploads.find((t) => t.id === 'u2')?.uploaded).toBe(0);
    });

    it('UPLOAD_INTERRUPTED 置 interrupted 可续传，badge=pending', () => {
        let s = filesReducer(initialFilesState(), { type: 'uploadStart', task });
        s = filesReducer(s, { type: 'uploadInterrupted', id: 'u1' });
        expect(s.uploads[0]?.status).toBe('interrupted');
        expect(s.badge).toBe('pending');
        expect(s.errorCode).toBe(ERROR_CODES[3005].code);
    });

    it('uploadDone 完成并回到 synced', () => {
        let s = filesReducer(initialFilesState(), { type: 'uploadStart', task });
        s = filesReducer(s, { type: 'uploadDone', id: 'u1' });
        expect(s.uploads[0]?.status).toBe('done');
        expect(s.uploads[0]?.uploaded).toBe(100);
        expect(s.badge).toBe('synced');
    });

    it('uploadFailed 按错误码分类角标', () => {
        let s = filesReducer(initialFilesState(), { type: 'uploadStart', task });
        s = filesReducer(s, {
            type: 'uploadFailed', id: 'u1', code: ERROR_CODES[2006].code,
        });
        expect(s.uploads[0]?.status).toBe('failed');
        expect(s.badge).toBe('error');
        s = filesReducer(s, {
            type: 'uploadFailed', id: 'u1', code: ERROR_CODES[3001].code,
        });
        expect(s.badge).toBe('offline');
    });

    it('removeUpload 清理任务', () => {
        let s = filesReducer(initialFilesState(), { type: 'uploadStart', task });
        s = filesReducer(s, { type: 'removeUpload', id: 'u1' });
        expect(s.uploads).toHaveLength(0);
    });

    it('recycleLoaded 载入回收站', () => {
        const s = filesReducer(initialFilesState(), {
            type: 'recycleLoaded', items: [file],
        });
        expect(s.recycle).toEqual([file]);
    });
});

describe('filesReducer 五态角标', () => {
    it('网络类错误 → offline（按 name）', () => {
        for (const code of [3001, 3002, 3003]) {
            expect(filesReducer(initialFilesState(), { type: 'error', code }).badge).toBe('offline');
        }
    });

    it('游标过期 → gap，其余 → error', () => {
        expect(filesReducer(initialFilesState(), {
            type: 'error', code: ERROR_CODES[3004].code,
        }).badge).toBe('gap');
        expect(filesReducer(initialFilesState(), {
            type: 'error', code: ERROR_CODES[5001].code,
        }).badge).toBe('error');
    });

    it('offline/gap/synced 动作', () => {
        let s = filesReducer(initialFilesState(), { type: 'offline' });
        expect(s.badge).toBe('offline');
        s = filesReducer(s, { type: 'gap' });
        expect(s.badge).toBe('gap');
        s = filesReducer(s, { type: 'synced' });
        expect(s.badge).toBe('synced');
    });
});
