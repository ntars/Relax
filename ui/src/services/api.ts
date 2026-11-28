// 所有跨进程调用统一走本封装层（invoke only）。
// 手册 9.2 信任边界第 9 条：invoke("…") 是唯一跨进程通道，禁止 window.__TAURI__ / fetch / eval 等。
import { invoke } from '@tauri-apps/api/core';
import { ERROR_CODES } from '../gen/error-codes';
import type { Role } from '../gen/capabilities';

export interface AuthToken {
    token: string;
    expires_at: number;
}

export interface EnrollRequest {
    inviteCode: string;
    name: string;
    deviceId: string;
    deviceLabel: string;
    password: string;
}

export interface LoginRequest {
    memberId: string;
    password: string;
    deviceId: string;
    totp?: string;
}

/** 从 invoke 抛出的错误里提取错误码；无码返回 null。 */
export function errorCodeOf(error: unknown): number | null {
    if (error instanceof Error) {
        const match = /^(\d{3,4}):/.exec(error.message);
        if (match !== null && match[1] !== undefined) {
            return Number(match[1]);
        }
    }
    return null;
}

/** 入网：走 Rust auth_enroll 命令。 */
export async function enroll(request: EnrollRequest): Promise<AuthToken> {
    return invoke<AuthToken>('auth_enroll', { request });
}

/** 登录（含 TOTP）：走 Rust auth_login 命令。 */
export async function loginRequest(request: LoginRequest): Promise<AuthToken> {
    return invoke<AuthToken>('auth_login', { request });
}

/** 绑定设备：走 Rust auth_bind 命令。 */
export async function bindDevice(deviceId: string, label: string): Promise<void> {
    await invoke('auth_bind', { deviceId, label });
}

export interface UserInfo {
    id: string;
    username: string;
    role: Role;
}

interface Envelope<T> {
    success: boolean;
    data?: T;
    error?: string;
}

/** 登录：走 Rust login 命令，失败用契约错误码 BAD_CREDENTIALS。 */
export async function login(username: string, password: string): Promise<AuthToken> {
    const env = await invoke<Envelope<AuthToken>>('login', {
        request: { username, password },
    });
    if (!env.success || env.data === undefined) {
        throw new Error(ERROR_CODES[1002].message);
    }
    return env.data;
}

/** 发送消息：走 Rust chat_send 命令。 */
export async function sendMessage(content: string): Promise<void> {
    await invoke('chat_send', { message: content });
}

/** 拉取消息列表：走 Rust chat_list 命令。 */
export async function chatList(): Promise<unknown> {
    return invoke('chat_list');
}

/** 触发同步：走 Rust sync_data 命令。 */
export async function syncData(): Promise<unknown> {
    return invoke('sync_data');
}

/** 列出待发 outbox：走 Rust outbox_list 命令。 */
export async function outboxList(): Promise<unknown> {
    return invoke('outbox_list');
}

/** 重放单条 op：走 Rust retry_op 命令。 */
export async function retryOp(opId: string): Promise<void> {
    await invoke('retry_op', { opId });
}
