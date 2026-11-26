import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

export function LoginPage() {
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const navigate = useNavigate();

    const handleLogin = async () => {
        try {
            const response = await fetch('/api/v1/auth', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    username,
                    password
                })
            });
            
            if (response.ok) {
                navigate('/dashboard');
            } else {
                alert('登录失败');
            }
        } catch (error) {
            console.error('Login failed:', error);
            alert('登录失败');
        }
    };

    return (
        <div className="login-page" style={{ 
            display: 'flex', 
            justifyContent: 'center', 
            alignItems: 'center',
            minHeight: '100vh'
        }}>
            <div style={{ 
                width: '300px', 
                padding: '40px',
                border: '1px solid #ccc', 
                borderRadius: '8px'
            }}>
                <h2 style={{ textAlign: 'center' }}>Relax 登录</h2>
                
                <input
                    type="text"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    placeholder="用户名"
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc',
                        marginBottom: '10px'
                    }}
                />
                
                <input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="密码"
                    style={{ 
                        width: '100%', 
                        padding: '10px', 
                        borderRadius: '8px',
                        border: '1px solid #ccc',
                        marginBottom: '20px'
                    }}
                />
                
                <button
                    onClick={handleLogin}
                    disabled={!username || !password}
                    style={{ 
                        width: '100%', 
                        padding: '10px',
                        borderRadius: '8px',
                        backgroundColor: username && password ? '#1976d2' : '#ccc',
                        color: 'white',
                        border: 'none'
                    }}
                >
                    登录
                </button>
            </div>
        </div>
    );
}
