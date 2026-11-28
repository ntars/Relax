// 公告屏（手册 C4.1）：未读角标 + 附件下载 + 五态角标（7.4）。
// invoke only：唯一跨进程通道是 services/api 封装；只按 error.code 分支，不读 message。
import { useReducer, useCallback, useEffect, useState } from 'react';
import type { BadgeState, DownloadState, UiAnnouncement } from './reducer';
import { announcementReducer, initialAnnouncementState } from './reducer';
import {
    announcementList,
    announcementMarkRead,
    announcementPublish,
    announcementDownloadAttachment,
    errorCodeOf,
} from '../../services/api';

const BADGE_LABEL: Record<BadgeState, string> = {
    synced: '已同步',
    pending: '下载中',
    offline: '服务端离线，内容已存本地',
    gap: '已完成但有缺口',
    error: '错误',
};

const DOWNLOAD_LABEL: Record<DownloadState, string> = {
    idle: '下载',
    downloading: '下载中…',
    done: '已下载',
    failed: '重试',
};

export function AnnouncementsScreen() {
    const [state, dispatch] = useReducer(announcementReducer, undefined, initialAnnouncementState);
    const [draftTitle, setDraftTitle] = useState('');
    const [draftBody, setDraftBody] = useState('');

    const hydrate = useCallback(async () => {
        try {
            const data = await announcementList();
            const items: UiAnnouncement[] = data.items.map((it) => ({
                id: it.id,
                title: it.title,
                body: it.body,
                pinned: it.pinned,
                read: it.read,
                attachmentIds: it.attachmentIds ?? [],
            }));
            dispatch({ type: 'hydrate', items });
        } catch (e) {
            const code = errorCodeOf(e);
            if (code !== null) {
                dispatch({ type: 'error', code });
            } else {
                dispatch({ type: 'offline' });
            }
        }
    }, [dispatch]);

    useEffect(() => {
        void hydrate();
    }, [hydrate]);

    const onRead = useCallback(
        async (id: string) => {
            dispatch({ type: 'markRead', id });
            try {
                await announcementMarkRead(id);
            } catch (e) {
                const code = errorCodeOf(e);
                if (code !== null) dispatch({ type: 'error', code });
            }
        },
        [dispatch],
    );

    const onDownload = useCallback(
        async (attachmentId: string) => {
            dispatch({ type: 'downloadStart', attachmentId });
            try {
                await announcementDownloadAttachment(attachmentId);
                dispatch({ type: 'downloadDone', attachmentId });
            } catch (e) {
                const code = errorCodeOf(e);
                dispatch({
                    type: 'downloadFailed',
                    attachmentId,
                    code: code ?? 3001,
                });
            }
        },
        [dispatch],
    );

    const onPublish = useCallback(
        async (title: string, body: string) => {
            try {
                await announcementPublish(title, body, []);
                await hydrate();
            } catch (e) {
                const code = errorCodeOf(e);
                if (code !== null) dispatch({ type: 'error', code });
            }
        },
        [dispatch, hydrate],
    );

    return (
        <div className="announcements-screen">
            <header>
                <h1>公告</h1>
                {state.unreadCount > 0 && (
                    <span className="badge-unread" aria-label="未读公告数">
                        {state.unreadCount}
                    </span>
                )}
                <span className={`badge badge-${state.badge}`}>{BADGE_LABEL[state.badge]}</span>
            </header>

            <section className="announcement-publish">
                <input
                    aria-label="公告标题"
                    value={draftTitle}
                    onChange={(e) => setDraftTitle(e.target.value)}
                    placeholder="公告标题"
                />
                <textarea
                    aria-label="公告内容"
                    value={draftBody}
                    onChange={(e) => setDraftBody(e.target.value)}
                    placeholder="公告内容"
                />
                <button
                    type="button"
                    onClick={() => {
                        void onPublish(draftTitle, draftBody);
                        setDraftTitle('');
                        setDraftBody('');
                    }}
                >
                    发布
                </button>
            </section>

            <ul className="announcement-items">
                {state.items.map((a) => (
                    <li key={a.id} className={a.read ? 'read' : 'unread'} onClick={() => void onRead(a.id)}>
                        <strong>{a.title}</strong>
                        <p>{a.body}</p>
                        <div className="attachments">
                            {a.attachmentIds.map((att) => (
                                <button
                                    key={att}
                                    type="button"
                                    onClick={(ev) => {
                                        ev.stopPropagation();
                                        void onDownload(att);
                                    }}
                                >
                                    {DOWNLOAD_LABEL[state.downloads[att] ?? 'idle']}
                                </button>
                            ))}
                        </div>
                    </li>
                ))}
            </ul>
        </div>
    );
}
