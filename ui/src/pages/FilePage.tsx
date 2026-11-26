import React, { useState } from 'react';

export function FilePage() {
    const [files, setFiles] = useState([]);

    return (
        <div className="file-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>📂 文件 NAS</h1>
            
            {/* 文件上传 */}
            <div style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                padding: '20px',
                marginBottom: '20px'
            }}>
                <h3>上传文件</h3>
                <input
                    type="file"
                    multiple
                    onChange={(e) => {
                        if (e.target.files) {
                            const newFiles = Array.from(e.target.files).map(f => ({
                                name: f.name,
                                size: f.size
                            }));
                            setFiles(prev => [...prev, ...newFiles]);
                        }
                    }}
                />
            </div>

            {/* 文件列表 */}
            <h3>我的文件</h3>
            <div className="file-list" style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px'
            }}>
                {files.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无文件</p>
                ) : (
                    files.map((file, idx) => (
                        <div key={idx} style={{ 
                            padding: '10px', 
                            borderBottom: '1px solid #eee'
                        }}>
                            <span>{file.name}</span>
                            <small style={{ color: '#999' }}>({(file.size / 1024).toFixed(2)} KB)</small>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
