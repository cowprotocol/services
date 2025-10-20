FROM node:22-bookworm-slim AS base
WORKDIR /app

# Native build deps for better-sqlite3
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    apt-get update && apt-get install -y python3 make g++ && rm -rf /var/lib/apt/lists/*

COPY package.json package-lock.json* yarn.lock* pnpm-lock.yaml* .npmrc* ./
RUN npm pkg delete scripts.prepare || true

# Install dependencies (will use whichever lockfile is present at runtime)
RUN if [ -f package-lock.json ]; then npm ci --omit=dev; \
    elif [ -f yarn.lock ]; then yarn install --frozen-lockfile --production; \
    elif [ -f pnpm-lock.yaml ]; then corepack enable && pnpm i --frozen-lockfile --prod; \
    else npm i --omit=dev; fi

FROM base AS build
COPY . .
RUN npm i && npm run build

FROM node:22-bookworm-slim
WORKDIR /app
ENV NODE_ENV=production

# Install runtime dependencies for better-sqlite3
RUN apt-get update && apt-get install -y python3 make g++ && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/dist ./dist
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/package-lock.json* ./
COPY --from=build /app/node_modules ./node_modules

# Rebuild native modules for the target architecture
RUN npm rebuild better-sqlite3

EXPOSE 8081
CMD ["node", "dist/server.js"]

