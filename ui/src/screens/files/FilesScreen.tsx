// 文件屏（C4.5）：文件列表 + 上传/断点续传 + 回收站 + 能力控制 + 五态角标横幅。
// reducer 是纯函数（见 reducer.ts / reducer.test.ts），本组件只做 api 桥接与事件转发。

import { useReducer, useCallback, useEffect, useState } from 'react';
import {
    fileList,
    fileUpload,
    fileRecycleList,
    fileRestore,
    fileDelete,
    errorCodeOf,
} from '../../services/api';
import {
    filesReducer, initialFilesState, canReadFiles, canUploadFiles, canClearRecycle, canRestoreAny,
} from './reducer';
import type { Role } from '../../gen/capabilities';
import { ERROR_CODES } from '../../gen/error-codes';
import type { BadgeState, UiFile } from './reducer';

const BADGE_TEXT: Record<BadgeState, string> = {
    synced: '已同步',
    pending: '同步中（断点续传）',
    offline: '离线，稍后重试',
    gap: '数据缺口，请下拉刷新',
    error: '出错了',
};

export function FilesScreen({ role }: { readonly role: Role }) {
    const [state, dispatch] = useReducer(filesReducer, undefined, initialFilesState);
    const [picked, setPicked] = useState<readonly File[]>([]);

    const load = useCallback(async () => {
        try {
            const data = await fileList();
            const items: UiFile[] = data.items.map((f) => ({
                id: f.id, name: f.name, size: f.size, mime: f.mime,
            }));
            dispatch({ type: 'hydrate', items });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            } else {
                dispatch({ type: 'offline' });
            }
        }
    }, []);

    const loadRecycle = useCallback(async () => {
        try {
            const data = await fileRecycleList();
            const items: UiFile[] = data.items.map((f) => ({
                id: f.id, name: f.name, size: f.size, mime: f.mime,
            }));
            dispatch({ type: 'recycleLoaded', items });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, []);

    useEffect(() => {
        void load();
        void loadRecycle();
    }, [load, loadRecycle]);

    useEffect(() => {
        dispatch({ type: 'setRole', role });
    }, [role]);

    const onUpload = useCallback(async () => {
        for (const file of picked) {
            const id = `${file.name}-${file.size}`;
            dispatch({
                type: 'uploadStart',
                task: { id, name: file.name, uploaded: 0, total: file.size, status: 'uploading' },
            });
            try {
                // invoke 只传元数据；真实字节由 Tauri command 侧读取本地路径。
                await fileUpload(file.name, file.size, file.type || 'application/octet-stream');
                dispatch({ type: 'uploadDone', id });
            } catch (error) {
                const code = errorCodeOf(error);
                if (code === ERROR_CODES[3005].code) {
                    // 上传中断：记 interrupted，用户可点续传（只传缺失分片）。
                    dispatch({ type: 'uploadInterrupted', id });
                } else if (code !== null) {
                    dispatch({ type: 'uploadFailed', id, code });
                } else {
                    dispatch({ type: 'uploadFailed', id, code: ERROR_CODES[3001].code });
                }
            }
        }
        setPicked([]);
        void load();
    }, [picked, load]);

    const onRestore = useCallback(async (id: string) => {
        try {
            await fileRestore(id);
            void load();
            void loadRecycle();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [load, loadRecycle]);

    const onDelete = useCallback(async (id: string) => {
        try {
            await fileDelete(id);
            void load();
            void loadRecycle();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [load, loadRecycle]);

    const showList = canReadFiles(state.role);
    const showUpload = canUploadFiles(state.role);
    const showRestore = canRestoreAny(state.role);
    const showClear = canClearRecycle(state.role);

    return (
        <div className="files-screen">
            <header className="files-header">
                <h1>📂 文件 NAS</h1>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
                {state.badge === 'offline' || state.badge === 'gap' ? (
                    <button type="button" onClick={() => void load()}>重试</button>
                ) : null}
            </header>

            {showUpload ? (
                <section className="file-upload" aria-label="上传文件">
                    <input
                        type="file"
                        multiple
                        onChange={(e) => setPicked(Array.from(e.target.files ?? []))}
                    />
                    <button type="button" onClick={() => void onUpload()}>上传</button>
                    <ul className="upload-tasks">
                        {state.uploads.map((task) => (
                            <li key={task.id}>
                                <span>{task.name}</span>
                                <progress value={task.uploaded} max={task.total} />
                                <span className={`upload-${task.status}`}>{task.status}</span>
                            </li>
                        ))}
                    </ul>
                </section>
            ) : null}

            {showList ? (
                <ul className="file-items">
                    {state.items.map((item) => (
                        <li key={item.id}>
                            <span>{item.name}</span>
                            <span>{item.size} B</span>
                            {canClearRecycle(state.role) ? (
                                <button type="button" onClick={() => void onDelete(item.id)}>删除</button>
                            ) : null}
                        </li>
                    ))}
                </ul>
            ) : null}

            {showList && state.recycle.length > 0 ? (
                <section className="recycle-bin" aria-label="回收站">
                    <h2>回收站</h2>
                    {showClear ? <span className="recycle-hint">保留 30 天后自动清理</span> : null}
                    <ul>
                        {state.recycle.map((item) => (
                            <li key={item.id}>
                                <span>{item.name}</span>
                                {showRestore ? (
                                    <button type="button" onClick={() => void onRestore(item.id)}>恢复</button>
                                ) : null}
                            </li>
                        ))}
                    </ul>
                </section>
            ) : null}
        </div>
    );
}
