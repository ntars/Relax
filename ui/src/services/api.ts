// 声明 Tauri 命令
declare global {
    interface Window {
        __tauri__: any;
    }
}

export async function login(username: string, password: string): Promise<AuthToken> {
    // return await invoke('login', { username, password });
    console.log('login called with:', { username });
    return { token: 'mock-token', expires_at: Date.now() + 3600 * 1000 };
}

export async function logout(): Promise<void> {
    // await invoke('logout');
    console.log('logout called');
}

export async function me(): Promise<UserInfo> {
    // return await invoke('me');
    console.log('me called');
    return { id: 'mock-id', username: 'test', role: 'PARENT' as const };
}

export interface AuthToken {
    token: string;
    expires_at: number;
}

export interface UserInfo {
    id: string;
    username: string;
    role: Role;
}

export type Role = 'ADMIN' | 'PARENT' | 'CHILD' | 'GUEST';
