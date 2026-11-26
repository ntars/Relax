import React, { useState } from 'react';

export function NewsPage() {
    const [newsList, setNewsList] = useState([]);

    return (
        <div className="news-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>📰 新闻聚合</h1>
            
            {/* 新闻列表 */}
            <div className="news-list" style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px'
            }}>
                {newsList.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无新闻</p>
                ) : (
                    newsList.map((news) => (
                        <div key={news.id} style={{ 
                            padding: '15px', 
                            borderBottom: '1px solid #eee'
                        }}>
                            <h4>{news.title}</h4>
                            {news.content && <p>{news.content}</p>}
                            {news.source_url && (
                                <a href={news.source_url} target="_blank" rel="noopener noreferrer">
                                    阅读原文
                                </a>
                            )}
                            <small style={{ color: '#999' }}>{news.created_at}</small>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
