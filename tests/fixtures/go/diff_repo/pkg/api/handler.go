package api

import "github.com/test/diffproject/pkg/db"

func Handle() {
	db.Query()
	db.FindUser()
}
