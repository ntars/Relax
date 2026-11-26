import React, { useState } from 'react';

export function AlbumPage() {
    const [photos, setPhotos] = useState([]);

    return (
        <div className="album-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>📸 手机照片备份</h1>
            
            {/* 照片上传 */}
            <div style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                padding: '20px',
                marginBottom: '20px'
            }}>
                <h3>上传照片</h3>
                <input
                    type="file"
                    accept="image/*"
                    multiple
                    onChange={(e) => {
                        if (e.target.files) {
                            const newPhotos = Array.from(e.target.files).map(f => ({
                                name: f.name,
                                size: f.size
                            }));
                            setPhotos(prev => [...prev, ...newPhotos]);
                        }
                    }}
                />
            </div>

            {/* 照片网格 */}
            <h3>家庭相册</h3>
            <div className="photo-grid" style={{ 
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))',
                gap: '10px'
            }}>
                {photos.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无照片</p>
                ) : (
                    photos.map((photo, idx) => (
                        <div key={idx} style={{ 
                            border: '1px solid #ccc', 
                            borderRadius: '8px',
                            padding: '10px',
                            textAlign: 'center'
                        }}>
                            <span>{photo.name}</span>
                            <br/>
                            <small style={{ color: '#999' }}>{(photo.size / 1024).toFixed(0)} KB</small>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
