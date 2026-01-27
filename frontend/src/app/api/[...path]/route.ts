import { NextRequest, NextResponse } from 'next/server';

const BACKEND_URL = process.env.BACKEND_URL || 'http://127.0.0.1:8000';

export async function GET(
    request: NextRequest,
    context: { params: Promise<{ path: string[] }> }
) {
    const { path } = await context.params;
    return proxyRequest(request, path);
}

export async function POST(
    request: NextRequest,
    context: { params: Promise<{ path: string[] }> }
) {
    const { path } = await context.params;
    return proxyRequest(request, path);
}

export async function PATCH(
    request: NextRequest,
    context: { params: Promise<{ path: string[] }> }
) {
    const { path } = await context.params;
    return proxyRequest(request, path);
}

export async function PUT(
    request: NextRequest,
    context: { params: Promise<{ path: string[] }> }
) {
    const { path } = await context.params;
    return proxyRequest(request, path);
}

export async function DELETE(
    request: NextRequest,
    context: { params: Promise<{ path: string[] }> }
) {
    const { path } = await context.params;
    return proxyRequest(request, path);
}

async function proxyRequest(request: NextRequest, pathSegments: string[]) {
    const path = pathSegments.join('/');
    const url = new URL(request.url);
    const targetUrl = `${BACKEND_URL}/api/${path}${url.search}`;

    const headers: HeadersInit = {};
    request.headers.forEach((value, key) => {
        // Skip host and other headers that shouldn't be forwarded
        if (!['host', 'connection', 'content-length'].includes(key.toLowerCase())) {
            headers[key] = value;
        }
    });

    const fetchOptions: RequestInit = {
        method: request.method,
        headers,
    };

    // Forward body for non-GET requests
    if (request.method !== 'GET' && request.method !== 'HEAD') {
        const contentType = request.headers.get('content-type') || '';

        if (contentType.includes('multipart/form-data')) {
            fetchOptions.body = await request.arrayBuffer();
        } else {
            fetchOptions.body = await request.text();
        }
    }

    try {
        console.log(`Proxying ${request.method} ${request.url} to ${targetUrl}`);
        const response = await fetch(targetUrl, fetchOptions);
        console.log(`Backend responded with status ${response.status}`);

        const responseHeaders = new Headers();
        response.headers.forEach((value, key) => {
            if (!['content-encoding', 'transfer-encoding', 'content-length'].includes(key.toLowerCase())) {
                responseHeaders.set(key, value);
            }
        });

        const body = await response.arrayBuffer();

        return new NextResponse(body, {
            status: response.status,
            statusText: response.statusText,
            headers: responseHeaders,
        });
    } catch (error: any) {
        console.error('Proxy error targeting:', targetUrl);
        console.error('Error detail:', error);
        return NextResponse.json(
            {
                error: 'Failed to proxy request to backend',
                details: error.message,
                target: targetUrl
            },
            { status: 502 }
        );
    }
}
