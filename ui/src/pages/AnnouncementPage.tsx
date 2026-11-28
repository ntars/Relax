import { useState } from 'react';
import type { Announcement } from '../types';

export function AnnouncementPage() {
    const [announcements] = useState<Announcement[]>([]);
    const [newTitle, setNewTitle] = useState('');
    const [newContent, setNewContent] = useState('');

    return (
        <div className="announcement-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>📢 公告发布</h1>
            
            {/* 发布公告表单 */}
            <div style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                padding: '20px',
                marginBottom: '20px'
            }}>
                <h3>发布新公告</h3>
                <input
                    type="text"
                    value={newTitle}
                    onChange={(e) => setNewTitle(e.target.value)}
                    placeholder="公告标题"
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc',
                        marginBottom: '10px'
                    }}
                />
                <textarea
                    value={newContent}
                    onChange={(e) => setNewContent(e.target.value)}
                    placeholder="公告内容"
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc',
                        minHeight: '100px'
                    }}
                />
            </div>

            {/* 公告列表 */}
            <h3>所有公告</h3>
            <div className="announcement-list" style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                maxHeight: '500px',
                overflowY: 'auto'
            }}>
                {announcements.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无公告</p>
                ) : (
                    announcements.map((ann) => (
                        <div key={ann.id} style={{ 
                            padding: '15px', 
                            borderBottom: '1px solid #eee'
                        }}>
                            <h4>{ann.title}</h4>
                            {ann.content && <p>{ann.content}</p>}
                            <small style={{ color: '#999' }}>{ann.created_at}</small>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
