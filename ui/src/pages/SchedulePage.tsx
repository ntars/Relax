import { useState } from 'react';
interface ScheduleItem { id: string; title: string; start_time: string; created_at: string; }

export function SchedulePage() {
    const [schedules] = useState<ScheduleItem[]>([]);
    const [newTitle, setNewTitle] = useState('');
    const [newStartTime, setNewStartTime] = useState('');

    return (
        <div className="schedule-page" style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
            <h1>📅 日程提醒</h1>
            
            {/* 添加日程表单 */}
            <div style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px', 
                padding: '20px',
                marginBottom: '20px'
            }}>
                <h3>添加日程</h3>
                <input
                    type="text"
                    value={newTitle}
                    onChange={(e) => setNewTitle(e.target.value)}
                    placeholder="日程标题"
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc',
                        marginBottom: '10px'
                    }}
                />
                <input
                    type="datetime-local"
                    value={newStartTime}
                    onChange={(e) => setNewStartTime(e.target.value)}
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc'
                    }}
                />
            </div>

            {/* 日程列表 */}
            <h3>我的日程</h3>
            <div className="schedule-list" style={{ 
                border: '1px solid #ccc', 
                borderRadius: '8px'
            }}>
                {schedules.length === 0 ? (
                    <p style={{ textAlign: 'center', color: '#666' }}>暂无日程</p>
                ) : (
                    schedules.map((sched) => (
                        <div key={sched.id} style={{ 
                            padding: '15px', 
                            borderBottom: '1px solid #eee'
                        }}>
                            <h4>{sched.title}</h4>
                            <p>开始时间: {sched.start_time}</p>
                            <small style={{ color: '#999' }}>{sched.created_at}</small>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
