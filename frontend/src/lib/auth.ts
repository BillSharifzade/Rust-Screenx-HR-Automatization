import { apiFetch } from './api';
import { tokenStore } from './store';

export type UserRole = 'admin' | 'hr' | 'manager';

export interface AuthUser {
    id: string;
    name: string;
    email: string;
    role: UserRole | string;
    is_active: boolean;
    must_change_password: boolean;
    last_login_at: string | null;
    created_at: string;
    updated_at: string;
}

export interface LoginResponse {
    token: string;
    must_change_password: boolean;
    user: AuthUser;
}

export interface CreateUserPayload {
    name: string;
    email: string;
    password: string;
    role: UserRole | string;
    is_active?: boolean;
    must_change_password?: boolean;
}

export interface UpdateUserPayload {
    name?: string;
    email?: string;
    role?: UserRole | string;
    is_active?: boolean;
}

export async function login(email: string, password: string): Promise<LoginResponse> {
    const res = await apiFetch<LoginResponse>('/api/auth/login', {
        method: 'POST',
        body: JSON.stringify({ email, password }),
    });
    if (res.token) tokenStore.setToken(res.token);
    return res;
}

export function logout(): void {
    tokenStore.removeToken();
}

export async function fetchMe(): Promise<AuthUser> {
    return apiFetch<AuthUser>('/api/auth/me');
}

export async function changeMyPassword(
    current_password: string,
    new_password: string,
): Promise<void> {
    await apiFetch('/api/auth/change-password', {
        method: 'POST',
        body: JSON.stringify({ current_password, new_password }),
    });
}

// ---- User management (admin only) -----------------------------------------

export async function listUsers(): Promise<AuthUser[]> {
    return apiFetch<AuthUser[]>('/api/auth/users');
}

export async function createUser(payload: CreateUserPayload): Promise<AuthUser> {
    return apiFetch<AuthUser>('/api/auth/users', {
        method: 'POST',
        body: JSON.stringify(payload),
    });
}

export async function updateUser(id: string, payload: UpdateUserPayload): Promise<AuthUser> {
    return apiFetch<AuthUser>(`/api/auth/users/${id}`, {
        method: 'PATCH',
        body: JSON.stringify(payload),
    });
}

export async function deleteUser(id: string): Promise<void> {
    await apiFetch(`/api/auth/users/${id}`, { method: 'DELETE' });
}

export async function resetUserPassword(
    id: string,
    new_password: string,
    must_change_password = true,
): Promise<AuthUser> {
    return apiFetch<AuthUser>(`/api/auth/users/${id}/password`, {
        method: 'POST',
        body: JSON.stringify({ new_password, must_change_password }),
    });
}
