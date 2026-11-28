// 新闻屏（C4.2）：新闻列表 + 源管理入口（ADMIN 可见，按能力）+ 五态角标横幅。
// reducer 是纯函数（见 reducer.ts / reducer.test.ts），本组件只做 api 桥接与事件转发。

import { useReducer, useCallback, useEffect, useState } from 'react';
import {
    newsList,
    newsAddFeed,
    newsRemoveFeed,
    newsMarkRead,
    errorCodeOf,
} from '../../services/api';
import { newsReducer, initialNewsState, canManageFeeds } from './reducer';
import type { Role } from '../../gen/capabilities';
import type { BadgeState } from './reducer';

const BADGE_TEXT: Record<BadgeState, string> = {
    synced: '已同步',
    pending: '同步中',
    offline: '离线，稍后重试',
    gap: '数据缺口，请下拉刷新',
    error: '出错了',
};

export function NewsScreen({ role }: { readonly role: Role }) {
    const [state, dispatch] = useReducer(newsReducer, undefined, initialNewsState);
    const [feedUrl, setFeedUrl] = useState('');
    const [feedTitle, setFeedTitle] = useState('');

    const load = useCallback(async () => {
        try {
            const data = await newsList();
            dispatch({ type: 'hydrate', items: data.items, feeds: data.feeds });
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

    const onAddFeed = useCallback(async () => {
        if (feedUrl.trim().length === 0) {
            return;
        }
        try {
            await newsAddFeed(feedUrl, feedTitle);
            setFeedUrl('');
            setFeedTitle('');
            await load();
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, [feedUrl, feedTitle, load]);

    const onRemoveFeed = useCallback(async (id: string) => {
        try {
            await newsRemoveFeed(id);
            dispatch({ type: 'removeFeed', id });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, []);

    const onMarkRead = useCallback(async (id: string) => {
        try {
            await newsMarkRead(id);
            dispatch({ type: 'markRead', id });
        } catch (error) {
            const code = errorCodeOf(error);
            if (code !== null) {
                dispatch({ type: 'error', code });
            }
        }
    }, []);

    const showFeeds = canManageFeeds(state.role);

    return (
        <div className="news-screen">
            <header className="news-header">
                <h1>📰 新闻聚合</h1>
                <span className={`badge badge-${state.badge}`}>{BADGE_TEXT[state.badge]}</span>
                {state.badge === 'offline' || state.badge === 'gap' ? (
                    <button type="button" onClick={() => void load()}>重试</button>
                ) : null}
            </header>

            {showFeeds ? (
                <section className="feed-manager" aria-label="新闻源管理">
                    <input
                        aria-label="源 URL"
                        value={feedUrl}
                        onChange={(e) => setFeedUrl(e.target.value)}
                        placeholder="https://example.com/feed.xml"
                    />
                    <input
                        aria-label="源标题"
                        value={feedTitle}
                        onChange={(e) => setFeedTitle(e.target.value)}
                        placeholder="源标题"
                    />
                    <button type="button" onClick={() => void onAddFeed()}>添加源</button>
                    <ul>
                        {state.feeds.map((feed) => (
                            <li key={feed.id}>
                                <span>{feed.title}</span>
                                <button type="button" onClick={() => void onRemoveFeed(feed.id)}>删除</button>
                            </li>
                        ))}
                    </ul>
                </section>
            ) : null}

            <ul className="news-items">
                {state.items.map((item) => (
                    <li key={item.id} className={item.read ? 'news-read' : 'news-unread'}>
                        <button type="button" onClick={() => void onMarkRead(item.id)}>
                            {item.title}
                        </button>
                        {item.read ? null : <span className="unread-dot" aria-label="未读" />}
                    </li>
                ))}
            </ul>
        </div>
    );
}
