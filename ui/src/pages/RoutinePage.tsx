import React, { useState } from 'react';

export function RoutinePage() {
    const [routines, setRoutines] = useState([]);

    return (
        <div className="routine-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>⏰ 作息安排</h1>
            
            {/* 添加作息 */}
            <div style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                padding: '20px',
                marginBottom: '20px'
            }}>
                <h3>添加作息</h3>
                <input
                    type="text"
                    placeholder="作息标题（如：早起、午休）"
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc'
                    }}
                />
            </div>

            {/* 作息列表 */}
            <h3>家庭作息</h3>
            <div className="routine-list" style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px'
            }}>
                {routines.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无作息安排</p>
                ) : (
                    routines.map((routine) => (
                        <div key={routine.id} style={{ 
                            padding: '15px', 
                            borderBottom: '1px solid #eee'
                        }}>
                            <h4>{routine.title}</h4>
                            {routine.description && <p>{routine.description}</p>}
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
