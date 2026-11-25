import React from 'react';
import { useNavigate } from 'react-router-dom';

export function LoginPage() {
    const [username, setUsername] = React.useState('');
    const [password, setPassword] = React.useState('');
    const [error, setError] = React.useState('');
    const navigate = useNavigate();

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError('');

        try {
            await window.__tauri__.invoke('login', { username, password });
            navigate('/dashboard');
        } catch (err) {
            setError('登录失败: ' + String(err));
        }
    };

    return (
        <div className="login-page">
            <div className="login-card">
                <h1>Relax 登录</h1>
                {error && <div className="error">{error}</div>}
                <form onSubmit={handleSubmit}>
                    <input
                        type="text"
                        placeholder="用户名"
                        value={username}
                        onChange={(e) => setUsername(e.target.value)}
                    />
                    <input
                        type="password"
                        placeholder="密码"
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                    />
                    <button type="submit">登录</button>
                </form>
            </div>
        </div>
    );
}
