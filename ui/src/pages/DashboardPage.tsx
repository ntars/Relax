import { Link } from 'react-router-dom';

export function DashboardPage() {
    return (
        <div className="dashboard-page">
            <h1>Relax 主页</h1>
            <div className="nav-grid">
                <Link to="/chat">💬 聊天</Link>
                <Link to="/announcement">📢 公告</Link>
                <Link to="/news">📰 新闻</Link>
                <Link to="/routine">⏰ 作息</Link>
                <Link to="/schedule">📅 日程</Link>
                <Link to="/file">📂 文件</Link>
                <Link to="/album">📸 相册</Link>
            </div>
        </div>
    );
}
