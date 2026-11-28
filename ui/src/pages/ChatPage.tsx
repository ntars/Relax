import { useState } from 'react';
import type { ChatMessage } from '../types';
import { sendMessage } from '../services/api';

export function ChatPage() {
    const [messages] = useState<ChatMessage[]>([]);
    const [newMessage, setNewMessage] = useState('');

    const submitMessage = async () => {
        if (!newMessage.trim()) return;

        try {
            await sendMessage(newMessage);
            setNewMessage('');
        } catch (error) {
            console.error('Send message failed:', error);
        }
    };

    return (
        <div className="chat-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>家庭群聊</h1>
            
            <div className="message-list" style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                height: '400px', 
                overflowY: 'auto',
                marginBottom: '20px'
            }}>
                {messages.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无消息，开始聊天吧！</p>
                ) : (
                    messages.map((msg) => (
                        <div key={msg.id} style={{ 
                            padding: '10px', 
                            borderBottom: '1px solid #eee'
                        }}>
                            <div>{msg.content}</div>
                        </div>
                    ))
                )}
            </div>

            <div className="message-input" style={{ display: 'flex', gap: '10px' }}>
                <textarea
                    value={newMessage}
                    onChange={(e) => setNewMessage(e.target.value)}
                    placeholder="输入消息，按 Enter 发送..."
                    style={{ 
                        flex: 1, 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc',
                        resize: 'none'
                    }}
                />
                <button
                    onClick={submitMessage}
                    disabled={!newMessage.trim()}
                    style={{ 
                        padding: '10px 20px',
                        borderRadius: '8px',
                        backgroundColor: newMessage.trim() ? '#1976d2' : '#ccc',
                        color: 'white',
                        border: 'none'
                    }}
                >
                    发送
                </button>
            </div>
        </div>
    );
}
