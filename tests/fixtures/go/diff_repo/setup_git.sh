#!/usr/bin/env bash
# Creates the git history needed for diff integration tests.
# Run from the diff_repo directory.
# Idempotent: skips if .git already has 2+ commits.
set -euo pipefail

cd "$(dirname "$0")"

if [ -d .git ] && [ "$(git rev-list --count HEAD 2>/dev/null)" -ge 2 ]; then
  exit 0
fi

rm -rf .git
git init
git config user.email "test@test.com"
git config user.name "Test"

# Commit 1: base state (simple dependency)
git add go.mod
mkdir -p pkg/api pkg/db
cat > pkg/api/handler.go << 'GOEOF'
package api

import "github.com/test/diffproject/pkg/db"

func Handle() {
	db.Query()
}
GOEOF

cat > pkg/db/db.go << 'GOEOF'
package db

func Query() string {
	return "result"
}
GOEOF

git add pkg/api/handler.go pkg/db/db.go
git commit -m "base: simple dependency"

# Commit 2: add more dependencies and modules
cat > pkg/api/handler.go << 'GOEOF'
package api

import "github.com/test/diffproject/pkg/db"

func Handle() {
	db.Query()
	db.FindUser()
}
GOEOF

cat > pkg/api/middleware.go << 'GOEOF'
package api

import "github.com/test/diffproject/pkg/utils"

func Middleware() {
	utils.Log("middleware")
}
GOEOF

cat > pkg/db/models.go << 'GOEOF'
package db

type User struct {
	Name string
}
GOEOF

cat > pkg/db/queries.go << 'GOEOF'
package db

func FindUser() User {
	return User{Name: "test"}
}
GOEOF

mkdir -p pkg/utils
cat > pkg/utils/logger.go << 'GOEOF'
package utils

import "fmt"

func Log(msg string) {
	fmt.Println(msg)
}
GOEOF

git add -A
git commit -m "add: more dependencies and modules"
