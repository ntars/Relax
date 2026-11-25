import { invoke } from '@tauri-apps/api/tao';

// 声明 Tauri 命令
declare global {
    interface Window {
        __tauri__: any;
    }
}

export async function login(username: string, password: string): Promise<AuthToken> {
    return await invoke('login', { username, password });
}

export async function logout(): Promise<void> {
    await invoke('logout');
}

export async function me(): Promise<UserInfo> {
    return await invoke('me');
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
