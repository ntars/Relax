// 相册屏（C4.6）：时间线 + 多选 + 加入相册 + 能力控制 + 五态角标横幅。
// reducer 是纯函数（见 reducer.ts / reducer.test.ts），本组件只做 api 桥接与事件转发。

import { useReducer, useCallback, useEffect, useState } from 'react';
import {
    photoList,
    albumList,
    albumCreate,
    albumAddFiles,
    errorCodeOf,
} from '../../services/api';
import {
    photosReducer, initialPhotosState, canReadPhotos, canManageAlbums, timeline,
} from './reducer';
import type { Role } from '../../gen/capabilities';
import type { BadgeState, UiPhoto } from './reducer';

const BADGE_TEXT: Record<BadgeState, string> = {
    synced: '已同步',
    pending: '同步中',
    offline: '离线，稍后重试',
    gap: '数据缺口，请下拉刷新',
    error: '出错了',
};

export function PhotosScreen({ role }: { readonly role: Role }) {
    const [state, dispatch] = useReducer(photosReducer, undefined, initialPhotosState);
    const [newAlbum, setNewAlbum] = useState('');

    const load = useCallback(async () => {
        try {
            const [photos, albums] = await Promise.all([photoList(), albumList()]);
            const items: UiPhoto[] = photos.items.map((p) => ({
                id: p.id,
                name: p.name,
                captureTime: p.captureTime,
                mime: p.mime,
                fileId: p.fileId,
            }));
            dispatch({ type: 'hydrate', items });
            dispatch({
                type: 'albumsLoaded',
                albums: albums.albums.map((a) => ({ id: a.id, name: a.name, count: a.count })),
            });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            } else {
                dispatch({ type: 'offline' });
            }
        }
    }, []);

    useEffect(() => {
        void load();
    }, [load]);

    useEffect(() => {
        dispatch({ type: 'setRole', role });
    }, [role]);

    const onCreateAlbum = useCallback(async () => {
        if (newAlbum.trim().length === 0) {
            return;
        }
        try {
            await albumCreate(newAlbum);
            setNewAlbum('');
            await load();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [newAlbum, load]);

    const onAddToAlbum = useCallback(async (albumId: string) => {
        if (state.selected.length === 0) {
            return;
        }
        try {
            await albumAddFiles(albumId, [...state.selected]);
            dispatch({ type: 'addedToAlbum' });
            await load();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [state.selected, load]);

    const showList = canReadPhotos(state.role);
    const showManage = canManageAlbums(state.role);
    const sections = timeline(state.items);

    return (
        <div className="photos-screen">
            <header className="photos-header">
                <h1>📸 家庭相册</h1>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
                {state.badge === 'offline' || state.badge === 'gap' ? (
                    <button type="button" onClick={() => void load()}>重试</button>
                ) : null}
            </header>

            {showManage ? (
                <section className="album-manager" aria-label="相册管理">
                    <input
                        aria-label="新相册名"
                        value={newAlbum}
                        onChange={(e) => setNewAlbum(e.target.value)}
                        placeholder="新相册名"
                    />
                    <button type="button" onClick={() => void onCreateAlbum()}>新建相册</button>
                    {state.albums.length > 0 ? (
                        <div className="album-actions">
                            {state.selected.length > 0 ? (
                                <span>已选 {state.selected.length} 张</span>
                            ) : null}
                            {state.albums.map((album) => (
                                <button
                                    key={album.id}
                                    type="button"
                                    disabled={state.selected.length === 0}
                                    onClick={() => void onAddToAlbum(album.id)}
                                >
                                    加入「{album.name}」
                                </button>
                            ))}
                        </div>
                    ) : null}
                </section>
            ) : null}

            {showList ? (
                <div className="photo-timeline">
                    {sections.map((section) => (
                        <section key={section.date} className="timeline-section">
                            <h2>{section.date}</h2>
                            <div className="photo-grid">
                                {section.items.map((photo) => (
                                    <button
                                        key={photo.id}
                                        type="button"
                                        className={state.selected.includes(photo.id) ? 'photo selected' : 'photo'}
                                        aria-pressed={state.selected.includes(photo.id)}
                                        onClick={() => dispatch({ type: 'toggleSelect', id: photo.id })}
                                    >
                                        <span>{photo.name}</span>
                                    </button>
                                ))}
                            </div>
                        </section>
                    ))}
                </div>
            ) : null}
        </div>
    );
}
