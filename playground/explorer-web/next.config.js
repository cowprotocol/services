/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  reactStrictMode: true,
  transpilePackages: [],
  env: {
    NEXT_PUBLIC_API_BASE: process.env.NEXT_PUBLIC_API_BASE || 'http://localhost:8081'
  },
  async redirects() {
    return [
      {
        source: '/token/:address',
        destination: '/address/:address',
        permanent: true,
      },
    ];
  },
};

module.exports = nextConfig;

