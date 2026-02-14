#!/bin/bash
# Generates a git repo fixture for diff testing.
# Creates two commits: a base state and a state with increased fan-out.

set -e

DIR="tests/fixtures/go/diff_repo"

# Clean up if exists
rm -rf "$DIR"
mkdir -p "$DIR"

cd "$DIR"
git init

# Create go.mod
cat > go.mod << 'GOMOD'
module github.com/test/diffproject

go 1.21
GOMOD

# Commit 1: base state with minimal imports
mkdir -p pkg/api pkg/db

cat > pkg/api/handler.go << 'GO'
package api

import "github.com/test/diffproject/pkg/db"

func Handle() {
	db.Query()
}
GO

cat > pkg/db/db.go << 'GO'
package db

func Query() string {
	return "result"
}
GO

git add .
git commit -m "base: simple dependency"

# Commit 2: add more imports (fan-out increase + new files)
cat > pkg/db/models.go << 'GO'
package db

type User struct {
	Name string
}
GO

cat > pkg/db/queries.go << 'GO'
package db

func FindUser() User {
	return User{Name: "test"}
}
GO

cat > pkg/api/handler.go << 'GO'
package api

import "github.com/test/diffproject/pkg/db"

func Handle() {
	db.Query()
	db.FindUser()
}
GO

mkdir -p pkg/utils

cat > pkg/utils/logger.go << 'GO'
package utils

import "fmt"

func Log(msg string) {
	fmt.Println(msg)
}
GO

cat > pkg/api/middleware.go << 'GO'
package api

import "github.com/test/diffproject/pkg/utils"

func Middleware() {
	utils.Log("middleware")
}
GO

git add .
git commit -m "add: more dependencies and modules"

echo "Done. Fixture created at $DIR"
echo "Base commit: $(git rev-parse HEAD~1)"
echo "Head commit: $(git rev-parse HEAD)"
