// 所有跨进程调用统一走本封装层（invoke only）。C2 起接入真实命令，当前为可编译的骨架实现。

export async function login(username: string, password: string): Promise<AuthToken> {
    // return await invoke('login', { username, password });
    console.log('login called with:', { username });
    return { token: 'mock-token', expires_at: Date.now() + 3600 * 1000 };
}

export async function logout(): Promise<void> {
    // await invoke('logout');
    console.log('logout called');
}

export async function sendMessage(content: string): Promise<void> {
    // await invoke('send_message', { content });
    console.log('sendMessage called:', { content });
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
