#!/bin/bash
set -e

echo "=== Sourcify Server Starting ==="

# Wait for database to be ready
echo "Waiting for database..."
until pg_isready -h "$SOURCIFY_POSTGRES_HOST" -p "$SOURCIFY_POSTGRES_PORT" -U "$SOURCIFY_POSTGRES_USER" -d "$SOURCIFY_POSTGRES_DB" > /dev/null 2>&1; do
    echo "Database not ready, waiting..."
    sleep 2
done
echo "Database is ready!"

# Run migrations
echo "Running database migrations..."
DATABASE_URL="postgres://${SOURCIFY_POSTGRES_USER}:${SOURCIFY_POSTGRES_PASSWORD}@${SOURCIFY_POSTGRES_HOST}:${SOURCIFY_POSTGRES_PORT}/${SOURCIFY_POSTGRES_DB}?sslmode=disable"
dbmate --url "$DATABASE_URL" --migrations-dir /migrations --no-dump-schema up
echo "Migrations complete!"

# Start the server
echo "Starting Sourcify server..."
exec node dist/server/cli.js
