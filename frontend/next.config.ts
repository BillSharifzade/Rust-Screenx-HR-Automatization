import type { NextConfig } from "next";

// Use 'http://127.0.0.1:8000' for local dev (backend runs on 8000), or the env var for Docker
const backendUrl = process.env.BACKEND_URL || 'http://127.0.0.1:8000';
console.log('Using Backend URL for Rewrites:', backendUrl);

const nextConfig: NextConfig = {
  output: 'standalone',
  reactCompiler: true,
  // Rewrites are handled by src/app/api/[...path]/route.ts
  async rewrites() {
    return [
      {
        source: '/uploads/:path*',
        destination: `${backendUrl}/uploads/:path*`,
      },
      {
        source: '/api/:path*',
        destination: `${backendUrl}/api/:path*`,
      },
    ];
  },
};

export default nextConfig;
