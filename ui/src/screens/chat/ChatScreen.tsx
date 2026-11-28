// 聊天屏（手册 7.2/7.4）：invoke only，唯一跨进程通道是 services/api 封装。
// 本屏只做：乐观入列（待发送）、五态角标、恢复同步后 hydrate。
import { useReducer, useCallback } from 'react';
import { ERROR_CODES } from '../../gen/error-codes';
import {
  sendMessage,
  chatList,
  outboxList,
  syncData,
  retryOp,
  errorCodeOf,
} from '../../services/api';
import type { BadgeState, UiMessage } from './reducer';
import { chatReducer, initialChatState } from './reducer';

const BADGE_LABEL: Record<BadgeState, string> = {
  synced: '已同步',
  pending: '待发送',
  offline: '服务端离线，内容已存本地',
  gap: '已完成但有缺口',
  error: '错误',
};

function localId(): string {
  const t = Date.now();
  return `op-${t}-${Math.floor(Math.random() * 100000)}`;
}

export function ChatScreen() {
  const [state, dispatch] = useReducer(chatReducer, undefined, initialChatState);

  const submit = useCallback(
    async (content: string) => {
      const id = localId();
      dispatch({ type: 'enqueue', id, content });
      try {
        await sendMessage(content);
        dispatch({ type: 'sent', id });
      } catch (error) {
        const code = errorCodeOf(error);
        if (code !== null) {
          dispatch({ type: 'failed', id, code });
        } else {
          dispatch({ type: 'offline' });
        }
      }
    },
    [dispatch],
  );

  const refresh = useCallback(async () => {
    try {
      const [messages, outbox] = await Promise.all([chatList(), outboxList()]);
      const pending = Array.isArray(outbox) ? outbox.length : 0;
      const hydrated: UiMessage[] = Array.isArray(messages)
        ? (messages as UiMessage[]).map((m) => ({ ...m, pending: false }))
        : [];
      dispatch({ type: 'hydrate', messages: hydrated, pendingCount: pending });
      await syncData();
      dispatch({ type: 'synced' });
    } catch (error) {
      const code = errorCodeOf(error);
      if (code !== null) {
        dispatch({ type: 'error', code });
      } else {
        dispatch({ type: 'offline' });
      }
    }
  }, [dispatch]);

  const retry = useCallback(
    async (id: string) => {
      await retryOp(id);
      dispatch({ type: 'synced' });
    },
    [dispatch],
  );

  const errorInfo =
    state.errorCode !== null
      ? ERROR_CODES[state.errorCode as keyof typeof ERROR_CODES]
      : undefined;

  return (
    <div className="chat-screen">
      <div className="chat-badge" data-state={state.badge}>
        {BADGE_LABEL[state.badge]}
        {state.badge === 'pending' ? ` ${state.pendingCount}` : ''}
      </div>
      {errorInfo !== undefined && (
        <div className="chat-error">
          <span>{errorInfo.message}</span>
          <button onClick={() => void retry('')}>{errorInfo.action}</button>
        </div>
      )}
      <button onClick={() => void refresh()}>同步</button>
      <ul className="chat-messages">
        {state.messages.map((msg) => (
          <li key={msg.id} data-pending={msg.pending}>
            {msg.content}
            {msg.pending && <span> · 待发送</span>}
          </li>
        ))}
      </ul>
      <ChatComposer onSubmit={submit} />
    </div>
  );
}

function ChatComposer({ onSubmit }: { readonly onSubmit: (content: string) => void }) {
  const [draft, setDraft] = useReducer((_prev: string, next: string) => next, '');
  return (
    <div className="chat-composer">
      <textarea
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        placeholder="输入消息…"
      />
      <button
        disabled={draft.trim().length === 0}
        onClick={() => {
          const text = draft.trim();
          if (text.length > 0) {
            onSubmit(text);
            setDraft('');
          }
        }}
      >
        发送
      </button>
    </div>
  );
}
