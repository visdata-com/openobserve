#!/bin/bash
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    -- OpenFGA
    CREATE USER openfga WITH PASSWORD 'openfga';
    CREATE DATABASE openfga;
    GRANT ALL PRIVILEGES ON DATABASE openfga TO openfga;

    -- Dex
    CREATE USER dex WITH PASSWORD 'dex';
    CREATE DATABASE dex;
    GRANT ALL PRIVILEGES ON DATABASE dex TO dex;

    -- OpenObserve 元数据
    CREATE USER openobserve WITH PASSWORD 'openobserve';
    CREATE DATABASE openobserve;
    GRANT ALL PRIVILEGES ON DATABASE openobserve TO openobserve;
EOSQL

# Schema 权限
for db in openfga dex openobserve; do
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" -d $db <<-EOSQL
        GRANT ALL ON SCHEMA public TO $db;
EOSQL
done