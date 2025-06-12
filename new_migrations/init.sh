#!/bin/zsh

# SQLite Database Reset Script
# Usage: ./reset_db.sh [database_file] [sql_script]

set -e  # Exit on any error

# Default values
DEFAULT_DB="sage.sqlite"
DEFAULT_SCRIPTS=("0001_tables.sql" "0002_indices.sql" "0003_triggers.sql" "0004_seed_data.sql")

# Get database file argument or use default
DB_FILE="${1:-$DEFAULT_DB}"

# If second argument provided, use it as single script, otherwise use default array
if [[ -n "$2" ]]; then
    SQL_SCRIPTS=("$2")
else
    SQL_SCRIPTS=("${DEFAULT_SCRIPTS[@]}")
fi

echo "🗃️  SQLite Database Reset Script"
echo "================================"
echo "Database: $DB_FILE"
echo "Scripts:  ${SQL_SCRIPTS[*]}"
echo

# Check if all SQL scripts exist
for script in "${SQL_SCRIPTS[@]}"; do
    if [[ ! -f "$script" ]]; then
        echo "❌ Error: SQL script '$script' not found!"
        echo "Usage: $0 [database_file] [sql_script]"
        echo "Default scripts: ${DEFAULT_SCRIPTS[*]}"
        exit 1
    fi
done

# Remove existing database if it exists
if [[ -f "$DB_FILE" ]]; then
    echo "🗑️  Removing existing database: $DB_FILE"
    rm "$DB_FILE"
else
    echo "ℹ️  Database file does not exist, will create new one"
fi

# Create new database and run scripts
echo "📝 Creating new database and executing scripts..."

success=true
for script in "${SQL_SCRIPTS[@]}"; do
    echo "   Executing: $script"
    if ! sqlite3 "$DB_FILE" < "$script"; then
        echo "❌ Failed to execute SQL script: $script"
        success=false
        break
    fi
done

if [[ "$success" == "true" ]]; then
    echo "✅ Database created successfully!"
    
    # Show some info about the new database
    echo
    echo "📊 Database Info:"
    echo "File size: $(ls -lh "$DB_FILE" | awk '{print $5}')"
    
    echo
    echo "📋 Tables created:"
    sqlite3 "$DB_FILE" ".tables"
    
    echo
    echo "🔍 You can inspect the database with:"
    echo "   sqlite3 $DB_FILE"
    echo "   sqlite3 $DB_FILE '.schema'"
else
    echo "❌ Failed to execute SQL scripts!"
    exit 1
fi