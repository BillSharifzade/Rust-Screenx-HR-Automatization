import { tokenStore } from './store';

// When NEXT_PUBLIC_API_URL is empty, use relative URLs which go through the Next.js proxy
const BASE_URL = process.env.NEXT_PUBLIC_API_URL || '';

type FetchOptions = RequestInit & {
    params?: Record<string, string | number | boolean | undefined>;
};

export async function apiFetch<T>(endpoint: string, options: FetchOptions = {}): Promise<T> {
    const { params, ...init } = options;

    let url = `${BASE_URL}${endpoint}`;

    if (params) {
        const searchParams = new URLSearchParams();
        Object.entries(params).forEach(([key, value]) => {
            if (value !== undefined) {
                searchParams.append(key, String(value));
            }
        });
        const queryString = searchParams.toString();
        if (queryString) {
            url += `?${queryString}`;
        }
    }

    const headers: HeadersInit = {
        ...(init.body instanceof FormData ? {} : { 'Content-Type': 'application/json' }),
        ...init.headers,
    };

    const token = tokenStore.getToken();
    if (token) {
        (headers as any)['Authorization'] = `Bearer ${token}`;
    }

    const response = await fetch(url, {
        ...init,
        headers,
    });

    if (!response.ok) {
        let errorMessage = `API Error ${response.status}`;
        try {
            const errorData = await response.json();
            if (errorData.message) {
                errorMessage = errorData.message;
            } else if (errorData.error) {
                errorMessage = errorData.error;
            } else if (typeof errorData === 'string') {
                errorMessage = errorData;
            }
        } catch (e) {
            // If text body exists, use it
            const text = await response.text().catch(() => '');
            if (text) errorMessage += `: ${text}`;
        }
        throw new Error(errorMessage);
    }

    // Handle empty responses (e.g. 204 No Content)
    if (response.status === 204) {
        return {} as T;
    }

    try {
        return await response.json();
    } catch (e) {
        // Fallback if response is not JSON
        return {} as T;
    }
}
export async function deleteCandidate(id: string): Promise<void> {
    await apiFetch(`/api/integration/candidates/${id}`, {
        method: 'DELETE',
    });
}

// ---- Responses ("Отклики") kanban ----

export interface ResponseCard {
    id: string;
    candidate_id: string;
    candidate_name: string;
    candidate_email: string;
    candidate_phone?: string | null;
    candidate_cv_url?: string | null;
    telegram_id?: number | null;
    vacancy_id: number;
    vacancy_title?: string | null;
    status: string;
    ai_grade?: number | null;
    ai_comment?: string | null;
    hr_comment?: string | null;
    test_attempt_id?: string | null;
    decision?: string | null;
    responded_at: string;
    updated_at: string;
}

export interface ResponsesFeed {
    stages: string[];
    items: ResponseCard[];
}

export interface UpdateResponseBody {
    status?: string;
    hr_comment?: string;
    decision?: "accepted" | "rejected";
    test_attempt_id?: string;
}

export function listResponses(): Promise<ResponsesFeed> {
    return apiFetch<ResponsesFeed>("/api/integration/responses");
}

export function updateResponse(id: string, body: UpdateResponseBody): Promise<ResponseCard> {
    return apiFetch<ResponseCard>(`/api/integration/responses/${id}`, {
        method: "PATCH",
        body: JSON.stringify(body),
    });
}
