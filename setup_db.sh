#!/bin/bash
set -e

DATA_DIR="/var/lib/postgres/data"

echo "Checking PostgreSQL data directory: $DATA_DIR"

if [ -z "$(sudo ls -A "$DATA_DIR")" ]; then
    echo "Initializing PostgreSQL database..."
    sudo -u postgres initdb -D "$DATA_DIR"
fi

echo "Starting PostgreSQL..."
sudo systemctl start postgresql 
sudo systemctl enable postgresql

echo "Waiting for PostgreSQL to be ready..."
until sudo -u postgres psql -c '\l' >/dev/null 2>&1; do
  echo "Waiting for Postgres..."
  sleep 1
done

echo "Creating database user..."
sudo -u postgres psql -c "DO \$\$ BEGIN CREATE ROLE postgres WITH LOGIN PASSWORD 'password' SUPERUSER; EXCEPTION WHEN DUPLICATE_OBJECT THEN NULL; END \$\$;"
sudo -u postgres psql -c "ALTER USER postgres WITH PASSWORD 'password';"

echo "Creating database..."
# Simple create, ignore error if exists
sudo -u postgres psql -c "CREATE DATABASE recruitment_db;" || echo "Database probably exists"

echo "Database setup complete! Postgres is running locally."
